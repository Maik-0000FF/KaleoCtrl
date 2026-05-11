<p align="center">
  <img src="docs/icon/icon.png" alt="KaleoCtrl" width="128" height="128">
</p>

<h1 align="center">KaleoCtrl</h1>

<p align="center">
  <b>Voice control and speech-to-text for the Linux desktop.</b><br>
  Speak to type. Speak to command. Fully offline. Fully private.
</p>

<p align="center">
  <a href="https://github.com/Maik-0000FF/KaleoCtrl/actions/workflows/ci.yml"><img src="https://github.com/Maik-0000FF/KaleoCtrl/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/status-early%20development-yellow" alt="Status">
  <img src="https://img.shields.io/badge/platform-Linux-blue" alt="Platform">
  <img src="https://img.shields.io/badge/backend-Rust%20%2F%20Tauri%20v2-orange" alt="Backend">
  <img src="https://img.shields.io/badge/frontend-Svelte-red" alt="Frontend">
  <img src="https://img.shields.io/badge/STT-whisper.cpp-green" alt="STT">
  <img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial-lightgrey" alt="License">
</p>

<p align="center">
  <a href="https://ko-fi.com/maik0000ff"><img src="https://img.shields.io/badge/Ko--fi-Support%20this%20project-ff5e5b?logo=ko-fi&logoColor=white&style=for-the-badge" alt="Ko-fi"></a>
</p>

> **Early Development** — This project is in an early stage. Features may change, break, or be incomplete. Contributions and feedback are welcome.

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

## Development Setup & Hardware

KaleoCtrl is being developed and tested on the following system:

| Component | Specification |
|-----------|--------------|
| **OS** | EndeavourOS (Arch-based) — Kernel 6.19 |
| **CPU** | Intel Core i9-13900H (20 threads) |
| **RAM** | 64 GB DDR5 |
| **GPU** | Intel Iris Xe Graphics (RPL-P) — integrated |
| **Display Server** | Wayland (KDE Plasma) |

### Whisper on Intel GPU

KaleoCtrl runs whisper.cpp with **Vulkan** backend to leverage the Intel Iris Xe integrated GPU for inference. This means no dedicated NVIDIA/AMD GPU is required — the model runs accelerated on the iGPU that's already in your laptop.

The setup:
- **whisper-rs** Rust bindings with `features = ["vulkan"]`
- **Mesa Vulkan driver** (`Intel open-source Mesa driver`) provides the GPU compute layer
- **Default model**: `large-v3-turbo` (809M params, GGML format) — best balance of accuracy and speed
- **Streaming mode**: audio is transcribed in real-time as you speak, not after you stop
- Inference runs at near real-time speed on the Iris Xe, with partial results delivered every ~1 second

**Limitation:** Real-time simultaneous speech-to-text is not achievable on this hardware. The Intel Iris Xe iGPU lacks the compute power for true simultaneous transcription — there is a noticeable delay between speaking and text output. For low-latency real-time STT, a dedicated GPU (e.g. NVIDIA with CUDA) is recommended.

Smaller models (`small`, quantized `q5_0`) are available for systems with less GPU memory or processing power and can reduce latency at the cost of accuracy.

---

## Screenshots

### Status Panel

<table>
<tr>
<td width="400">
<img src="docs/screenshots/01_status.png" alt="Status Panel" width="400">
</td>
<td valign="top">

**The main dashboard — real-time application state at a glance.**

- **Mode** — Switch between `desktop`, `dictation`, `terminal`, `sleep`
- **Listening** — Toggle microphone capture. Green "Active" when recording
- **Engine** — Currently loaded STT engine (whisper.cpp with streaming)
- **Model** — Load/unload the whisper model. Shows model name when loaded
- **Assistant** — Your custom wake word / command prefix (e.g. "Kaleo")
- **Language** — Active language for keyword matching
- **Mic Level** — Real-time audio input level meter

All values update live. The status panel refreshes automatically every 2 seconds and reacts instantly to backend events.

</td>
</tr>
</table>

---

### Settings Panel

<table>
<tr>
<td width="400">
<img src="docs/screenshots/02_settings.png" alt="Settings Panel" width="400">
</td>
<td valign="top">

**Configure core application settings. All changes apply instantly — no save button, no restart.**

- **Assistant Name** — The wake word that prefixes all voice commands (e.g. *"Kaleo open firefox"*)
- **Language** — Active language for keyword recognition. Determines which keyword file is loaded
- **STT Model** — Select which whisper.cpp model to use. Smaller models run faster on weaker hardware
- **Default Mode** — The mode KaleoCtrl starts in after launch

