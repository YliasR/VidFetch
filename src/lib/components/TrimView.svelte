<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { ipc, type MediaInfo } from '$lib/ipc';

  /** Video containers the Trim view will load. GIFs go through the GIF tab. */
  const VIDEO_EXTS = ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'ts'];
  const isVideoPath = (p: string) => VIDEO_EXTS.includes(p.split('.').pop()?.toLowerCase() ?? '');

  /** A copy cut is only accurate within this many seconds of a keyframe. */
  const KEYFRAME_EPSILON = 0.05;

  type Phase = 'idle' | 'probing' | 'ready' | 'encoding' | 'done' | 'error';

  let phase: Phase = 'idle';
  let source: MediaInfo | null = null;
  let error: string | null = null;

  let keyframes: number[] = [];
  let keyframesLoading = false;

  let startText = '0:00';
  let endText = '';
  let forceReencode = false;
  let outputPath = '';

  let jobId: string | null = null;
  let fraction = 0;

  let unlisteners: UnlistenFn[] = [];

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
          } else if (payload.status === 'error') {
            phase = 'error';
            error = payload.message ?? 'trim failed';
          } else if (payload.status === 'canceled') {
            phase = 'ready';
            fraction = 0;
          }
        },
      ),
      listen<{ id: string; fraction: number }>('edit://progress', ({ payload }) => {
        if (payload.id !== jobId) return;
        fraction = payload.fraction;
      }),
      getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type !== 'drop' || trimming) return;
        const video = event.payload.paths.find(isVideoPath);
        if (video) void loadSource(video);
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
    keyframes = [];
    try {
      source = await ipc.probeMedia(path);
      startText = '0:00';
      endText = source.duration != null ? formatTime(source.duration) : '';
      forceReencode = false;
      outputPath = defaultOutput(path);
      phase = 'ready';
      void loadKeyframes(path);
    } catch (err) {
      phase = 'error';
      error = String(err);
    }
  }

  /** Keyframes load in the background — the badge stays neutral until they
   *  arrive, then resolves to lossless / re-encoded. */
  async function loadKeyframes(path: string) {
    keyframesLoading = true;
    try {
      keyframes = await ipc.listKeyframes(path);
    } catch {
      keyframes = [];
    } finally {
      keyframesLoading = false;
    }
  }

  /** Insert `-trim` before the extension so we don't clobber the source. */
  function defaultOutput(input: string): string {
    return input.replace(/(\.[^\\/.]+)$/, '-trim$1');
  }

  async function pickOutput() {
    const ext = outputPath.split('.').pop()?.toLowerCase() || 'mp4';
    const picked = await saveDialog({
      defaultPath: outputPath || undefined,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (typeof picked === 'string' && picked) outputPath = picked;
  }

  /** Accepts plain seconds ("90", "12.5") or clock time ("1:30", "1:02:05.5"). */
  function parseTime(text: string): number | null {
    const clean = text.trim();
    if (!clean) return null;
    const parts = clean.split(':');
    if (parts.some((p) => p === '' || Number.isNaN(Number(p)))) return null;
    return parts.reduce((acc, p) => acc * 60 + Number(p), 0);
  }

  function formatTime(secs: number): string {
    const total = Math.max(0, secs);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const sText = (Math.round(s * 10) / 10).toFixed(s % 1 === 0 ? 0 : 1).padStart(2, '0');
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${sText}` : `${m}:${sText}`;
  }

  /** Largest keyframe at or before `t`, or null if none/unknown. */
  function keyframeAtOrBefore(t: number): number | null {
    let best: number | null = null;
    for (const k of keyframes) {
      if (k <= t + KEYFRAME_EPSILON) best = k;
      else break;
    }
    return best;
  }

  function isOnKeyframe(t: number): boolean {
    if (t <= KEYFRAME_EPSILON) return true; // start of file is always a keyframe
    return keyframes.some((k) => Math.abs(k - t) <= KEYFRAME_EPSILON);
  }

  function snapStart() {
    const t = start ?? 0;
    const k = keyframeAtOrBefore(t);
    if (k != null) startText = formatTime(k);
  }

  $: start = parseTime(startText);
  $: end = parseTime(endText);
  $: rangeError = validateRange(start, end);
  $: trimming = phase === 'encoding';
  $: startOnKeyframe = keyframes.length > 0 && isOnKeyframe(start ?? 0);
  // Lossless when copying is safe (start on a keyframe) and the user hasn't
  // forced a re-encode. Until keyframes load we can't promise lossless.
  $: willBeLossless = !forceReencode && startOnKeyframe;
  $: reencode = !willBeLossless;

  $: canTrim =
    source != null &&
    !trimming &&
    !!outputPath &&
    rangeError == null;

  function validateRange(start: number | null, end: number | null): string | null {
    if (startText.trim() && start == null) return 'Invalid start time';
    if (endText.trim() && end == null) return 'Invalid end time';
    if (start != null && end != null && end <= start) return 'End must be after start';
    const duration = source?.duration;
    if (duration != null) {
      if (start != null && start >= duration) return 'Start is past the end of the video';
      if (end != null && end > duration + 1) return 'End is past the end of the video';
    }
    return null;
  }

  async function startTrim() {
    if (!source || !canTrim) return;
    error = null;
    fraction = 0;
    try {
      jobId = await ipc.trimVideo({
        inputPath: source.path,
        outputPath,
        start: start && start > 0 ? start : null,
        end: end ?? null,
        reencode,
      });
      phase = 'encoding';
    } catch (err) {
      phase = 'error';
      error = String(err);
    }
  }

  async function cancelTrim() {
    if (jobId) await ipc.cancelExport(jobId);
  }

  async function revealOutput() {
    const dir = outputPath.replace(/[\\/][^\\/]+$/, '');
    if (dir) await openPath(dir);
  }

  function fileName(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
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
    <button class="btn" on:click={pickSource} disabled={trimming || phase === 'probing'}>
      {phase === 'probing' ? 'Probing…' : 'Browse'}
    </button>
  </div>
  {#if source}
    <div class="meta muted">
      {#if source.width != null && source.height != null}
        <span>{source.width}×{source.height}</span>
      {/if}
      {#if source.fps != null}
        <span class="dot">·</span>
        <span>{Math.round(source.fps)} fps</span>
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
    <div class="label">Trim range</div>
    <div class="grid">
      <label class="field">
        <span class="field-label">Start</span>
        <input class="input" type="text" bind:value={startText} placeholder="0:00" disabled={trimming} />
      </label>
      <label class="field">
        <span class="field-label">End</span>
        <input class="input" type="text" bind:value={endText} placeholder="end" disabled={trimming} />
      </label>
    </div>
    {#if rangeError}
      <div class="error"><code>{rangeError}</code></div>
    {/if}

    <div class="mode-row">
      {#if keyframesLoading}
        <span class="badge neutral">Analyzing keyframes…</span>
      {:else if willBeLossless}
        <span class="badge lossless">Lossless · stream copy</span>
      {:else}
        <span class="badge reencode">Re-encoded</span>
      {/if}

      {#if !keyframesLoading && keyframes.length > 0 && !startOnKeyframe && !forceReencode}
        <button class="btn btn-sm" on:click={snapStart} disabled={trimming}>
          Snap start to keyframe
        </button>
        <span class="hint muted">Start isn't on a keyframe, so the cut is re-encoded.</span>
      {/if}
    </div>

    <label class="check">
      <input type="checkbox" bind:checked={forceReencode} disabled={trimming} />
      <span>Always re-encode (frame-accurate, slower)</span>
    </label>
  </div>

  <div class="card">
    <div class="label">Output</div>
    <div class="source-row">
      <input class="input mono" type="text" bind:value={outputPath} disabled={trimming} />
      <button class="btn" on:click={pickOutput} disabled={trimming}>Browse</button>
    </div>

    <div class="export-row">
      {#if trimming}
        <div class="progress">
          <div class="progress-track">
            <div class="progress-fill" style="width: {Math.round(fraction * 100)}%"></div>
          </div>
          <span class="progress-text muted">Trimming {Math.round(fraction * 100)}%</span>
        </div>
        <button class="btn btn-ghost danger" on:click={cancelTrim}>Cancel</button>
      {:else}
        <button class="btn btn-primary" on:click={startTrim} disabled={!canTrim}>
          {willBeLossless ? 'Trim (lossless)' : 'Trim'}
        </button>
        {#if phase === 'done'}
          <span class="done-text">Saved {fileName(outputPath)}</span>
          <button class="btn" on:click={revealOutput}>Open folder</button>
        {/if}
      {/if}
    </div>

    {#if phase === 'error' && error}
      <div class="error"><code>{error}</code></div>
    {/if}
  </div>
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

  .source-row {
    display: flex;
    gap: 8px;
  }

  .source-row .input {
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

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
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

  .mode-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: 999px;
  }

  .badge.lossless {
    color: var(--success, #3fb950);
    background: color-mix(in srgb, var(--success, #3fb950) 16%, transparent);
  }

  .badge.reencode {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .badge.neutral {
    color: var(--fg-muted);
    background: var(--surface-3);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    cursor: pointer;
  }

  .check input {
    accent-color: var(--accent);
  }

  .hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
  }

  .mono {
    font-family: 'Consolas', 'Menlo', monospace;
    font-size: 12.5px;
  }

  .export-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 2px;
  }

  .progress {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
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
