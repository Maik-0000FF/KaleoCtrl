//! Speech-to-action pipeline.
//!
//! The flow is split into two pure-ish layers:
//!
//! * [`plan`] turns a recognized utterance + current mode + config into an
//!   [`Action`]. It performs no side effects and is fully unit-testable.
//! * [`execute`] takes an [`Action`] and performs the side effect (spawn a
//!   process, inject a key, etc.), returning a [`CommandResult`] for the UI.
//!
//! Adding a new voice command means: add an [`Action`] variant, handle it in
//! [`system_action`] / [`dictation_action`] (or wherever the planner produces
//! the variant), and handle it in [`execute`]. Tests target [`plan`] and the
//! small mapping helpers — no mocking of subprocesses needed.

use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::injection;
use crate::keywords::KeywordConfig;

// ────────────────────────────────────────────────────────────────────────────
// Text normalization
// ────────────────────────────────────────────────────────────────────────────

/// Strip surrounding punctuation/quotes and lower-case the text.
/// Whisper often emits trailing "." / "?" / "!" or wraps text in smart quotes.
pub fn strip_punctuation(text: &str) -> String {
    text.trim()
        .trim_matches(|c: char| {
            c.is_ascii_punctuation()
                || matches!(
                    c,
                    '\u{201E}' | '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}'
                )
        })
        .to_lowercase()
}

// ────────────────────────────────────────────────────────────────────────────
// Application mode
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

// ────────────────────────────────────────────────────────────────────────────
// Action: pure description of what should happen
// ────────────────────────────────────────────────────────────────────────────

/// What [`plan`] decided to do. No side effects yet — [`execute`] runs them.
///
/// New voice commands typically add one variant here, one branch in the
/// matching planner helper, and one branch in [`execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// No matched intent (empty utterance, unmatched desktop command).
    Nothing,
    /// Speech occurred in [`AppMode::Sleep`] but was not the wake phrase.
    AsleepIgnored,
    /// Switch to a new mode.
    SetMode(AppMode),
    /// Launch an application (args = everything after the keyword).
    OpenApp(String),
    /// Close a window. `None` = active window, `Some(name)` = match by name.
    CloseWindow(Option<String>),
    /// Switch focus to a named window.
    SwitchToWindow(String),
    MinimizeActiveWindow,
    MaximizeActiveWindow,
    FullscreenActiveWindow,
    /// "Stop" command (sends `Escape`).
    StopCurrent,
    Undo,
    /// Command keyword whose action name isn't in the built-in set.
    /// Currently logged-only; reserved for user-defined commands later.
    UnknownSystemAction { action: String, args: String },
    /// Single-key press, addressed via the key-prefix path
    /// (e.g. "taste enter" → `PressKey("Return")`).
    PressKey(String),
    /// Named dictation command resolved to a key sequence
    /// (e.g. "new_paragraph" → `["Return", "Return"]`).
    DictationKeys {
        action_name: String,
        keys: Vec<String>,
    },
    /// Dictation command name that isn't in the built-in set.
    UnknownDictationAction(String),
    /// Verbatim text to type into the active window.
    TypeText(String),
}

// ────────────────────────────────────────────────────────────────────────────
// CommandResult: reported back to the UI
// ────────────────────────────────────────────────────────────────────────────

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

// ────────────────────────────────────────────────────────────────────────────
// Planner (pure)
// ────────────────────────────────────────────────────────────────────────────

