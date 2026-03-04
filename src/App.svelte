<script lang="ts">
  import "./app.css";
  import type { AppConfig, KeywordConfig } from "./lib/types";
  import { getConfig, getKeywords, getAvailableLanguages, getAvailableModels } from "./lib/api";
  import Sidebar from "./components/Sidebar.svelte";
  import StatusPanel from "./components/StatusPanel.svelte";
  import SettingsPanel from "./components/SettingsPanel.svelte";
  import KeywordsPanel from "./components/KeywordsPanel.svelte";
  import OverlayPanel from "./components/OverlayPanel.svelte";

  const isOverlay =
    new URLSearchParams(window.location.search).get("window") === "overlay";

  let activePanel = $state("status");
  let config = $state<AppConfig | null>(null);
  let keywords = $state<KeywordConfig | null>(null);
  let languages = $state<string[]>([]);
  let models = $state<string[]>([]);

  async function loadData() {
    try {
      config = await getConfig();
      languages = await getAvailableLanguages();
      models = await getAvailableModels();
      keywords = await getKeywords();
    } catch (e) {
      console.error("Failed to load data:", e);
    }
  }

  async function handleConfigChanged(updated: AppConfig) {
    config = { ...updated };
    keywords = await getKeywords(updated.language);
  }

  $effect(() => {
    if (!isOverlay) {
      loadData();
    }
  });
</script>

{#if isOverlay}
  <OverlayPanel />
{:else}
  <Sidebar {activePanel} onNavigate={(p) => (activePanel = p)} />

  <main class="content">
    {#if activePanel === "status"}
      <StatusPanel {config} />
    {:else if activePanel === "settings"}
      <SettingsPanel {config} {languages} {models} onConfigChanged={handleConfigChanged} />
    {:else if activePanel === "keywords"}
      <KeywordsPanel {keywords} onKeywordsChanged={(kw) => (keywords = { ...kw })} />
    {/if}
  </main>
{/if}

<style>
  .content {
    flex: 1;
    overflow-y: auto;
  }
</style>
