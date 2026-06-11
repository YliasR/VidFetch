<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import Header from './Header.svelte';
  import Sidebar from './Sidebar.svelte';
  import MainPane from './MainPane.svelte';
  import { initDragDrop, disposeDragDrop, dropHover, dropMessage } from '$lib/dragdrop';

  onMount(() => {
    void initDragDrop();
  });
  onDestroy(disposeDragDrop);
</script>

<div class="shell">
  <Header />
  <div class="body">
    <Sidebar />
    <main class="main">
      <MainPane />
    </main>
  </div>

  {#if $dropHover}
    <div class="drop-overlay">
      <div class="drop-box">
        <span class="drop-icon">⬇</span>
        <span>Drop URLs or a link list to enqueue</span>
      </div>
    </div>
  {/if}

  {#if $dropMessage}
    <div class="drop-toast">{$dropMessage}</div>
  {/if}
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 28px 32px;
  }

  .drop-overlay {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    backdrop-filter: blur(2px);
    pointer-events: none;
  }

  .drop-box {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px 32px;
    border: 2px dashed var(--accent);
    border-radius: 14px;
    background: var(--surface-1, var(--bg));
    color: var(--fg);
    font-size: 15px;
    font-weight: 600;
  }

  .drop-icon {
    font-size: 22px;
    color: var(--accent);
  }

  .drop-toast {
    position: fixed;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 60;
    padding: 10px 18px;
    border-radius: 999px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--fg);
    font-size: 13px;
    font-weight: 600;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.25);
  }
</style>
