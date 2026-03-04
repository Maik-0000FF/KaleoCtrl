pub mod streaming;
pub mod whisper;

use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub duration_ms: u64,
}

#[allow(dead_code)]
pub trait SttEngine: Send + Sync {
    fn engine_name(&self) -> &str;
    fn load_model(&mut self, model_path: &str) -> Result<(), AppError>;
    fn transcribe(
        &self,
        audio: &[f32],
        sample_rate: u32,
        language: Option<&str>,
    ) -> Result<TranscriptionResult, AppError>;
    fn unload_model(&mut self);
    fn is_model_loaded(&self) -> bool;
    fn current_model(&self) -> Option<&str>;
}

#[derive(Debug, Clone, Serialize)]
pub struct SttStatus {
    pub engine: String,
    pub model_loaded: bool,
    pub current_model: Option<String>,
}
