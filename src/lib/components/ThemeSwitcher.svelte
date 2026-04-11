<script lang="ts">
  import { themeState, setTheme, type Theme } from '$lib/stores/theme';

  const options: { value: Theme; label: string; icon: string }[] = [
    { value: 'dark', label: 'Dark', icon: '●' },
    { value: 'light', label: 'Light', icon: '○' },
  ];

  $: current = $themeState.current;
  $: foxVisible = $themeState.foxUnlocked;
</script>

<div class="switcher" role="group" aria-label="Theme">
  {#each options as opt (opt.value)}
    <button
      class="opt"
      class:active={current === opt.value}
      on:click={() => setTheme(opt.value)}
      title="{opt.label} theme"
      aria-pressed={current === opt.value}
    >
      <span class="icon">{opt.icon}</span>
      <span class="label">{opt.label}</span>
    </button>
  {/each}
  {#if foxVisible}
    <button
      class="opt"
      class:active={current === 'fox'}
      on:click={() => setTheme('fox')}
      title="Fox theme :3"
      aria-pressed={current === 'fox'}
    >
      <span class="icon">🦊</span>
      <span class="label">Fox</span>
    </button>
  {/if}
</div>

<style>
  .switcher {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
  }

  .opt {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 7px;
    color: var(--fg-muted);
    font-size: 12.5px;
    font-weight: 500;
    transition:
      background-color 140ms ease,
      color 140ms ease;
  }

  .opt:hover {
    color: var(--fg);
  }

  .opt.active {
    background: var(--surface-3);
    color: var(--fg);
    box-shadow: var(--shadow-sm);
  }

  .icon {
    font-size: 12px;
    line-height: 1;
  }
</style>
