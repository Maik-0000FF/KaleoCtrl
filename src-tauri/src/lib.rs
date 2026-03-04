mod audio;
mod commander;
mod config;
mod error;
mod injection;
mod keywords;
mod stt;

use std::sync::Mutex;

use config::{resolve_config_dir, AppConfig, ConfigState};
use error::AppError;
use keywords::KeywordConfig;
use stt::whisper::WhisperEngine;
use stt::{SttEngine, SttStatus};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::webview::WebviewWindowBuilder;
use tauri::{Emitter, Manager, State};

use audio::AudioCapture;
use commander::{AppMode, CommandResult};

// --- App State ---

pub struct SttState {
    engine: Mutex<WhisperEngine>,
}

pub struct AudioState {
    capture: Mutex<AudioCapture>,
}

pub struct ModeState {
    mode: Mutex<AppMode>,
}

// --- Config Commands ---

#[tauri::command]
fn get_config(state: State<ConfigState>) -> Result<AppConfig, AppError> {
    let config = state.config.lock().unwrap();
    Ok(config.clone())
}

#[tauri::command]
fn update_config(state: State<ConfigState>, config: AppConfig) -> Result<(), AppError> {
    {
        let mut current = state.config.lock().unwrap();
        *current = config;
    }
    state.save()?;
    Ok(())
}

#[tauri::command]
fn get_keywords(
    state: State<ConfigState>,
    language: Option<String>,
) -> Result<KeywordConfig, AppError> {
    let lang = match language {
        Some(l) => l,
        None => {
            let config = state.config.lock().unwrap();
            config.language.clone()
        }
    };
    keywords::load_keywords(&state.config_dir, &lang)
}

#[tauri::command]
fn save_keywords(state: State<ConfigState>, keywords: KeywordConfig) -> Result<(), AppError> {
    keywords::save_keywords(&state.config_dir, &keywords)
}

#[tauri::command]
fn get_available_languages(state: State<ConfigState>) -> Result<Vec<String>, AppError> {
    keywords::list_available_languages(&state.config_dir)
}

// --- STT Commands ---

#[tauri::command]
fn load_stt_model(
    state: State<SttState>,
    config_state: State<ConfigState>,
    model_path: String,
) -> Result<(), AppError> {
    // Resolve relative path from project root (config_dir parent)
    let resolved = if std::path::Path::new(&model_path).is_absolute() {
        std::path::PathBuf::from(&model_path)
    } else {
        config_state
            .config_dir
            .parent()
            .unwrap_or(&config_state.config_dir)
            .join(&model_path)
    };

    let resolved_str = resolved.to_string_lossy();
    if !resolved.exists() {
        return Err(AppError::Config(format!(
            "Model file not found: {}",
            resolved_str
        )));
    }

    let mut engine = state.engine.lock().unwrap();
    engine.load_model(&resolved_str)
}

#[tauri::command]
fn unload_stt_model(
    stt_state: State<SttState>,
    audio_state: State<AudioState>,
) -> Result<(), AppError> {
    // Stop listening first
    let mut capture = audio_state.capture.lock().unwrap();
    capture.stop();
    drop(capture);

    // Unload model to free GPU/RAM
    let mut engine = stt_state.engine.lock().unwrap();
    engine.unload_model();
    Ok(())
}

#[tauri::command]
fn get_stt_status(state: State<SttState>) -> SttStatus {
    let engine = state.engine.lock().unwrap();
    SttStatus {
        engine: engine.engine_name().to_string(),
        model_loaded: engine.is_model_loaded(),
        current_model: engine.current_model().map(|s| s.to_string()),
    }
}

// --- Audio Commands ---

#[tauri::command]
fn start_listening(audio_state: State<AudioState>) -> Result<(), AppError> {
    let mut capture = audio_state.capture.lock().unwrap();
    capture.start()
}

#[tauri::command]
fn stop_listening(audio_state: State<AudioState>) -> Result<(), AppError> {
    let mut capture = audio_state.capture.lock().unwrap();
    capture.stop();
    Ok(())
}

#[tauri::command]
fn get_listening_status(audio_state: State<AudioState>) -> bool {
    let capture = audio_state.capture.lock().unwrap();
    capture.is_recording.load(std::sync::atomic::Ordering::Relaxed)
}

// --- Mode Commands ---

#[tauri::command]
fn get_mode(mode_state: State<ModeState>) -> AppMode {
    let mode = mode_state.mode.lock().unwrap();
    *mode
}

#[tauri::command]
fn set_mode(mode_state: State<ModeState>, mode: AppMode) -> AppMode {
    let mut current = mode_state.mode.lock().unwrap();
    *current = mode;
    *current
}

// --- Overlay Commands ---

#[tauri::command]
fn toggle_overlay(app: tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window("overlay") {
        if window.is_visible().unwrap_or(false) {
            window.hide().map_err(|e| AppError::Config(e.to_string()))?;
        } else {
            window.show().map_err(|e| AppError::Config(e.to_string()))?;
            window
                .set_focus()
                .map_err(|e| AppError::Config(e.to_string()))?;
        }
    }
    Ok(())
}

