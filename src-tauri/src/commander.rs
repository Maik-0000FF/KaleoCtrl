use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::injection;
use crate::keywords::KeywordConfig;

/// Strip trailing/leading punctuation and normalize to lowercase.
/// Whisper often appends ".", "?", "!" or wraps in quotes.
fn strip_punctuation(text: &str) -> String {
    text.trim()
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == '\u{201E}' || c == '\u{201C}')
        .to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    Desktop,
    Dictation,
    Terminal,
    Sleep,
}

impl AppMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "desktop" => Some(AppMode::Desktop),
            "dictation" | "diktat" => Some(AppMode::Dictation),
            "terminal" => Some(AppMode::Terminal),
            "sleep" | "schlaf" => Some(AppMode::Sleep),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum CommandResult {
    ModeChanged(AppMode),
    SystemCommand(String),
    TextInjected(String),
    DictationAction(String),
    KeyPressed(String),
    Ignored,
    Sleeping,
}

/// Process recognized speech and execute the appropriate action
pub fn process_speech(
    text: &str,
    mode: &mut AppMode,
    keywords: &KeywordConfig,
    assistant_name: &str,
) -> Result<CommandResult, AppError> {
    let text = strip_punctuation(text);
    if text.is_empty() {
        return Ok(CommandResult::Ignored);
    }

    let name = assistant_name.to_lowercase();

    // Sleep mode: only listen for wake phrase
    if *mode == AppMode::Sleep {
        let wake = format!("{} {}", name, keywords.wake_phrase.to_lowercase());
        if text.contains(&wake) {
            *mode = AppMode::Desktop;
            return Ok(CommandResult::ModeChanged(AppMode::Desktop));
        }
        return Ok(CommandResult::Sleeping);
    }

    // Check for sleep command (any mode)
    let sleep_cmd = format!("{} {}", name, keywords.sleep_phrase.to_lowercase());
    if text.contains(&sleep_cmd) {
        *mode = AppMode::Sleep;
        return Ok(CommandResult::ModeChanged(AppMode::Sleep));
    }

    // Check for mode switch: "<name> <mode_switch> <mode>"
    let mode_prefix = format!("{} {}", name, keywords.mode_switch.to_lowercase());
    if let Some(rest) = text.strip_prefix(&mode_prefix) {
        let rest = rest.trim();
        // Match against mode names from keywords
        for (key, mode_info) in &keywords.modes {
            if rest == key || rest == mode_info.name.to_lowercase() {
                if let Some(new_mode) = AppMode::from_str(key) {
                    *mode = new_mode;
                    return Ok(CommandResult::ModeChanged(new_mode));
                }
            }
        }
    }

    // Check for key press: "<key_prefix> <key_name>" (works in all active modes)
    if let Some(result) = check_key_command(&text, keywords) {
        return result;
    }

    match mode {
        AppMode::Desktop => process_desktop_command(&text, keywords, &name),
        AppMode::Dictation => process_dictation(&text, keywords, &name),
        AppMode::Terminal => process_terminal_command(&text, &name),
        AppMode::Sleep => Ok(CommandResult::Sleeping),
    }
}

fn process_desktop_command(
    text: &str,
    keywords: &KeywordConfig,
    assistant_name: &str,
) -> Result<CommandResult, AppError> {
    // In desktop mode, commands work directly (no prefix needed)
    // But if prefixed with assistant name, strip it
    let command_text = text
        .strip_prefix(assistant_name)
        .map(|s| s.trim())
        .unwrap_or(text);

    // Match against system command keywords
    for (action, keyword) in &keywords.commands {
        let kw = keyword.to_lowercase();
        if let Some(args) = command_text.strip_prefix(&kw) {
            let args = args.trim();
            return execute_system_command(action, args);
        }
    }

    // No command matched — in desktop mode, ignore unrecognized speech
    Ok(CommandResult::Ignored)
}