/// Decide what action a recognized utterance should trigger.
///
/// Pure: no side effects. Returns the planned [`Action`] and the new mode
/// (mode changes are part of the decision, not of execution).
pub fn plan(
    text: &str,
    mode: AppMode,
    keywords: &KeywordConfig,
    assistant_name: &str,
) -> (Action, AppMode) {
    let text = strip_punctuation(text);
    if text.is_empty() {
        return (Action::Nothing, mode);
    }

    let name = assistant_name.to_lowercase();

    // Sleep mode: only the wake phrase is honored.
    if mode == AppMode::Sleep {
        let wake = format!("{} {}", name, keywords.wake_phrase.to_lowercase());
        if text.contains(&wake) {
            return (Action::SetMode(AppMode::Desktop), AppMode::Desktop);
        }
        return (Action::AsleepIgnored, AppMode::Sleep);
    }

    // Global sleep command.
    let sleep_cmd = format!("{} {}", name, keywords.sleep_phrase.to_lowercase());
    if text.contains(&sleep_cmd) {
        return (Action::SetMode(AppMode::Sleep), AppMode::Sleep);
    }

    // Mode switch: "<name> <mode_switch> <mode-name>".
    let mode_prefix = format!("{} {}", name, keywords.mode_switch.to_lowercase());
    if let Some(rest) = text.strip_prefix(&mode_prefix) {
        let rest = rest.trim();
        for (key, info) in &keywords.modes {
            if rest == key || rest == info.name.to_lowercase() {
                if let Some(new_mode) = AppMode::from_str(key) {
                    return (Action::SetMode(new_mode), new_mode);
                }
            }
        }
    }

    // Key-press shortcut: "<key_prefix> <key_name>", works in any active mode.
    if let Some(key) = plan_key_command(&text, keywords) {
        return (Action::PressKey(key), mode);
    }

    let action = match mode {
        AppMode::Desktop => plan_desktop(&text, keywords, &name),
        AppMode::Dictation => plan_dictation(&text, keywords, &name),
        AppMode::Terminal => plan_terminal(&text, &name),
        AppMode::Sleep => Action::AsleepIgnored, // unreachable: handled above
    };
    (action, mode)
}

fn plan_desktop(text: &str, keywords: &KeywordConfig, name: &str) -> Action {
    // In desktop mode the prefix is optional; if present, strip it.
    let command_text = text
        .strip_prefix(name)
        .map(str::trim)
        .unwrap_or(text);

    match match_command_keyword(command_text, &keywords.commands) {
        Some((action_name, args)) => system_action(action_name, args),
        None => Action::Nothing,
    }
}

fn plan_dictation(text: &str, keywords: &KeywordConfig, name: &str) -> Action {
    // In dictation mode the prefix is mandatory for commands.
    if let Some(command_text) = text.strip_prefix(name) {
        let command_text = command_text.trim();

        if let Some((action_name, args)) = match_command_keyword(command_text, &keywords.commands) {
            return system_action(action_name, args);
        }

        if let Some(action_name) = match_dictation_keyword(command_text, &keywords.dictation) {
            return dictation_action(action_name);
        }
    }

    // Default: inject verbatim.
    Action::TypeText(text.to_string())
}

fn plan_terminal(text: &str, name: &str) -> Action {
    let command_text = text
        .strip_prefix(name)
        .map(str::trim)
        .unwrap_or(text);
    Action::TypeText(command_text.to_string())
}

/// Pure: if `text` starts with the key-prefix (or one of its aliases) and the
/// remainder matches a spoken key name, return the key spec to press.
fn plan_key_command(text: &str, keywords: &KeywordConfig) -> Option<String> {
    let prefix = keywords.key_prefix.to_lowercase();
    if prefix.is_empty() {
        return None;
    }

    // Collect main prefix + aliases, then try the longest first so a longer
    // alias ("keys") wins over a shorter prefix it contains ("key").
    let mut prefixes: Vec<String> = std::iter::once(prefix)
        .chain(keywords.key_prefix_aliases.iter().map(|s| s.to_lowercase()))
        .collect();
    prefixes.sort_by_key(|p| std::cmp::Reverse(p.len()));

    let rest = prefixes
        .iter()
        .find_map(|p| text.strip_prefix(p.as_str()))
        .map(strip_punctuation)
        .filter(|s| !s.is_empty())?;

    keywords
        .keys
        .iter()
        .find(|(spoken, _)| spoken.to_lowercase() == rest)
        .map(|(_, key_action)| key_action.clone())
}

