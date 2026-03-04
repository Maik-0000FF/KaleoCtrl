<script lang="ts">
  import type { SttStatus, AppMode } from "../lib/types";
  import { getSttStatus, getListeningStatus, getMode } from "../lib/api";
  import { listen } from "@tauri-apps/api/event";
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

  async function refreshStatus() {
    try {
      sttStatus = await getSttStatus();
      listening = await getListeningStatus();
      currentMode = await getMode();
    } catch (e) {
      console.error("Overlay refresh failed:", e);
    }
  }

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

  {#if lastTranscription}
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
</style>
