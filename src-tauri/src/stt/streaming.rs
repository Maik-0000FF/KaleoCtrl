use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

use crate::audio::Vad;
use crate::error::AppError;

/// Emit partial results every second during speech
const PARTIAL_INTERVAL: Duration = Duration::from_millis(1000);
/// Minimum audio length (samples) before first partial — 0.5 second at 16kHz
const MIN_SAMPLES_FOR_PARTIAL: usize = 8000;
/// Auto-finalize if buffer exceeds 25 seconds (whisper max is 30s)
const MAX_BUFFER_SAMPLES: usize = 16000 * 25;
/// Finalize buffered speech if no audio arrives for this duration
const AUDIO_TIMEOUT: Duration = Duration::from_millis(300);

/// Known whisper hallucination phrases that appear during silence
const HALLUCINATIONS: &[&str] = &[
    "thank you",
    "thanks for watching",
    "subscribe",
    "like and subscribe",
    "bye",
    "goodbye",
    "see you next time",
    "see you",
    "you",
    "the",
    "i",
    "a",
    "it",
    "so",
    "oh",
    "danke",
    "danke fürs zuschauen",
    "danke für's zuschauen",
    "danke schön",
    "tschüss",
    "bis zum nächsten mal",
    "untertitel",
    "untertitelung",
    "untertitel der amara.org-community",
    "copyright",
    "vielen dank",
    "vielen dank fürs zuschauen",
    "ja",
    "nein",
    "und",
    "die",
    "der",
    "das",
    "ich",
    "er",
    "sie",
    "es",
    "wir",
    "man",
    "also",
    "na",
    "ach",
    "oh",
    "hm",
    "mhm",
    "ok",
    "okay",
    "so",
    "nun",
    "gut",
    "genau",
];

/// Minimum length for plausible speech (single-char hallucinations)
const MIN_TEXT_LENGTH: usize = 2;

fn is_hallucination(text: &str) -> bool {
    let lower = text.to_lowercase();
    let stripped = lower.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace());

    if stripped.len() < MIN_TEXT_LENGTH {
        return true;
    }

    // Exact match against known hallucinations
    if HALLUCINATIONS.contains(&stripped) {
        return true;
    }

    // Repetition detection: "Key Key Key", "Ja ja ja", "..." etc.
    let words: Vec<&str> = stripped.split_whitespace().collect();
    if words.len() >= 2 && words.iter().all(|w| *w == words[0]) {
        return true;
    }

    // Pure punctuation / ellipsis
    if stripped.chars().all(|c| c == '.' || c == ',' || c == '!' || c == '?') {
        return true;
    }

    false
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StreamEvent {
    Partial {
        text: String,
        duration_ms: u64,
    },
    Final {
        text: String,
        language: Option<String>,
        duration_ms: u64,
    },
}

/// Streaming speech-to-text transcriber.
///
/// Owns a dedicated worker thread that holds WhisperContext + WhisperState
/// (created once, reused across transcriptions). Audio flows in via channel,
/// VAD runs in the worker, partial results every ~2s, final on speech end.
pub struct StreamingTranscriber {
    audio_tx: Option<mpsc::Sender<Vec<f32>>>,
    pub event_rx: Option<mpsc::Receiver<StreamEvent>>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    language: Arc<Mutex<String>>,
    prompt: Arc<Mutex<String>>,
    model_path: Option<String>,
}

impl StreamingTranscriber {
    pub fn new() -> Self {
        Self {
            audio_tx: None,
            event_rx: None,
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
            language: Arc::new(Mutex::new("de".into())),
            prompt: Arc::new(Mutex::new(String::new())),
            model_path: None,
        }
    }

    /// Start the worker thread. Blocks until model is loaded.
    pub fn start(&mut self, model_path: &str, language: &str) -> Result<(), AppError> {
        self.stop();

        let (init_tx, init_rx) = mpsc::channel();
        let (audio_tx, audio_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let running = Arc::new(AtomicBool::new(true));
        let lang = Arc::new(Mutex::new(language.to_string()));
        let prompt = Arc::new(Mutex::new(self.prompt.lock().unwrap().clone()));

        let model = model_path.to_string();
        let r = running.clone();
        let l = lang.clone();
        let p = prompt.clone();

        let handle = thread::spawn(move || {
            worker_loop(model, audio_rx, event_tx, r, l, p, init_tx);
        });

        // Block until model is loaded (or error)
        let result = init_rx
            .recv()
            .map_err(|_| AppError::Stt("Worker thread crashed during init".into()))?;
        result?;

        self.audio_tx = Some(audio_tx);
        self.event_rx = Some(event_rx);
        self.running = running;
        self.thread = Some(handle);
        self.language = lang;
        self.prompt = prompt;
        self.model_path = Some(model_path.to_string());

        log::info!("StreamingTranscriber: started with {}", model_path);
        Ok(())
    }

    /// Clone the audio sender for use by AudioCapture.
    pub fn audio_sender(&self) -> Option<mpsc::Sender<Vec<f32>>> {
        self.audio_tx.clone()
    }

    /// Stop the worker thread and free resources.
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::Relaxed) && self.thread.is_none() {
            return;
        }
        self.running.store(false, Ordering::Relaxed);
        self.audio_tx = None;
        self.event_rx = None;
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        self.model_path = None;
        log::info!("StreamingTranscriber: stopped");
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn set_language(&self, lang: &str) {
        *self.language.lock().unwrap() = lang.to_string();
    }

    pub fn set_prompt(&self, prompt: &str) {
        *self.prompt.lock().unwrap() = prompt.to_string();
        log::info!("STT prompt updated ({} chars)", prompt.len());
    }

