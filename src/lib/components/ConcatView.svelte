<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { ipc, type MediaInfo, type ConcatPlan } from '$lib/ipc';

  const VIDEO_EXTS = ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'ts', 'gif'];
  const isVideoPath = (p: string) => VIDEO_EXTS.includes(p.split('.').pop()?.toLowerCase() ?? '');

  type Phase = 'idle' | 'probing' | 'ready' | 'encoding' | 'done' | 'error';
  type Clip = MediaInfo & { thumb: string | null };

  let phase: Phase = 'idle';
  let clips: Clip[] = [];
  let error: string | null = null;
  let outputPath = '';
  let jobId: string | null = null;
  let fraction = 0;
  let doneMessage: string | null = null;
  let plan: ConcatPlan | null = null;
  let unlisteners: UnlistenFn[] = [];

  $: exporting = phase === 'encoding';
  $: totalDuration = clips.reduce((sum, clip) => sum + (clip.duration ?? 0), 0);
  $: canConcat = clips.length >= 2 && !!outputPath && !exporting && phase !== 'probing';

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
            error = payload.message ?? 'concat failed';
          } else if (payload.status === 'canceled') {
            phase = clips.length > 0 ? 'ready' : 'idle';
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
        const paths = event.payload.paths.filter(isVideoPath);
        if (paths.length > 0) void addClips(paths);
      }),
    ]);
  });

  onDestroy(() => {
    for (const unlisten of unlisteners) unlisten();
  });

  async function pickClips() {
    const picked = await openDialog({
      multiple: true,
      filters: [
        { name: 'Video / GIF', extensions: VIDEO_EXTS },
        { name: 'All files', extensions: ['*'] },
      ],
    });
    const paths = Array.isArray(picked) ? picked.filter(Boolean) : [];
    if (paths.length > 0) await addClips(paths);
  }

  async function addClips(paths: string[]) {
    phase = 'probing';
    error = null;
    doneMessage = null;
    try {
      const existing = new Set(clips.map((clip) => clip.path));
      const fresh = paths.filter((path) => !existing.has(path));
      const loaded: Clip[] = [];
      for (const path of fresh) {
        const info = await ipc.probeMedia(path);
        loaded.push({ ...info, thumb: null });
      }
      clips = [...clips, ...loaded];
      if (!outputPath && clips.length > 0) outputPath = defaultOutput(clips[0].path);
      phase = clips.length > 0 ? 'ready' : 'idle';
      for (const clip of loaded) void loadThumb(clip);
      void updatePlan();
    } catch (err) {
      phase = clips.length > 0 ? 'ready' : 'error';
      error = String(err);
    }
  }

  async function loadThumb(clip: Clip) {
    try {
      const at = clip.duration != null ? Math.min(clip.duration * 0.2, 3) : 0;
      const thumb = await ipc.thumbnailAt(clip.path, at, 220);
      clips = clips.map((item) => (item.path === clip.path ? { ...item, thumb } : item));
    } catch {
      // Preview remains as a placeholder.
    }
  }

  function defaultOutput(input: string): string {
    return input.replace(/(\.[^\\/.]+)$/, '-joined$1');
  }

  async function pickOutput() {
    const ext = outputPath.split('.').pop()?.toLowerCase() || 'mp4';
    const picked = await saveDialog({
      defaultPath: outputPath || undefined,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (typeof picked === 'string' && picked) outputPath = picked;
  }

  function moveClip(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= clips.length || exporting) return;
    const next = [...clips];
    [next[index], next[target]] = [next[target], next[index]];
    clips = next;
  }

  function removeClip(index: number) {
    if (exporting) return;
    clips = clips.filter((_, i) => i !== index);
    if (clips.length === 0) phase = 'idle';
    void updatePlan();
  }

  /** Ask the backend whether the current set joins by stream-copy or needs a
   * normalize re-encode, so the UI can show the right badge before starting.
   * Order doesn't affect the decision, so we only recompute when the set
   * changes. */
  async function updatePlan() {
    if (clips.length < 2) {
      plan = null;
      return;
    }
    try {
      plan = await ipc.planConcat(clips.map((clip) => clip.path));
    } catch {
      plan = null;
    }
  }

  async function startConcat() {
    if (!canConcat) return;
    error = null;
    fraction = 0;
    doneMessage = null;
    try {
      jobId = await ipc.concatClips({
        inputPaths: clips.map((clip) => clip.path),
        outputPath,
      });
      phase = 'encoding';
    } catch (err) {
      phase = 'error';
      error = String(err);
    }
  }

  async function cancelConcat() {
    if (jobId) await ipc.cancelExport(jobId);
  }

  async function revealOutput() {
    const dir = outputPath.replace(/[\\/][^\\/]+$/, '');
    if (dir) await openPath(dir);
  }

  function clearClips() {
    if (exporting) return;
    clips = [];
    phase = 'idle';
    error = null;
    doneMessage = null;
    plan = null;
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
  <div class="label">Clips</div>
  <div class="drop-row">
    <button class="btn" on:click={pickClips} disabled={exporting || phase === 'probing'}>
      {phase === 'probing' ? 'Probing...' : 'Add clips'}
    </button>
    {#if clips.length > 0}
      <button class="btn btn-ghost" on:click={clearClips} disabled={exporting}>Clear</button>
      <span class="hint muted">
        {clips.length} clips
        {#if totalDuration > 0}
          - {formatTime(totalDuration)}
        {/if}
      </span>
    {:else}
      <span class="hint muted">Drop two or more local clips here, then arrange their order.</span>
    {/if}
  </div>
</div>

{#if clips.length > 0}
  <div class="card">
    <div class="label">Sequence preview</div>
    <div class="sequence">
      {#each clips as clip, i (clip.path)}
        <div class="tile">
          {#if clip.thumb}
            <img src={clip.thumb} alt={`Preview of ${fileName(clip.path)}`} />
          {:else}
            <div class="thumb-placeholder"></div>
          {/if}
          <span class="tile-index">{i + 1}</span>
          <span class="tile-name">{fileName(clip.path)}</span>
        </div>
      {/each}
    </div>
    {#if plan?.mode === 'copy'}
      <span class="badge copy">Fast join — identical clips, stream copy (lossless)</span>
    {:else if plan?.mode === 'reencode'}
      <span class="badge reencode">
        Re-encode — {plan.reason ?? 'mixed sources'}; clips are normalized to a common format
      </span>
    {:else}
      <span class="hint muted">Add at least two clips to plan the join.</span>
    {/if}
  </div>

  <div class="card">
    <div class="label">Order</div>
    <div class="clip-list">
      {#each clips as clip, i (clip.path)}
        <div class="clip-row">
          <span class="order">{i + 1}</span>
          <div class="clip-main">
            <strong>{fileName(clip.path)}</strong>
            <span class="muted mono">{clip.path}</span>
          </div>
          <div class="clip-meta muted">
            {#if clip.width != null && clip.height != null}
              <span>{clip.width}x{clip.height}</span>
            {/if}
            {#if clip.fps != null}
              <span>{Math.round(clip.fps)} fps</span>
            {/if}
            {#if clip.duration != null}
              <span>{formatTime(clip.duration)}</span>
            {/if}
          </div>
          <div class="row-actions">
            <button class="icon-btn" title="Move up" on:click={() => moveClip(i, -1)} disabled={exporting || i === 0}>
              ^
            </button>
            <button
              class="icon-btn"
              title="Move down"
              on:click={() => moveClip(i, 1)}
              disabled={exporting || i === clips.length - 1}
            >
              v
            </button>
            <button class="icon-btn danger" title="Remove" on:click={() => removeClip(i)} disabled={exporting}>
              x
            </button>
          </div>
        </div>
      {/each}
    </div>
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
          <span class="progress-text muted">Joining {Math.round(fraction * 100)}%</span>
        </div>
        <button class="btn btn-ghost danger" on:click={cancelConcat}>Cancel</button>
      {:else}
        <button class="btn btn-primary" on:click={startConcat} disabled={!canConcat}>
          Join clips
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

  .drop-row,
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

  .hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
  }

  .badge {
    align-self: flex-start;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    line-height: 1.4;
  }

  .badge.copy {
    color: var(--success, #3fb950);
    background: color-mix(in srgb, var(--success, #3fb950) 14%, transparent);
  }

  .badge.reencode {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .sequence {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .tile {
    position: relative;
    flex: 0 0 132px;
    height: 82px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--surface-3);
  }

  .tile img,
  .thumb-placeholder {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .thumb-placeholder {
    background: linear-gradient(
      90deg,
      var(--surface-3) 0%,
      var(--surface-2) 50%,
      var(--surface-3) 100%
    );
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

  .tile-index,
  .tile-name {
    position: absolute;
    color: #fff;
    background: rgba(0, 0, 0, 0.64);
  }

  .tile-index {
    top: 5px;
    left: 5px;
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 800;
  }

  .tile-name {
    left: 5px;
    right: 5px;
    bottom: 5px;
    padding: 2px 5px;
    border-radius: 4px;
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .clip-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .clip-row {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border: 1px solid var(--surface-3);
    border-radius: 8px;
    background: var(--surface-1);
  }

  .order {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 800;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .clip-main {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .clip-main strong,
  .clip-main span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .clip-main strong {
    font-size: 13px;
    font-weight: 650;
  }

  .clip-meta {
    display: flex;
    gap: 8px;
    font-size: 12px;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    gap: 4px;
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border: none;
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--fg);
    cursor: pointer;
  }

  .icon-btn:disabled {
    opacity: 0.45;
    cursor: default;
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

  @media (max-width: 760px) {
    .clip-row {
      grid-template-columns: 28px minmax(0, 1fr);
    }

    .clip-meta,
    .row-actions {
      grid-column: 2;
    }
  }
</style>
