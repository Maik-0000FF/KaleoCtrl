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
use stt::streaming::{StreamEvent, StreamingTranscriber};
use stt::{SttStatus, TranscriptionResult};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::webview::WebviewWindowBuilder;
use tauri::{Emitter, Manager, State};

use audio::AudioCapture;
use commander::{AppMode, CommandResult};
use serde::Serialize;

// --- App State ---

pub struct StreamState {
    transcriber: Mutex<StreamingTranscriber>,
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
fn update_config(
    state: State<ConfigState>,
    stream_state: State<StreamState>,
    config: AppConfig,
) -> Result<(), AppError> {
    let new_lang = config.language.clone();
    let assistant_name = config.assistant_name.clone();
    {
        let mut current = state.config.lock().unwrap();
        *current = config;
    }
    state.save()?;

    // Update streaming worker language + prompt
    let transcriber = stream_state.transcriber.lock().unwrap();
    if transcriber.is_running() {
        transcriber.set_language(&new_lang);
        let prompt = build_vocab_prompt(&state.config_dir, &new_lang, &assistant_name);
        transcriber.set_prompt(&prompt);
    }

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

// --- Model Catalog & Download ---

#[derive(Clone, Serialize)]
struct DownloadableModel {
    name: String,
    filename: String,
    size_mb: u64,
    description: String,
    url: String,
}

fn get_model_catalog() -> Vec<DownloadableModel> {
    vec![
        DownloadableModel {
            name: "large-v3-turbo".into(),
            filename: "ggml-large-v3-turbo.bin".into(),
            size_mb: 1550,
            description: "Best accuracy/speed balance — recommended for most systems".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
        },
        DownloadableModel {
            name: "large-v3-turbo-q5_0".into(),
            filename: "ggml-large-v3-turbo-q5_0.bin".into(),
            size_mb: 547,
            description: "Quantized large-v3-turbo — less memory, slightly lower accuracy".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".into(),
        },
        DownloadableModel {
            name: "medium".into(),
            filename: "ggml-medium.bin".into(),
            size_mb: 1457,
            description: "Good accuracy, slower than turbo models".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
        },
        DownloadableModel {
            name: "small".into(),
            filename: "ggml-small.bin".into(),
            size_mb: 464,
            description: "Fast and lightweight — good for weaker hardware".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
        },
        DownloadableModel {
            name: "base".into(),
            filename: "ggml-base.bin".into(),
            size_mb: 141,
            description: "Very fast, basic accuracy — for quick testing".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
        },
        DownloadableModel {
            name: "tiny".into(),
            filename: "ggml-tiny.bin".into(),
            size_mb: 74,
            description: "Fastest, lowest accuracy — minimal resource usage".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
        },
    ]
}

#[derive(Clone, Serialize)]
struct ModelCatalogEntry {
    name: String,
    size_mb: u64,
    description: String,
    downloaded: bool,
}

#[tauri::command]
fn get_model_catalog_list(config_state: State<ConfigState>) -> Vec<ModelCatalogEntry> {
    let models_dir = config_state
        .config_dir
        .parent()
        .unwrap_or(&config_state.config_dir)
        .join("models");

    get_model_catalog()
        .into_iter()
        .map(|m| {
            let downloaded = models_dir.join(&m.filename).exists();
            ModelCatalogEntry {
                name: m.name,
                size_mb: m.size_mb,
                description: m.description,
                downloaded,
            }
        })
        .collect()
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    model: String,
    downloaded_mb: u64,
    total_mb: u64,
    done: bool,
    error: Option<String>,
}

#[tauri::command]
fn download_model(
    app: tauri::AppHandle,
    config_state: State<ConfigState>,
    model_name: String,
) -> Result<(), AppError> {
    let catalog = get_model_catalog();
    let entry = catalog
        .iter()
        .find(|m| m.name == model_name)
        .ok_or_else(|| AppError::Config(format!("Unknown model: {}", model_name)))?
        .clone();

    let models_dir = config_state
        .config_dir
        .parent()
        .unwrap_or(&config_state.config_dir)
        .join("models");

    std::fs::create_dir_all(&models_dir)
        .map_err(|e| AppError::Config(format!("Cannot create models dir: {}", e)))?;

    let dest = models_dir.join(&entry.filename);
    if dest.exists() {
        return Err(AppError::Config("Model already downloaded".into()));
    }

    // Download in background thread
    std::thread::spawn(move || {
        let emit_progress = |downloaded_mb: u64, total_mb: u64, done: bool, error: Option<String>| {
            let _ = app.emit(
                "download_progress",
                DownloadProgress {
                    model: entry.name.clone(),
                    downloaded_mb,
                    total_mb,
                    done,
                    error,
                },
            );
        };

        emit_progress(0, entry.size_mb, false, None);

        let tmp_path = dest.with_extension("bin.part");

        let result = (|| -> Result<(), String> {
            let resp = ureq::get(&entry.url)
                .call()
                .map_err(|e| format!("Download failed: {}", e))?;

            let total = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(entry.size_mb * 1024 * 1024);
            let total_mb = total / (1024 * 1024);

            let mut reader = resp.into_body().into_reader();
            let mut file = std::fs::File::create(&tmp_path)
                .map_err(|e| format!("Cannot create file: {}", e))?;

            let mut buf = [0u8; 65536];
            let mut downloaded: u64 = 0;
            let mut last_emit: u64 = 0;

            loop {
                let n = std::io::Read::read(&mut reader, &mut buf)
                    .map_err(|e| format!("Read error: {}", e))?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut file, &buf[..n])
                    .map_err(|e| format!("Write error: {}", e))?;
                downloaded += n as u64;

                let mb = downloaded / (1024 * 1024);
                if mb > last_emit {
                    emit_progress(mb, total_mb, false, None);
                    last_emit = mb;
                }
            }

            std::fs::rename(&tmp_path, &dest)
                .map_err(|e| format!("Cannot rename file: {}", e))?;

            Ok(())
        })();

        match result {
            Ok(()) => emit_progress(entry.size_mb, entry.size_mb, true, None),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_path);
                emit_progress(0, entry.size_mb, true, Some(e));
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn delete_model(
    config_state: State<ConfigState>,
    model_name: String,
) -> Result<(), AppError> {
    let catalog = get_model_catalog();
    let entry = catalog
        .iter()
        .find(|m| m.name == model_name)
        .ok_or_else(|| AppError::Config(format!("Unknown model: {}", model_name)))?;

    let models_dir = config_state
        .config_dir
        .parent()
        .unwrap_or(&config_state.config_dir)
        .join("models");

    let path = models_dir.join(&entry.filename);
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| AppError::Config(format!("Cannot delete model: {}", e)))?;
    }
    Ok(())
}

// --- Model Discovery ---

#[tauri::command]
fn get_available_models(config_state: State<ConfigState>) -> Vec<String> {
    let models_dir = config_state
        .config_dir
        .parent()
        .unwrap_or(&config_state.config_dir)
        .join("models");

    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Match ggml-*.bin pattern and extract the model name
            if let Some(model_name) = name
                .strip_prefix("ggml-")
                .and_then(|s| s.strip_suffix(".bin"))
            {
                models.push(model_name.to_string());
            }
        }
    }
    models.sort();
    models
}

