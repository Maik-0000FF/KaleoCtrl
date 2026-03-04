<script lang="ts">
  import type { AppConfig } from "../lib/types";
  import { updateConfig } from "../lib/api";

  interface Props {
    config: AppConfig | null;
    languages: string[];
    onConfigChanged: (config: AppConfig) => void;
  }

  let { config, languages, onConfigChanged }: Props = $props();

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let showSaved = $state(false);

  function debounceSave() {
    if (!config) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      if (!config) return;
      try {
        await updateConfig(config);
        onConfigChanged(config);
        showSaved = true;
        setTimeout(() => (showSaved = false), 2000);
      } catch (e) {
        console.error("Failed to save config:", e);
      }
    }, 300);
  }

  const modes = ["desktop", "dictation", "terminal", "sleep"];
</script>

<div class="panel">
  <div class="header">
    <h2>Settings</h2>
    {#if showSaved}
      <span class="saved-indicator">Saved</span>
    {/if}
  </div>

  {#if config}
    <div class="settings-grid">
      <div class="field">
        <label for="assistant-name">Assistant Name</label>
        <input
          id="assistant-name"
          type="text"
          bind:value={config.assistant_name}
          oninput={debounceSave}
        />
        <span class="hint">Wake word / command prefix</span>
      </div>

      <div class="field">
        <label for="language">Language</label>
        <select
          id="language"
          bind:value={config.language}
          onchange={debounceSave}
        >
          {#each languages as lang}
            <option value={lang}>{lang}</option>
          {/each}
        </select>
        <span class="hint">Active language for keyword matching</span>
      </div>

      <div class="field">
        <label for="stt-model">STT Model</label>
        <input
          id="stt-model"
          type="text"
          bind:value={config.stt_model}
          oninput={debounceSave}
        />
        <span class="hint">whisper.cpp model name</span>
      </div>

      <div class="field">
        <label for="default-mode">Default Mode</label>
        <select
          id="default-mode"
          bind:value={config.default_mode}
          onchange={debounceSave}
        >
          {#each modes as mode}
            <option value={mode}>{mode}</option>
          {/each}
        </select>
        <span class="hint">Mode on startup</span>
      </div>
    </div>
  {:else}
    <p class="loading">Loading...</p>
  {/if}
</div>

<style>
  .panel {
    padding: 24px;
  }

  .header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 20px;
  }

  h2 {
    color: var(--accent);
    font-size: 1.3rem;
    font-weight: 600;
  }

  .saved-indicator {
    font-size: 12px;
    color: var(--success);
    background: rgba(102, 187, 106, 0.1);
    padding: 2px 10px;
    border-radius: 12px;
    animation: fade-in 0.2s;
  }

  @keyframes fade-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 20px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .hint {
    font-size: 11px;
    color: var(--text-muted);
  }

  input,
  select {
    width: 100%;
  }

  .loading {
    color: var(--text-muted);
  }
</style>
