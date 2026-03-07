<p align="center">
  <img src="docs/icon/icon.png" alt="KaleoCtrl" width="128" height="128">
</p>

<h1 align="center">KaleoCtrl</h1>

<p align="center">
  <b>Voice control and speech-to-text for the Linux desktop.</b><br>
  Speak to type. Speak to command. Fully offline. Fully private.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Linux-blue" alt="Platform">
  <img src="https://img.shields.io/badge/backend-Rust%20%2F%20Tauri%20v2-orange" alt="Backend">
  <img src="https://img.shields.io/badge/frontend-Svelte-red" alt="Frontend">
  <img src="https://img.shields.io/badge/STT-whisper.cpp-green" alt="STT">
  <img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License">
</p>

---

## What is KaleoCtrl?

KaleoCtrl turns your voice into actions on the Linux desktop. It captures speech through your microphone, converts it to text using a local whisper.cpp model, and either **types the text** into any active application or **executes system commands** — all without sending a single byte to the cloud.

### Key Principles

- **100% offline** — all audio processing happens locally on your machine
- **Privacy-first** — no cloud, no telemetry, no data leaves your system
- **Hardware-flexible** — GPU acceleration via CUDA, Vulkan, Metal, or SYCL; runs on CPU too
- **Multilingual** — supports 99+ languages out of the box (whisper large-v3-turbo)
- **Extensible** — add new languages by dropping a keyword file, swap STT models via config

---

## Screenshots

### Status Panel

The main dashboard showing real-time application state at a glance.

<p align="center">
  <img src="docs/screenshots/01_status.png" alt="Status Panel" width="400">
</p>

| Element | Description |
|---------|-------------|
| **Mode** | Switch between operating modes — `desktop`, `dictation`, `terminal`, `sleep` |
| **Listening** | Toggle microphone capture on/off. Green "Active" when recording |
| **Engine** | Currently loaded STT engine (whisper.cpp with streaming support) |
| **Model** | Load or unload the whisper model. Shows model name when loaded |
| **Assistant** | Your custom wake word / command prefix (e.g. "prometheus") |
| **Language** | Active language for keyword matching |
| **Mic Level** | Real-time audio input level meter |

---

### Settings Panel

Configure the core application settings. All changes apply instantly — no save button, no restart needed.

<p align="center">
  <img src="docs/screenshots/02_settings.png" alt="Settings Panel" width="400">
</p>

| Setting | Description |
|---------|-------------|
| **Assistant Name** | The wake word that prefixes all voice commands (e.g. "prometheus open firefox") |
| **Language** | Active language for keyword recognition. Determines which keyword file is loaded |
| **STT Model** | Select which whisper.cpp model to use. Smaller models run faster on weaker hardware |
| **Default Mode** | The mode KaleoCtrl starts in after launch |

---

### Keywords Panel — Commands & Dictation

Define the voice keywords that map to actions. Fully editable, per-language, auto-saved.

<p align="center">
  <img src="docs/screenshots/03_keywords.png" alt="Keywords Panel - Commands" width="400">
</p>

| Section | Description |
|---------|-------------|
| **Control Phrases** | Core phrases: `mode_switch` (triggers mode changes), `wake_phrase` / `sleep_phrase` (wake/sleep the assistant), `key_prefix` (activates key command mode) |
| **System Commands** | Voice-to-action mappings for desktop control: open/close apps, switch windows, minimize, maximize, fullscreen, undo |
| **Dictation Commands** | Text editing controls used in dictation mode: new line, new paragraph, delete word, delete sentence, select all, copy, paste, cut |

Each entry shows the internal action on the left and the spoken keyword on the right. Click the **x** to remove, or use the input row at the bottom to add new mappings.

---

### Keywords Panel — Key Commands & Modes

<p align="center">
  <img src="docs/screenshots/04_keywords_scroll.png" alt="Keywords Panel - Key Commands & Modes" width="400">
</p>

| Section | Description |
|---------|-------------|
| **Key Commands** | Map spoken words to keyboard key presses. Say the `key_prefix` followed by a key name to press that key. Example: *"keyboard enter"* sends the Return key. Supports all standard keys: arrows, home/end, page up/down, tab, escape, delete, space, and more |
| **Modes** | Overview of the four operating modes with descriptions. Read-only display showing each mode's purpose |

---

### Key Command in Action — Waiting for Key Name

When you say the key prefix (e.g. *"keyboard"*), KaleoCtrl enters **key pending mode**. An orange indicator appears, prompting you to say the key name.

<p align="center">
  <img src="docs/screenshots/05_key_pending.png" alt="Key Pending State" width="400">
</p>

**What's happening here:**
1. The user said **"keyboard"** (the configured `key_prefix`)
2. KaleoCtrl recognized it and entered key pending mode
3. The orange **"Taste? Sage den Tastennamen..."** box appears (translates to: *"Key? Say the key name..."*)
4. The transcription box shows **"keyboard"** as the last recognized speech
5. Listening is **Active** (green), model is **loaded**, mode is **desktop**

