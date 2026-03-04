<script lang="ts">
  import type { AppConfig, SttStatus, AppMode } from "../lib/types";
  import {
    getSttStatus,
    getListeningStatus,
    getMode,
    startListening,
    stopListening,
    setMode,
    loadSttModel,
    unloadSttModel,
  } from "../lib/api";
  import { listen } from "@tauri-apps/api/event";
  import AudioLevelMeter from "./AudioLevelMeter.svelte";

  interface Props {
    config: AppConfig | null;
  }

  let { config }: Props = $props();

  let sttStatus = $state<SttStatus | null>(null);
  let listening = $state(false);
  let currentMode = $state<AppMode>("desktop");
  let lastTranscription = $state("");
  let modelLoading = $state(false);
  let modelError = $state("");

  async function refreshStatus() {
    try {
      sttStatus = await getSttStatus();
      listening = await getListeningStatus();
      currentMode = await getMode();
    } catch (e) {
      console.error("Failed to refresh status:", e);
    }
  }

  async function toggleModel() {
    if (!config) return;
    modelError = "";
    modelLoading = true;
    try {
      if (sttStatus?.model_loaded) {
        await unloadSttModel();
      } else {
        const modelPath = `models/ggml-${config.stt_model}.bin`;
        await loadSttModel(modelPath);
      }
      sttStatus = await getSttStatus();
    } catch (e) {
      modelError = String(e);
    } finally {
      modelLoading = false;
    }
  }

  async function toggleListening() {
    try {
      if (listening) {
        await stopListening();
      } else {
        await startListening();
      }
      listening = await getListeningStatus();
    } catch (e) {
      console.error("Failed to toggle listening:", e);
    }
  }

  async function switchMode(mode: AppMode) {
    try {
      currentMode = await setMode(mode);
    } catch (e) {
      console.error("Failed to switch mode:", e);
    }
  }

  const modes: AppMode[] = ["desktop", "dictation", "terminal", "sleep"];

  $effect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 2000);

    const unlistenTranscription = listen<{ text: string }>(
      "transcription",
      (event) => {
        lastTranscription = event.payload.text;
      },
    );

    const unlistenMode = listen<AppMode>("mode_changed", (event) => {
      currentMode = event.payload;
    });

    return () => {
      clearInterval(interval);
      unlistenTranscription.then((f) => f());
      unlistenMode.then((f) => f());
    };
  });
</script>

<div class="panel">
  <h2>Status</h2>

  {#if config}
    <div class="status-grid">
      <div class="status-card">
        <span class="label">Mode</span>
        <div class="mode-selector">
          {#each modes as mode}
            <button
              class="mode-btn"
              class:active={currentMode === mode}
              onclick={() => switchMode(mode)}
            >
              {mode}
            </button>
          {/each}
        </div>
      </div>

      <div class="status-card">
        <span class="label">Listening</span>
        <button
          class="listen-btn"
          class:active={listening}
          onclick={toggleListening}
        >
          {listening ? "Active" : "Inactive"}
        </button>
      </div>

      <div class="status-card">
        <span class="label">Engine</span>
        <span class="value">{sttStatus?.engine ?? "whisper.cpp"}</span>
      </div>

      <div class="status-card model-card">
        <span class="label">Model</span>
        <div class="model-row">
          <span class="value" class:idle={!sttStatus?.model_loaded}>
            {#if modelLoading}
              Loading...
            {:else if sttStatus?.model_loaded}
              {config.stt_model}
            {:else}
              Not loaded
            {/if}
          </span>
          <button
            class="model-btn"
            class:loaded={sttStatus?.model_loaded}
            onclick={toggleModel}
            disabled={modelLoading}
          >
            {#if modelLoading}
              ...
            {:else if sttStatus?.model_loaded}
              Unload
            {:else}
              Load
            {/if}
          </button>
        </div>
        {#if modelError}
          <span class="model-error">{modelError}</span>
        {/if}
      </div>

      <div class="status-card">
        <span class="label">Assistant</span>
        <span class="value accent">{config.assistant_name}</span>
      </div>

      <div class="status-card">
        <span class="label">Language</span>
        <span class="value">{config.language}</span>
      </div>

      <div class="status-card">
        <span class="label">Mic Level</span>
        <AudioLevelMeter />
      </div>
    </div>

    {#if lastTranscription}
      <div class="transcription-box">
        <span class="label">Last Transcription</span>
        <p class="transcription-text">{lastTranscription}</p>
      </div>
    {/if}
  {:else}
    <p class="loading">Loading...</p>
  {/if}
</div>

<style>
  .panel {
    padding: 24px;
  }

  h2 {
    color: var(--accent);
    font-size: 1.3rem;
    margin-bottom: 20px;
    font-weight: 600;
  }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 12px;
  }

  .status-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .label {
    font-size: 12px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .value {
    font-size: 16px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .value.accent {
    color: var(--accent);
  }

  .value.idle {
    color: var(--text-muted);
  }

  .mode-selector {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .mode-btn {
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .mode-btn:hover {
    border-color: var(--accent-dim);
    color: var(--text-primary);
  }

  .mode-btn.active {
    background: var(--accent-dim);
    border-color: var(--accent);
    color: var(--accent);
  }

  .listen-btn {
    padding: 6px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-muted);
    font-size: 14px;
    cursor: pointer;
    transition: all 0.15s;
    align-self: flex-start;
  }

  .listen-btn:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .listen-btn.active {
    background: rgba(102, 187, 106, 0.15);
    border-color: var(--success);
    color: var(--success);
  }

  .model-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .model-btn {
    padding: 5px 14px;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    background: var(--accent-dim);
    color: var(--accent);
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
    flex-shrink: 0;
  }

  .model-btn:hover:not(:disabled) {
    background: var(--accent);
    color: var(--bg-primary);
  }

  .model-btn.loaded {
    border-color: var(--border);
    background: transparent;
    color: var(--text-secondary);
  }

  .model-btn.loaded:hover:not(:disabled) {
    border-color: #e57373;
    color: #e57373;
    background: rgba(229, 115, 115, 0.1);
  }

  .model-btn:disabled {
    opacity: 0.5;
    cursor: wait;
  }

  .model-error {
    font-size: 12px;
    color: #e57373;
    word-break: break-all;
  }

  .transcription-box {
    margin-top: 20px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
  }

  .transcription-text {
    color: var(--text-primary);
    font-size: 14px;
    margin-top: 6px;
    line-height: 1.6;
  }

  .loading {
    color: var(--text-muted);
  }
</style>
