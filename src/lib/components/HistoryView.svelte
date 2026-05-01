<script lang="ts">
  import { onMount } from 'svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import {
    historyStore,
    initHistory,
    removeHistoryEntry,
    clearHistory,
    type HistoryEntry,
  } from '$lib/stores/history';
  import { addToQueue } from '$lib/stores/queue';
  import { currentView } from '$lib/stores/nav';

  onMount(initHistory);

  $: entries = $historyStore.entries;
  $: hasEntries = entries.length > 0;

  function formatBytes(bytes: number | null | undefined): string {
    if (bytes == null || bytes <= 0) return '—';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function formatDate(ts: number): string {
    const d = new Date(ts);
    const now = new Date();
    const sameDay = d.toDateString() === now.toDateString();
    if (sameDay) {
      return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }
    return d.toLocaleDateString([], {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  async function openFolder(entry: HistoryEntry) {
    try {
      await openPath(entry.outputDir);
    } catch (err) {
      console.warn('[history] open folder failed', err);
    }
  }

  function reDownload(entry: HistoryEntry) {
    addToQueue({
      options: entry.options,
      display: {
        title: entry.title,
        thumbnail: entry.thumbnail,
        uploader: entry.uploader,
        duration: null,
      },
    });
    currentView.set('queue');
  }

  async function confirmClear() {
    if (entries.length === 0) return;
    const ok = window.confirm(`Clear all ${entries.length} history entries?`);
    if (ok) await clearHistory();
  }
</script>

<section class="view">
  <header class="title">
    <div class="title-row">
      <h2>History</h2>
      {#if hasEntries}
        <button class="btn btn-ghost" on:click={confirmClear}>Clear all</button>
      {/if}
    </div>
    <p class="muted">Completed downloads. Open the folder or re-queue with the same options.</p>
  </header>

  {#if !hasEntries}
    <div class="card empty">
      <p class="empty-title">Nothing here yet</p>
      <p class="muted">
        Finished downloads land here. Head to the
        <button class="link-btn" on:click={() => currentView.set('download')}>Download</button>
        view to add something.
      </p>
    </div>
  {:else}
    <div class="items">
      {#each entries as entry (entry.id)}
        <div class="card item">
          {#if entry.thumbnail}
            <img class="thumb" src={entry.thumbnail} alt="" referrerpolicy="no-referrer" />
          {:else}
            <div class="thumb thumb-placeholder"></div>
          {/if}

          <div class="body">
            <h3 title={entry.title}>{entry.title}</h3>
            <div class="meta muted">
              <span class="preset-tag">{entry.preset}</span>
              {#if entry.outputFormat && entry.outputFormat !== 'auto'}
                <span class="dot">·</span>
                <span class="num">{entry.outputFormat}</span>
              {/if}
              {#if entry.uploader}
                <span class="dot">·</span>
                <span>{entry.uploader}</span>
              {/if}
              <span class="dot">·</span>
              <span class="num">{formatBytes(entry.sizeBytes)}</span>
              <span class="dot">·</span>
              <span class="num">{formatDate(entry.completedAt)}</span>
            </div>
            <div class="path muted" title={entry.outputDir}>{entry.outputDir}</div>
          </div>

          <div class="actions">
            <button class="icon-btn" title="Open folder" on:click={() => openFolder(entry)}>📁</button>
            <button class="icon-btn" title="Re-download" on:click={() => reDownload(entry)}>↻</button>
            <button class="icon-btn danger" title="Remove from history" on:click={() => removeHistoryEntry(entry.id)}>✕</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .view {
    max-width: 920px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .title h2 {
    font-size: 24px;
    font-weight: 650;
    margin: 0;
    letter-spacing: -0.2px;
  }

  .title p {
    margin: 4px 0 0 0;
  }

  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .empty {
    padding: 32px;
    text-align: center;
  }

  .empty-title {
    margin: 0 0 6px 0;
    font-weight: 600;
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    font-weight: 600;
    cursor: pointer;
  }

  .link-btn:hover {
    text-decoration: underline;
  }

  .items {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .item {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }

  .thumb {
    width: 120px;
    height: 68px;
    object-fit: cover;
    border-radius: 8px;
    background: var(--surface-3);
    flex-shrink: 0;
  }

  .thumb-placeholder {
    display: block;
  }

  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .body h3 {
    margin: 0;
    font-size: 14.5px;
    font-weight: 600;
    line-height: 1.35;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    font-size: 11.5px;
  }

  .preset-tag {
    padding: 1px 7px;
    border-radius: 4px;
    background: var(--surface-3);
    color: var(--fg);
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .dot {
    opacity: 0.6;
  }

  .num {
    font-variant-numeric: tabular-nums;
  }

  .path {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: 'Consolas', 'Menlo', monospace;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex-shrink: 0;
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--fg-muted);
    font-size: 13px;
    transition: background-color 120ms ease, color 120ms ease, border-color 120ms ease;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--fg);
  }

  .icon-btn.danger:hover:not(:disabled) {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 40%, transparent);
  }
</style>
