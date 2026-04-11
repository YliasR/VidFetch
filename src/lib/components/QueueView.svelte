<script lang="ts">
  import { onMount } from 'svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import {
    queueStore,
    initQueue,
    cancelItem,
    removeFromQueue,
    clearCompleted,
    moveItem,
    setConcurrency,
    type QueueItem,
    type QueueItemStatus,
  } from '$lib/stores/queue';
  import { currentView } from '$lib/stores/nav';

  let initialized = false;

  onMount(async () => {
    if (!initialized) {
      await initQueue();
      initialized = true;
    }
  });

  $: state = $queueStore;
  $: items = state.items;
  $: hasItems = items.length > 0;
  $: hasCompleted = items.some(
    (i) => i.status === 'done' || i.status === 'canceled' || i.status === 'error'
  );

  function formatBytes(bytes: number | null | undefined): string {
    if (bytes == null || bytes <= 0) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function formatSpeed(bps: number | null): string {
    if (bps == null) return '—';
    return `${formatBytes(bps)}/s`;
  }

  function formatEta(seconds: number | null): string {
    if (seconds == null) return '—';
    if (seconds < 60) return `${seconds}s`;
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    if (m < 60) return `${m}m ${s}s`;
    const h = Math.floor(m / 60);
    return `${h}h ${m % 60}m`;
  }

  function percentOf(item: QueueItem): number | null {
    if (item.status === 'done') return 100;
    if (item.total && item.total > 0) {
      return Math.min(100, (item.downloaded / item.total) * 100);
    }
    return null;
  }

  function isActive(s: QueueItemStatus): boolean {
    return s === 'downloading' || s === 'postprocess' || s === 'starting';
  }

  function statusLabel(s: QueueItemStatus): string {
    switch (s) {
      case 'queued': return 'Queued';
      case 'starting': return 'Starting…';
      case 'downloading': return 'Downloading';
      case 'postprocess': return 'Post-processing';
      case 'done': return '✓ Done';
      case 'canceled': return 'Canceled';
      case 'error': return 'Error';
    }
  }

  async function openFolder(item: QueueItem) {
    try {
      await openPath(item.outputDir);
    } catch (err) {
      console.warn('[queue] open folder failed', err);
    }
  }

  function onConcurrencyChange(e: Event) {
    const v = parseInt((e.target as HTMLInputElement).value, 10);
    if (!Number.isNaN(v)) setConcurrency(v);
  }
</script>

<section class="view">
  <header class="title">
    <div class="title-row">
      <h2>Queue</h2>
      <div class="controls">
        <label class="conc">
          <span>Concurrent</span>
          <input
            type="number"
            min="1"
            max="5"
            value={state.concurrency}
            on:change={onConcurrencyChange}
          />
        </label>
        {#if hasCompleted}
          <button class="btn btn-ghost" on:click={clearCompleted}>Clear completed</button>
        {/if}
      </div>
    </div>
    <p class="muted">Downloads run up to {state.concurrency} at a time. New items wait their turn.</p>
  </header>

  {#if !hasItems}
    <div class="card empty">
      <p class="empty-title">No downloads yet</p>
      <p class="muted">
        Head to the
        <button class="link-btn" on:click={() => currentView.set('download')}>Download</button>
        view to add something.
      </p>
    </div>
  {:else}
    <div class="items">
      {#each items as item, idx (item.id)}
        {@const pct = percentOf(item)}
        <div class="card item" class:active={isActive(item.status)}>
          {#if item.info?.thumbnail}
            <img class="thumb" src={item.info.thumbnail} alt="" referrerpolicy="no-referrer" />
          {:else}
            <div class="thumb thumb-placeholder"></div>
          {/if}

          <div class="body">
            <div class="head">
              <h3 title={item.info?.title ?? item.url}>
                {item.info?.title ?? item.url}
              </h3>
              <span class="status" class:done={item.status === 'done'} class:err={item.status === 'error' || item.status === 'canceled'}>
                {statusLabel(item.status)}
              </span>
            </div>

            <div class="meta muted">
              <span class="preset-tag">{item.preset}</span>
              {#if item.info?.uploader}
                <span class="dot">·</span>
                <span>{item.info.uploader}</span>
              {/if}
              {#if item.status === 'downloading' || item.status === 'postprocess'}
                <span class="dot">·</span>
                <span class="num">
                  {formatBytes(item.downloaded)}{item.total ? ` / ${formatBytes(item.total)}` : ''}
                </span>
                <span class="dot">·</span>
                <span class="num">{formatSpeed(item.speed)}</span>
                <span class="dot">·</span>
                <span class="num">ETA {formatEta(item.eta)}</span>
              {/if}
            </div>

            <div class="bar" class:indeterminate={pct === null && isActive(item.status)}>
              <div
                class="fill"
                class:done={item.status === 'done'}
                class:error={item.status === 'error' || item.status === 'canceled'}
                style:width={pct !== null ? `${pct}%` : undefined}
              ></div>
            </div>

            {#if item.message && (item.status === 'error' || item.status === 'canceled')}
              <pre class="message">{item.message}</pre>
            {/if}
          </div>

          <div class="actions">
            {#if item.status === 'queued'}
              <button
                class="icon-btn"
                title="Move up"
                disabled={idx === 0}
                on:click={() => moveItem(item.id, -1)}
              >↑</button>
              <button
                class="icon-btn"
                title="Move down"
                disabled={idx === items.length - 1}
                on:click={() => moveItem(item.id, 1)}
              >↓</button>
              <button class="icon-btn danger" title="Remove" on:click={() => removeFromQueue(item.id)}>✕</button>
            {:else if isActive(item.status)}
              <button class="icon-btn danger" title="Cancel" on:click={() => cancelItem(item.id)}>✕</button>
            {:else if item.status === 'done'}
              <button class="icon-btn" title="Open folder" on:click={() => openFolder(item)}>📁</button>
              <button class="icon-btn" title="Remove" on:click={() => removeFromQueue(item.id)}>✕</button>
            {:else}
              <button class="icon-btn" title="Remove" on:click={() => removeFromQueue(item.id)}>✕</button>
            {/if}
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

  .controls {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .conc {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--fg-muted);
    text-transform: uppercase;
    letter-spacing: 0.4px;
    font-weight: 600;
  }

  .conc input {
    width: 56px;
    padding: 6px 8px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--fg);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    text-align: center;
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
    gap: 6px;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }

  .head h3 {
    margin: 0;
    font-size: 14.5px;
    font-weight: 600;
    line-height: 1.35;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.4px;
    color: var(--accent);
    flex-shrink: 0;
  }

  .status.done {
    color: var(--success);
  }

  .status.err {
    color: var(--danger);
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

  .bar {
    position: relative;
    height: 6px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
    margin-top: 2px;
  }

  .fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-strong));
    border-radius: inherit;
    transition: width 160ms ease;
  }

  .fill.done {
    width: 100% !important;
    background: var(--success);
  }

  .fill.error {
    width: 100% !important;
    background: var(--danger);
  }

  .bar.indeterminate .fill {
    width: 40%;
    animation: slide 1.3s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  @keyframes slide {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(300%); }
  }

  .message {
    margin: 4px 0 0 0;
    padding: 8px 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    font-family: 'Consolas', 'Menlo', monospace;
    font-size: 11.5px;
    white-space: pre-wrap;
    color: var(--fg-muted);
    max-height: 100px;
    overflow: auto;
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

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
