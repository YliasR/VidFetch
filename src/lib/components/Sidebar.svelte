<script lang="ts">
  import { currentView, type View } from '$lib/stores/nav';

  const items: { id: View; label: string; icon: string }[] = [
    { id: 'download', label: 'Download', icon: '↓' },
    { id: 'queue', label: 'Queue', icon: '≡' },
    { id: 'history', label: 'History', icon: '⟲' },
    { id: 'presets', label: 'Presets', icon: '★' },
    { id: 'settings', label: 'Settings', icon: '⚙' },
  ];
</script>

<nav class="sidebar">
  {#each items as item (item.id)}
    <button
      class="nav-item"
      class:active={$currentView === item.id}
      on:click={() => currentView.set(item.id)}
    >
      <span class="icon">{item.icon}</span>
      <span class="label">{item.label}</span>
    </button>
  {/each}
</nav>

<style>
  .sidebar {
    width: 208px;
    flex-shrink: 0;
    padding: 16px 12px;
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: 8px;
    color: var(--fg-muted);
    font-size: 13.5px;
    font-weight: 500;
    text-align: left;
    transition:
      background-color 140ms ease,
      color 140ms ease;
  }

  .nav-item:hover {
    background: var(--surface-2);
    color: var(--fg);
  }

  .nav-item.active {
    background: var(--accent-muted);
    color: var(--accent);
  }

  .icon {
    display: inline-flex;
    width: 18px;
    justify-content: center;
    font-size: 15px;
  }
</style>