    pub fn current_model(&self) -> Option<&str> {
        self.model_path.as_deref()
    }
}

impl Drop for StreamingTranscriber {
    fn drop(&mut self) {
        self.stop();
    }
}

// --- Worker Thread ---

fn worker_loop(
    model_path: String,
    audio_rx: mpsc::Receiver<Vec<f32>>,
    event_tx: mpsc::Sender<StreamEvent>,
    running: Arc<AtomicBool>,
    language: Arc<Mutex<String>>,
    prompt: Arc<Mutex<String>>,
    init_tx: mpsc::Sender<Result<(), AppError>>,
) {
    log::info!("Worker: loading model {}", model_path);

    let mut params = WhisperContextParameters::default();
    params.flash_attn(true);
    let ctx = match WhisperContext::new_with_params(&model_path, params) {
        Ok(c) => c,
        Err(e) => {
            let _ = init_tx.send(Err(AppError::Stt(format!(
                "Failed to load model '{}': {}",
                model_path, e
            ))));
            return;
        }
    };

    let mut state = match ctx.create_state() {
        Ok(s) => s,
        Err(e) => {
            let _ = init_tx.send(Err(AppError::Stt(format!(
                "Failed to create whisper state: {}",
                e
            ))));
            return;
        }
    };

    // Signal successful init
    let _ = init_tx.send(Ok(()));
    log::info!("Worker: model loaded, entering main loop");

    let mut vad = Vad::new();
    let mut buffer: Vec<f32> = Vec::with_capacity(16000 * 10);
    let mut last_partial = Instant::now();
    let mut last_audio = Instant::now();

    loop {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        let mut got_audio = false;
        let mut speech_ended = false;

        match audio_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(chunk) => {
                last_audio = Instant::now();
                got_audio = true;
                let (is_speech, ended) = vad.process(&chunk);
                if is_speech {
                    buffer.extend_from_slice(&chunk);
                }
                if ended {
                    speech_ended = true;
                }

                // Drain pending chunks
                while let Ok(more) = audio_rx.try_recv() {
                    let (is_speech, ended) = vad.process(&more);
                    if is_speech {
                        buffer.extend_from_slice(&more);
                    }
                    if ended {
                        speech_ended = true;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let lang = language.lock().unwrap().clone();
        let prmpt = prompt.lock().unwrap().clone();

        // Auto-finalize if buffer exceeds max duration
        if buffer.len() >= MAX_BUFFER_SAMPLES {
            if let Some((text, detected_lang, dur)) =
                transcribe_buffer(&mut state, &buffer, &lang, &prmpt)
            {
                let _ = event_tx.send(StreamEvent::Final {
                    text,
                    language: detected_lang,
                    duration_ms: dur,
                });
            }
            buffer.clear();
            vad.reset();
            last_partial = Instant::now();
            continue;
        }

        // Speech ended (VAD detected silence) → final transcription
        if speech_ended && !buffer.is_empty() {
            if let Some((text, detected_lang, dur)) =
                transcribe_buffer(&mut state, &buffer, &lang, &prmpt)
            {
                let _ = event_tx.send(StreamEvent::Final {
                    text,
                    language: detected_lang,
                    duration_ms: dur,
                });
            }
            buffer.clear();
            last_partial = Instant::now();
            continue;
        }

        // No audio for a while but buffer has data → finalize (e.g. mic stopped)
        if !got_audio
            && !buffer.is_empty()
            && last_audio.elapsed() >= AUDIO_TIMEOUT
        {
            if let Some((text, detected_lang, dur)) =
                transcribe_buffer(&mut state, &buffer, &lang, &prmpt)
            {
                let _ = event_tx.send(StreamEvent::Final {
                    text,
                    language: detected_lang,
                    duration_ms: dur,
                });
            }
            buffer.clear();
            vad.reset();
            last_partial = Instant::now();
            continue;
        }

        // Partial transcription during active speech
        if vad.is_speaking()
            && buffer.len() >= MIN_SAMPLES_FOR_PARTIAL
            && last_partial.elapsed() >= PARTIAL_INTERVAL
        {
            if let Some((text, _, dur)) = transcribe_buffer(&mut state, &buffer, &lang, &prmpt) {
                let _ = event_tx.send(StreamEvent::Partial {
                    text,
                    duration_ms: dur,
                });
            }
            last_partial = Instant::now();
        }
    }

    log::info!("Worker: exiting");
}

fn transcribe_buffer(
    state: &mut WhisperState,
    audio: &[f32],
    language: &str,
    prompt: &str,
) -> Option<(String, Option<String>, u64)> {
    if audio.is_empty() {
        return None;
    }

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(16);
    params.set_single_segment(true);
    params.set_no_timestamps(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_translate(false);
    params.set_language(Some(language));
    // Prevent infinite decode loops (hallucination "Key Key Key...")
    params.set_max_tokens(32);
    // Suppress blank/silence tokens
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_no_context(true);

    // Vocabulary bias: guide the model toward expected words
    if !prompt.is_empty() {
        params.set_initial_prompt(prompt);
    }

    let start = Instant::now();

    if let Err(e) = state.full(params, audio) {
        log::error!("Transcription failed: {}", e);
        return None;
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    let num_segments = state.full_n_segments();
    let mut text = String::new();
    for i in 0..num_segments {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(s) = segment.to_str() {
                text.push_str(s);
            }
        }
    }

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() || is_hallucination(&trimmed) {
        return None;
    }

    let lang_id = state.full_lang_id_from_state();
    let detected_lang = whisper_rs::get_lang_str(lang_id).map(|s| s.to_string());

    Some((trimmed, detected_lang, duration_ms))
}