---

### Key Command in Action — Key Executed

After saying the key name, KaleoCtrl presses the corresponding key and returns to normal mode.

<p align="center">
  <img src="docs/screenshots/06_key_executed.png" alt="Key Executed" width="400">
</p>

**What's happening here:**
1. The user said **"enter"** while in key pending mode
2. KaleoCtrl matched "enter" to the `Return` key
3. The Return key was injected into the active application
4. The orange key pending indicator disappeared
5. The transcription box shows **"enter"** as the last recognized speech

---

## How It Works

### Architecture

```
Microphone → Audio Capture (cpal) → Voice Activity Detection
                                            ↓
                                   Streaming Transcriber
                                     (whisper.cpp FFI)
                                            ↓
                                    Speech Recognition
                                            ↓
                        ┌───────────────────┼───────────────────┐
                        ↓                   ↓                   ↓
                  Command Parser      Key Command         Text Injection
                  (open, close,     (keyboard + key)     (type into active
                   switch, etc.)                           application)
                        ↓                   ↓                   ↓
                  System Actions      Key Press Sim.      Text Output
                  (wmctrl, xdg)       (xdotool/wtype)    (wtype/xdotool)
```

### Speech Processing Pipeline

1. **Audio Capture** — continuous microphone input via CPAL at 16kHz
2. **Voice Activity Detection** — detects when you start and stop speaking
3. **Streaming Transcription** — partial results while you speak, final result on silence
4. **Command Routing** — text is checked against keywords before being typed:
   - Mode switch commands (`"<name> mode desktop"`)
   - Wake/sleep commands (`"<name> wake up"` / `"<name> sleep"`)
   - System commands (`"open firefox"`, `"close window"`)
   - Key commands (`"keyboard enter"`, `"keyboard tab"`)
   - Dictation commands (`"new line"`, `"delete word"`)
   - If no command matches → text is typed into the active application

### Operating Modes

| Mode | Behavior |
|------|----------|
| **Desktop** | System commands are active directly — say *"open firefox"* without any prefix |
| **Dictation** | Pure speech-to-text. Everything you say gets typed. Commands require the assistant name as prefix (e.g. *"prometheus open firefox"*) |
| **Terminal** | Voice commands are sent to the terminal |
| **Sleep** | KaleoCtrl is paused. Only listens for the wake phrase |

Switch modes by saying: `"<assistant_name> mode <mode_name>"` — e.g. *"prometheus mode dictation"*

### The Assistant Name

You assign a custom name to your assistant (default: *"prometheus"*). This name acts as a **global command prefix** and works in every mode:

- In **desktop mode**: commands work with or without prefix
- In **dictation mode**: the prefix distinguishes commands from dictated text
- In **sleep mode**: only `"<name> wake up"` is recognized

This prevents false triggers — in dictation mode, saying *"open the file"* just types that text, while *"prometheus open firefox"* executes the command.

---

## Multilingual Support

KaleoCtrl supports any language that whisper.cpp can transcribe. Keywords are defined in per-language JSON files:

```
config/keywords_en.json    # English keywords
config/keywords_de.json    # German keywords
config/keywords_<lang>.json  # Add your own
```

To add a new language, create a keyword file following the same schema and select it in settings. The STT model (large-v3-turbo) supports 99+ languages natively.

---

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Framework | Tauri v2 |
| Backend | Rust |
| Frontend | Svelte + CSS |
| STT Engine | whisper.cpp (via Rust FFI) |
| Default Model | whisper-large-v3-turbo (GGML, 809M params) |
| Audio Capture | CPAL |
| GPU Support | CUDA, Vulkan, Metal, SYCL |
| Config | JSON |

---

## Build & Run

### Prerequisites

- Rust toolchain (rustup)
- Node.js + npm
- Tauri v2 CLI (`cargo install tauri-cli`)
- System libraries: WebKitGTK, GTK3, libayatana-appindicator (see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))
- For GPU: CUDA/Vulkan SDK (optional, falls back to CPU)

### Development

```bash
npm install
cargo tauri dev
```

### Production Build

```bash
cargo tauri build
```

### Rust Checks

```bash
cargo check                  # type-check
cargo clippy                 # lint
cargo test                   # run tests
```

---

## Configuration

All config files live in `config/`:

### `config/settings.json`

```json
{
  "assistant_name": "prometheus",
  "language": "de",
  "stt_model": "large-v3-turbo",
  "default_mode": "dictation"
}
```

### `config/keywords_<lang>.json`

Contains per-language definitions for:
- Mode names and descriptions
- Control phrases (mode switch, wake/sleep, key prefix)
- System command keywords
- Dictation command keywords
- Key command mappings

---

## Planned Features

- [ ] Overlay window — compact floating status display
- [ ] Custom voice command scripting
- [ ] Application-specific keyword profiles
- [ ] Training mode for improving recognition of custom terms
- [ ] Plugin system for third-party integrations
- [ ] Wayland-native text injection improvements
- [ ] Audio device selection in GUI

---

## License

MIT
