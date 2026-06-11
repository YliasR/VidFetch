<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { check, type Update } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { ipc, type Versions } from '$lib/ipc';
  import {
    notifPrefs,
    initNotifications,
    setNotificationsEnabled,
  } from '$lib/stores/notifications';

  type UpdateState =
    | { phase: 'idle' }
    | { phase: 'checking' }
    | { phase: 'up-to-date' }
    | { phase: 'available'; update: Update }
    | { phase: 'downloading'; downloaded: number; total: number | null }
    | { phase: 'installing' }
    | { phase: 'done' }
    | { phase: 'error'; message: string };

  let appVersion = '…';
  let updateState: UpdateState = { phase: 'idle' };
  let versions: Versions = { ytdlp: null, ffmpeg: null };
  let ytdlpBusy = false;
  let ytdlpError: string | null = null;

  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = '?';
    }
    await initNotifications();
    await refreshBinaryVersions();
  });

  async function onToggleNotifications(e: Event) {
    const checked = (e.target as HTMLInputElement).checked;
    await setNotificationsEnabled(checked);
  }

  async function refreshBinaryVersions() {
    try {
      versions = await ipc.getVersions();
    } catch (err) {
      console.warn('[settings] getVersions failed', err);
    }
  }

  async function checkForUpdates() {
    updateState = { phase: 'checking' };
    try {
      const update = await check();
      if (!update) {
        updateState = { phase: 'up-to-date' };
      } else {
        updateState = { phase: 'available', update };
      }
    } catch (err) {
      updateState = { phase: 'error', message: String(err) };
    }
  }

  async function installUpdate() {
    if (updateState.phase !== 'available') return;
    const update = updateState.phase === 'available' ? updateState.update : null;
    if (!update) return;

    updateState = { phase: 'downloading', downloaded: 0, total: null };
    try {
      let downloaded = 0;
      let total: number | null = null;
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          total = event.data.contentLength ?? null;
          updateState = { phase: 'downloading', downloaded: 0, total };
        } else if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          updateState = { phase: 'downloading', downloaded, total };
        } else if (event.event === 'Finished') {
          updateState = { phase: 'installing' };
        }
      });
      updateState = { phase: 'done' };
      await relaunch();
    } catch (err) {
      updateState = { phase: 'error', message: String(err) };
    }
  }

  async function updateYtDlp() {
    ytdlpBusy = true;
    ytdlpError = null;
    try {
      await ipc.installYtdlp();
      await refreshBinaryVersions();
    } catch (err) {
      ytdlpError = String(err);
    } finally {
      ytdlpBusy = false;
    }
  }

  async function updateFfmpeg() {
    ytdlpBusy = true;
    ytdlpError = null;
    try {
      await ipc.installFfmpeg();
      await refreshBinaryVersions();
    } catch (err) {
      ytdlpError = String(err);
    } finally {
      ytdlpBusy = false;
    }
  }

  function formatBytes(bytes: number | null): string {
    if (bytes == null || bytes <= 0) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
</script>

<section class="view">
  <header class="title">
    <h2>Settings</h2>
    <p class="muted">Versions, update controls, and app info.</p>
  </header>

  <!-- App updates -->
  <div class="card">
    <div class="row">
      <div class="label-col">
        <div class="label">VidFetch</div>
        <div class="value">{appVersion}</div>
      </div>
      <div class="action-col">
        {#if updateState.phase === 'idle' || updateState.phase === 'up-to-date' || updateState.phase === 'error'}
          <button class="btn" on:click={checkForUpdates}>Check for updates</button>
        {:else if updateState.phase === 'checking'}
          <button class="btn" disabled>Checking…</button>
        {:else if updateState.phase === 'available'}
          <button class="btn btn-primary" on:click={installUpdate}>
            Install v{updateState.update.version}
          </button>
        {:else if updateState.phase === 'downloading'}
          <button class="btn" disabled>
            Downloading {formatBytes(updateState.downloaded)}{updateState.total ? ` / ${formatBytes(updateState.total)}` : ''}
          </button>
        {:else if updateState.phase === 'installing'}
          <button class="btn" disabled>Installing…</button>
        {:else if updateState.phase === 'done'}
          <button class="btn" disabled>Restarting…</button>
        {/if}
      </div>
    </div>

    {#if updateState.phase === 'up-to-date'}
      <p class="muted small status">You're on the latest version.</p>
    {:else if updateState.phase === 'available'}
      <div class="update-notes">
        <div class="update-head">
          <strong>New version available: {updateState.update.version}</strong>
          <span class="muted small">released {updateState.update.date ?? 'recently'}</span>
        </div>
        {#if updateState.update.body}
          <pre class="notes-body">{updateState.update.body}</pre>
        {/if}
      </div>
    {:else if updateState.phase === 'error'}
      <div class="error">
        <strong>Couldn't check for updates.</strong>
        <code>{updateState.message}</code>
      </div>
    {/if}
  </div>

  <!-- yt-dlp -->
  <div class="card">
    <div class="row">
      <div class="label-col">
        <div class="label">yt-dlp</div>
        <div class="value">{versions.ytdlp ?? '(not installed)'}</div>
      </div>
      <div class="action-col">
        <button class="btn" on:click={updateYtDlp} disabled={ytdlpBusy}>
          {ytdlpBusy ? 'Working…' : 'Update now'}
        </button>
      </div>
    </div>
    <p class="muted small">
      yt-dlp updates weekly when sites change their APIs. Running this pulls the
      latest release from GitHub.
    </p>
  </div>

  <!-- ffmpeg -->
  <div class="card">
    <div class="row">
      <div class="label-col">
        <div class="label">ffmpeg</div>
        <div class="value">{versions.ffmpeg ?? '(not installed)'}</div>
      </div>
      <div class="action-col">
        <button class="btn" on:click={updateFfmpeg} disabled={ytdlpBusy}>
          {ytdlpBusy ? 'Working…' : 'Reinstall'}
        </button>
      </div>
    </div>
    <p class="muted small">
      Bundled ffmpeg comes from a trusted static build for your platform. Rarely needs updating.
    </p>
    {#if ytdlpError}
      <div class="error">
        <code>{ytdlpError}</code>
      </div>
    {/if}
  </div>

  <!-- Notifications -->
  <div class="card">
    <div class="row">
      <div class="label-col">
        <div class="label">Notifications</div>
        <div class="value">Desktop alerts on job completion</div>
      </div>
      <div class="action-col">
        <label class="switch">
          <input
            type="checkbox"
            checked={$notifPrefs.enabled}
            on:change={onToggleNotifications}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
    <p class="muted small">
      Sends a system notification when a download finishes or fails. First time you
      enable it, your system may ask for permission.
    </p>
  </div>

  <!-- Links -->
  <div class="card about">
    <div class="label">About</div>
    <div class="links">
      <button class="link" on:click={() => openUrl('https://github.com/YliasR/VidFetch')}>GitHub repo</button>
      <button class="link" on:click={() => openUrl('https://github.com/yt-dlp/yt-dlp')}>yt-dlp</button>
      <button class="link" on:click={() => openUrl('https://ffmpeg.org')}>ffmpeg</button>
      <button class="link" on:click={() => openUrl('https://tauri.app')}>Tauri</button>
    </div>
  </div>
</section>

<style>
  .view {
    max-width: 720px;
    display: flex;
    flex-direction: column;
    gap: 14px;
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

  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .label-col {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--fg-muted);
  }

  .value {
    font-size: 15px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--fg);
  }

  .action-col {
    flex-shrink: 0;
  }

  .status {
    margin: 8px 0 0 0;
  }

  .small {
    font-size: 12px;
  }

  .muted.small {
    font-size: 12px;
    margin: 8px 0 0 0;
  }

  .update-notes {
    margin-top: 10px;
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .update-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }

  .notes-body {
    margin: 0;
    font-family: 'Consolas', 'Menlo', monospace;
    font-size: 12px;
    line-height: 1.4;
    white-space: pre-wrap;
    color: var(--fg-muted);
    max-height: 200px;
    overflow: auto;
  }

  .error {
    margin-top: 10px;
    padding: 10px 14px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    color: var(--danger);
    font-size: 12.5px;
  }

  .error strong {
    display: block;
    margin-bottom: 4px;
  }

  .error code {
    display: block;
    font-family: 'Consolas', 'Menlo', monospace;
    white-space: pre-wrap;
    color: inherit;
    opacity: 0.9;
  }

  .about {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
  }

  .link:hover {
    text-decoration: underline;
  }

  .switch {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    inset: 0;
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: 999px;
    transition: background-color 160ms ease, border-color 160ms ease;
    cursor: pointer;
  }

  .slider::before {
    content: '';
    position: absolute;
    width: 18px;
    height: 18px;
    left: 2px;
    top: 50%;
    transform: translateY(-50%);
    background: var(--fg-muted);
    border-radius: 50%;
    transition: transform 160ms ease, background-color 160ms ease;
  }

  .switch input:checked + .slider {
    background: var(--accent);
    border-color: var(--accent);
  }

  .switch input:checked + .slider::before {
    transform: translate(20px, -50%);
    background: var(--accent-fg);
  }
</style>