fn process_dictation(
    text: &str,
    keywords: &KeywordConfig,
    assistant_name: &str,
) -> Result<CommandResult, AppError> {
    // In dictation mode, commands require assistant name prefix
    if let Some(command_text) = text.strip_prefix(assistant_name) {
        let command_text = command_text.trim();

        // Check system commands
        for (action, keyword) in &keywords.commands {
            let kw = keyword.to_lowercase();
            if let Some(args) = command_text.strip_prefix(&kw) {
                let args = args.trim();
                return execute_system_command(action, args);
            }
        }

        // Check dictation commands
        for (action, keyword) in &keywords.dictation {
            let kw = keyword.to_lowercase();
            if command_text == kw || command_text.starts_with(&kw) {
                return execute_dictation_command(action);
            }
        }
    }

    // No prefix or no command matched — inject as text
    injection::type_text(text)?;
    Ok(CommandResult::TextInjected(text.to_string()))
}

fn process_terminal_command(
    text: &str,
    assistant_name: &str,
) -> Result<CommandResult, AppError> {
    // In terminal mode, inject text as-is (typed into terminal)
    let command_text = text
        .strip_prefix(assistant_name)
        .map(|s| s.trim())
        .unwrap_or(text);

    injection::type_text(command_text)?;
    Ok(CommandResult::TextInjected(command_text.to_string()))
}

fn execute_system_command(action: &str, args: &str) -> Result<CommandResult, AppError> {
    let description = format!("{} {}", action, args);

    match action {
        "open" => {
            if !args.is_empty() {
                Command::new("setsid")
                    .args([args])
                    .spawn()
                    .map_err(|e| AppError::Config(format!("Failed to open '{}': {}", args, e)))?;
            }
        }
        "close" => {
            // wmctrl -c to close focused or named window
            let wmctrl_args = if args.is_empty() {
                vec!["-c", ":ACTIVE:"]
            } else {
                vec!["-c", args]
            };
            let _ = Command::new("wmctrl").args(&wmctrl_args).output();
        }
        "switch" => {
            if !args.is_empty() {
                let _ = Command::new("wmctrl").args(["-a", args]).output();
            }
        }
        "minimize" => {
            let _ = Command::new("xdotool")
                .args(["getactivewindow", "windowminimize"])
                .output();
        }
        "maximize" => {
            let _ = Command::new("wmctrl")
                .args(["-r", ":ACTIVE:", "-b", "toggle,maximized_vert,maximized_horz"])
                .output();
        }
        "fullscreen" => {
            let _ = Command::new("wmctrl")
                .args(["-r", ":ACTIVE:", "-b", "toggle,fullscreen"])
                .output();
        }
        "stop" => {
            // Stop current operation (Escape key)
            let _ = injection::send_key("Escape");
        }
        "undo" => {
            let _ = injection::send_key("ctrl+z");
        }
        _ => {
            // Custom command: try executing as-is
            log::info!("Custom command: {} {}", action, args);
        }
    }

    Ok(CommandResult::SystemCommand(description))
}

/// Check if text matches "<key_prefix> <key_name>" and simulate the key press
pub fn check_key_command(
    text: &str,
    keywords: &KeywordConfig,
) -> Option<Result<CommandResult, AppError>> {
    let prefix = keywords.key_prefix.to_lowercase();
    if prefix.is_empty() {
        return None;
    }

    // Try main prefix and all aliases (e.g. "kaste" for "taste")
    let mut prefixes = vec![prefix];
    for alias in &keywords.key_prefix_aliases {
        prefixes.push(alias.to_lowercase());
    }

    let mut rest_text = None;
    for p in &prefixes {
        if let Some(r) = text.strip_prefix(p.as_str()) {
            let stripped = strip_punctuation(r);
            if !stripped.is_empty() {
                rest_text = Some(stripped);
                break;
            }
        }
    }
    let rest = rest_text?;

    for (spoken, key_action) in &keywords.keys {
        if rest == spoken.to_lowercase() {
            return Some(
                injection::send_key(key_action)
                    .map(|_| CommandResult::KeyPressed(key_action.clone())),
            );
        }
    }

    None
}