pub fn run() {
    env_logger::init();

    let config_dir = resolve_config_dir();
    let config_state =
        ConfigState::load(&config_dir).expect("Failed to load config/settings.json");

    let default_mode = AppMode::from_str(
        &config_state
            .config
            .lock()
            .unwrap()
            .default_mode,
    )
    .unwrap_or(AppMode::Desktop);

    let stt_state = SttState {
        engine: Mutex::new(WhisperEngine::new()),
    };
    let audio_state = AudioState {
        capture: Mutex::new(AudioCapture::new()),
    };
    let mode_state = ModeState {
        mode: Mutex::new(default_mode),
    };

    tauri::Builder::default()
        .manage(config_state)
        .manage(stt_state)
        .manage(audio_state)
        .manage(mode_state)
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            get_keywords,
            save_keywords,
            get_available_languages,
            load_stt_model,
            unload_stt_model,
            get_stt_status,
            start_listening,
            stop_listening,
            get_listening_status,
            get_mode,
            set_mode,
            toggle_overlay,
        ])
        .setup(|app| {
            // --- Overlay Window ---
            let overlay_url = tauri::WebviewUrl::App("/?window=overlay".into());
            let _overlay = WebviewWindowBuilder::new(app, "overlay", overlay_url)
                .title("KaleoCtrl Overlay")
                .inner_size(280.0, 200.0)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible(false)
                .resizable(false)
                .build()?;

            // --- System Tray ---
            let show = MenuItemBuilder::with_id("show", "Show").build(app)?;
            let hide = MenuItemBuilder::with_id("hide", "Hide").build(app)?;
            let toggle_ov = MenuItemBuilder::with_id("toggle_overlay", "Toggle Overlay").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show)
                .item(&hide)
                .separator()
                .item(&toggle_ov)
                .separator()
                .item(&quit)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "hide" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                        "toggle_overlay" => {
                            if let Some(overlay) = app.get_webview_window("overlay") {
                                if overlay.is_visible().unwrap_or(false) {
                                    let _ = overlay.hide();
                                } else {
                                    let _ = overlay.show();
                                    let _ = overlay.set_focus();
                                }
                            }
                        }
                        "quit" => {
                            cleanup(app);
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // --- Speech Processing Loop ---
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                speech_processing_loop(app_handle);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide both main and overlay windows instead of closing
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running KaleoCtrl");
}

/// Stop audio + unload model to free resources
fn cleanup(app: &tauri::AppHandle) {
    // Close overlay window
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.destroy();
    }

    let audio_state = app.state::<AudioState>();
    audio_state.capture.lock().unwrap().stop();

    let stt_state = app.state::<SttState>();
    stt_state.engine.lock().unwrap().unload_model();

    log::info!("Cleanup complete — resources freed");
}

/// Background loop: take speech segments, transcribe, process commands
fn speech_processing_loop(app: tauri::AppHandle) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Emit audio level (~10x/s matches our 100ms sleep)
        let segments = {
            let audio_state = app.state::<AudioState>();
            let capture = audio_state.capture.lock().unwrap();
            let recording =
                capture.is_recording.load(std::sync::atomic::Ordering::Relaxed);

            if recording {
                // Scale RMS to 0.0–1.0 range (typical speech RMS is ~0.01–0.15)
                let rms = capture.rms();
                let level = (rms / 0.15).clamp(0.0, 1.0);
                let _ = app.emit("audio_level", level);
            } else {
                let _ = app.emit("audio_level", 0.0f32);
                continue;
            }

            capture.take_segments()
        };

        if segments.is_empty() {
            continue;
        }

        // Check if STT model is loaded
        let stt_state = app.state::<SttState>();
        let engine = stt_state.engine.lock().unwrap();
        if !engine.is_model_loaded() {
            continue;
        }

        // Read language once per batch
        let config_state = app.state::<ConfigState>();
        let config = config_state.config.lock().unwrap();
        let assistant_name = config.assistant_name.clone();
        let language = config.language.clone();
        drop(config);

        for segment in segments {
            match engine.transcribe(&segment, 16000, Some(&language)) {
                Ok(result) if !result.text.is_empty() => {
                    log::info!("Transcribed: '{}' ({}ms)", result.text, result.duration_ms);

                    // Emit transcription event to frontend
                    let _ = app.emit("transcription", &result);

                    if let Ok(keywords) =
                        keywords::load_keywords(&config_state.config_dir, &language)
                    {
                        let mode_state = app.state::<ModeState>();
                        let mut mode = mode_state.mode.lock().unwrap();

                        match commander::process_speech(
                            &result.text,
                            &mut mode,
                            &keywords,
                            &assistant_name,
                        ) {
                            Ok(cmd_result) => {
                                let _ = app.emit("command_result", &cmd_result);
                                if let CommandResult::ModeChanged(new_mode) = cmd_result {
                                    let _ = app.emit("mode_changed", &new_mode);
                                }
                            }
                            Err(e) => {
                                log::error!("Command processing error: {}", e);
                            }
                        }
                    }
                }
                Ok(_) => {} // Empty transcription
                Err(e) => {
                    log::error!("Transcription error: {}", e);
                }
            }
        }
    }
}