A "Saved" indicator briefly appears after each change to confirm the setting was applied.

</td>
</tr>
</table>

---

### Keywords Panel — Commands & Dictation

<table>
<tr>
<td width="400">
<img src="docs/screenshots/03_keywords.png" alt="Keywords Panel - Commands" width="400">
</td>
<td valign="top">

**Define voice keywords that map to actions. Fully editable, per-language, auto-saved.**

- **Control Phrases** — Core phrases: `mode_switch` triggers mode changes, `wake_phrase` / `sleep_phrase` wake or sleep the assistant, `key_prefix` activates key command mode
- **System Commands** — Voice-to-action mappings for desktop control: open/close apps, switch windows, minimize, maximize, fullscreen, undo
- **Dictation Commands** — Text editing controls for dictation mode: new line, new paragraph, delete word/sentence, select all, copy, paste, cut

Each entry shows the internal action on the left and the spoken keyword on the right. Click **x** to remove, or use the input row at the bottom to add new mappings.

</td>
</tr>
</table>

---

### Keywords Panel — Key Commands & Modes

<table>
<tr>
<td width="400">
<img src="docs/screenshots/04_keywords_scroll.png" alt="Keywords Panel - Key Commands & Modes" width="400">
</td>
<td valign="top">

**Map spoken words to keyboard keys and view operating modes.**

- **Key Commands** — Say the `key_prefix` followed by a key name to press that key. Example: *"keyboard enter"* sends the Return key. Supports arrows, home/end, page up/down, tab, escape, delete, space, and more
- **Modes** — Read-only overview of the four operating modes with their descriptions

Key commands bridge the gap between voice and keyboard — any key that can be typed can be triggered by voice.

</td>
</tr>
</table>

---

### Key Command in Action — Waiting for Key Name

<table>
<tr>
<td width="400">
<img src="docs/screenshots/05_key_pending.png" alt="Key Pending State" width="400">
</td>
<td valign="top">

**The user said *"keyboard"* — KaleoCtrl is now waiting for a key name.**

1. The configured `key_prefix` (*"keyboard"*) was recognized
2. KaleoCtrl entered **key pending mode**
3. The orange **"Taste? Sage den Tastennamen..."** indicator appears (*"Key? Say the key name..."*)
4. Last transcription shows **"keyboard"**
5. Listening is **Active** (green), model is **loaded**, mode is **desktop**

The orange box pulses to clearly signal that the app is waiting for the next spoken word to be interpreted as a key press.

</td>
</tr>
</table>

---

### Key Command in Action — Key Executed

<table>
<tr>
<td width="400">
<img src="docs/screenshots/06_key_executed.png" alt="Key Executed" width="400">
</td>
<td valign="top">

**The user said *"enter"* — the Return key was pressed.**

1. *"enter"* was spoken while in key pending mode
2. KaleoCtrl matched it to the `Return` key
3. The key press was injected into the active application
4. The orange indicator disappeared — back to normal mode
5. Last transcription shows **"enter"**

The entire flow — say *"keyboard"*, wait for prompt, say *"enter"* — takes under 2 seconds. Any mapped key can be triggered this way.

</td>
</tr>
</table>

---

## How It Works

### Architecture

```
Microphone → Audio Capture (cpal) → Voice Activity Detection
                                            ↓
                                   Streaming Transcriber
                                     (whisper.cpp FFI)
                                            ↓
                                  Killswitch check ─── (emergency stop, any mode)
                                            ↓
                              Command Planner (pure, unit-testable)
                                 text + mode + keywords → Action
                                            ↓
                                Command Executor (trait)
                                            ↓
                        ┌───────────────────┼───────────────────┐
                        ↓                   ↓                   ↓
                  System Command      Key Command         Text Injection
                  (open, close,      (key prefix +       (type into active
                   switch, etc.)        key name)          application)
                        ↓                   ↓                   ↓
                  System Actions      Key Press Sim.      Text Output
                  (wmctrl, xdg)       (xdotool/wtype)    (wtype/xdotool)
```

The planner (`commander::plan`) is pure: same input always produces the same output, no side effects, fully unit-tested. The executor is behind a `CommandExecutor` trait, allowing the planner to be exercised with a mock executor in tests.

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
| **Dictation** | Pure speech-to-text. Everything you say gets typed. Commands require the assistant name as prefix (e.g. *"Kaleo open firefox"*) |
| **Terminal** | Voice commands are sent to the terminal |
| **Sleep** | KaleoCtrl is paused. Only listens for the wake phrase |

Switch modes by saying: `"<assistant_name> mode <mode_name>"` — e.g. *"Kaleo mode dictation"*