/// Search the command map for a keyword that prefixes `text`. Returns
/// `(action_name, args)` where `args` is the remainder after the keyword,
/// trimmed.
fn match_command_keyword<'a>(
    text: &'a str,
    commands: &'a std::collections::HashMap<String, String>,
) -> Option<(&'a str, &'a str)> {
    for (action_name, keyword) in commands {
        let kw = keyword.to_lowercase();
        // We need keyword as &str for strip_prefix; build a stable lowercase
        // by relying on the keyword *already* being lowercase in the file.
        // The map stores canonical keywords; lowercase them on the fly:
        if let Some(args) = text
            .strip_prefix(&kw)
            .or_else(|| text.strip_prefix(keyword.as_str()))
        {
            return Some((action_name.as_str(), args.trim()));
        }
    }
    None
}

/// Match a dictation keyword that the user spoke (exact or prefix match).
fn match_dictation_keyword<'a>(
    text: &str,
    dictation: &'a std::collections::HashMap<String, String>,
) -> Option<&'a str> {
    for (action_name, keyword) in dictation {
        let kw = keyword.to_lowercase();
        if text == kw || text.starts_with(&kw) {
            return Some(action_name.as_str());
        }
    }
    None
}

/// Map a system-command action name + args to the planned [`Action`].
/// Adding a new system command starts here.
fn system_action(action: &str, args: &str) -> Action {
    let args = args.to_string();
    match action {
        "open" => Action::OpenApp(args),
        "close" => Action::CloseWindow(if args.is_empty() { None } else { Some(args) }),
        "switch" => Action::SwitchToWindow(args),
        "minimize" => Action::MinimizeActiveWindow,
        "maximize" => Action::MaximizeActiveWindow,
        "fullscreen" => Action::FullscreenActiveWindow,
        "stop" => Action::StopCurrent,
        "undo" => Action::Undo,
        _ => Action::UnknownSystemAction {
            action: action.to_string(),
            args,
        },
    }
}

