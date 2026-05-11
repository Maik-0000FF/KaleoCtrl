use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::error::AppError;

const TARGET_SAMPLE_RATE: u32 = 16000;

/// Energy-based Voice Activity Detection with hysteresis
pub struct Vad {
    /// RMS threshold to start speech detection
    speech_threshold: f32,
    /// RMS threshold to stop speech detection (lower than start for hysteresis)
    silence_threshold: f32,
    /// Consecutive silent frames before ending speech segment
    silence_frames_required: usize,
    /// Current state
    is_speaking: bool,
    /// Counter for consecutive silent frames
    silent_frames: usize,
}

impl Vad {
    pub fn new() -> Self {
        Self {
            speech_threshold: 0.025,
            silence_threshold: 0.012,
            silence_frames_required: 15, // ~0.25s at typical chunk sizes
            is_speaking: false,
            silent_frames: 0,
        }
    }

    /// Process a chunk of audio, returns (is_speech, speech_just_ended)
    pub fn process(&mut self, samples: &[f32]) -> (bool, bool) {
        let rms = Self::calculate_rms(samples);

        if self.is_speaking {
            if rms < self.silence_threshold {
                self.silent_frames += 1;
                if self.silent_frames >= self.silence_frames_required {
                    self.is_speaking = false;
                    self.silent_frames = 0;
                    return (false, true); // Speech just ended
                }
            } else {
                self.silent_frames = 0;
            }
            (true, false)
        } else if rms >= self.speech_threshold {
            self.is_speaking = true;
            self.silent_frames = 0;
            (true, false)
        } else {
            (false, false)
        }
    }

    pub fn reset(&mut self) {
        self.is_speaking = false;
        self.silent_frames = 0;
    }

    pub fn is_speaking(&self) -> bool {
        self.is_speaking
    }

    pub fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        (sum / samples.len() as f32).sqrt()
    }
}

/// Manages microphone capture and streams audio to the transcription worker.
pub struct AudioCapture {
    stream: Option<Stream>,
    pub is_recording: Arc<AtomicBool>,
    current_rms: Arc<AtomicU32>,
    audio_sink: Arc<Mutex<Option<mpsc::SyncSender<Vec<f32>>>>>,
    sample_rate: u32,
}

impl AudioCapture {
    pub fn new() -> Self {
        Self {
            stream: None,
            is_recording: Arc::new(AtomicBool::new(false)),
            current_rms: Arc::new(AtomicU32::new(0)),
            audio_sink: Arc::new(Mutex::new(None)),
            sample_rate: TARGET_SAMPLE_RATE,
        }
    }

    /// Set the audio sink — audio chunks will be sent here for processing.
    pub fn set_audio_sink(&self, tx: mpsc::SyncSender<Vec<f32>>) {
        *self.audio_sink.lock().unwrap() = Some(tx);
    }

    /// Remove the audio sink.
    pub fn clear_audio_sink(&self) {
        *self.audio_sink.lock().unwrap() = None;
    }

    pub fn start(&mut self) -> Result<(), AppError> {
        if self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AppError::Config("No microphone found".into()))?;

        let supported = device
            .supported_input_configs()
            .map_err(|e| AppError::Config(format!("Failed to query mic configs: {}", e)))?
            .find(|c| {
                c.sample_format() == SampleFormat::F32
                    && c.min_sample_rate() <= TARGET_SAMPLE_RATE
                    && c.max_sample_rate() >= TARGET_SAMPLE_RATE
                    && c.channels() >= 1
            });

        let config = match supported {
            Some(range) => range.with_sample_rate(TARGET_SAMPLE_RATE),
            None => {
                // Fallback: use default config and resample later
                let default = device
                    .default_input_config()
                    .map_err(|e| AppError::Config(format!("No usable mic config: {}", e)))?;
                self.sample_rate = default.sample_rate();
                default
            }
        };

        let channels = config.channels() as usize;
        let device_sample_rate = config.sample_rate();
        let is_recording = self.is_recording.clone();
        let current_rms = self.current_rms.clone();
        let audio_sink = self.audio_sink.clone();
        let needs_resample = device_sample_rate != TARGET_SAMPLE_RATE;

        // Scratch buffers reused across callback invocations. Allocating in the
        // cpal audio thread can cause glitches; the buffers grow once and then
        // their capacity is reused for every block.
        let mut mono_buf: Vec<f32> = Vec::with_capacity(4096);
        let mut resampled_buf: Vec<f32> = Vec::with_capacity(4096);

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !is_recording.load(Ordering::Relaxed) {
                        return;
                    }

