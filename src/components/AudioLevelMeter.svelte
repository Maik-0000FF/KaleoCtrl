<script lang="ts">
  import { safeListen } from "../lib/api";

  let level = $state(0);

  $effect(() => {
    return safeListen<number>("audio_level", (event) => {
      level = event.payload;
    });
  });

  const barColor = $derived(
    level > 0.8 ? "var(--level-red)" : level > 0.5 ? "var(--level-orange)" : "var(--level-blue)",
  );
</script>

<div class="level-meter">
  <div
    class="level-bar"
    style="width: {level * 100}%; background: {barColor};"
  ></div>
</div>

<style>
  .level-meter {
    width: 100%;
    height: 8px;
    background: var(--bg-primary, #1a1a2e);
    border-radius: 4px;
    overflow: hidden;

    --level-blue: #42a5f5;
    --level-orange: #ffa726;
    --level-red: #ef5350;
  }

  .level-bar {
    height: 100%;
    border-radius: 4px;
    transition: width 0.1s ease, background 0.1s ease;
    min-width: 0;
  }
</style>
