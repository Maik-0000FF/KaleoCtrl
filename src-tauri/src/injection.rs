use std::process::Command;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    Wayland,
    X11,
}

impl DisplayServer {
    pub fn detect() -> Self {
        if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
            match session.to_lowercase().as_str() {
                "wayland" => return DisplayServer::Wayland,
                "x11" => return DisplayServer::X11,
                _ => {}
            }
        }
        // Fallback: check for WAYLAND_DISPLAY
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return DisplayServer::Wayland;
        }
        if std::env::var("DISPLAY").is_ok() {
            return DisplayServer::X11;
        }
        // Default to X11
        DisplayServer::X11
    }
}

/// Type text into the currently focused application
pub fn type_text(text: &str) -> Result<(), AppError> {
    if text.is_empty() {
        return Ok(());
    }

    match DisplayServer::detect() {
        DisplayServer::Wayland => type_text_wayland(text),
        DisplayServer::X11 => type_text_x11(text),
    }
}

/// Simulate a key combination (e.g. "ctrl+v", "Return", "BackSpace")
pub fn send_key(key: &str) -> Result<(), AppError> {
    match DisplayServer::detect() {
        DisplayServer::Wayland => send_key_wayland(key),
        DisplayServer::X11 => send_key_x11(key),
    }
}

// --- Wayland ---

fn type_text_wayland(text: &str) -> Result<(), AppError> {
    // Try wtype first (needs virtual-keyboard protocol, e.g. wlroots compositors)
    if try_wtype_text(text) {
        return Ok(());
    }

    // Primary method: ydotool (works on all Wayland compositors)
    if try_ydotool_type(text) {
        return Ok(());
    }

    // Last resort: clipboard-based (wl-copy + Ctrl+V via xdotool/XWayland)
    type_text_clipboard(text)
}

fn try_wtype_text(text: &str) -> bool {
    Command::new("wtype")
        .arg("--")
        .arg(text)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn try_ydotool_type(text: &str) -> bool {
    Command::new("ydotool")
        .args(["type", "--", text])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn type_text_clipboard(text: &str) -> Result<(), AppError> {
    // Save current clipboard
    let old_clip = Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

    Command::new("wl-copy")
        .arg("--")
        .arg(text)
        .output()
        .map_err(|e| {
            AppError::Config(format!("wl-copy failed: {} (is wl-clipboard installed?)", e))
        })?;

    std::thread::sleep(std::time::Duration::from_millis(30));

    // Try ydotool for paste keystroke, fall back to xdotool
    if !try_ydotool_key("29:1", "47:1", "47:0", "29:0") {
        // ctrl down, v down, v up, ctrl up
        let _ = send_key_x11("ctrl+v");
    }

    // Restore previous clipboard
    std::thread::sleep(std::time::Duration::from_millis(50));
    if let Some(old) = old_clip {
        let _ = Command::new("wl-copy").arg("--").arg(&old).output();
    }

    Ok(())
}

/// Send raw keycodes via ydotool (press/release pairs)
fn try_ydotool_key(k1: &str, k2: &str, k3: &str, k4: &str) -> bool {
    Command::new("ydotool")
        .args(["key", k1, k2, k3, k4])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn send_key_wayland(key: &str) -> Result<(), AppError> {
    // Try wtype first
    if try_wtype_key(key) {
        return Ok(());
    }

    // Try ydotool with evdev keycodes
    if let Some(codes) = key_to_evdev(key) {
        let args: Vec<String> = codes
            .iter()
            .flat_map(|&code| {
                // press then release
                [format!("{}:1", code), format!("{}:0", code)]
            })
            .collect();
        let result = Command::new("ydotool")
            .arg("key")
            .args(&args)
            .output();
        if let Ok(output) = result {
            if output.status.success() {
                return Ok(());
            }
        }
    }

    // Fallback to xdotool via XWayland
    send_key_x11(key)
}

fn try_wtype_key(key: &str) -> bool {
    let parts: Vec<&str> = key.split('+').collect();
    let mut args: Vec<&str> = Vec::new();

    for modifier in parts.iter().take(parts.len().saturating_sub(1)) {
        args.push("-M");
        args.push(modifier);
    }
    if let Some(final_key) = parts.last() {
        args.push("-k");
        args.push(final_key);
    }
    for modifier in parts.iter().take(parts.len().saturating_sub(1)).rev() {
        args.push("-m");
        args.push(modifier);
    }

    Command::new("wtype")
        .args(&args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Map common key combos to evdev keycodes
fn key_to_evdev(key: &str) -> Option<Vec<u16>> {
    let parts: Vec<&str> = key.split('+').collect();
    let mut codes = Vec::new();

    for part in &parts {
        let code = match part.to_lowercase().as_str() {
            "ctrl" | "control" => 29,
            "shift" => 42,
            "alt" => 56,
            "super" | "meta" => 125,
            "return" | "enter" => 28,
            "escape" => 1,
            "backspace" => 14,
            "delete" => 111,
            "tab" => 15,
            "home" => 102,
            "end" => 107,
            "a" => 30,
            "b" => 48,
            "c" => 46,
            "v" => 47,
            "x" => 45,
            "z" => 44,
            _ => return None,
        };
        codes.push(code);
    }

    Some(codes)
}

// --- X11 ---

fn type_text_x11(text: &str) -> Result<(), AppError> {
    Command::new("xdotool")
        .args(["type", "--clearmodifiers", "--delay", "10", "--", text])
        .output()
        .map_err(|e| AppError::Config(format!("xdotool failed: {} (is xdotool installed?)", e)))?;
    Ok(())
}

fn send_key_x11(key: &str) -> Result<(), AppError> {
    Command::new("xdotool")
        .args(["key", "--clearmodifiers", key])
        .output()
        .map_err(|e| AppError::Config(format!("xdotool key failed: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_server_detect() {
        // Should not panic regardless of environment
        let _ds = DisplayServer::detect();
    }

    #[test]
    fn test_key_to_evdev() {
        assert_eq!(key_to_evdev("ctrl+v"), Some(vec![29, 47]));
        assert_eq!(key_to_evdev("ctrl+c"), Some(vec![29, 46]));
        assert_eq!(key_to_evdev("Return"), Some(vec![28]));
        assert_eq!(key_to_evdev("unknown_key"), None);
    }
}