/// Map a dictation action name to a planned key sequence.
/// Adding a new dictation command starts here.
fn dictation_action(action: &str) -> Action {
    let keys: Vec<&str> = match action {
        "new_line" => vec!["Return"],
        "new_paragraph" => vec!["Return", "Return"],
        "delete_word" => vec!["ctrl+BackSpace"],
        "delete_sentence" => vec!["Home", "shift+End", "Delete"],
        "select_all" => vec!["ctrl+a"],
        "copy" => vec!["ctrl+c"],
        "paste" => vec!["ctrl+v"],
        "cut" => vec!["ctrl+x"],
        _ => return Action::UnknownDictationAction(action.to_string()),
    };
    Action::DictationKeys {
        action_name: action.to_string(),
        keys: keys.into_iter().map(String::from).collect(),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Executor abstraction: Action → side effects
// ────────────────────────────────────────────────────────────────────────────

/// Performs the side effect for a planned [`Action`].
///
/// Production code uses [`RealExecutor`], which spawns processes and injects
/// keys/text. Tests pass a fake implementation that records actions so the
/// dispatch path can be verified without touching the host system.
pub trait CommandExecutor {
    fn run(&mut self, action: Action) -> Result<CommandResult, AppError>;
}

/// Production executor: invokes `setsid`/`wmctrl`/`xdotool` and the injection
/// layer. **Never** runnable in test builds — the trait impl panics on
/// `#[cfg(test)]` so an accidental call from a unit test fails loudly instead
/// of leaking a key press or a typed sentence into the developer's session.
pub struct RealExecutor;

#[cfg(not(test))]
impl CommandExecutor for RealExecutor {
    fn run(&mut self, action: Action) -> Result<CommandResult, AppError> {
        match action {
            Action::Nothing => Ok(CommandResult::Ignored),
            Action::AsleepIgnored => Ok(CommandResult::Sleeping),

            Action::SetMode(m) => Ok(CommandResult::ModeChanged(m)),

            Action::OpenApp(args) => {
                if !args.is_empty() {
                    Command::new("setsid")
                        .args([&args])
                        .spawn()
                        .map_err(|e| {
                            AppError::Config(format!("Failed to open '{}': {}", args, e))
                        })?;
                }
                Ok(CommandResult::SystemCommand(format!("open {}", args)))
            }

            Action::CloseWindow(target) => {
                let arg = target.clone().unwrap_or_else(|| ":ACTIVE:".to_string());
                let _ = Command::new("wmctrl").args(["-c", &arg]).output();
                Ok(CommandResult::SystemCommand(format!(
                    "close {}",
                    target.unwrap_or_default()
                )))
            }

            Action::SwitchToWindow(args) => {
                if !args.is_empty() {
                    let _ = Command::new("wmctrl").args(["-a", &args]).output();
                }
                Ok(CommandResult::SystemCommand(format!("switch {}", args)))
            }

            Action::MinimizeActiveWindow => {
                let _ = Command::new("xdotool")
                    .args(["getactivewindow", "windowminimize"])
                    .output();
                Ok(CommandResult::SystemCommand("minimize".into()))
            }

            Action::MaximizeActiveWindow => {
                let _ = Command::new("wmctrl")
                    .args(["-r", ":ACTIVE:", "-b", "toggle,maximized_vert,maximized_horz"])
                    .output();
                Ok(CommandResult::SystemCommand("maximize".into()))
            }

            Action::FullscreenActiveWindow => {
                let _ = Command::new("wmctrl")
                    .args(["-r", ":ACTIVE:", "-b", "toggle,fullscreen"])
                    .output();
                Ok(CommandResult::SystemCommand("fullscreen".into()))
            }

            Action::StopCurrent => {
                let _ = injection::send_key("Escape");
                Ok(CommandResult::SystemCommand("stop".into()))
            }

            Action::Undo => {
                let _ = injection::send_key("ctrl+z");
                Ok(CommandResult::SystemCommand("undo".into()))
            }

            Action::UnknownSystemAction { action, args } => {
                log::info!("Custom system command: {} {}", action, args);
                Ok(CommandResult::SystemCommand(format!("{} {}", action, args)))
            }

            Action::PressKey(key) => {
                injection::send_key(&key)?;
                Ok(CommandResult::KeyPressed(key))
            }

            Action::DictationKeys { action_name, keys } => {
                for k in &keys {
                    injection::send_key(k)?;
                }
                Ok(CommandResult::DictationAction(action_name))
            }

            Action::UnknownDictationAction(name) => {
                log::info!("Custom dictation command: {}", name);
                Ok(CommandResult::DictationAction(name))
            }

            Action::TypeText(text) => {
                injection::type_text(&text)?;
                Ok(CommandResult::TextInjected(text))
            }
        }
    }
}

#[cfg(test)]
impl CommandExecutor for RealExecutor {
    fn run(&mut self, action: Action) -> Result<CommandResult, AppError> {
        panic!(
            "RealExecutor::run() invoked in test build (action: {:?}). \
             Tests must either: (a) call plan() and assert on the returned \
             Action, or (b) pass a fake CommandExecutor to \
             process_speech_with() / check_key_command_with(). RealExecutor \
             spawns processes and injects keys — never let it run in tests.",
            action
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Public API used by lib.rs
// ────────────────────────────────────────────────────────────────────────────

/// Process recognized speech with [`RealExecutor`]. In test builds the
/// executor panics, so unit tests must use [`process_speech_with`] + fake.
pub fn process_speech(
    text: &str,
    mode: &mut AppMode,
    keywords: &KeywordConfig,
    assistant_name: &str,
) -> Result<CommandResult, AppError> {
    process_speech_with(text, mode, keywords, assistant_name, &mut RealExecutor)
}

/// Same as [`process_speech`] but with a caller-supplied executor — the
/// extension point for tests (fake executor) and for any future caller that
/// wants to intercept actions before they hit the system.
pub fn process_speech_with<E: CommandExecutor>(
    text: &str,
    mode: &mut AppMode,
    keywords: &KeywordConfig,
    assistant_name: &str,
    executor: &mut E,
) -> Result<CommandResult, AppError> {
    let (action, new_mode) = plan(text, *mode, keywords, assistant_name);
    *mode = new_mode;
    executor.run(action)
}

/// Buffered key-prefix path: lib.rs calls this with `"<prefix> <key>"` after
/// the user paused between the prefix and the key name. Returns `None` if no
/// key matched (caller falls through to normal processing).
pub fn check_key_command(
    text: &str,
    keywords: &KeywordConfig,
) -> Option<Result<CommandResult, AppError>> {
    check_key_command_with(text, keywords, &mut RealExecutor)
}

/// Same as [`check_key_command`] with a caller-supplied executor.
pub fn check_key_command_with<E: CommandExecutor>(
    text: &str,
    keywords: &KeywordConfig,
    executor: &mut E,
) -> Option<Result<CommandResult, AppError>> {
    let text = strip_punctuation(text);
    let key = plan_key_command(&text, keywords)?;
    Some(executor.run(Action::PressKey(key)))
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    //! TEST ISOLATION
    //!
    //! Three layers prevent the test run from touching the developer's
    //! session (subprocesses, key injection, typed text):
    //!
    //! 1. Tests assert on the output of `plan()` and the pure helpers
    //!    (`system_action`, `dictation_action`, `plan_key_command`,
    //!    `strip_punctuation`, `AppMode::from_str`).
    //! 2. When the full dispatch path needs to be tested, pass a
    //!    `FakeExecutor` (see `fake_executor_records_*`) to
    //!    `process_speech_with` / `check_key_command_with`. The fake
    //!    records actions and performs no I/O.
    //! 3. `RealExecutor::run` is `#[cfg(test)] panic!()`. Anything that
    //!    accidentally reaches it crashes loudly instead of silently
    //!    leaking side effects.
    //!
    //! If you find yourself wanting to test `process_speech` or
    //! `check_key_command` directly: don't — use the `_with` variants.
    use super::*;
    use std::collections::HashMap;

    use crate::keywords::ModeInfo;

    // ── Fixtures ──────────────────────────────────────────────────────────

    fn fixture_keywords() -> KeywordConfig {
        KeywordConfig {
            language: "en".into(),
            modes: HashMap::from([
                ("desktop".into(), ModeInfo {
                    name: "desktop".into(),
                    description: "Desktop mode".into(),
                }),
                ("dictation".into(), ModeInfo {
                    name: "dictation".into(),
                    description: "Dictation mode".into(),
                }),
                ("terminal".into(), ModeInfo {
                    name: "terminal".into(),
                    description: "Terminal mode".into(),
                }),
                ("sleep".into(), ModeInfo {
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
                ("minimize".into(), "minimize".into()),
                ("maximize".into(), "maximize".into()),
                ("fullscreen".into(), "fullscreen".into()),
                ("stop".into(), "stop".into()),
                ("undo".into(), "undo".into()),
            ]),
            dictation: HashMap::from([
                ("new_line".into(), "new line".into()),
                ("new_paragraph".into(), "new paragraph".into()),
                ("copy".into(), "copy".into()),
                ("paste".into(), "paste".into()),
                ("delete_sentence".into(), "delete sentence".into()),
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

    fn plan_only(text: &str, mode: AppMode, name: &str) -> Action {
        plan(text, mode, &fixture_keywords(), name).0
    }

    // ── Text normalization ────────────────────────────────────────────────

    #[test]
    fn strip_punctuation_strips_trailing_ascii_punct() {
        assert_eq!(strip_punctuation("Hello world!"), "hello world");
        assert_eq!(strip_punctuation("Hello, world?"), "hello, world");
    }

    #[test]
    fn strip_punctuation_strips_smart_quotes() {
        // German low-9 and English curly quotes
        assert_eq!(strip_punctuation("\u{201E}hello\u{201C}"), "hello");
        assert_eq!(strip_punctuation("\u{2018}hello\u{2019}"), "hello");
        assert_eq!(strip_punctuation("\u{201C}hello\u{201D}"), "hello");
    }

    #[test]
    fn strip_punctuation_lowercases() {
        assert_eq!(strip_punctuation("FIREFOX"), "firefox");
    }

    #[test]
    fn strip_punctuation_handles_empty() {
        assert_eq!(strip_punctuation("   "), "");
        assert_eq!(strip_punctuation("..."), "");
    }

    // ── AppMode parsing ───────────────────────────────────────────────────

    #[test]
    fn appmode_from_str_known_en_de() {
        assert_eq!(AppMode::from_str("desktop"), Some(AppMode::Desktop));
        assert_eq!(AppMode::from_str("dictation"), Some(AppMode::Dictation));
        assert_eq!(AppMode::from_str("diktat"), Some(AppMode::Dictation));
        assert_eq!(AppMode::from_str("sleep"), Some(AppMode::Sleep));
        assert_eq!(AppMode::from_str("schlaf"), Some(AppMode::Sleep));
        assert_eq!(AppMode::from_str("TERMINAL"), Some(AppMode::Terminal));
    }

    #[test]
    fn appmode_from_str_rejects_unknown() {
        assert_eq!(AppMode::from_str("nope"), None);
        assert_eq!(AppMode::from_str(""), None);
    }

    // ── Sleep mode ────────────────────────────────────────────────────────

    #[test]
    fn sleep_mode_ignores_arbitrary_speech() {
        let (action, new_mode) = plan("hello world", AppMode::Sleep, &fixture_keywords(), "pilot");
        assert_eq!(action, Action::AsleepIgnored);
        assert_eq!(new_mode, AppMode::Sleep);
    }

    #[test]
    fn sleep_mode_wakes_on_phrase_with_trailing_punct() {
        let (action, new_mode) = plan("pilot wake up.", AppMode::Sleep, &fixture_keywords(), "pilot");
        assert_eq!(action, Action::SetMode(AppMode::Desktop));
        assert_eq!(new_mode, AppMode::Desktop);
    }

    #[test]
    fn sleep_command_works_from_any_mode() {
        for m in [AppMode::Desktop, AppMode::Dictation, AppMode::Terminal] {
            let (action, new_mode) = plan("pilot sleep", m, &fixture_keywords(), "pilot");
            assert_eq!(action, Action::SetMode(AppMode::Sleep));
            assert_eq!(new_mode, AppMode::Sleep);
        }
    }

    // ── Mode switch ───────────────────────────────────────────────────────

    #[test]
    fn mode_switch_to_dictation() {
        let (action, new_mode) =
            plan("pilot mode dictation", AppMode::Desktop, &fixture_keywords(), "pilot");
        assert_eq!(action, Action::SetMode(AppMode::Dictation));
        assert_eq!(new_mode, AppMode::Dictation);
    }

    #[test]
    fn mode_switch_unknown_target_falls_through() {
        // "pilot mode bogus" — no such mode, falls through to desktop command lookup.
        let action = plan_only("pilot mode bogus", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::Nothing);
    }

    // ── Desktop mode ──────────────────────────────────────────────────────

    #[test]
    fn desktop_open_with_args() {
        let action = plan_only("open firefox", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::OpenApp("firefox".into()));
    }

    #[test]
    fn desktop_open_prefixed_with_assistant_name() {
        let action = plan_only("pilot open firefox", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::OpenApp("firefox".into()));
    }

    #[test]
    fn desktop_close_without_args_targets_active() {
        let action = plan_only("close", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::CloseWindow(None));
    }

    #[test]
    fn desktop_close_with_args_targets_named() {
        let action = plan_only("close terminal", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::CloseWindow(Some("terminal".into())));
    }

    #[test]
    fn desktop_minimize_maximize_fullscreen() {
        assert_eq!(plan_only("minimize", AppMode::Desktop, "pilot"), Action::MinimizeActiveWindow);
        assert_eq!(plan_only("maximize", AppMode::Desktop, "pilot"), Action::MaximizeActiveWindow);
        assert_eq!(plan_only("fullscreen", AppMode::Desktop, "pilot"), Action::FullscreenActiveWindow);
    }

    #[test]
    fn desktop_stop_and_undo() {
        assert_eq!(plan_only("stop", AppMode::Desktop, "pilot"), Action::StopCurrent);
        assert_eq!(plan_only("undo", AppMode::Desktop, "pilot"), Action::Undo);
    }

    #[test]
    fn desktop_unrecognized_speech_is_nothing() {
        let action = plan_only("yodeling", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::Nothing);
    }

    // ── Dictation mode ────────────────────────────────────────────────────

    #[test]
    fn dictation_text_without_prefix_is_typed() {
        let action = plan_only("hello world", AppMode::Dictation, "pilot");
        assert_eq!(action, Action::TypeText("hello world".into()));
    }

    #[test]
    fn dictation_prefix_copy_emits_keys() {
        let action = plan_only("pilot copy", AppMode::Dictation, "pilot");
        assert_eq!(
            action,
            Action::DictationKeys {
                action_name: "copy".into(),
                keys: vec!["ctrl+c".into()],
            }
        );
    }

    #[test]
    fn dictation_prefix_new_paragraph_emits_two_returns() {
        let action = plan_only("pilot new paragraph", AppMode::Dictation, "pilot");
        assert_eq!(
            action,
            Action::DictationKeys {
                action_name: "new_paragraph".into(),
                keys: vec!["Return".into(), "Return".into()],
            }
        );
    }

    #[test]
    fn dictation_prefix_delete_sentence_emits_three_keys() {
        let action = plan_only("pilot delete sentence", AppMode::Dictation, "pilot");
        assert_eq!(
            action,
            Action::DictationKeys {
                action_name: "delete_sentence".into(),
                keys: vec!["Home".into(), "shift+End".into(), "Delete".into()],
            }
        );
    }

    #[test]
    fn dictation_prefix_system_command_works() {
        // System commands also work in dictation mode, but only with the prefix.
        let action = plan_only("pilot open firefox", AppMode::Dictation, "pilot");
        assert_eq!(action, Action::OpenApp("firefox".into()));
    }

    // ── Terminal mode ─────────────────────────────────────────────────────

    #[test]
    fn terminal_types_text_verbatim() {
        let action = plan_only("ls minus l", AppMode::Terminal, "pilot");
        assert_eq!(action, Action::TypeText("ls minus l".into()));
    }

    #[test]
    fn terminal_strips_assistant_prefix() {
        let action = plan_only("pilot ls minus l", AppMode::Terminal, "pilot");
        assert_eq!(action, Action::TypeText("ls minus l".into()));
    }

    // ── Key prefix ────────────────────────────────────────────────────────

    #[test]
    fn key_prefix_matches_known_key() {
        let action = plan_only("key enter", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::PressKey("Return".into()));
    }

    #[test]
    fn key_prefix_alias_works() {
        // "keys" is an alias for "key" in the fixture.
        let action = plan_only("keys tab", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::PressKey("Tab".into()));
    }

    #[test]
    fn key_prefix_unknown_key_does_not_match() {
        let action = plan_only("key unknown", AppMode::Desktop, "pilot");
        assert_eq!(action, Action::Nothing);
    }

    #[test]
    fn key_prefix_works_in_dictation() {
        let action = plan_only("key escape", AppMode::Dictation, "pilot");
        assert_eq!(action, Action::PressKey("Escape".into()));
    }

    // The public `check_key_command` wrapper goes through `execute` and
    // would press a real key — tested only at the pure planning layer
    // (`plan_key_command`) below to keep the test run isolated.

    #[test]
    fn plan_key_command_returns_none_without_prefix() {
        assert_eq!(plan_key_command("enter", &fixture_keywords()), None);
    }

    #[test]
    fn plan_key_command_returns_none_for_unknown_key() {
        assert_eq!(plan_key_command("key wibble", &fixture_keywords()), None);
    }

    #[test]
    fn plan_key_command_returns_key_spec_for_known_key() {
        assert_eq!(
            plan_key_command("key enter", &fixture_keywords()),
            Some("Return".into())
        );
    }

    // ── Pure mapping helpers ──────────────────────────────────────────────

    #[test]
    fn system_action_known_names() {
        assert_eq!(system_action("open", "firefox"), Action::OpenApp("firefox".into()));
        assert_eq!(system_action("close", ""), Action::CloseWindow(None));
        assert_eq!(system_action("close", "x"), Action::CloseWindow(Some("x".into())));
        assert_eq!(system_action("minimize", ""), Action::MinimizeActiveWindow);
    }

    #[test]
    fn system_action_unknown_falls_through_to_custom() {
        assert_eq!(
            system_action("salsa", "spicy"),
            Action::UnknownSystemAction {
                action: "salsa".into(),
                args: "spicy".into()
            }
        );
    }

    #[test]
    fn dictation_action_known_names() {
        assert!(matches!(
            dictation_action("copy"),
            Action::DictationKeys { ref action_name, .. } if action_name == "copy"
        ));
    }

    #[test]
    fn dictation_action_unknown_falls_through() {
        assert_eq!(
            dictation_action("groove"),
            Action::UnknownDictationAction("groove".into())
        );
    }

    // ── plan() returns the right new_mode for the wrapper to write back ──

    #[test]
    fn plan_returns_new_mode_on_switch() {
        let kw = fixture_keywords();
        let (action, new_mode) = plan("pilot sleep", AppMode::Desktop, &kw, "pilot");
        assert_eq!(action, Action::SetMode(AppMode::Sleep));
        assert_eq!(new_mode, AppMode::Sleep);
    }

    #[test]
    fn plan_keeps_mode_when_no_switch() {
        let kw = fixture_keywords();
        let (_, new_mode) = plan("hello world", AppMode::Dictation, &kw, "pilot");
        assert_eq!(new_mode, AppMode::Dictation);
    }

    // ── Dependency injection: dispatch path via FakeExecutor ──────────────

    /// Records every action passed to it and returns a benign `Ignored`
    /// result. Performs no I/O.
    struct FakeExecutor {
        recorded: Vec<Action>,
    }

    impl FakeExecutor {
        fn new() -> Self {
            Self { recorded: Vec::new() }
        }
    }

    impl CommandExecutor for FakeExecutor {
        fn run(&mut self, action: Action) -> Result<CommandResult, AppError> {
            self.recorded.push(action);
            Ok(CommandResult::Ignored)
        }
    }

    #[test]
    fn fake_executor_records_open_app_in_desktop() {
        let kw = fixture_keywords();
        let mut mode = AppMode::Desktop;
        let mut fake = FakeExecutor::new();
        let _ = process_speech_with("open firefox", &mut mode, &kw, "pilot", &mut fake).unwrap();
        assert_eq!(fake.recorded, vec![Action::OpenApp("firefox".into())]);
    }

    #[test]
    fn fake_executor_records_type_text_in_dictation() {
        let kw = fixture_keywords();
        let mut mode = AppMode::Dictation;
        let mut fake = FakeExecutor::new();
        let _ = process_speech_with("hello world", &mut mode, &kw, "pilot", &mut fake).unwrap();
        assert_eq!(fake.recorded, vec![Action::TypeText("hello world".into())]);
    }

    #[test]
    fn fake_executor_records_dictation_keys_for_copy() {
        let kw = fixture_keywords();
        let mut mode = AppMode::Dictation;
        let mut fake = FakeExecutor::new();
        let _ = process_speech_with("pilot copy", &mut mode, &kw, "pilot", &mut fake).unwrap();
        assert_eq!(
            fake.recorded,
            vec![Action::DictationKeys {
                action_name: "copy".into(),
                keys: vec!["ctrl+c".into()],
            }]
        );
    }

    #[test]
    fn check_key_command_with_fake_records_press_key() {
        let kw = fixture_keywords();
        let mut fake = FakeExecutor::new();
        let result = check_key_command_with("key enter", &kw, &mut fake);
        assert!(result.is_some());
        assert_eq!(fake.recorded, vec![Action::PressKey("Return".into())]);
    }

    // ── Guard: RealExecutor must never run in tests ───────────────────────

    #[test]
    #[should_panic(expected = "RealExecutor::run() invoked in test build")]
    fn real_executor_panics_in_tests() {
        let _ = RealExecutor.run(Action::Nothing);
    }
}