                    // Mono mix into scratch buffer
                    mono_buf.clear();
                    if channels > 1 {
                        mono_buf.reserve(data.len() / channels);
                        for frame in data.chunks(channels) {
                            let sum: f32 = frame.iter().sum();
                            mono_buf.push(sum / channels as f32);
                        }
                    } else {
                        mono_buf.extend_from_slice(data);
                    }

                    let processed: &[f32] = if needs_resample {
                        resample_into(
                            &mono_buf,
                            device_sample_rate,
                            TARGET_SAMPLE_RATE,
                            &mut resampled_buf,
                        );
                        &resampled_buf
                    } else {
                        &mono_buf
                    };

                    // Store current RMS for level meter (lock-free)
                    let rms = Vad::calculate_rms(processed);
                    current_rms.store(rms.to_bits(), Ordering::Relaxed);

                    // Forward to streaming worker. Non-blocking on two fronts:
                    // try_lock so we never wait on a poisoned/contended mutex,
                    // try_send so we drop the chunk instead of blocking the cpal
                    // audio thread when the transcriber lags behind.
                    if let Ok(guard) = audio_sink.try_lock() {
                        if let Some(tx) = guard.as_ref() {
                            let _ = tx.try_send(processed.to_vec());
                        }
                    }
                },
                |err| {
                    log::error!("Audio input error: {}", err);
                },
                None,
            )
            .map_err(|e| AppError::Config(format!("Failed to build audio stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| AppError::Config(format!("Failed to start audio stream: {}", e)))?;

        self.is_recording.store(true, Ordering::Relaxed);
        self.stream = Some(stream);
        log::info!(
            "Audio capture started ({}Hz, {} channels)",
            device_sample_rate,
            channels
        );

        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_recording.store(false, Ordering::Relaxed);
        self.stream = None;
        self.current_rms.store(0f32.to_bits(), Ordering::Relaxed);
        log::info!("Audio capture stopped");
    }

    /// Current RMS level (lock-free read)
    pub fn rms(&self) -> f32 {
        f32::from_bits(self.current_rms.load(Ordering::Relaxed))
    }

    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// Simple linear interpolation resampler. Writes into the caller-provided
/// `output` buffer so the audio callback can reuse capacity across calls.
fn resample_into(input: &[f32], from_rate: u32, to_rate: u32, output: &mut Vec<f32>) {
    output.clear();
    if input.is_empty() {
        return;
    }
    if from_rate == to_rate {
        output.extend_from_slice(input);
        return;
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (input.len() as f64 / ratio) as usize;
    output.reserve(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        let sample = if src_idx + 1 < input.len() {
            input[src_idx] as f64 * (1.0 - frac) + input[src_idx + 1] as f64 * frac
        } else {
            input[src_idx] as f64
        };

        output.push(sample as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_silence() {
        let mut vad = Vad::new();
        let silence = vec![0.0f32; 160];
        let (is_speech, ended) = vad.process(&silence);
        assert!(!is_speech);
        assert!(!ended);
    }

    #[test]
    fn test_vad_speech_detection() {
        let mut vad = Vad::new();

        // Loud signal
        let speech: Vec<f32> = (0..160).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let (is_speech, _) = vad.process(&speech);
        assert!(is_speech);
    }

    #[test]
    fn test_vad_hysteresis() {
        let mut vad = Vad::new();

        // Start speaking
        let loud: Vec<f32> = vec![0.1; 160];
        vad.process(&loud);
        assert!(vad.is_speaking());

        // Quiet but above silence threshold — should still be speaking
        let medium: Vec<f32> = vec![0.01; 160];
        let (is_speech, _) = vad.process(&medium);
        assert!(is_speech);
    }

    #[test]
    fn test_resample_identity() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = Vec::new();
        resample_into(&input, 16000, 16000, &mut output);
        assert_eq!(input, output);
    }

    #[test]
    fn test_resample_downsample() {
        let input: Vec<f32> = (0..320).map(|i| i as f32).collect();
        let mut output = Vec::new();
        resample_into(&input, 48000, 16000, &mut output);
        // 48kHz -> 16kHz = 1/3 samples
        assert!((output.len() as f32 - 106.0).abs() < 2.0);
    }

    #[test]
    fn test_resample_into_reuses_buffer() {
        let mut output = Vec::with_capacity(128);
        let cap = output.capacity();
        let input: Vec<f32> = (0..96).map(|i| i as f32).collect();
        resample_into(&input, 48000, 16000, &mut output);
        // Capacity should not shrink — buffer is reused across calls.
        assert!(output.capacity() >= cap);
        let cap2 = output.capacity();
        resample_into(&input, 48000, 16000, &mut output);
        assert_eq!(cap2, output.capacity());
    }
}
