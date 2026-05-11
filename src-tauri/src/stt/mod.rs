pub mod streaming;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SttStatus {
    pub engine: String,
    pub model_loaded: bool,
    pub current_model: Option<String>,
}
