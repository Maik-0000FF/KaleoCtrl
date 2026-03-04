use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

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
            speech_threshold: 0.015,
            silence_threshold: 0.008,
            silence_frames_required: 30, // ~0.5s at typical chunk sizes
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

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.is_speaking = false;
        self.silent_frames = 0;
    }

    pub fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        (sum / samples.len() as f32).sqrt()
    }
}

/// Manages microphone capture and speech segment collection
pub struct AudioCapture {
    stream: Option<Stream>,
    pub is_recording: Arc<AtomicBool>,
    pub speech_buffer: Arc<Mutex<Vec<f32>>>,
    pub pending_segments: Arc<Mutex<Vec<Vec<f32>>>>,
    current_rms: Arc<AtomicU32>,
    sample_rate: u32,
}

impl AudioCapture {
    pub fn new() -> Self {
        Self {
            stream: None,
            is_recording: Arc::new(AtomicBool::new(false)),
            speech_buffer: Arc::new(Mutex::new(Vec::new())),
            pending_segments: Arc::new(Mutex::new(Vec::new())),
            current_rms: Arc::new(AtomicU32::new(0)),
            sample_rate: TARGET_SAMPLE_RATE,
        }
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
        let speech_buffer = self.speech_buffer.clone();
        let pending_segments = self.pending_segments.clone();
        let current_rms = self.current_rms.clone();

        let vad = Arc::new(Mutex::new(Vad::new()));

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !is_recording.load(Ordering::Relaxed) {
                        return;
                    }

                    // Convert to mono if needed
                    let mono: Vec<f32> = if channels > 1 {
                        data.chunks(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                            .collect()
                    } else {
                        data.to_vec()
                    };

                    // Simple resample if needed (linear interpolation)
                    let resampled = if device_sample_rate != TARGET_SAMPLE_RATE {
                        resample(&mono, device_sample_rate, TARGET_SAMPLE_RATE)
                    } else {
                        mono
                    };

                    // Store current RMS for level meter (lock-free)
                    let rms = Vad::calculate_rms(&resampled);
                    current_rms.store(rms.to_bits(), Ordering::Relaxed);

                    let mut vad = vad.lock().unwrap();
                    let (is_speech, speech_ended) = vad.process(&resampled);

                    let mut buffer = speech_buffer.lock().unwrap();
                    if is_speech {
                        buffer.extend_from_slice(&resampled);
                    }

                    if speech_ended && !buffer.is_empty() {
                        let segment = std::mem::take(&mut *buffer);
                        let mut segments = pending_segments.lock().unwrap();
                        segments.push(segment);
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
        self.speech_buffer.lock().unwrap().clear();
        self.current_rms.store(0f32.to_bits(), Ordering::Relaxed);
        log::info!("Audio capture stopped");
    }

    /// Current RMS level (lock-free read)
    pub fn rms(&self) -> f32 {
        f32::from_bits(self.current_rms.load(Ordering::Relaxed))
    }

    /// Take all pending speech segments
    pub fn take_segments(&self) -> Vec<Vec<f32>> {
        let mut segments = self.pending_segments.lock().unwrap();
        std::mem::take(&mut *segments)
    }

    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// Simple linear interpolation resampler
fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

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

    output
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
        assert!(vad.is_speaking);

        // Quiet but above silence threshold — should still be speaking
        let medium: Vec<f32> = vec![0.01; 160];
        let (is_speech, _) = vad.process(&medium);
        assert!(is_speech);
    }

    #[test]
    fn test_resample_identity() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = resample(&input, 16000, 16000);
        assert_eq!(input, output);
    }

    #[test]
    fn test_resample_downsample() {
        let input: Vec<f32> = (0..320).map(|i| i as f32).collect();
        let output = resample(&input, 48000, 16000);
        // 48kHz -> 16kHz = 1/3 samples
        assert!((output.len() as f32 - 106.0).abs() < 2.0);
    }
}
