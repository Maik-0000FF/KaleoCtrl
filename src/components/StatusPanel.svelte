<script lang="ts">
  import { MODES, type AppConfig, type SttStatus, type AppMode } from "../lib/types";
  import {
    getSttStatus,
    getListeningStatus,
    getMode,
    startListening,
    stopListening,
    setMode,
    loadSttModel,
    unloadSttModel,
    reactivateAfterKillswitch,
    safeListen,
  } from "../lib/api";
  import AudioLevelMeter from "./AudioLevelMeter.svelte";

  interface Props {
    config: AppConfig | null;
  }

  let { config }: Props = $props();

  let sttStatus = $state<SttStatus | null>(null);
  let listening = $state(false);
  let currentMode = $state<AppMode>("desktop");
  let lastTranscription = $state("");
  let partialText = $state("");
  let keyPending = $state(false);
  let modelLoading = $state(false);
  let modelError = $state("");
  let killswitchActive = $state(false);
  let killswitchTimer: ReturnType<typeof setTimeout> | null = null;

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

  async function reactivate() {
    try {
      currentMode = await reactivateAfterKillswitch();
      listening = true;
      killswitchActive = false;
      if (killswitchTimer) clearTimeout(killswitchTimer);
    } catch (e) {
      console.error("Failed to reactivate:", e);
    }
  }

  $effect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 2000);

    const offKeyPending = safeListen<boolean>("key_pending", (event) => {
      keyPending = event.payload;
    });

    const offPartial = safeListen<{ text: string }>(
      "transcription_partial",
      (event) => {
        partialText = event.payload.text;
      },
    );

    const offTranscription = safeListen<{ text: string }>(
      "transcription",
      (event) => {
        lastTranscription = event.payload.text;
        partialText = "";
      },
    );

    const offMode = safeListen<AppMode>("mode_changed", (event) => {
      currentMode = event.payload;
    });

    const offKillswitch = safeListen<null>("killswitch_triggered", () => {
      killswitchActive = true;
      listening = false;
      if (killswitchTimer) clearTimeout(killswitchTimer);
      killswitchTimer = setTimeout(() => {
        killswitchActive = false;
      }, 6000);
    });

    return () => {
      clearInterval(interval);
      if (killswitchTimer) clearTimeout(killswitchTimer);
      offKeyPending();
      offPartial();
      offTranscription();
      offMode();
      offKillswitch();
    };
  });
</script>

<div class="panel">
  <h2>Status</h2>

  {#if killswitchActive}
    <div class="killswitch-banner" role="alert">
      <span class="killswitch-icon" aria-hidden="true">■</span>
      <div class="killswitch-text">
        <strong>Killswitch ausgelöst</strong>
        <span>Mic gestoppt, Mode auf Sleep. Mit „Reaktivieren" zurück in den Default-Mode.</span>
      </div>
      <button class="killswitch-reactivate" onclick={reactivate}>Reaktivieren</button>
      <button class="killswitch-dismiss" onclick={() => (killswitchActive = false)} aria-label="Dismiss">×</button>
    </div>
  {/if}

  {#if config}
    <div class="status-grid">
      <div class="status-card">
        <span class="label">Mode</span>
        <div class="mode-selector">
          {#each MODES as mode}
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

    {#if keyPending}
      <div class="key-pending-box">
        <span class="key-pending-label">Taste?</span>
        <span class="key-pending-hint">Sage den Tastennamen...</span>
      </div>
    {/if}

    {#if partialText}
      <div class="transcription-box partial">
        <span class="label">Speaking...</span>
        <p class="transcription-text partial-text">{partialText}</p>
      </div>
    {/if}

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

  .transcription-box.partial {
    border-color: var(--accent-dim, rgba(79, 195, 247, 0.3));
  }

  .partial-text {
    color: var(--text-muted);
    font-style: italic;
  }

  .key-pending-box {
    margin-top: 20px;
    background: rgba(255, 167, 38, 0.1);
    border: 1px solid #ffa726;
    border-radius: var(--radius);
    padding: 14px 16px;
    display: flex;
    align-items: center;
    gap: 12px;
    animation: pulse-border 1s ease-in-out infinite alternate;
  }

  .key-pending-label {
    font-size: 18px;
    font-weight: 700;
    color: #ffa726;
  }

  .key-pending-hint {
    font-size: 13px;
    color: var(--text-secondary);
  }

  @keyframes pulse-border {
    from { border-color: #ffa726; }
    to { border-color: rgba(255, 167, 38, 0.3); }
  }

  .loading {
    color: var(--text-muted);
  }

  .killswitch-banner {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 18px;
    margin-bottom: 18px;
    background: rgba(229, 57, 53, 0.12);
    border: 1px solid #e53935;
    border-radius: var(--radius);
    animation: killswitch-pulse 0.6s ease-out;
  }

  .killswitch-icon {
    font-size: 24px;
    color: #e53935;
    line-height: 1;
  }

  .killswitch-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .killswitch-text strong {
    color: #e53935;
    font-size: 14px;
    font-weight: 700;
  }

  .killswitch-text span {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .killswitch-reactivate {
    background: #e53935;
    color: #fff;
    border: none;
    border-radius: var(--radius);
    padding: 6px 14px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }

  .killswitch-reactivate:hover {
    background: #d32f2f;
  }

  .killswitch-dismiss {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 22px;
    cursor: pointer;
    padding: 0 6px;
    line-height: 1;
  }

  .killswitch-dismiss:hover {
    color: #e53935;
  }

  @keyframes killswitch-pulse {
    from {
      transform: scale(0.98);
      opacity: 0;
    }
    to {
      transform: scale(1);
      opacity: 1;
    }
  }
</style>
