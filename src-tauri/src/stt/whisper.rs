use std::time::Instant;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::AppError;
use crate::stt::{SttEngine, TranscriptionResult};

#[allow(dead_code)]
const HALLUCINATIONS: &[&str] = &[
    "thank you",
    "thanks for watching",
    "subscribe",
    "like and subscribe",
    "bye",
    "goodbye",
    "see you next time",
    "see you",
    "danke",
    "danke fürs zuschauen",
    "danke schön",
    "tschüss",
    "bis zum nächsten mal",
    "untertitel",
    "copyright",
    "vielen dank",
];

#[allow(dead_code)]
fn is_hallucination(text: &str) -> bool {
    let lower = text.to_lowercase();
    let stripped = lower.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace());
    HALLUCINATIONS.contains(&stripped)
}

#[allow(dead_code)]
pub struct WhisperEngine {
    model_path: Option<String>,
    context: Option<WhisperContext>,
}

#[allow(dead_code)]
impl WhisperEngine {
    pub fn new() -> Self {
        Self {
            model_path: None,
            context: None,
        }
    }
}

impl SttEngine for WhisperEngine {
    fn engine_name(&self) -> &str {
        "whisper.cpp"
    }

    fn load_model(&mut self, model_path: &str) -> Result<(), AppError> {
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path, params)
            .map_err(|e| AppError::Stt(format!("Failed to load model '{}': {}", model_path, e)))?;

        self.context = Some(ctx);
        self.model_path = Some(model_path.to_string());
        log::info!("WhisperEngine: loaded model {}", model_path);
        Ok(())
    }

    fn transcribe(
        &self,
        audio: &[f32],
        _sample_rate: u32,
        language: Option<&str>,
    ) -> Result<TranscriptionResult, AppError> {
        let ctx = self
            .context
            .as_ref()
            .ok_or_else(|| AppError::Stt("No model loaded".into()))?;

        let mut state = ctx
            .create_state()
            .map_err(|e| AppError::Stt(format!("Failed to create state: {}", e)))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(16);
        params.set_single_segment(true);
        params.set_no_timestamps(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_translate(false);

        // Set language to avoid auto-detect defaulting to English
        if let Some(lang) = language {
            params.set_language(Some(lang));
        }

        let start = Instant::now();

        state
            .full(params, audio)
            .map_err(|e| AppError::Stt(format!("Transcription failed: {}", e)))?;

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

        // Filter known whisper hallucinations (silence artifacts)
        if is_hallucination(&trimmed) {
            return Ok(TranscriptionResult {
                text: String::new(),
                language: None,
                duration_ms,
            });
        }

        let lang_id = state.full_lang_id_from_state();
        let language = whisper_rs::get_lang_str(lang_id).map(|s| s.to_string());

        Ok(TranscriptionResult {
            text: trimmed,
            language,
            duration_ms,
        })
    }

    fn unload_model(&mut self) {
        self.context = None;
        self.model_path = None;
        log::info!("WhisperEngine: model unloaded");
    }

    fn is_model_loaded(&self) -> bool {
        self.context.is_some()
    }

    fn current_model(&self) -> Option<&str> {
        self.model_path.as_deref()
    }
}
