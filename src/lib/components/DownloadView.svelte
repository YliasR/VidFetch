<script lang="ts">
  import { onMount } from 'svelte';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import {
    downloadStore,
    initDownload,
    probe,
    setOutputDir,
    setPreset,
    resetProbe,
  } from '$lib/stores/download';
  import { addToQueue } from '$lib/stores/queue';
  import { currentView } from '$lib/stores/nav';
  import type { QualityPreset } from '$lib/types';

  let urlInput = '';
  let initialized = false;
  let lastAddedTitle: string | null = null;

  onMount(async () => {
    if (!initialized) {
      await initDownload();
      initialized = true;
    }
  });

  const presets: { id: QualityPreset; label: string; note: string }[] = [
    { id: 'best', label: 'Best', note: 'Highest quality available' },
    { id: 'p1080', label: '1080p', note: 'Cap at 1080p' },
    { id: 'p720', label: '720p', note: 'Cap at 720p' },
    { id: 'audio-mp3', label: 'MP3', note: 'Audio only, mp3' },
    { id: 'audio-opus', label: 'Opus', note: 'Audio only, opus' },
  ];

  $: state = $downloadStore;
  $: info = state.probe.info;

  function formatDuration(seconds: number | null): string {
    if (seconds == null) return '';
    const s = Math.floor(seconds);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
    return `${m}:${String(sec).padStart(2, '0')}`;
  }

  async function handleProbe() {
    if (!urlInput.trim()) return;
    lastAddedTitle = null;
    await probe(urlInput);
  }

  async function pickFolder() {
    const picked = await openDialog({
      directory: true,
      multiple: false,
      defaultPath: state.outputDir || undefined,
    });
    if (typeof picked === 'string' && picked) {
      await setOutputDir(picked);
    }
  }

  function handleAdd() {
    if (state.probe.phase !== 'ready' || !state.probe.info || !state.probe.url) return;
    if (!state.outputDir) return;

    addToQueue({
      url: state.probe.url,
      preset: state.preset,
      outputDir: state.outputDir,
      info: state.probe.info,
    });

    lastAddedTitle = state.probe.info.title;
    urlInput = '';
    resetProbe();
  }

  function clearAndReset() {
    urlInput = '';
    lastAddedTitle = null;
    resetProbe();
  }

  function goToQueue() {
    currentView.set('queue');
  }
</script>