fn execute_dictation_command(action: &str) -> Result<CommandResult, AppError> {
    match action {
        "new_line" => injection::send_key("Return")?,
        "new_paragraph" => {
            injection::send_key("Return")?;
            injection::send_key("Return")?;
        }
        "delete_word" => injection::send_key("ctrl+BackSpace")?,
        "delete_sentence" => {
            injection::send_key("Home")?;
            injection::send_key("shift+End")?;
            injection::send_key("Delete")?;
        }
        "select_all" => injection::send_key("ctrl+a")?,
        "copy" => injection::send_key("ctrl+c")?,
        "paste" => injection::send_key("ctrl+v")?,
        "cut" => injection::send_key("ctrl+x")?,
        _ => {
            log::info!("Custom dictation command: {}", action);
        }
    }

    Ok(CommandResult::DictationAction(action.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_from_str() {
        assert_eq!(AppMode::from_str("desktop"), Some(AppMode::Desktop));
        assert_eq!(AppMode::from_str("dictation"), Some(AppMode::Dictation));
        assert_eq!(AppMode::from_str("diktat"), Some(AppMode::Dictation));
        assert_eq!(AppMode::from_str("sleep"), Some(AppMode::Sleep));
        assert_eq!(AppMode::from_str("schlaf"), Some(AppMode::Sleep));
        assert_eq!(AppMode::from_str("unknown"), None);
    }

    #[test]
    fn test_sleep_only_responds_to_wake() {
        let keywords = test_keywords();
        let mut mode = AppMode::Sleep;

        // Random speech in sleep mode
        let result = process_speech("hello world", &mut mode, &keywords, "pilot").unwrap();
        assert!(matches!(result, CommandResult::Sleeping));
        assert_eq!(mode, AppMode::Sleep);

        // Wake phrase
        let result = process_speech("pilot wake up", &mut mode, &keywords, "pilot").unwrap();
        assert!(matches!(result, CommandResult::ModeChanged(AppMode::Desktop)));
        assert_eq!(mode, AppMode::Desktop);
    }

    #[test]
    fn test_mode_switch() {
        let keywords = test_keywords();
        let mut mode = AppMode::Desktop;

        let result =
            process_speech("pilot mode dictation", &mut mode, &keywords, "pilot").unwrap();
        assert!(matches!(result, CommandResult::ModeChanged(AppMode::Dictation)));
        assert_eq!(mode, AppMode::Dictation);
    }

    #[test]
    fn test_sleep_command() {
        let keywords = test_keywords();
        let mut mode = AppMode::Desktop;

        let result = process_speech("pilot sleep", &mut mode, &keywords, "pilot").unwrap();
        assert!(matches!(result, CommandResult::ModeChanged(AppMode::Sleep)));
    }

    #[test]
    fn test_key_command_check() {
        let keywords = test_keywords();

        let result = check_key_command("key enter", &keywords);
        assert!(result.is_some());

        let result = check_key_command("key tab", &keywords);
        assert!(result.is_some());

        // No match
        let result = check_key_command("key unknown", &keywords);
        assert!(result.is_none());

        // Not prefixed
        let result = check_key_command("enter", &keywords);
        assert!(result.is_none());
    }

    fn test_keywords() -> KeywordConfig {
        use std::collections::HashMap;

        KeywordConfig {
            language: "en".into(),
            modes: HashMap::from([
                ("desktop".into(), crate::keywords::ModeInfo {
                    name: "desktop".into(),
                    description: "Desktop mode".into(),
                }),
                ("dictation".into(), crate::keywords::ModeInfo {
                    name: "dictation".into(),
                    description: "Dictation mode".into(),
                }),
                ("terminal".into(), crate::keywords::ModeInfo {
                    name: "terminal".into(),
                    description: "Terminal mode".into(),
                }),
                ("sleep".into(), crate::keywords::ModeInfo {
                    name: "sleep".into(),
                    description: "Sleep mode".into(),
                }),
            ]),
            mode_switch: "mode".into(),
            wake_phrase: "wake up".into(),
            sleep_phrase: "sleep".into(),
            commands: HashMap::from([
                ("open".into(), "open".into()),
                ("close".into(), "close".into()),
                ("switch".into(), "switch".into()),
            ]),
            dictation: HashMap::from([
                ("new_line".into(), "new line".into()),
                ("copy".into(), "copy".into()),
            ]),
            key_prefix: "key".into(),
            key_prefix_aliases: vec!["keys".into()],
            keys: HashMap::from([
                ("enter".into(), "Return".into()),
                ("tab".into(), "Tab".into()),
                ("escape".into(), "Escape".into()),
            ]),
        }
    }
}