/// Build a short vocabulary prompt from keywords to bias whisper recognition.
/// Must stay under ~50 tokens — longer prompts cause hallucination loops.
fn build_vocab_prompt(
    config_dir: &std::path::Path,
    language: &str,
    assistant_name: &str,
) -> String {
    let mut words: Vec<String> = Vec::new();

    if let Ok(kw) = keywords::load_keywords(config_dir, language) {
        // Only the key prefix (most commonly misrecognized)
        if !kw.key_prefix.is_empty() {
            words.push(kw.key_prefix.clone());
        }
        for alias in &kw.key_prefix_aliases {
            words.push(alias.clone());
        }
        // Mode switch keyword
        words.push(kw.mode_switch.clone());
    }

    // Assistant name (highest priority)
    words.push(assistant_name.to_string());

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    words.retain(|w| seen.insert(w.to_lowercase()));

    let prompt = words.join(", ");
    log::info!("Vocab prompt: {}", prompt);
    prompt
}

// --- STT Commands ---

#[tauri::command]
fn load_stt_model(
    stream_state: State<StreamState>,
    audio_state: State<AudioState>,
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

    let config = config_state.config.lock().unwrap();
    let language = config.language.clone();
    let assistant_name = config.assistant_name.clone();
    drop(config);

    // Build vocabulary prompt from keywords to bias recognition
    let prompt = build_vocab_prompt(&config_state.config_dir, &language, &assistant_name);

    let mut transcriber = stream_state.transcriber.lock().unwrap();
    transcriber.set_prompt(&prompt);
    transcriber.start(&resolved_str, &language)?;

    // Connect audio sink to streaming worker
    if let Some(tx) = transcriber.audio_sender() {
        let capture = audio_state.capture.lock().unwrap();
        capture.set_audio_sink(tx);
    }

    Ok(())
}

