<script lang="ts">
  import type { KeywordConfig } from "../lib/types";
  import { saveKeywords } from "../lib/api";

  interface Props {
    keywords: KeywordConfig | null;
    onKeywordsChanged: (keywords: KeywordConfig) => void;
  }

  let { keywords, onKeywordsChanged }: Props = $props();

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let showSaved = $state(false);
  let newCommandKey = $state("");
  let newCommandValue = $state("");
  let newDictationKey = $state("");
  let newDictationValue = $state("");

  function debounceSave() {
    if (!keywords) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      if (!keywords) return;
      try {
        await saveKeywords(keywords);
        onKeywordsChanged(keywords);
        showSaved = true;
        setTimeout(() => (showSaved = false), 2000);
      } catch (e) {
        console.error("Failed to save keywords:", e);
      }
    }, 300);
  }

  function addCommand() {
    if (!keywords || !newCommandKey.trim() || !newCommandValue.trim()) return;
    const key = newCommandKey.trim().toLowerCase().replace(/\s+/g, "_");
    keywords.commands[key] = newCommandValue.trim().toLowerCase();
    newCommandKey = "";
    newCommandValue = "";
    debounceSave();
  }

  function removeCommand(key: string) {
    if (!keywords) return;
    delete keywords.commands[key];
    keywords = { ...keywords, commands: { ...keywords.commands } };
    debounceSave();
  }

  function addDictation() {
    if (!keywords || !newDictationKey.trim() || !newDictationValue.trim()) return;
    const key = newDictationKey.trim().toLowerCase().replace(/\s+/g, "_");
    keywords.dictation[key] = newDictationValue.trim().toLowerCase();
    newDictationKey = "";
    newDictationValue = "";
    debounceSave();
  }

  function removeDictation(key: string) {
    if (!keywords) return;
    delete keywords.dictation[key];
    keywords = { ...keywords, dictation: { ...keywords.dictation } };
    debounceSave();
  }
</script>

<div class="panel">
  <div class="header">
    <h2>Keywords</h2>
    {#if showSaved}
      <span class="saved-indicator">Saved</span>
    {/if}
  </div>

  {#if keywords}
    <div class="section">
      <h3>Control Phrases</h3>
      <div class="edit-grid">
        <div class="edit-row">
          <span class="edit-label">Mode Switch</span>
          <input
            type="text"
            bind:value={keywords.mode_switch}
            oninput={debounceSave}
          />
        </div>
        <div class="edit-row">
          <span class="edit-label">Wake Phrase</span>
          <input
            type="text"
            bind:value={keywords.wake_phrase}
            oninput={debounceSave}
          />
        </div>
        <div class="edit-row">
          <span class="edit-label">Sleep Phrase</span>
          <input
            type="text"
            bind:value={keywords.sleep_phrase}
            oninput={debounceSave}
          />
        </div>
      </div>
    </div>

    <div class="section">
      <h3>System Commands</h3>
      <div class="edit-grid">
        {#each Object.entries(keywords.commands) as [key, value]}
          <div class="edit-row">
            <span class="edit-label">{key}</span>
            <input
              type="text"
              value={value}
              oninput={(e) => {
                if (!keywords) return;
                keywords.commands[key] = (e.target as HTMLInputElement).value;
                debounceSave();
              }}
            />
            <button class="btn-remove" onclick={() => removeCommand(key)} title="Remove">x</button>
          </div>
        {/each}
      </div>
      <div class="add-row">
        <input
          type="text"
          placeholder="Action (e.g. screen_right)"
          bind:value={newCommandKey}
          onkeydown={(e) => e.key === "Enter" && addCommand()}
        />
        <input
          type="text"
          placeholder="Keyword (e.g. rechts)"
          bind:value={newCommandValue}
          onkeydown={(e) => e.key === "Enter" && addCommand()}
        />
        <button class="btn-add" onclick={addCommand}>+</button>
      </div>
    </div>

    <div class="section">
      <h3>Dictation Commands</h3>
      <div class="edit-grid">
        {#each Object.entries(keywords.dictation) as [key, value]}
          <div class="edit-row">
            <span class="edit-label">{key.replace(/_/g, " ")}</span>
            <input
              type="text"
              value={value}
              oninput={(e) => {
                if (!keywords) return;
                keywords.dictation[key] = (e.target as HTMLInputElement).value;
                debounceSave();
              }}
            />
            <button class="btn-remove" onclick={() => removeDictation(key)} title="Remove">x</button>
          </div>
        {/each}
      </div>
      <div class="add-row">
        <input
          type="text"
          placeholder="Action (e.g. tab)"
          bind:value={newDictationKey}
          onkeydown={(e) => e.key === "Enter" && addDictation()}
        />
        <input
          type="text"
          placeholder="Keyword (e.g. tabulator)"
          bind:value={newDictationValue}
          onkeydown={(e) => e.key === "Enter" && addDictation()}
        />
        <button class="btn-add" onclick={addDictation}>+</button>
      </div>
    </div>

    <div class="section">
      <h3>Modes</h3>
      <div class="edit-grid">
        {#each Object.entries(keywords.modes) as [_key, mode]}
          <div class="mode-card">
            <span class="mode-name">{mode.name}</span>
            <span class="mode-desc">{mode.description}</span>
          </div>
        {/each}
      </div>
    </div>
  {:else}
    <p class="loading">Loading...</p>
  {/if}
</div>

<style>
  .panel {
    padding: 24px;
    overflow-y: auto;
    max-height: 100vh;
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

  .section {
    margin-bottom: 28px;
  }

  h3 {
    font-size: 14px;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 10px;
    font-weight: 500;
  }

  .edit-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .edit-row {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 8px 12px;
  }

  .edit-label {
    min-width: 140px;
    font-size: 13px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .edit-row input {
    flex: 1;
    border: none;
    background: var(--bg-tertiary);
    padding: 6px 10px;
  }

  .btn-remove {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-muted);
    width: 28px;
    height: 28px;
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.15s;
  }

  .btn-remove:hover {
    border-color: #e57373;
    color: #e57373;
    background: rgba(229, 115, 115, 0.1);
  }

  .add-row {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .add-row input {
    flex: 1;
    padding: 8px 12px;
  }

  .btn-add {
    background: var(--accent-dim);
    border: 1px solid var(--accent);
    color: var(--accent);
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.15s;
  }

  .btn-add:hover {
    background: var(--accent);
    color: var(--bg-primary);
  }

  .mode-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .mode-name {
    font-size: 14px;
    color: var(--accent);
    font-weight: 500;
  }

  .mode-desc {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .loading {
    color: var(--text-muted);
  }
</style>
