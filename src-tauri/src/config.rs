use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub assistant_name: String,
    pub language: String,
    pub stt_model: String,
    pub default_mode: String,
}

pub struct ConfigState {
    pub config: Mutex<AppConfig>,
    pub config_dir: PathBuf,
}

impl ConfigState {
    pub fn load(config_dir: &Path) -> Result<Self, AppError> {
        let path = config_dir.join("settings.json");
        let content = fs::read_to_string(&path).map_err(|e| {
            AppError::Config(format!("Failed to read {}: {}", path.display(), e))
        })?;
        let config: AppConfig = serde_json::from_str(&content)?;
        Ok(Self {
            config: Mutex::new(config),
            config_dir: config_dir.to_path_buf(),
        })
    }

    pub fn save(&self) -> Result<(), AppError> {
        let config = self.config.lock().unwrap();
        let path = self.config_dir.join("settings.json");
        let content = serde_json::to_string_pretty(&*config)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

/// Resolve the config directory by checking CWD/config/, parent/config/, fallback CWD/config/
pub fn resolve_config_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();

    // Check CWD/config/
    let candidate = cwd.join("config");
    if candidate.is_dir() {
        return candidate;
    }

    // Check parent/config/ (useful when running from src-tauri/)
    if let Some(parent) = cwd.parent() {
        let candidate = parent.join("config");
        if candidate.is_dir() {
            return candidate;
        }
    }

    // Fallback
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_config() {
        let config_dir = resolve_config_dir();
        if !config_dir.join("settings.json").exists() {
            return; // Skip if no config available
        }
        let state = ConfigState::load(&config_dir).unwrap();
        let config = state.config.lock().unwrap();
        assert!(!config.assistant_name.is_empty());
        assert!(!config.language.is_empty());
    }

    #[test]
    fn test_roundtrip_config() {
        let dir = std::env::temp_dir().join("kaleoctrl_test_config");
        fs::create_dir_all(&dir).unwrap();

        let config = AppConfig {
            assistant_name: "test".into(),
            language: "en".into(),
            stt_model: "large-v3-turbo".into(),
            default_mode: "desktop".into(),
        };

        let path = dir.join("settings.json");
        fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

        let state = ConfigState::load(&dir).unwrap();
        let loaded = state.config.lock().unwrap();
        assert_eq!(loaded.assistant_name, "test");
        assert_eq!(loaded.language, "en");

        fs::remove_dir_all(&dir).ok();
    }
}
