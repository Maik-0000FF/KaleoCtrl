<script lang="ts">
  import { MODES, type AppConfig } from "../lib/types";
  import {
    updateConfig,
    getModelCatalog,
    downloadModel,
    deleteModel,
    safeListen,
    type ModelCatalogEntry,
  } from "../lib/api";

  interface Props {
    config: AppConfig | null;
    languages: string[];
    models: string[];
    onConfigChanged: (config: AppConfig) => void;
    onModelsChanged: () => void;
  }

  let { config, languages, models, onConfigChanged, onModelsChanged }: Props = $props();

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let showSaved = $state(false);
  let catalog = $state<ModelCatalogEntry[]>([]);
  let downloading = $state<string | null>(null);
  let downloadProgress = $state(0);
  let downloadTotal = $state(0);
  let downloadError = $state("");

  async function loadCatalog() {
    try {
      catalog = await getModelCatalog();
    } catch (e) {
      console.error("Failed to load model catalog:", e);
    }
  }

  async function handleDownload(name: string) {
    downloading = name;
    downloadProgress = 0;
    downloadError = "";
    try {
      await downloadModel(name);
    } catch (e) {
      downloadError = String(e);
      downloading = null;
    }
  }

  async function handleDelete(name: string) {
    try {
      await deleteModel(name);
      await loadCatalog();
      onModelsChanged();
    } catch (e) {
      console.error("Failed to delete model:", e);
    }
  }

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

  $effect(() => {
    loadCatalog();

    return safeListen<{
      model: string;
      downloaded_mb: number;
      total_mb: number;
      done: boolean;
      error: string | null;
    }>("download_progress", (event) => {
      const p = event.payload;
      downloadProgress = p.downloaded_mb;
      downloadTotal = p.total_mb;
      if (p.done) {
        if (p.error) {
          downloadError = p.error;
        }
        downloading = null;
        loadCatalog();
        onModelsChanged();
      }
    });
  });
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
        <select
          id="stt-model"
          bind:value={config.stt_model}
          onchange={debounceSave}
        >
          {#each models as model}
            <option value={model}>{model}</option>
          {/each}
          {#if models.length === 0}
            <option value={config.stt_model}>{config.stt_model}</option>
          {/if}
        </select>
        <span class="hint">whisper.cpp model (from models/ directory)</span>
      </div>

      <div class="field">
        <label for="default-mode">Default Mode</label>
        <select
          id="default-mode"
          bind:value={config.default_mode}
          onchange={debounceSave}
        >
          {#each MODES as mode}
            <option value={mode}>{mode}</option>
          {/each}
        </select>
        <span class="hint">Mode on startup</span>
      </div>
    </div>

    <h3 class="section-title">Model Manager</h3>
    <div class="model-catalog">
      {#each catalog as entry}
        <div class="catalog-entry" class:downloaded={entry.downloaded}>
          <div class="catalog-info">
            <span class="catalog-name">{entry.name}</span>
            <span class="catalog-size">{entry.size_mb >= 1000 ? (entry.size_mb / 1024).toFixed(1) + " GB" : entry.size_mb + " MB"}</span>
          </div>
          <p class="catalog-desc">{entry.description}</p>
          <div class="catalog-actions">
            {#if downloading === entry.name}
              <div class="progress-bar">
                <div class="progress-fill" style="width: {downloadTotal > 0 ? (downloadProgress / downloadTotal) * 100 : 0}%"></div>
              </div>
              <span class="progress-text">{downloadProgress} / {downloadTotal} MB</span>
            {:else if entry.downloaded}
              <span class="downloaded-badge">Installed</span>
              <button class="delete-btn" onclick={() => handleDelete(entry.name)}>Remove</button>
            {:else}
              <button class="download-btn" onclick={() => handleDownload(entry.name)} disabled={downloading !== null}>Download</button>
            {/if}
          </div>
        </div>
      {/each}
      {#if downloadError}
        <p class="download-error">{downloadError}</p>
      {/if}
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

  .section-title {
    color: var(--accent);
    font-size: 1rem;
    font-weight: 600;
    margin-top: 32px;
    margin-bottom: 12px;
  }

  .model-catalog {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .catalog-entry {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px 16px;
  }

  .catalog-entry.downloaded {
    border-color: var(--accent-dim);
  }

  .catalog-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .catalog-name {
    font-weight: 600;
    color: var(--text-primary);
    font-size: 14px;
  }

  .catalog-size {
    font-size: 12px;
    color: var(--text-muted);
  }

  .catalog-desc {
    font-size: 12px;
    color: var(--text-secondary);
    margin: 4px 0 8px;
  }

  .catalog-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .download-btn {
    padding: 4px 14px;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    background: var(--accent-dim);
    color: var(--accent);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .download-btn:hover:not(:disabled) {
    background: var(--accent);
    color: var(--bg-primary);
  }

  .download-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .delete-btn {
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .delete-btn:hover {
    border-color: #e57373;
    color: #e57373;
    background: rgba(229, 115, 115, 0.1);
  }

  .downloaded-badge {
    font-size: 12px;
    color: var(--success);
    background: rgba(102, 187, 106, 0.1);
    padding: 2px 10px;
    border-radius: 12px;
  }

  .progress-bar {
    flex: 1;
    height: 6px;
    background: var(--bg-tertiary);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.3s;
  }

  .progress-text {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .download-error {
    font-size: 12px;
    color: #e57373;
    margin-top: 4px;
  }
</style>
