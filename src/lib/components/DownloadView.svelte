<script lang="ts">
  import { onMount } from 'svelte';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { openPath } from '@tauri-apps/plugin-opener';
  import {
    downloadStore,
    initDownload,
    probe,
    setOutputDir,
    setPreset,
    startDownload,
    cancelCurrent,
    resetProbe,
  } from '$lib/stores/download';
  import type { QualityPreset } from '$lib/types';

  let urlInput = '';
  let initialized = false;

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
  $: job = state.job;
  $: downloading =
    job.status === 'queued' ||
    job.status === 'downloading' ||
    job.status === 'postprocess';

  $: percent =
    job.total && job.total > 0
      ? Math.min(100, (job.downloaded / job.total) * 100)
      : null;

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

  async function openOutputDir() {
    if (state.outputDir) {
      try {
        await openPath(state.outputDir);
      } catch (err) {
        console.warn('[download] open folder failed', err);
      }
    }
  }

  function clearAndReset() {
    urlInput = '';
    resetProbe();
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
        disabled={state.probe.phase === 'probing' || downloading}
        spellcheck="false"
        autocomplete="off"
      />
      <button
        class="btn btn-primary"
        on:click={handleProbe}
        disabled={!urlInput.trim() || state.probe.phase === 'probing' || downloading}
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
            disabled={downloading}
          >
            <span class="preset-label">{p.label}</span>
            <span class="preset-note">{p.note}</span>
          </button>
        {/each}
      </div>

      <label class="label" for="outputDir">Save to</label>
      <div class="folder-row">
        <input id="outputDir" class="input" value={state.outputDir} readonly />
        <button class="btn" on:click={pickFolder} disabled={downloading}>Browse…</button>
      </div>

      <div class="primary-action">
        {#if downloading}
          <button class="btn" on:click={cancelCurrent}>Cancel</button>
        {:else if job.status === 'done'}
          <button class="btn" on:click={openOutputDir}>Open folder</button>
          <button class="btn btn-primary" on:click={clearAndReset}>Download another</button>
        {:else}
          <button class="btn btn-primary" on:click={startDownload}>Download</button>
        {/if}
      </div>
    </div>

    {#if job.id || job.status !== 'idle'}
      <div class="card progress-card">
        <div class="progress-head">
          <strong>
            {#if job.status === 'queued'}
              Starting…
            {:else if job.status === 'downloading'}
              Downloading
            {:else if job.status === 'postprocess'}
              Post-processing
            {:else if job.status === 'done'}
              ✓ Done
            {:else if job.status === 'canceled'}
              Canceled
            {:else if job.status === 'error'}
              Error
            {/if}
          </strong>
          <span class="muted num">
            {#if job.status === 'downloading' || job.status === 'postprocess'}
              {formatBytes(job.downloaded)}{job.total ? ` / ${formatBytes(job.total)}` : ''}
              · {formatSpeed(job.speed)}
              · ETA {formatEta(job.eta)}
            {/if}
          </span>
        </div>
        <div class="bar" class:indeterminate={percent === null && downloading}>
          <div
            class="fill"
            class:done={job.status === 'done'}
            class:error={job.status === 'error' || job.status === 'canceled'}
            style:width={percent !== null ? `${percent}%` : undefined}
          ></div>
        </div>
        {#if job.message}
          <pre class="message">{job.message}</pre>
        {/if}
      </div>
    {/if}
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

  .progress-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .progress-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 10px;
  }

  .num {
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .bar {
    position: relative;
    height: 10px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
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
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(300%);
    }
  }

  .message {
    margin: 0;
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    font-family: 'Consolas', 'Menlo', monospace;
    font-size: 12px;
    white-space: pre-wrap;
    color: var(--fg-muted);
    max-height: 160px;
    overflow: auto;
  }
</style>
