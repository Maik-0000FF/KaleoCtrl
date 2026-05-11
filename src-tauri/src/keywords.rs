use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordConfig {
    pub language: String,
    pub modes: HashMap<String, ModeInfo>,
    pub mode_switch: String,
    pub wake_phrase: String,
    pub sleep_phrase: String,
    pub commands: HashMap<String, String>,
    pub dictation: HashMap<String, String>,
    /// Prefix word for key press commands (e.g. "taste", "key")
    #[serde(default)]
    pub key_prefix: String,
    /// Alternative spellings / misrecognitions of the key prefix (e.g. "kaste" for "taste")
    #[serde(default)]
    pub key_prefix_aliases: Vec<String>,
    /// Mapping: spoken name → key to simulate (e.g. "enter" → "Return")
    #[serde(default)]
    pub keys: HashMap<String, String>,
}

pub fn load_keywords(config_dir: &Path, language: &str) -> Result<KeywordConfig, AppError> {
    let filename = format!("keywords_{}.json", language);
    let path = config_dir.join(&filename);

    if !path.exists() {
        return Err(AppError::KeywordFileNotFound(filename));
    }

    let content = fs::read_to_string(&path)?;
    let keywords: KeywordConfig = serde_json::from_str(&content)?;
    Ok(keywords)
}

pub fn save_keywords(config_dir: &Path, keywords: &KeywordConfig) -> Result<(), AppError> {
    let filename = format!("keywords_{}.json", keywords.language);
    let path = config_dir.join(&filename);
    let content = serde_json::to_string_pretty(keywords)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn list_available_languages(config_dir: &Path) -> Result<Vec<String>, AppError> {
    let mut languages = Vec::new();

    for entry in fs::read_dir(config_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if let Some(lang) = name
            .strip_prefix("keywords_")
            .and_then(|s| s.strip_suffix(".json"))
        {
            languages.push(lang.to_string());
        }
    }

    languages.sort();
    Ok(languages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::resolve_config_dir;

    #[test]
    fn test_load_english_keywords() {
        let config_dir = resolve_config_dir();
        let path = config_dir.join("keywords_en.json");
        if !path.exists() {
            return;
        }
        let kw = load_keywords(&config_dir, "en").unwrap();
        assert_eq!(kw.language, "en");
        assert!(kw.modes.contains_key("desktop"));
        assert!(kw.commands.contains_key("open"));
    }

    #[test]
    fn test_list_languages() {
        let config_dir = resolve_config_dir();
        if !config_dir.is_dir() {
            return;
        }
        let langs = list_available_languages(&config_dir).unwrap();
        assert!(!langs.is_empty());
    }

    #[test]
    fn test_missing_language_returns_error() {
        let config_dir = resolve_config_dir();
        let result = load_keywords(&config_dir, "xx_nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_minimal_json_uses_defaults_for_key_fields() {
        // No key_prefix / key_prefix_aliases / keys → fields default to empty.
        let json = r#"{
            "language": "test",
            "modes": {},
            "mode_switch": "mode",
            "wake_phrase": "wake",
            "sleep_phrase": "sleep",
            "commands": {},
            "dictation": {}
        }"#;
        let kw: KeywordConfig = serde_json::from_str(json).unwrap();
        assert_eq!(kw.key_prefix, "");
        assert!(kw.key_prefix_aliases.is_empty());
        assert!(kw.keys.is_empty());
    }

    #[test]
    fn serde_round_trip_preserves_all_fields() {
        let mut commands = HashMap::new();
        commands.insert("open".to_string(), "open".to_string());

        let mut keys = HashMap::new();
        keys.insert("enter".to_string(), "Return".to_string());

        let original = KeywordConfig {
            language: "en".into(),
            modes: HashMap::new(),
            mode_switch: "mode".into(),
            wake_phrase: "wake up".into(),
            sleep_phrase: "sleep".into(),
            commands,
            dictation: HashMap::new(),
            key_prefix: "key".into(),
            key_prefix_aliases: vec!["keys".into(), "taste".into()],
            keys,
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: KeywordConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.language, original.language);
        assert_eq!(parsed.key_prefix, original.key_prefix);
        assert_eq!(parsed.key_prefix_aliases, original.key_prefix_aliases);
        assert_eq!(parsed.commands, original.commands);
        assert_eq!(parsed.keys, original.keys);
    }
}