<section class="view">
  <header class="title">
    <h2>Download</h2>
    <p class="muted">Paste any URL supported by yt-dlp — videos, audio, playlists (single item for now).</p>
  </header>

  <div class="card url-card">
    <div class="url-row">
      <input
        class="input"
        type="url"
        placeholder="https://www.youtube.com/watch?v=…"
        bind:value={urlInput}
        on:keydown={(e) => e.key === 'Enter' && handleProbe()}
        disabled={state.probe.phase === 'probing'}
        spellcheck="false"
        autocomplete="off"
      />
      <button
        class="btn btn-primary"
        on:click={handleProbe}
        disabled={!urlInput.trim() || state.probe.phase === 'probing'}
      >
        {#if state.probe.phase === 'probing'}
          Fetching…
        {:else}
          Fetch
        {/if}
      </button>
    </div>

    {#if state.probe.phase === 'error'}
      <div class="error-inline">
        <strong>Couldn't fetch that URL.</strong>
        <code>{state.probe.error}</code>
      </div>
    {/if}

    {#if lastAddedTitle && !info}
      <div class="added-banner">
        <span class="check">✓</span>
        <span class="added-text">Added <strong>{lastAddedTitle}</strong> to the queue.</span>
        <button class="link-btn" on:click={goToQueue}>View queue →</button>
      </div>
    {/if}
  </div>

  {#if info}
    <div class="card metadata">
      {#if info.thumbnail}
        <img class="thumb" src={info.thumbnail} alt="" referrerpolicy="no-referrer" />
      {:else}
        <div class="thumb thumb-placeholder"></div>
      {/if}
      <div class="meta">
        <h3>{info.title}</h3>
        <div class="meta-row muted">
          {#if info.uploader}
            <span>{info.uploader}</span>
          {/if}
          {#if info.duration}
            <span class="dot">·</span>
            <span>{formatDuration(info.duration)}</span>
          {/if}
          {#if info.extractor}
            <span class="dot">·</span>
            <span>{info.extractor}</span>
          {/if}
          {#if info.isLive}
            <span class="dot">·</span>
            <span class="live">LIVE</span>
          {/if}
        </div>
      </div>
      <button class="btn btn-ghost reset" on:click={clearAndReset} title="Clear">✕</button>
    </div>

    <div class="card options">
      <div class="label">Quality</div>
      <div class="presets">
        {#each presets as p (p.id)}
          <button
            class="preset"
            class:active={state.preset === p.id}
            on:click={() => setPreset(p.id)}
          >
            <span class="preset-label">{p.label}</span>
            <span class="preset-note">{p.note}</span>
          </button>
        {/each}
      </div>

      <label class="label" for="outputDir">Save to</label>
      <div class="folder-row">
        <input id="outputDir" class="input" value={state.outputDir} readonly />
        <button class="btn" on:click={pickFolder}>Browse…</button>
      </div>

      <div class="primary-action">
        <button class="btn btn-primary" on:click={handleAdd} disabled={!state.outputDir}>
          Add to queue
        </button>
      </div>
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
    margin: 0 0 4px 0;
    letter-spacing: -0.2px;
  }

  .title p {
    margin: 0;
  }

  .url-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .url-row {
    display: flex;
    gap: 10px;
  }

  .url-row .input {
    flex: 1;
  }

  .error-inline {
    padding: 10px 14px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    color: var(--danger);
    font-size: 12.5px;
    line-height: 1.5;
  }

  .error-inline strong {
    display: block;
    margin-bottom: 4px;
  }

  .error-inline code {
    display: block;
    font-family: 'Consolas', 'Menlo', monospace;
    white-space: pre-wrap;
    color: inherit;
    opacity: 0.9;
  }

  .added-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--success) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--success) 35%, transparent);
    color: var(--success);
    font-size: 12.5px;
  }

  .added-banner .check {
    font-weight: 700;
  }

  .added-text {
    flex: 1;
  }

  .added-text strong {
    color: var(--fg);
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .link-btn:hover {
    text-decoration: underline;
  }

  .metadata {
    display: flex;
    gap: 16px;
    align-items: flex-start;
  }

  .thumb {
    width: 180px;
    height: 101px;
    object-fit: cover;
    border-radius: 10px;
    background: var(--surface-3);
    flex-shrink: 0;
  }

  .thumb-placeholder {
    display: block;
  }

  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .meta h3 {
    margin: 0;
    font-size: 17px;
    font-weight: 650;
    line-height: 1.35;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    font-size: 12.5px;
  }

  .dot {
    opacity: 0.6;
  }

  .live {
    color: var(--danger);
    font-weight: 600;
    letter-spacing: 0.4px;
  }

  .reset {
    width: 32px;
    height: 32px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    color: var(--fg-muted);
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .label {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.4px;
    color: var(--fg-muted);
  }

  .presets {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 8px;
  }

  .preset {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 10px 14px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    text-align: left;
    transition:
      background-color 140ms ease,
      border-color 140ms ease,
      color 140ms ease;
  }

  .preset:hover:not(:disabled) {
    border-color: var(--border-strong);
  }

  .preset.active {
    background: var(--accent-muted);
    border-color: var(--accent);
    color: var(--accent);
  }

  .preset:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .preset-label {
    font-size: 13.5px;
    font-weight: 650;
  }

  .preset-note {
    font-size: 11.5px;
    color: var(--fg-muted);
  }

  .preset.active .preset-note {
    color: var(--accent);
    opacity: 0.85;
  }

  .folder-row {
    display: flex;
    gap: 10px;
  }

  .folder-row .input {
    flex: 1;
    font-family: 'Consolas', 'Menlo', monospace;
    font-size: 12.5px;
  }

  .primary-action {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 4px;
  }
</style>