### The Assistant Name

You assign a custom name to your assistant (default: *"Kaleo"*). This name acts as a **global command prefix** and works in every mode:

- In **desktop mode**: commands work with or without prefix
- In **dictation mode**: the prefix distinguishes commands from dictated text
- In **sleep mode**: only `"<name> wake up"` is recognized

This prevents false triggers — in dictation mode, saying *"open the file"* just types that text, while *"Kaleo open firefox"* executes the command.

### Killswitch (Emergency Stop)

If KaleoCtrl misbehaves — gets stuck waiting for a key, starts injecting unwanted text, or you simply want a hard stop — say the **killswitch phrase** (default: *"killswitch"*).

It wins over every other rule, in **any mode including sleep**:
- Audio capture stops immediately
- Any pending key/dictation state is cleared
- The assistant is forced into `sleep` mode so spurious commands won't fire if audio is restarted from the UI
- The frontend receives a `killswitch_triggered` event for one-click recovery

The phrase is per language, configured under `killswitch_phrase` in `keywords_<lang>.json`.

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

## Installation

### Quick Install (recommended)

The install script detects your distribution and handles everything:

```bash
# Prerequisites: git, curl
git clone https://github.com/Maik-0000FF/KaleoCtrl.git
cd KaleoCtrl
./install.sh
```

The script offers three modes:
1. **Full install** — installs all dependencies, builds the app, and adds it to your application menu
2. **Dev setup** — installs dependencies only, for development with `cargo tauri dev`
3. **Build only** — skips dependency installation, just builds and installs

Supported distributions: **Arch/EndeavourOS/Manjaro**, **Ubuntu/Debian/Mint**, **Fedora/Nobara**, **openSUSE**

After installation, open **Settings > Model Manager** in the app to download a whisper model.

### Uninstall

A matching `uninstall.sh` is shipped alongside the installer:

```bash
./uninstall.sh
```

Three modes, all idempotent and prompted:
1. **Standard** — removes the binary (`~/.local/bin/kaleoctrl`), icons, and desktop entry
2. **Deep clean** — also offers to delete `models/`, `node_modules/`, `dist/`, and `src-tauri/target/` (each prompted individually)
3. **Full wipe** — adds optional removal of `config/` and, with explicit warnings, the system packages installed by `install.sh`

The uninstall script never touches your Rust toolchain or Node.js installation, and never deletes anything without an interactive confirmation.

### Manual Build

If you prefer to install dependencies yourself:

```bash
npm install          # frontend dependencies
cargo tauri dev      # development mode (hot reload)
cargo tauri build    # production build
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
  "assistant_name": "Kaleo",
  "language": "de",
  "stt_model": "large-v3-turbo",
  "default_mode": "dictation"
}
```

### `config/keywords_<lang>.json`

Contains per-language definitions for:
- `language` — language code
- `modes` — mode names and descriptions (`desktop`, `dictation`, `terminal`, `sleep`)
- `mode_switch` — the word that triggers mode changes (e.g. *"mode"* / *"modus"*)
- `wake_phrase` / `sleep_phrase` — wake/sleep the assistant
- `killswitch_phrase` — emergency stop, recognized in any mode (default: *"killswitch"*)
- `commands` — system command keywords (`open`, `close`, `switch`, `minimize`, `maximize`, `fullscreen`, `stop`, `undo`)
- `dictation` — dictation control keywords (`new_line`, `new_paragraph`, `delete_word`, `delete_sentence`, `select_all`, `copy`, `paste`, `cut`)
- `key_prefix` — word that activates key-press mode (default: *"key"* in en, *"keyboard"* in de)
- `key_prefix_aliases` — additional words that also activate key-press mode
- `keys` — mapping from spoken word to keyboard key name (e.g. `"enter" → "Return"`)

---

## Planned Features

- [ ] Overlay window — compact floating status display
- [ ] Custom voice command scripting
- [ ] Application-specific keyword profiles
- [ ] Personal vocabulary — user-defined correction dictionary and custom terms that bias whisper recognition via `initial_prompt` and post-processing (not model fine-tuning)
- [ ] Plugin system for third-party integrations
- [ ] Wayland-native text injection improvements
- [ ] Audio device selection in GUI
- [ ] Pre-built release binaries (deb/rpm/AppImage) — release pipeline (CI for tests is already in place)

---

## License

This project is licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE).

You are free to use, modify, and share this software for any **noncommercial** purpose — personal use, research, education, hobby projects. Commercial use (including selling or embedding in commercial products) requires explicit permission from the author.