#[tauri::command]
fn unload_stt_model(
    stream_state: State<StreamState>,
    audio_state: State<AudioState>,
) -> Result<(), AppError> {
    // Disconnect audio sink first
    let capture = audio_state.capture.lock().unwrap();
    capture.clear_audio_sink();
    drop(capture);

    // Stop streaming worker (frees GPU/RAM)
    let mut transcriber = stream_state.transcriber.lock().unwrap();
    transcriber.stop();
    Ok(())
}

#[tauri::command]
fn get_stt_status(state: State<StreamState>) -> SttStatus {
    let transcriber = state.transcriber.lock().unwrap();
    SttStatus {
        engine: "whisper.cpp (streaming)".to_string(),
        model_loaded: transcriber.is_running(),
        current_model: transcriber.current_model().map(|s| s.to_string()),
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

/// Recover from a killswitch trigger: restart mic capture and switch back
/// to the user's default mode in one atomic step. The frontend banner calls
/// this so the recovery is one click instead of three (mic on, dismiss
/// banner, leave sleep mode).
#[tauri::command]
fn reactivate_after_killswitch(
    audio_state: State<AudioState>,
    mode_state: State<ModeState>,
    config_state: State<ConfigState>,
) -> Result<AppMode, AppError> {
    {
        let mut capture = audio_state.capture.lock().unwrap();
        capture.start()?;
    }

    let default_mode_str = {
        let config = config_state.config.lock().unwrap();
        config.default_mode.clone()
    };
    let new_mode = AppMode::from_str(&default_mode_str).unwrap_or(AppMode::Desktop);
    *mode_state.mode.lock().unwrap() = new_mode;
    Ok(new_mode)
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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

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

    let stream_state = StreamState {
        transcriber: Mutex::new(StreamingTranscriber::new()),
    };
    let audio_state = AudioState {
        capture: Mutex::new(AudioCapture::new()),
    };
    let mode_state = ModeState {
        mode: Mutex::new(default_mode),
    };

    tauri::Builder::default()
        .manage(config_state)
        .manage(stream_state)
        .manage(audio_state)
        .manage(mode_state)
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            get_keywords,
            save_keywords,
            get_available_languages,
            get_available_models,
            get_model_catalog_list,
            download_model,
            delete_model,
            load_stt_model,
            unload_stt_model,
            get_stt_status,
            start_listening,
            stop_listening,
            reactivate_after_killswitch,
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

            // --- Event Processing Loop ---
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                event_processing_loop(app_handle);
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

    let stream_state = app.state::<StreamState>();
    stream_state.transcriber.lock().unwrap().stop();

    log::info!("Cleanup complete — resources freed");
}

/// Background loop: emit audio levels, process stream events, execute commands
fn event_processing_loop(app: tauri::AppHandle) {
    // Buffer for split key commands: user says "taste" [pause] "enter" as two segments
    let mut pending_key_prefix = false;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Emit audio level (~20x/s)
        {
            let audio_state = app.state::<AudioState>();
            let capture = audio_state.capture.lock().unwrap();
            let recording = capture
                .is_recording
                .load(std::sync::atomic::Ordering::Relaxed);

            if recording {
                let rms = capture.rms();
                let level = (rms / 0.15).clamp(0.0, 1.0);
                let _ = app.emit("audio_level", level);
            } else {
                let _ = app.emit("audio_level", 0.0f32);
            }
        }

        // Collect stream events (briefly lock transcriber, non-blocking try_recv)
        let events: Vec<StreamEvent> = {
            let stream_state = app.state::<StreamState>();
            let transcriber = stream_state.transcriber.lock().unwrap();
            let mut events = Vec::new();
            if let Some(rx) = &transcriber.event_rx {
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }
            }
            events
        };

        if events.is_empty() {
            continue;
        }

        // Read config once per batch
        let config_state = app.state::<ConfigState>();
        let config = config_state.config.lock().unwrap();
        let assistant_name = config.assistant_name.clone();
        let language = config.language.clone();
        drop(config);

        let kw_result = keywords::load_keywords(&config_state.config_dir, &language);

        for event in events {
            match &event {
                StreamEvent::Partial { text, duration_ms } => {
                    if !text.is_empty() {
                        log::debug!("Partial: '{}' ({}ms)", text, duration_ms);
                        let payload = TranscriptionResult {
                            text: text.clone(),
                            language: None,
                            duration_ms: *duration_ms,
                        };
                        let _ = app.emit("transcription_partial", &payload);
                    }
                }
                StreamEvent::Final {
                    text,
                    language: detected_lang,
                    duration_ms,
                } => {
                    if text.is_empty() {
                        continue;
                    }

                    log::info!("Final: '{}' ({}ms)", text, duration_ms);

                    let payload = TranscriptionResult {
                        text: text.clone(),
                        language: detected_lang.clone(),
                        duration_ms: *duration_ms,
                    };
                    let _ = app.emit("transcription", &payload);

                    let Ok(ref keywords) = kw_result else {
                        continue;
                    };

                    let lower = text
                        .trim()
                        .trim_matches(|c: char| c.is_ascii_punctuation())
                        .to_lowercase();

                    // Key prefix buffering: "taste" [pause] "enter"
                    if pending_key_prefix {
                        pending_key_prefix = false;
                        let _ = app.emit("key_pending", false);
                        let combined =
                            format!("{} {}", keywords.key_prefix.to_lowercase(), lower);
                        if let Some(result) =
                            commander::check_key_command(&combined, keywords)
                        {
                            match result {
                                Ok(cmd_result) => {
                                    log::info!("Key command (buffered): {:?}", cmd_result);
                                    let _ = app.emit("command_result", &cmd_result);
                                }
                                Err(e) => {
                                    log::error!("Key command error: {}", e);
                                }
                            }
                            continue;
                        }
                        // No key matched — fall through to normal processing
                    }

                    // Check if text is just the key prefix (or alias) → buffer for next segment
                    if !keywords.key_prefix.is_empty() {
                        let is_prefix = lower == keywords.key_prefix.to_lowercase()
                            || keywords
                                .key_prefix_aliases
                                .iter()
                                .any(|a| lower == a.to_lowercase());
                        if is_prefix {
                            pending_key_prefix = true;
                            let _ = app.emit("key_pending", true);
                            log::info!("Key input mode — waiting for key name...");
                            continue;
                        }
                    }

                    // Normal command processing
                    let mode_state = app.state::<ModeState>();
                    let mut mode = mode_state.mode.lock().unwrap();

                    match commander::process_speech(
                        text,
                        &mut mode,
                        keywords,
                        &assistant_name,
                    ) {
                        Ok(cmd_result) => {
                            let _ = app.emit("command_result", &cmd_result);
                            match &cmd_result {
                                CommandResult::ModeChanged(new_mode) => {
                                    let _ = app.emit("mode_changed", new_mode);
                                }
                                CommandResult::EmergencyStopped => {
                                    // The planner has already forced mode to Sleep.
                                    // Stop audio capture, clear pending state, notify the UI.
                                    log::warn!("Killswitch triggered");
                                    let audio_state = app.state::<AudioState>();
                                    audio_state.capture.lock().unwrap().stop();
                                    pending_key_prefix = false;
                                    let _ = app.emit("key_pending", false);
                                    let _ = app.emit("mode_changed", &*mode);
                                    let _ = app.emit("killswitch_triggered", ());
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            log::error!("Command processing error: {}", e);
                        }
                    }
                }
            }
        }
    }
}
