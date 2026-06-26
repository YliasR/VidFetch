<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { openPath } from '@tauri-apps/plugin-opener';
  import {
    ipc,
    type MediaInfo,
    type AudioFormat,
    type ReplaceAudioMode,
    type ReplaceAudioAlign,
  } from '$lib/ipc';

  /** Audio ops on a single video source. */
  type AudioOp = 'remove' | 'replace' | 'extract' | 'volume';
  type Phase = 'idle' | 'probing' | 'ready' | 'encoding' | 'done' | 'error';

  const VIDEO_EXTS = ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'ts'];
  const AUDIO_EXTS = ['mp3', 'opus', 'm4a', 'aac', 'flac', 'wav', 'ogg', 'wma'];
  const isVideoPath = (p: string) => VIDEO_EXTS.includes(p.split('.').pop()?.toLowerCase() ?? '');

  const OPS: { id: AudioOp; label: string }[] = [
    { id: 'remove', label: 'Remove audio' },
    { id: 'replace', label: 'Replace audio' },
    { id: 'extract', label: 'Extract audio' },
    { id: 'volume', label: 'Volume' },
  ];

  let phase: Phase = 'idle';
  let source: MediaInfo | null = null;
  let error: string | null = null;
  let op: AudioOp = 'remove';
  let outputPath = '';
  let jobId: string | null = null;
  let fraction = 0;
  let doneMessage: string | null = null;

  // Replace-audio controls.
  let audioPath = '';
  let replaceMode: ReplaceAudioMode = 'replace';
  let replaceAlign: ReplaceAudioAlign = 'trim';
  let fadeIn = 0;
  let fadeOut = 0;

  // Extract-audio control.
  let extractFormat: AudioFormat = 'mp3';

  // Volume control + waveform preview.
  let gainDb = 0;
  let waveform: string | null = null;
  let waveformPath = '';

  let unlisteners: UnlistenFn[] = [];

  $: exporting = phase === 'encoding';
  $: noAudio = source != null && !source.hasAudio;
  $: opNeedsAudio = op === 'remove' || op === 'extract' || op === 'volume';

  onMount(async () => {
    unlisteners = await Promise.all([
      listen<{ id: string; status: string; message: string | null }>(
        'edit://status',
        ({ payload }) => {
          if (payload.id !== jobId) return;
          if (payload.status === 'encoding') phase = 'encoding';
          else if (payload.status === 'done') {
            phase = 'done';
            fraction = 1;
            doneMessage = payload.message;
          } else if (payload.status === 'error') {
            phase = 'error';
            error = payload.message ?? 'operation failed';
          } else if (payload.status === 'canceled') {
            phase = source ? 'ready' : 'idle';
            fraction = 0;
          }
        },
      ),
      listen<{ id: string; fraction: number }>('edit://progress', ({ payload }) => {
        if (payload.id !== jobId) return;
        fraction = payload.fraction;
      }),
      getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type !== 'drop' || exporting) return;
        const media = event.payload.paths.find(isVideoPath);
        if (media) void loadSource(media);
      }),
    ]);
  });

  onDestroy(() => {
    for (const unlisten of unlisteners) unlisten();
  });

  async function pickSource() {
    const picked = await openDialog({
      multiple: false,
      filters: [
        { name: 'Video', extensions: VIDEO_EXTS },
        { name: 'All files', extensions: ['*'] },
      ],
    });
    if (typeof picked !== 'string' || !picked) return;
    await loadSource(picked);
  }

  async function loadSource(path: string) {
    phase = 'probing';
    error = null;
    source = null;
    waveform = null;
    waveformPath = '';
    try {
      source = await ipc.probeMedia(path);
      // Mix only makes sense when there is already a track to mix into.
      if (!source.hasAudio) replaceMode = 'replace';
      outputPath = suggestOutput();
      phase = 'ready';
      if (op === 'volume') void loadWaveform();
    } catch (err) {
      phase = 'error';
      error = String(err);
    }
  }

  async function pickAudio() {
    const picked = await openDialog({
      multiple: false,
      filters: [
        { name: 'Audio', extensions: [...AUDIO_EXTS, ...VIDEO_EXTS] },
        { name: 'All files', extensions: ['*'] },
      ],
    });
    if (typeof picked === 'string' && picked) audioPath = picked;
  }

  /** Suggest an output path from the source + current op. Re-run on op change. */
  function suggestOutput(): string {
    if (!source) return '';
    const path = source.path;
    const stem = path.replace(/\.[^\\/.]+$/, '');
    const ext = path.split('.').pop()?.toLowerCase() || 'mp4';
    switch (op) {
      case 'remove':
        return `${stem}-muted.${ext}`;
      case 'replace':
        return `${stem}-newaudio.${ext}`;
      case 'extract':
        return `${stem}.${extractFormat}`;
      case 'volume':
        return `${stem}-vol.${ext}`;
    }
  }

  function selectOp(next: AudioOp) {
    if (exporting) return;
    op = next;
    phase = source ? 'ready' : phase === 'done' || phase === 'error' ? 'idle' : phase;
    error = null;
    doneMessage = null;
    if (source) outputPath = suggestOutput();
    if (op === 'volume' && source) void loadWaveform();
  }

  function onExtractFormatChange() {
    if (source) outputPath = suggestOutput();
  }

  async function loadWaveform() {
    if (!source || !source.hasAudio || waveformPath === source.path) return;
    try {
      const img = await ipc.audioWaveform(source.path, 760, 96);
      waveform = img;
      waveformPath = source.path;
    } catch {
      waveform = null;
    }
  }

  async function pickOutput() {
    const ext = outputPath.split('.').pop()?.toLowerCase() || 'mp4';
    const picked = await saveDialog({
      defaultPath: outputPath || undefined,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (typeof picked === 'string' && picked) outputPath = picked;
  }

  $: canRun =
    source != null &&
    !exporting &&
    phase !== 'probing' &&
    !!outputPath &&
    !(opNeedsAudio && noAudio) &&
    (op !== 'replace' || (!!audioPath && fadeIn >= 0 && fadeOut >= 0));

  async function run() {
    if (!source || !canRun) return;
    error = null;
    fraction = 0;
    doneMessage = null;
    try {
      if (op === 'remove') {
        jobId = await ipc.removeAudio({ inputPath: source.path, outputPath });
      } else if (op === 'extract') {
        jobId = await ipc.extractAudio({
          inputPath: source.path,
          outputPath,
          format: extractFormat,
        });
      } else if (op === 'volume') {
        jobId = await ipc.adjustVolume({ inputPath: source.path, outputPath, gainDb });
      } else {
        jobId = await ipc.replaceAudio({
          inputPath: source.path,
          audioPath,
          outputPath,
          mode: replaceMode,
          align: replaceAlign,
          fadeIn,
          fadeOut,
        });
      }
      phase = 'encoding';
    } catch (err) {
      phase = 'error';
      error = String(err);
    }
  }

  async function cancel() {
    if (jobId) await ipc.cancelExport(jobId);
  }

  async function revealOutput() {
    const dir = outputPath.replace(/[\\/][^\\/]+$/, '');
    if (dir) await openPath(dir);
  }

  function fileName(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function formatTime(secs: number): string {
    const total = Math.max(0, secs);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = Math.floor(total % 60);
    return h > 0
      ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
      : `${m}:${String(s).padStart(2, '0')}`;
  }
</script>

<div class="card">
  <div class="label">Source</div>
  <div class="source-row">
    <input
      class="input mono"
      type="text"
      readonly
      placeholder="No file selected — browse or drop a video"
      value={source?.path ?? ''}
    />
    <button class="btn" on:click={pickSource} disabled={exporting || phase === 'probing'}>
      {phase === 'probing' ? 'Probing…' : 'Browse'}
    </button>
  </div>
  {#if source}
    <div class="meta muted">
      {#if source.hasAudio}
        <span class="badge has">Has audio</span>
      {:else}
        <span class="badge none">No audio track</span>
      {/if}
      {#if source.width != null && source.height != null}
        <span>{source.width}×{source.height}</span>
      {/if}
      {#if source.duration != null}
        <span class="dot">·</span>
        <span>{formatTime(source.duration)}</span>
      {/if}
    </div>
  {/if}
</div>

{#if source}
  <div class="card">
    <div class="label">Operation</div>
    <div class="modes" role="tablist">
      {#each OPS as o (o.id)}
        <button
          class="mode-tab"
          class:active={op === o.id}
          role="tab"
          aria-selected={op === o.id}
          disabled={exporting}
          on:click={() => selectOp(o.id)}
        >
          {o.label}
        </button>
      {/each}
    </div>

    {#if opNeedsAudio && noAudio}
      <p class="hint warn">This file has no audio track, so there's nothing to {op}.</p>
    {/if}

    {#if op === 'remove'}
      <p class="hint muted">
        Strips the audio track and copies the video untouched — lossless and near-instant.
      </p>
    {:else if op === 'extract'}
      <div class="grid">
        <label class="field">
          <span class="field-label">Format</span>
          <select class="input" bind:value={extractFormat} on:change={onExtractFormatChange} disabled={exporting}>
            <option value="mp3">MP3 (libmp3lame -q:a 2)</option>
            <option value="opus">Opus (160k)</option>
            <option value="flac">FLAC (lossless)</option>
          </select>
        </label>
      </div>
    {:else if op === 'volume'}
      <div class="vol-row">
        <span class="field-label">Gain</span>
        <input
          class="slider"
          type="range"
          min="-30"
          max="30"
          step="0.5"
          bind:value={gainDb}
          disabled={exporting || noAudio}
        />
        <span class="gain-value mono">{gainDb > 0 ? '+' : ''}{gainDb} dB</span>
        <button class="btn btn-ghost btn-sm" on:click={() => (gainDb = 0)} disabled={exporting}>Reset</button>
      </div>
      {#if waveform}
        <img class="waveform" src={waveform} alt="Audio waveform of the source" />
      {:else if !noAudio}
        <div class="waveform placeholder"></div>
      {/if}
    {:else if op === 'replace'}
      <div class="source-row">
        <input
          class="input mono"
          type="text"
          readonly
          placeholder="No audio file selected"
          value={audioPath}
        />
        <button class="btn" on:click={pickAudio} disabled={exporting}>Browse</button>
      </div>
      <div class="grid">
        <label class="field">
          <span class="field-label">Mode</span>
          <select class="input" bind:value={replaceMode} disabled={exporting}>
            <option value="replace">Replace existing</option>
            <option value="mix" disabled={noAudio}>Mix with existing</option>
          </select>
        </label>
        <label class="field">
          <span class="field-label">Align to video</span>
          <select class="input" bind:value={replaceAlign} disabled={exporting}>
            <option value="trim">Trim / pad</option>
            <option value="loop">Loop</option>
          </select>
        </label>
        <label class="field">
          <span class="field-label">Fade in (s)</span>
          <input class="input" type="number" min="0" max="60" step="0.5" bind:value={fadeIn} disabled={exporting} />
        </label>
        <label class="field">
          <span class="field-label">Fade out (s)</span>
          <input class="input" type="number" min="0" max="60" step="0.5" bind:value={fadeOut} disabled={exporting} />
        </label>
      </div>
      <p class="hint muted">
        {#if replaceAlign === 'loop'}
          The new audio repeats to fill the whole video.
        {:else}
          The new audio is trimmed if longer than the video, or padded with silence if shorter.
        {/if}
        {#if replaceMode === 'mix'}
          It's blended with the original track at full level.
        {/if}
      </p>
    {/if}
  </div>

  <div class="card">
    <div class="label">Output</div>
    <div class="source-row">
      <input class="input mono" type="text" bind:value={outputPath} disabled={exporting} />
      <button class="btn" on:click={pickOutput} disabled={exporting}>Browse</button>
    </div>

    <div class="export-row">
      {#if exporting}
        <div class="progress">
          <div class="progress-track">
            <div class="progress-fill" style="width: {Math.round(fraction * 100)}%"></div>
          </div>
          <span class="progress-text muted">Processing {Math.round(fraction * 100)}%</span>
        </div>
        <button class="btn btn-ghost danger" on:click={cancel}>Cancel</button>
      {:else}
        <button class="btn btn-primary" on:click={run} disabled={!canRun}>
          {OPS.find((o) => o.id === op)?.label}
        </button>
        {#if phase === 'done'}
          <span class="done-text">Saved {fileName(doneMessage ?? outputPath)}</span>
          <button class="btn" on:click={revealOutput}>Open folder</button>
        {/if}
      {/if}
    </div>

    {#if phase === 'error' && error}
      <div class="error"><code>{error}</code></div>
    {/if}
  </div>
{:else if phase === 'error' && error}
  <div class="error"><code>{error}</code></div>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--fg-muted);
  }

  .source-row,
  .export-row,
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .source-row .input,
  .progress {
    flex: 1;
  }

  .meta {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
  }

  .dot {
    opacity: 0.6;
  }

  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 999px;
  }

  .badge.has {
    color: var(--success, #3fb950);
    background: color-mix(in srgb, var(--success, #3fb950) 16%, transparent);
  }

  .badge.none {
    color: var(--fg-muted);
    background: var(--surface-3);
  }

  .modes {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 3px;
    border-radius: 10px;
    background: var(--surface-3);
    align-self: flex-start;
  }

  .mode-tab {
    border: none;
    background: transparent;
    color: var(--fg-muted);
    font-size: 13px;
    font-weight: 600;
    padding: 6px 14px;
    border-radius: 7px;
    cursor: pointer;
    transition: background 150ms ease, color 150ms ease;
  }

  .mode-tab:hover:not(:disabled) {
    color: var(--fg);
  }

  .mode-tab.active {
    background: var(--surface-1);
    color: var(--fg);
  }

  .mode-tab:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
  }

  .vol-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .slider {
    flex: 1;
    accent-color: var(--accent);
  }

  .gain-value {
    min-width: 64px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .waveform {
    display: block;
    width: 100%;
    height: 96px;
    border-radius: 8px;
    background: var(--surface-3);
    object-fit: cover;
  }

  .waveform.placeholder {
    animation: shimmer 1.4s ease-in-out infinite;
  }

  @keyframes shimmer {
    0%,
    100% {
      opacity: 0.5;
    }
    50% {
      opacity: 1;
    }
  }

  .hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
  }

  .hint.warn {
    color: var(--danger);
  }

  .mono {
    font-family: 'Consolas', 'Menlo', monospace;
    font-size: 12.5px;
  }

  .progress-track {
    flex: 1;
    height: 8px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
    transition: width 200ms ease;
  }

  .progress-text {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .done-text {
    font-size: 12.5px;
    color: var(--success, #3fb950);
  }

  .error {
    padding: 10px 14px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    color: var(--danger);
    font-size: 12.5px;
    word-break: break-all;
  }

  .danger {
    color: var(--danger);
  }
</style>
