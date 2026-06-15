<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { ipc, type MediaInfo, type GifDither, type GifAppendPosition } from '$lib/ipc';

  /** Extensions the Edit tab will load as a source clip (video or GIF). */
  const MEDIA_EXTS = ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'ts', 'gif'];
  const isMediaPath = (p: string) => MEDIA_EXTS.includes(p.split('.').pop()?.toLowerCase() ?? '');
  const isGifPath = (p: string) => p.split('.').pop()?.toLowerCase() === 'gif';

  type LoopMode = 'forever' | 'once' | 'custom';

  type Phase = 'idle' | 'probing' | 'ready' | 'palette' | 'encoding' | 'done' | 'error';

  let phase: Phase = 'idle';
  let source: MediaInfo | null = null;
  let error: string | null = null;

  let startText = '0:00';
  let endText = '';
  let width = 480;
  let fps = 15;
  let dither: GifDither = 'sierra2_4a';
  let loopMode: LoopMode = 'forever';
  let loopTimes = 3;
  let outputPath = '';

  // Append-a-clip (#4): tack a second clip's range onto the source.
  let appendEnabled = false;
  let appendSource: MediaInfo | null = null;
  let appendStartText = '0:00';
  let appendEndText = '';
  let appendPosition: GifAppendPosition = 'back';

  let jobId: string | null = null;
  let fraction = 0;

  const DITHER_OPTIONS: { id: GifDither; label: string }[] = [
    { id: 'sierra2_4a', label: 'Sierra (default)' },
    { id: 'floyd_steinberg', label: 'Floyd–Steinberg' },
    { id: 'bayer', label: 'Bayer (patterned)' },
    { id: 'none', label: 'None (smallest file)' },
  ];

  let unlisteners: UnlistenFn[] = [];

  onMount(async () => {
    unlisteners = await Promise.all([
      listen<{ id: string; status: string; message: string | null }>(
        'edit://status',
        ({ payload }) => {
          if (payload.id !== jobId) return;
          if (payload.status === 'palette') phase = 'palette';
          else if (payload.status === 'encoding') phase = 'encoding';
          else if (payload.status === 'done') {
            phase = 'done';
            fraction = 1;
          } else if (payload.status === 'error') {
            phase = 'error';
            error = payload.message ?? 'export failed';
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
      // Drop a video or GIF onto the Edit tab to load it as the source clip.
      // The global URL drop handler ignores media files, so this is the only
      // consumer. Only mounted while the Edit view is active.
      getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type !== 'drop' || exporting) return;
        const media = event.payload.paths.find(isMediaPath);
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
        {
          name: 'Video',
          extensions: ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'ts', 'gif'],
        },
        { name: 'All files', extensions: ['*'] },
      ],
    });
    if (typeof picked !== 'string' || !picked) return;
    await loadSource(picked);
  }

  /** Probe a path and load it as the source clip (from Browse or a drop). */
  async function loadSource(path: string) {
    phase = 'probing';
    error = null;
    source = null;
    try {
      source = await ipc.probeMedia(path);
      startText = '0:00';
      endText = source.duration != null ? formatTime(source.duration) : '';
      if (source.width != null) width = Math.min(480, source.width);
      if (source.fps != null) fps = Math.min(15, Math.round(source.fps));
      outputPath = defaultOutput(path);
      phase = 'ready';
    } catch (err) {
      phase = 'error';
      error = String(err);
    }
  }

  /** Default to `<name>.gif`; if the source is already a GIF that would
   *  overwrite it, fall back to `<name>-edited.gif`. */
  function defaultOutput(input: string): string {
    const out = input.replace(/\.[^\\/.]+$/, '') + '.gif';
    return out.toLowerCase() === input.toLowerCase()
      ? input.replace(/\.[^\\/.]+$/, '') + '-edited.gif'
      : out;
  }

  async function pickAppendClip() {
    const picked = await openDialog({
      multiple: false,
      filters: [
        { name: 'Video / GIF', extensions: MEDIA_EXTS },
        { name: 'All files', extensions: ['*'] },
      ],
    });
    if (typeof picked !== 'string' || !picked) return;
    try {
      appendSource = await ipc.probeMedia(picked);
      appendStartText = '0:00';
      appendEndText = appendSource.duration != null ? formatTime(appendSource.duration) : '';
    } catch (err) {
      error = String(err);
    }
  }

  async function pickOutput() {
    const picked = await saveDialog({
      defaultPath: outputPath || undefined,
      filters: [{ name: 'GIF', extensions: ['gif'] }],
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

  $: start = parseTime(startText);
  $: end = parseTime(endText);
  $: rangeError = validateRange(start, end);
  $: exporting = phase === 'palette' || phase === 'encoding';
  $: isGifSource = source != null && isGifPath(source.path);
  $: loopCount = loopMode === 'forever' ? 0 : loopMode === 'once' ? -1 : Math.max(1, Math.round(loopTimes));

  $: appendStart = parseTime(appendStartText);
  $: appendEnd = parseTime(appendEndText);
  $: appendRangeError = validateAppendRange(appendStart, appendEnd);
  $: appendActive = appendEnabled && appendSource != null;

  $: canExport =
    source != null &&
    !exporting &&
    !!outputPath &&
    rangeError == null &&
    fps >= 1 &&
    fps <= 60 &&
    (!appendEnabled || appendSource != null) &&
    (!appendActive || appendRangeError == null);

  function validateAppendRange(start: number | null, end: number | null): string | null {
    if (!appendEnabled || appendSource == null) return null;
    if (appendStartText.trim() && start == null) return 'Invalid clip start time';
    if (appendEndText.trim() && end == null) return 'Invalid clip end time';
    if (start != null && end != null && end <= start) return 'Clip end must be after start';
    const duration = appendSource.duration;
    if (duration != null) {
      if (start != null && start >= duration) return 'Clip start is past the end';
      if (end != null && end > duration + 1) return 'Clip end is past the end';
    }
    return null;
  }

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

  async function startExport() {
    if (!source || !canExport) return;
    error = null;
    fraction = 0;
    try {
      if (appendActive && appendSource) {
        jobId = await ipc.appendToGif({
          basePath: source.path,
          clipPath: appendSource.path,
          clipStart: appendStart && appendStart > 0 ? appendStart : null,
          clipEnd: appendEnd ?? null,
          position: appendPosition,
          outputPath,
          width: width > 0 ? width : null,
          fps,
          dither,
          loopCount,
        });
      } else {
        jobId = await ipc.exportGif({
          inputPath: source.path,
          outputPath,
          start: start && start > 0 ? start : null,
          end: end ?? null,
          width: width > 0 ? width : null,
          fps,
          dither,
          loopCount,
        });
      }
      phase = 'palette';
    } catch (err) {
      phase = 'error';
      error = String(err);
    }
  }

  async function cancelExport() {
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

<section class="view">
  <header class="title">
    <h2>Edit</h2>
    <p class="muted">
      Turn a video into a GIF, re-edit an existing GIF, or append a clip onto one. Drop a video or
      GIF anywhere on this tab to load it.
    </p>
  </header>

  <div class="card">
    <div class="label">Source</div>
    <div class="source-row">
      <input
        class="input mono"
        type="text"
        readonly
        placeholder="No file selected — browse or drop a video / GIF"
        value={source?.path ?? ''}
      />
      <button class="btn" on:click={pickSource} disabled={exporting || phase === 'probing'}>
        {phase === 'probing' ? 'Probing…' : 'Browse'}
      </button>
    </div>
    {#if source}
      <div class="meta muted">
        {#if isGifSource}
          <span class="badge">GIF</span>
        {/if}
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
      <div class="label">GIF settings</div>
      <div class="grid">
        <label class="field">
          <span class="field-label">Start</span>
          <input class="input" type="text" bind:value={startText} placeholder="0:00" disabled={exporting} />
        </label>
        <label class="field">
          <span class="field-label">End</span>
          <input class="input" type="text" bind:value={endText} placeholder="end" disabled={exporting} />
        </label>
        <label class="field">
          <span class="field-label">Width (px)</span>
          <input class="input" type="number" min="16" max="3840" bind:value={width} disabled={exporting} />
        </label>
        <label class="field">
          <span class="field-label">FPS</span>
          <input class="input" type="number" min="1" max="60" bind:value={fps} disabled={exporting} />
        </label>
        <label class="field">
          <span class="field-label">Dithering</span>
          <select class="input" bind:value={dither} disabled={exporting}>
            {#each DITHER_OPTIONS as opt (opt.id)}
              <option value={opt.id}>{opt.label}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="field-label">Loop</span>
          <select class="input" bind:value={loopMode} disabled={exporting}>
            <option value="forever">Forever</option>
            <option value="once">Play once</option>
            <option value="custom">Custom…</option>
          </select>
        </label>
        {#if loopMode === 'custom'}
          <label class="field">
            <span class="field-label">Loop count</span>
            <input class="input" type="number" min="1" max="1000" bind:value={loopTimes} disabled={exporting} />
          </label>
        {/if}
      </div>
      {#if rangeError}
        <div class="error"><code>{rangeError}</code></div>
      {/if}
    </div>

    <div class="card">
      <div class="label">Append a clip</div>
      <label class="check">
        <input type="checkbox" bind:checked={appendEnabled} disabled={exporting} />
        <span>Tack another clip onto this {isGifSource ? 'GIF' : 'video'}</span>
      </label>
      {#if appendEnabled}
        <div class="source-row">
          <input
            class="input mono"
            type="text"
            readonly
            placeholder="No clip selected"
            value={appendSource?.path ?? ''}
          />
          <button class="btn" on:click={pickAppendClip} disabled={exporting}>Browse</button>
        </div>
        {#if appendSource}
          <div class="grid">
            <label class="field">
              <span class="field-label">Clip start</span>
              <input class="input" type="text" bind:value={appendStartText} placeholder="0:00" disabled={exporting} />
            </label>
            <label class="field">
              <span class="field-label">Clip end</span>
              <input class="input" type="text" bind:value={appendEndText} placeholder="end" disabled={exporting} />
            </label>
            <label class="field">
              <span class="field-label">Position</span>
              <select class="input" bind:value={appendPosition} disabled={exporting}>
                <option value="back">After (end)</option>
                <option value="front">Before (start)</option>
              </select>
            </label>
          </div>
          <p class="hint muted">
            The appended clip is scaled to {width}px and shares one color palette with the base, so
            colors stay consistent across the join.
          </p>
          {#if appendRangeError}
            <div class="error"><code>{appendRangeError}</code></div>
          {/if}
        {/if}
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
              <div
                class="progress-fill"
                class:indeterminate={phase === 'palette'}
                style="width: {phase === 'palette' ? 100 : Math.round(fraction * 100)}%"
              ></div>
            </div>
            <span class="progress-text muted">
              {phase === 'palette'
                ? 'Building palette…'
                : `Encoding ${Math.round(fraction * 100)}%`}
            </span>
          </div>
          <button class="btn btn-ghost danger" on:click={cancelExport}>Cancel</button>
        {:else}
          <button class="btn btn-primary" on:click={startExport} disabled={!canExport}>
            Export GIF
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
  }

  .title p {
    margin: 0;
  }

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

  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 999px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
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

  .progress-fill.indeterminate {
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.4;
    }
    50% {
      opacity: 1;
    }
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
