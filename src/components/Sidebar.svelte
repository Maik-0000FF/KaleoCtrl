<script lang="ts">
  interface Props {
    activePanel: string;
    onNavigate: (panel: string) => void;
  }

  let { activePanel, onNavigate }: Props = $props();

  const items = [
    { id: "status", label: "Status", icon: "◉" },
    { id: "settings", label: "Settings", icon: "⚙" },
    { id: "keywords", label: "Keywords", icon: "⌘" },
  ];
</script>

<nav class="sidebar">
  <div class="logo">
    <span class="logo-text">KaleoCtrl</span>
  </div>
  <ul class="nav-items">
    {#each items as item}
      <li>
        <button
          class="nav-btn"
          class:active={activePanel === item.id}
          onclick={() => onNavigate(item.id)}
        >
          <span class="nav-icon">{item.icon}</span>
          <span class="nav-label">{item.label}</span>
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sidebar {
    width: var(--sidebar-width);
    min-width: var(--sidebar-width);
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .logo {
    padding: 20px 16px;
    border-bottom: 1px solid var(--border);
  }

  .logo-text {
    font-size: 1.2rem;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.5px;
  }

  .nav-items {
    list-style: none;
    padding: 8px;
    flex: 1;
  }

  .nav-btn {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-secondary);
    font-size: 14px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .nav-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-btn.active {
    background: var(--bg-tertiary);
    color: var(--accent);
  }

  .nav-icon {
    font-size: 16px;
    width: 20px;
    text-align: center;
  }
</style>
