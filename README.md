<p align="center">
  <img src="docs/icon/icon.png" alt="KaleoCtrl" width="128" height="128">
</p>

<h1 align="center">KaleoCtrl</h1>

<p align="center">
  <b>Voice control and speech-to-text for the Linux desktop.</b><br>
  Dictation and voice-triggered actions. Audio processing runs locally.
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

KaleoCtrl captures speech through the microphone, converts it to text using a local whisper.cpp model, and either types the text into the active application or executes system commands. Transcription happens locally; no audio is sent to a network service.

### Properties

- **Local processing** — audio is processed on the local machine; no network calls during transcription
- **No telemetry** — the application does not collect or transmit usage data
- **GPU acceleration** — CUDA, Vulkan, Metal, or SYCL; CPU fallback available
- **Language coverage** — whisper large-v3-turbo supports the 99+ languages listed in its model card
- **Configurable** — additional languages via JSON keyword files; STT models swappable via config

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

KaleoCtrl runs whisper.cpp with the **Vulkan** backend so inference can use the Intel Iris Xe integrated GPU. A dedicated NVIDIA or AMD GPU is not required for this setup.

The setup:
- **whisper-rs** Rust bindings with `features = ["vulkan"]`
- **Mesa Vulkan driver** (`Intel open-source Mesa driver`) provides the GPU compute layer
- **Default model**: `large-v3-turbo` (809M params, GGML format) — chosen as accuracy/speed compromise
- **Streaming mode**: audio is transcribed incrementally during speech, not only after silence
- On the Iris Xe, partial results are emitted approximately every second

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

**Main dashboard showing current application state.**

- **Mode** — Switch between `desktop`, `dictation`, `terminal`, `sleep`
- **Listening** — Toggle microphone capture. Green "Active" when recording
- **Engine** — Currently loaded STT engine (whisper.cpp with streaming)
- **Model** — Load/unload the whisper model. Shows model name when loaded
- **Assistant** — Custom wake word / command prefix (e.g. "Kaleo")
- **Language** — Active language for keyword matching
- **Mic Level** — Audio input level meter

Values are polled every 2 seconds and also updated on backend events.

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

**Core application settings. Changes are written to `config/settings.json` and applied at runtime without restart.**

- **Assistant Name** — The wake word that prefixes voice commands (e.g. *"Kaleo open firefox"*)
- **Language** — Active language for keyword recognition. Determines which keyword file is loaded
- **STT Model** — Selects which whisper.cpp model is loaded. Smaller models reduce latency at the cost of accuracy
- **Default Mode** — The mode KaleoCtrl starts in after launch

A "Saved" indicator appears briefly after each change.

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

**Voice keywords that map to actions. Editable per language; changes are written to the keyword JSON file.**

- **Control Phrases** — `mode_switch` triggers mode changes, `wake_phrase` / `sleep_phrase` wake or sleep the assistant, `key_prefix` activates key command mode
- **System Commands** — Voice-to-action mappings for desktop control: open/close apps, switch windows, minimize, maximize, fullscreen, undo
- **Dictation Commands** — Text editing controls for dictation mode: new line, new paragraph, delete word/sentence, select all, copy, paste, cut

Each entry shows the internal action on the left and the spoken keyword on the right. Click **x** to remove an entry; use the input row at the bottom to add a new mapping.

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

**Mapping from spoken words to keyboard keys, plus an overview of operating modes.**

- **Key Commands** — Say the `key_prefix` followed by a key name to press that key. Example: *"keyboard enter"* sends the Return key. Supported entries include arrows, home/end, page up/down, tab, escape, delete, space, and others defined in `keys`
- **Modes** — Read-only overview of the four operating modes with their descriptions

Any key listed in the `keys` mapping can be triggered this way.

</td>
</tr>
</table>

---

### Key Command Flow — Waiting for Key Name

<table>
<tr>
<td width="400">
<img src="docs/screenshots/05_key_pending.png" alt="Key Pending State" width="400">
</td>
<td valign="top">

**The user said *"keyboard"* — KaleoCtrl is waiting for a key name.**

