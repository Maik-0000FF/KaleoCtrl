<script lang="ts">
  import type { SttStatus, AppMode } from "../lib/types";
  import { getSttStatus, getListeningStatus, getMode, reactivateAfterKillswitch, safeListen } from "../lib/api";
  import AudioLevelMeter from "./AudioLevelMeter.svelte";

  const modeColors: Record<AppMode, string> = {
    desktop: "#4fc3f7",
    dictation: "#66bb6a",
    terminal: "#ffa726",
    sleep: "#999",
  };

  let sttStatus = $state<SttStatus | null>(null);
  let listening = $state(false);
  let currentMode = $state<AppMode>("desktop");
  let lastTranscription = $state("");
  let partialText = $state("");
  let keyPending = $state(false);
  let killswitchActive = $state(false);
  let killswitchTimer: ReturnType<typeof setTimeout> | null = null;

  async function refreshStatus() {
    try {
      sttStatus = await getSttStatus();
      listening = await getListeningStatus();
      currentMode = await getMode();
    } catch (e) {
      console.error("Overlay refresh failed:", e);
    }
  }

  async function reactivate() {
    try {
      currentMode = await reactivateAfterKillswitch();
      listening = true;
      killswitchActive = false;
      if (killswitchTimer) clearTimeout(killswitchTimer);
    } catch (e) {
      console.error("Overlay reactivate failed:", e);
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

<div class="overlay" data-tauri-drag-region>
  <div class="header" data-tauri-drag-region>
    <div class="status-row" data-tauri-drag-region>
      <span
        class="mode-dot"
        style="background: {modeColors[currentMode]};"
      ></span>
      <span class="mode-label">{currentMode}</span>
      <span class="badge" class:active={listening}>
        {listening ? "Listening" : "Off"}
      </span>
    </div>
    <span class="model-info">
      {sttStatus?.model_loaded ? sttStatus.current_model ?? "loaded" : "No model"}
    </span>
  </div>

  <div class="meter-row">
    <AudioLevelMeter />
  </div>

  {#if killswitchActive}
    <button class="killswitch" onclick={reactivate} aria-label="Killswitch ausgelöst — klicken zum Reaktivieren">
      <span aria-hidden="true">■</span> Killswitch — klicken zum Reaktivieren
    </button>
  {:else if keyPending}
    <div class="key-pending">Taste?</div>
  {:else if partialText}
    <div class="transcription partial" title={partialText}>
      {partialText}
    </div>
  {:else if lastTranscription}
    <div class="transcription" title={lastTranscription}>
      {lastTranscription}
    </div>
  {/if}
</div>

<style>
  .overlay {
    background: var(--bg-primary, #0f0f1a);
    color: var(--text-primary, #e0e0e0);
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    font-size: 13px;
    padding: 12px;
    height: 100vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-radius: 10px;
    border: 1px solid var(--border, #333355);
    cursor: grab;
    user-select: none;
  }

  .header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .mode-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .mode-label {
    font-weight: 600;
    text-transform: capitalize;
    font-size: 14px;
  }

  .badge {
    margin-left: auto;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 11px;
    background: rgba(102, 102, 102, 0.2);
    color: var(--text-muted, #666);
  }

  .badge.active {
    background: rgba(102, 187, 106, 0.2);
    color: #66bb6a;
  }

  .model-info {
    font-size: 11px;
    color: var(--text-muted, #666);
    padding-left: 18px;
  }

  .meter-row {
    padding: 0 2px;
  }

  .transcription {
    font-size: 12px;
    color: var(--text-secondary, #999);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 6px 8px;
    background: var(--bg-secondary, #1a1a2e);
    border-radius: 6px;
    margin-top: auto;
  }

  .transcription.partial {
    color: var(--text-muted, #666);
    font-style: italic;
    border-left: 2px solid var(--accent, #4fc3f7);
  }

  .key-pending {
    font-size: 14px;
    font-weight: 700;
    color: #ffa726;
    text-align: center;
    padding: 6px 8px;
    background: rgba(255, 167, 38, 0.1);
    border: 1px solid #ffa726;
    border-radius: 6px;
    margin-top: auto;
    animation: pulse-border 1s ease-in-out infinite alternate;
  }

  @keyframes pulse-border {
    from { border-color: #ffa726; }
    to { border-color: rgba(255, 167, 38, 0.3); }
  }

  .killswitch {
    font-size: 13px;
    font-weight: 700;
    color: #e53935;
    text-align: center;
    padding: 6px 8px;
    background: rgba(229, 57, 53, 0.12);
    border: 1px solid #e53935;
    border-radius: 6px;
    margin-top: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    cursor: pointer;
    width: 100%;
    font-family: inherit;
  }

  .killswitch:hover {
    background: rgba(229, 57, 53, 0.2);
  }
</style>
