<script lang="ts">
  import { ytdlpStore, runInstall, resetError } from '$lib/stores/ytdlp';

  $: state = $ytdlpStore;

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function percent(downloaded: number, total: number | null): number | null {
    if (!total || total <= 0) return null;
    return Math.min(100, (downloaded / total) * 100);
  }

  $: ytdlpPct = percent(state.progress.ytdlp.downloaded, state.progress.ytdlp.total);
  $: ffmpegPct = percent(state.progress.ffmpeg.downloaded, state.progress.ffmpeg.total);
</script>

<div class="wizard">
  <div class="card">
    <div class="logo" aria-hidden="true">
      <svg viewBox="0 0 64 64" width="56" height="56" class="logo-glyph">
        <defs>
          <linearGradient id="wg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="var(--accent)" />
            <stop offset="1" stop-color="var(--accent-strong)" />
          </linearGradient>
        </defs>
        <rect x="4" y="4" width="56" height="56" rx="14" fill="url(#wg)" />
        <path
          d="M20 24 L32 44 L44 24"
          stroke="var(--accent-fg)"
          stroke-width="5"
          stroke-linecap="round"
          stroke-linejoin="round"
          fill="none"
        />
      </svg>
    </div>

    <h1>Welcome to VidFetch</h1>

    {#if state.boot === 'needsInstall'}
      <p class="muted">
        We need to fetch two small helper programs before you can start downloading videos:
      </p>
      <ul class="reqs">
        <li>
          <span class="name">yt-dlp</span>
          <span class="muted">— the core downloader</span>
          {#if state.status?.ytdlp}
            <span class="tag tag-ok">installed</span>
          {:else}
            <span class="tag">~4 MB</span>
          {/if}
        </li>
        <li>
          <span class="name">ffmpeg</span>
          <span class="muted">— merges video + audio, converts formats</span>
          {#if state.status?.ffmpeg && state.status?.ffprobe}
            <span class="tag tag-ok">installed</span>
          {:else}
            <span class="tag">~100 MB</span>
          {/if}
        </li>
      </ul>
      <div class="actions">
        <button class="btn btn-primary" on:click={runInstall}>Install now</button>
      </div>
      <p class="fine muted">
        Both are downloaded from their official sources and saved inside VidFetch's app data folder.
        You can update or reinstall them any time from Settings.
      </p>
    {:else if state.boot === 'installing'}
      <p class="muted">Setting things up…</p>

      <section class="target">
        <div class="target-head">
          <span class="name">yt-dlp</span>
          <span class="phase muted">
            {#if state.progress.ytdlp.phase === 'done' || state.status?.ytdlp}
              ✓ installed
            {:else if state.progress.ytdlp.phase === 'fetching'}
              fetching release info…
            {:else if state.progress.ytdlp.phase === 'downloading'}
              {formatBytes(state.progress.ytdlp.downloaded)}{state.progress.ytdlp.total
                ? ` / ${formatBytes(state.progress.ytdlp.total)}`
                : ''}
            {:else}
              queued
            {/if}
          </span>
        </div>
        <div class="bar" class:indeterminate={ytdlpPct === null && state.progress.ytdlp.phase !== 'idle' && state.progress.ytdlp.phase !== 'done'}>
          <div class="fill" style:width={ytdlpPct !== null ? `${ytdlpPct}%` : undefined} class:done={state.progress.ytdlp.phase === 'done' || state.status?.ytdlp}></div>
        </div>
      </section>

      <section class="target">
        <div class="target-head">
          <span class="name">ffmpeg</span>
          <span class="phase muted">
            {#if state.progress.ffmpeg.phase === 'done' || (state.status?.ffmpeg && state.status?.ffprobe)}
              ✓ installed
            {:else if state.progress.ffmpeg.phase === 'fetching'}
              fetching release info…
            {:else if state.progress.ffmpeg.phase === 'downloading'}
              {formatBytes(state.progress.ffmpeg.downloaded)}{state.progress.ffmpeg.total
                ? ` / ${formatBytes(state.progress.ffmpeg.total)}`
                : ''}
            {:else if state.progress.ffmpeg.phase === 'extracting'}
              extracting…
            {:else}
              queued
            {/if}
          </span>
        </div>
        <div class="bar" class:indeterminate={ffmpegPct === null && state.progress.ffmpeg.phase !== 'idle' && state.progress.ffmpeg.phase !== 'done'}>
          <div class="fill" style:width={ffmpegPct !== null ? `${ffmpegPct}%` : undefined} class:done={state.progress.ffmpeg.phase === 'done' || (state.status?.ffmpeg && state.status?.ffprobe)}></div>
        </div>
      </section>
    {:else if state.boot === 'error'}
      <h2 class="danger">Something went wrong</h2>
      <pre class="error-box">{state.error}</pre>
      <div class="actions">
        <button class="btn" on:click={resetError}>Back</button>
        <button class="btn btn-primary" on:click={runInstall}>Retry</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .wizard {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    width: 100vw;
    padding: 24px;
    background:
      radial-gradient(1200px 600px at 20% -10%, var(--accent-muted), transparent 60%),
      radial-gradient(900px 500px at 100% 110%, var(--accent-muted), transparent 60%),
      var(--bg);
  }

  .card {
    max-width: 520px;
    width: 100%;
    padding: 36px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 18px;
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .logo {
    display: flex;
    justify-content: flex-start;
  }

  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.3px;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 650;
  }

  .danger {
    color: var(--danger);
  }

  p {
    margin: 0;
  }

  .reqs {
    list-style: none;
    padding: 0;
    margin: 4px 0 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .reqs li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 12px 14px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
  }

  .name {
    font-weight: 600;
  }

  .tag {
    margin-left: auto;
    padding: 2px 10px;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--fg-muted);
    font-size: 11.5px;
    font-weight: 600;
  }

  .tag-ok {
    background: var(--accent-muted);
    color: var(--accent);
  }

  .actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 8px;
  }

  .fine {
    font-size: 12px;
    line-height: 1.5;
  }

  .target {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .target-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .phase {
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }

  .bar {
    position: relative;
    height: 8px;
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

  .error-box {
    max-height: 180px;
    overflow: auto;
    padding: 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    font-family: 'Consolas', 'Menlo', monospace;
    font-size: 12px;
    white-space: pre-wrap;
    color: var(--danger);
  }
</style>