1. The configured `key_prefix` (*"keyboard"*) was recognized
2. KaleoCtrl entered **key pending mode**
3. The orange **"Taste? Sage den Tastennamen..."** indicator appears (*"Key? Say the key name..."*)
4. Last transcription shows **"keyboard"**
5. Listening is **Active** (green), model is **loaded**, mode is **desktop**

The orange box pulses while the app waits for the next spoken word, which will be interpreted as a key press.

</td>
</tr>
</table>

---

### Key Command Flow — Key Executed

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

Total latency depends on hardware and the loaded model; on the reference setup above the prefix-plus-key sequence completes in a few seconds.

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

The planner (`commander::plan`) is a pure function: same input produces the same output, with no side effects. The executor is behind a `CommandExecutor` trait, which allows the planner to be exercised with a mock executor in tests.

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
| **Desktop** | System commands are active directly — *"open firefox"* works without prefix |
| **Dictation** | Speech is transcribed and typed. Commands require the assistant name as prefix (e.g. *"Kaleo open firefox"*) |
| **Terminal** | Voice commands are sent to the terminal |
| **Sleep** | KaleoCtrl is paused. Only the wake phrase is recognized |

Switch modes by saying: `"<assistant_name> mode <mode_name>"` — e.g. *"Kaleo mode dictation"*

### The Assistant Name

The assistant has a configurable name (default: *"Kaleo"*). This name acts as a global command prefix and is recognized in every mode:

- In **desktop mode**: commands are accepted with or without the prefix
- In **dictation mode**: the prefix distinguishes commands from dictated text
- In **sleep mode**: only `"<name> wake up"` is recognized

This separates dictation from commands — in dictation mode, *"open the file"* is typed verbatim, while *"Kaleo open firefox"* is executed as a command.

### Killswitch (Emergency Stop)

If KaleoCtrl gets stuck waiting for a key, starts injecting unwanted text, or a hard stop is needed, say the **killswitch phrase** (default: *"killswitch"*).

The phrase is checked before any other command parsing and is recognized in every mode, including sleep:
- Audio capture stops immediately
- Any pending key/dictation state is cleared
- The assistant is set to `sleep` mode so commands do not fire if audio is restarted from the UI
- The frontend receives a `killswitch_triggered` event so the UI can reflect the new state

The phrase is configured per language under `killswitch_phrase` in `keywords_<lang>.json`.

---

## Multilingual Support

KaleoCtrl works with any language whisper.cpp can transcribe. Keywords are defined in per-language JSON files:

```
config/keywords_en.json    # English keywords
config/keywords_de.json    # German keywords
config/keywords_<lang>.json  # Add your own
```

To add a new language, create a keyword file following the same schema and select it in settings. The default STT model (large-v3-turbo) covers the languages listed in its model card.

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

### Install Script

`install.sh` detects the distribution and installs the required packages:

```bash
# Prerequisites: git, curl
git clone https://github.com/Maik-0000FF/KaleoCtrl.git
cd KaleoCtrl
./install.sh
```

The script provides three modes:
1. **Full install** — installs dependencies, builds the app, and adds a desktop entry
2. **Dev setup** — installs dependencies only, for development with `cargo tauri dev`
3. **Build only** — skips dependency installation, builds and installs the binary

Supported distributions: **Arch/EndeavourOS/Manjaro**, **Ubuntu/Debian/Mint**, **Fedora/Nobara**, **openSUSE**

After installation, open **Settings > Model Manager** in the app to download a whisper model.

### Uninstall

A matching `uninstall.sh` is shipped alongside the installer:

```bash
./uninstall.sh
```

Three modes; each step is idempotent and requires interactive confirmation:
1. **Standard** — removes the binary (`~/.local/bin/kaleoctrl`), icons, and desktop entry
2. **Deep clean** — additionally offers to delete `models/`, `node_modules/`, `dist/`, and `src-tauri/target/` (each prompted individually)
3. **Full wipe** — additionally offers to remove `config/` and the system packages installed by `install.sh` (with warnings)

The uninstall script does not modify the Rust toolchain or Node.js installation and does not delete any path without a confirmation prompt.

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
