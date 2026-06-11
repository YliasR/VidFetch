<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { readText } from '@tauri-apps/plugin-clipboard-manager';
  import {
    downloadStore,
    initDownload,
    probe,
    setOutputDir,
    setPreset,
    setSubtitleMode,
    toggleSubtitleLang,
    setAutoGenSubs,
    setSponsorblock,
    updateAdvanced,
    resetProbe,
    singleInfo,
    playlistInfo,
    applyDownloadConfig,
    DEFAULT_OUTPUT_TEMPLATE,
    KNOWN_BROWSERS,
  } from '$lib/stores/download';
  import { addToQueue, newGroupId } from '$lib/stores/queue';
  import { currentView } from '$lib/stores/nav';
  import { classifyError } from '$lib/errors';
  import { ipc } from '$lib/ipc';
  import { presetsStore, initPresets, setActivePreset } from '$lib/stores/presets';
  import type {
    ConflictMode,
    CookiesSource,
    DownloadOptions,
    FormatInfo,
    OutputFormat,
    PlaylistEntry,
    QualityPreset,
    SponsorblockMode,
    SubtitleMode,
    VideoInfo,
  } from '$lib/types';

  let urlInput = '';
  let initialized = false;
  let lastAdded: { title: string; count: number } | null = null;
  let advancedOpen = false;

  // Format browser — exact -f pick that overrides the quality preset.
  let formatBrowserOpen = false;
  let selectedFormat: FormatInfo | null = null;
  let formatForVideoId: string | null = null;

  // Clipboard watcher — offer to fetch a URL the user just copied.
  let clipboardSuggestion: string | null = null;
  let dismissedClipboard: string | null = null;
  let clipboardTimer: ReturnType<typeof setInterval> | null = null;

  const CLIPBOARD_URL_RE = /^https?:\/\/\S+$/i;

  async function checkClipboard() {
    try {
      const text = ((await readText()) ?? '').trim();
      if (!text || text.length > 2000 || !CLIPBOARD_URL_RE.test(text)) return;
      if (text === dismissedClipboard) return;
      if (text === urlInput.trim() || text === state.probe.url) return;
      clipboardSuggestion = text;
    } catch {
      // Clipboard empty or holding non-text content — nothing to offer.
    }
  }

  function acceptClipboard() {
    if (!clipboardSuggestion) return;
    urlInput = clipboardSuggestion;
    dismissedClipboard = clipboardSuggestion;
    clipboardSuggestion = null;
    void handleProbe();
  }

  function dismissClipboard() {
    dismissedClipboard = clipboardSuggestion;
    clipboardSuggestion = null;
  }

  onDestroy(() => {
    if (clipboardTimer) clearInterval(clipboardTimer);
  });

  // Playlist selection state — tracked locally, reset on each probe.
  let selectedIdx = new Set<number>();
  let rangePattern = '';
  let autoSelectedForPlaylistId: string | null = null;

  onMount(async () => {
    if (!initialized) {
      await initDownload();
      await initPresets();
      initialized = true;
    }
    void checkClipboard();
    clipboardTimer = setInterval(checkClipboard, 2000);
  });

  const presets: { id: QualityPreset; label: string; note: string }[] = [
    { id: 'best', label: 'Best', note: 'Highest quality available' },
    { id: 'p1080', label: '1080p', note: 'Cap at 1080p' },
    { id: 'p720', label: '720p', note: 'Cap at 720p' },
    { id: 'audio-mp3', label: 'MP3', note: 'Audio only, mp3' },
    { id: 'audio-opus', label: 'Opus', note: 'Audio only, opus' },
  ];

  const sponsorblockModes: { id: SponsorblockMode; label: string; note: string }[] = [
    { id: 'off', label: 'Off', note: 'Keep everything' },
    { id: 'mark', label: 'Mark', note: 'Add chapter markers' },
    { id: 'remove', label: 'Remove', note: 'Cut sponsors out' },
  ];

  const cookiesSources: { id: CookiesSource; label: string }[] = [
    { id: 'none', label: 'None' },
    { id: 'browser', label: 'From browser' },
    { id: 'file', label: 'From cookies.txt' },
  ];

  const conflictModes: { id: ConflictMode; label: string; note: string }[] = [
    { id: 'skip', label: 'Skip', note: 'Keep the existing file' },
    { id: 'overwrite', label: 'Overwrite', note: 'Replace it' },
  ];

  const outputFormats: { id: OutputFormat; label: string; note: string }[] = [
    { id: 'auto', label: 'Auto', note: 'Whatever source provides' },
    { id: 'mp4', label: 'MP4', note: 'Discord / iOS safe' },
    { id: 'mkv', label: 'MKV', note: 'Lossless merge' },
    { id: 'webm', label: 'WebM', note: 'Smaller, VP9/Opus' },
  ];

  const templatePresets: { label: string; template: string }[] = [
    { label: 'Title', template: '%(title)s.%(ext)s' },
    { label: 'Uploader - Title', template: '%(uploader)s - %(title)s.%(ext)s' },
    { label: 'Date - Title', template: '%(upload_date)s - %(title)s.%(ext)s' },
    { label: 'Channel / Title', template: '%(channel)s/%(title)s.%(ext)s' },
  ];

  $: state = $downloadStore;
  $: single = singleInfo(state);
  $: playlist = playlistInfo(state);
  $: adv = state.advanced;
  $: savedPresets = $presetsStore.presets;
  $: activePresetId = $presetsStore.activePresetId ?? '';
  $: activeSavedPreset = savedPresets.find((p) => p.id === activePresetId) ?? null;
  $: isAudioPreset = state.preset === 'audio-mp3' || state.preset === 'audio-opus';

  $: {
    // Auto-select all entries the first time a new playlist arrives.
    // Track the id so user can later deselect without being overridden.
    if (playlist && autoSelectedForPlaylistId !== playlist.id) {
      selectedIdx = new Set(playlist.entries.map((_, i) => i));
      autoSelectedForPlaylistId = playlist.id;
    } else if (!playlist) {
      autoSelectedForPlaylistId = null;
    }
  }

  $: {
    // Drop the format pick whenever a different video (or none) is probed.
    const id = single?.id ?? null;
    if (id !== formatForVideoId) {
      formatForVideoId = id;
      selectedFormat = null;
      formatBrowserOpen = false;
    }
  }

  $: sortedFormats = single ? sortFormats(single.formats) : [];

  function sortFormats(formats: FormatInfo[]): FormatInfo[] {
    // Video formats first (highest resolution → lowest), audio-only after.
    return [...formats].sort((a, b) => {
      const aVideo = a.vcodec != null;
      const bVideo = b.vcodec != null;
      if (aVideo !== bVideo) return aVideo ? -1 : 1;
      const h = (b.height ?? 0) - (a.height ?? 0);
      if (h !== 0) return h;
      const fps = (b.fps ?? 0) - (a.fps ?? 0);
      if (fps !== 0) return fps;
      return (b.tbr ?? 0) - (a.tbr ?? 0);
    });
  }

  /** Video-only picks get best audio merged in; everything else is exact. */
  function selectorFor(f: FormatInfo): string {
    if (f.vcodec && !f.acodec) return `${f.formatId}+bestaudio/${f.formatId}`;
    return f.formatId;
  }

  function pickFormat(f: FormatInfo) {
    selectedFormat = selectedFormat?.formatId === f.formatId ? null : f;
  }

  function fmtSize(bytes: number | null): string {
    if (bytes == null || bytes <= 0) return '—';
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
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
    lastAdded = null;
    selectedIdx = new Set();
    rangePattern = '';
    await probe(urlInput);
  }

  $: probeHint =
    state.probe.phase === 'error' ? classifyError(state.probe.error) : null;

  let probeFixBusy = false;

  async function runProbeFix() {
    if (!probeHint?.action) return;
    probeFixBusy = true;
    try {
      if (probeHint.action === 'reinstall-ffmpeg') {
        await ipc.installFfmpeg();
      } else {
        await ipc.installYtdlp();
      }
      await handleProbe();
    } catch (err) {
      console.warn('[download] recovery action failed', err);
    } finally {
      probeFixBusy = false;
    }
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

  function buildOptions(url: string): DownloadOptions {
    return {
      url,
      outputDir: state.outputDir,
      preset: state.preset,
      subtitleLangs: adv.subtitleLangs,
      subtitleMode: adv.subtitleMode,
      autoGenSubs: adv.autoGenSubs,
      sponsorblock: adv.sponsorblock,
      cookiesSource: adv.cookiesSource,
      cookiesBrowser: adv.cookiesBrowser || null,
      cookiesFile: adv.cookiesFile || null,
      rateLimit: adv.rateLimit || null,
      retries: adv.retries,
      fragmentRetries: adv.fragmentRetries,
      outputTemplate: adv.outputTemplate || null,
      conflictMode: adv.conflictMode,
      embedThumbnail: adv.embedThumbnail,
      embedMetadata: adv.embedMetadata,
      embedChapters: adv.embedChapters,
      outputFormat: adv.outputFormat,
      downloadArchive:
        activeSavedPreset?.archiveEnabled && activeSavedPreset.archivePath
          ? activeSavedPreset.archivePath
          : null,
      formatSelector: selectedFormat ? selectorFor(selectedFormat) : null,
    };
  }

  function extForPreset(preset: QualityPreset): string {
    if (preset === 'audio-mp3') return 'mp3';
    if (preset === 'audio-opus') return 'opus';
    return 'mp4';
  }

  function previewTemplate(template: string, info: VideoInfo | null, preset: QualityPreset): string {
    const tpl = template.trim() || DEFAULT_OUTPUT_TEMPLATE;
    const ext = extForPreset(preset);
    const subs: Record<string, string> = {
      title: info?.title ?? 'Sample Title',
      uploader: info?.uploader ?? 'Channel Name',
      channel: info?.uploader ?? 'Channel Name',
      id: info?.id ?? 'abc123',
      extractor: info?.extractor ?? 'youtube',
      upload_date: '20260415',
      ext,
    };
    return tpl.replace(/%\((\w+)\)s/g, (_, k) => subs[k] ?? `%(${k})s`);
  }

  async function pickCookiesFile() {
    const picked = await openDialog({
      directory: false,
      multiple: false,
      filters: [{ name: 'Cookies', extensions: ['txt'] }],
    });
    if (typeof picked === 'string' && picked) {
      await updateAdvanced({ cookiesFile: picked });
    }
  }

  function resetTemplate() {
    void updateAdvanced({ outputTemplate: DEFAULT_OUTPUT_TEMPLATE });
  }

  function displayFromVideo(info: VideoInfo) {
    return {
      title: info.title,
      thumbnail: info.thumbnail,
      uploader: info.uploader,
      duration: info.duration,
    };
  }

  function displayFromEntry(entry: PlaylistEntry) {
    return {
      title: entry.title,
      thumbnail: entry.thumbnail,
      uploader: entry.uploader,
      duration: entry.duration,
    };
  }

  function handleAddSingle() {
    if (!single || !state.probe.url || !state.outputDir) return;
    addToQueue({
      options: buildOptions(state.probe.url),
      display: displayFromVideo(single),
    });
    lastAdded = { title: single.title, count: 1 };
    urlInput = '';
    resetProbe();
  }

  function handleAddPlaylist() {
    if (!playlist || !state.outputDir) return;
    const selected = playlist.entries.filter((_, i) => selectedIdx.has(i));
    if (selected.length === 0) return;
    // Multiple entries from one probe collapse into a queue group.
    const group =
      selected.length > 1 ? { id: newGroupId(), title: playlist.title } : null;
    for (const entry of selected) {
      addToQueue({
        options: buildOptions(entry.url),
        display: displayFromEntry(entry),
        group,
      });
    }
    lastAdded = { title: playlist.title, count: selected.length };
    urlInput = '';
    resetProbe();
  }

  function toggleEntry(i: number) {
    const next = new Set(selectedIdx);
    if (next.has(i)) next.delete(i);
    else next.add(i);
    selectedIdx = next;
  }

  function selectAll() {
    if (!playlist) return;
    selectedIdx = new Set(playlist.entries.map((_, i) => i));
  }

  function selectNone() {
    selectedIdx = new Set();
  }

  function applyRange() {
    if (!playlist) return;
    const parsed = parseRange(rangePattern, playlist.entries.length);
    if (parsed) selectedIdx = parsed;
  }

  function parseRange(pattern: string, total: number): Set<number> | null {
    const out = new Set<number>();
    const parts = pattern.split(',').map((p) => p.trim()).filter(Boolean);
    if (parts.length === 0) return null;
    for (const part of parts) {
      const m = part.match(/^(\d+)\s*-\s*(\d+)$/);
      if (m) {
        let a = parseInt(m[1], 10);
        let b = parseInt(m[2], 10);
        if (a > b) [a, b] = [b, a];
        for (let i = a; i <= b; i++) {
          if (i >= 1 && i <= total) out.add(i - 1);
        }
      } else {
        const n = parseInt(part, 10);
        if (!Number.isNaN(n) && n >= 1 && n <= total) out.add(n - 1);
      }
    }
    return out;
  }

  function clearAndReset() {
    urlInput = '';
    lastAdded = null;
    resetProbe();
  }

  function goToQueue() {
    currentView.set('queue');
  }

  async function onSubtitleModeChange(e: Event) {
    await setSubtitleMode((e.target as HTMLSelectElement).value as SubtitleMode);
  }

  async function onSavedPresetChange(e: Event) {
    const id = (e.target as HTMLSelectElement).value;
    await setActivePreset(id || null);
    const preset = savedPresets.find((p) => p.id === id);
    if (preset) {
      await applyDownloadConfig({
        preset: preset.preset,
        advanced: preset.advanced,
      });
    }
  }
</script>

<section class="view">
  <header class="title">
    <h2>Download</h2>
    <p class="muted">Paste any URL supported by yt-dlp — videos, audio, playlists.</p>
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

    {#if clipboardSuggestion && state.probe.phase !== 'probing'}
      <div class="clip-suggest">
        <span class="clip-text" title={clipboardSuggestion}>
          📋 You copied <span class="num">{clipboardSuggestion}</span>
        </span>
        <button class="btn btn-primary small" on:click={acceptClipboard}>Fetch it</button>
        <button class="btn btn-ghost small" title="Dismiss" on:click={dismissClipboard}>✕</button>
      </div>
    {/if}

    {#if state.probe.phase === 'error'}
      <div class="error-inline">
        <strong>{probeHint?.title ?? "Couldn't fetch that URL."}</strong>
        {#if probeHint}
          <span class="hint-text">{probeHint.hint}</span>
        {/if}
        <code>{state.probe.error}</code>
        {#if probeHint?.action}
          <div class="hint-actions">
            <button class="btn btn-sm" disabled={probeFixBusy} on:click={runProbeFix}>
              {probeFixBusy ? 'Working…' : probeHint.actionLabel}
            </button>
          </div>
        {/if}
      </div>
    {/if}

    {#if lastAdded && !state.probe.result}
      <div class="added-banner">
        <span class="check">✓</span>
        <span class="added-text">
          Added
          <strong>
            {#if lastAdded.count === 1}
              {lastAdded.title}
            {:else}
              {lastAdded.count} items from "{lastAdded.title}"
            {/if}
          </strong>
          to the queue.
        </span>
        <button class="link-btn" on:click={goToQueue}>View queue →</button>
      </div>
    {/if}
  </div>

  {#if single}
    <div class="card metadata">
      {#if single.thumbnail}
        <img class="thumb" src={single.thumbnail} alt="" referrerpolicy="no-referrer" />
      {:else}
        <div class="thumb thumb-placeholder"></div>
      {/if}
      <div class="meta">
        <h3>{single.title}</h3>
        <div class="meta-row muted">
          {#if single.uploader}<span>{single.uploader}</span>{/if}
          {#if single.duration}<span class="dot">·</span><span>{formatDuration(single.duration)}</span>{/if}
          {#if single.extractor}<span class="dot">·</span><span>{single.extractor}</span>{/if}
          {#if single.isLive}<span class="dot">·</span><span class="live">LIVE</span>{/if}
        </div>
      </div>
      <button class="btn btn-ghost reset" on:click={clearAndReset} title="Clear">✕</button>
    </div>

    {#if single.formats.length > 0}
      <div class="card fmt-card">
        <button
          class="advanced-toggle"
          on:click={() => (formatBrowserOpen = !formatBrowserOpen)}
          aria-expanded={formatBrowserOpen}
        >
          <span class="chevron" class:open={formatBrowserOpen}>▸</span>
          <span>Format browser</span>
          {#if selectedFormat}
            <span class="fmt-pill num">-f {selectorFor(selectedFormat)}</span>
          {/if}
        </button>

        {#if formatBrowserOpen}
          <p class="muted small fmt-note">
            Pick an exact format instead of the quality preset. Video-only rows get the best audio merged in.
          </p>
          <div class="fmt-table-wrap">
            <table class="fmt-table">
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Ext</th>
                  <th>Resolution</th>
                  <th>FPS</th>
                  <th>Video codec</th>
                  <th>Audio codec</th>
                  <th>Bitrate</th>
                  <th>Size</th>
                </tr>
              </thead>
              <tbody>
                {#each sortedFormats as f (f.formatId)}
                  <tr
                    class:selected={selectedFormat?.formatId === f.formatId}
                    on:click={() => pickFormat(f)}
                  >
                    <td class="num">{f.formatId}</td>
                    <td>{f.ext}</td>
                    <td class="num">{f.resolution ?? (f.vcodec ? '—' : 'audio only')}</td>
                    <td class="num">{f.fps ? Math.round(f.fps) : '—'}</td>
                    <td>{f.vcodec ?? '—'}</td>
                    <td>{f.acodec ?? '—'}</td>
                    <td class="num">{f.tbr ? `${Math.round(f.tbr)} kbps` : '—'}</td>
                    <td class="num">{fmtSize(f.filesize)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
          {#if selectedFormat}
            <button class="btn btn-ghost small" on:click={() => (selectedFormat = null)}>
              Clear pick — use quality preset
            </button>
          {/if}
        {/if}
      </div>
    {/if}
  {/if}

  {#if playlist}
    <div class="card metadata">
      {#if playlist.thumbnail}
        <img class="thumb" src={playlist.thumbnail} alt="" referrerpolicy="no-referrer" />
      {:else}
        <div class="thumb thumb-placeholder"></div>
      {/if}
      <div class="meta">
        <h3>{playlist.title}</h3>
        <div class="meta-row muted">
          <span class="pill">Playlist</span>
          <span>{playlist.count} items</span>
          {#if playlist.uploader}<span class="dot">·</span><span>{playlist.uploader}</span>{/if}
          {#if playlist.extractor}<span class="dot">·</span><span>{playlist.extractor}</span>{/if}
        </div>
      </div>
      <button class="btn btn-ghost reset" on:click={clearAndReset} title="Clear">✕</button>
    </div>

    <div class="card playlist-picker">
      <div class="picker-head">
        <div class="label">Items ({selectedIdx.size} / {playlist.entries.length} selected)</div>
        <div class="picker-controls">
          <button class="btn btn-ghost small" on:click={selectAll}>All</button>
          <button class="btn btn-ghost small" on:click={selectNone}>None</button>
        </div>
      </div>
      <div class="range-row">
        <input
          class="input small"
          type="text"
          placeholder="Range, e.g. 1,3,5-7"
          bind:value={rangePattern}
          on:keydown={(e) => e.key === 'Enter' && applyRange()}
        />
        <button class="btn small" on:click={applyRange} disabled={!rangePattern.trim()}>Apply</button>
      </div>
      <div class="entries">
        {#each playlist.entries as entry, i (entry.id || i)}
          <label class="entry" class:selected={selectedIdx.has(i)}>
            <input
              type="checkbox"
              checked={selectedIdx.has(i)}
              on:change={() => toggleEntry(i)}
            />
            <span class="entry-idx">{i + 1}</span>
            {#if entry.thumbnail}
              <img class="entry-thumb" src={entry.thumbnail} alt="" referrerpolicy="no-referrer" />
            {:else}
              <div class="entry-thumb entry-thumb-placeholder"></div>
            {/if}
            <span class="entry-title" title={entry.title}>{entry.title}</span>
            {#if entry.duration}
              <span class="entry-duration muted">{formatDuration(entry.duration)}</span>
            {/if}
          </label>
        {/each}
      </div>
    </div>
  {/if}

  {#if single || playlist}
    <div class="card options">
      {#if savedPresets.length > 0}
        <label class="label" for="savedPreset">Preset</label>
        <div class="preset-select-row">
          <select
            id="savedPreset"
            class="input"
            value={activePresetId}
            on:change={onSavedPresetChange}
          >
            <option value="">Current settings</option>
            {#each savedPresets as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
          {#if activeSavedPreset?.archiveEnabled}
            <span class="archive-pill" title={activeSavedPreset.archivePath}>Archive on</span>
          {/if}
        </div>
      {/if}

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

      {#if !isAudioPreset}
        <div class="label">Format</div>
        <div class="formats">
          {#each outputFormats as f (f.id)}
            <button
              class="preset"
              class:active={adv.outputFormat === f.id}
              on:click={() => updateAdvanced({ outputFormat: f.id })}
            >
              <span class="preset-label">{f.label}</span>
              <span class="preset-note">{f.note}</span>
            </button>
          {/each}
        </div>
      {/if}

      <label class="label" for="outputDir">Save to</label>
      <div class="folder-row">
        <input id="outputDir" class="input" value={state.outputDir} readonly />
        <button class="btn" on:click={pickFolder}>Browse…</button>
      </div>

      <button
        class="advanced-toggle"
        on:click={() => (advancedOpen = !advancedOpen)}
        aria-expanded={advancedOpen}
      >
        <span class="chevron" class:open={advancedOpen}>▸</span>
        <span>Advanced options</span>
      </button>

      {#if advancedOpen}
        <div class="advanced">
          <!-- Subtitles -->
          <div class="section">
            <div class="label">Subtitles</div>
            {#if isAudioPreset}
              <p class="muted small">Subtitles don't apply to audio-only downloads.</p>
            {:else if single && single.availableSubs.length === 0 && single.availableAutoSubs.length === 0}
              <p class="muted small">No subtitles available for this video.</p>
            {:else}
              <div class="sub-row">
                <label class="sub-field">
                  <span class="sub-label">Mode</span>
                  <select class="input small" value={adv.subtitleMode} on:change={onSubtitleModeChange}>
                    <option value="none">None</option>
                    <option value="embed">Embed in video</option>
                    <option value="separate">Save as separate file</option>
                  </select>
                </label>
                {#if !single}
                  <p class="muted small">Subtitles will apply to all queued items if available.</p>
                {/if}
              </div>

              {#if adv.subtitleMode !== 'none'}
                {@const langs = single ? single.availableSubs : []}
                {@const autoLangs = single ? single.availableAutoSubs : []}
                {#if single}
                  {#if langs.length > 0}
                    <div class="sub-sublabel">Available languages</div>
                    <div class="chips">
                      {#each langs as lang}
                        <button
                          class="chip"
                          class:active={adv.subtitleLangs.includes(lang)}
                          on:click={() => toggleSubtitleLang(lang)}
                        >{lang}</button>
                      {/each}
                    </div>
                  {/if}
                  {#if autoLangs.length > 0}
                    <label class="toggle">
                      <input
                        type="checkbox"
                        checked={adv.autoGenSubs}
                        on:change={(e) => setAutoGenSubs((e.target as HTMLInputElement).checked)}
                      />
                      <span>Include auto-generated subtitles ({autoLangs.length} langs available)</span>
                    </label>
                  {/if}
                {:else}
                  <p class="muted small">
                    Enter languages to fetch (comma-separated) — applied if available:
                  </p>
                  <input
                    class="input small"
                    type="text"
                    placeholder="en, fr, nl"
                    value={adv.subtitleLangs.join(', ')}
                    on:change={(e) => {
                      const raw = (e.target as HTMLInputElement).value;
                      const langs = raw.split(',').map((s) => s.trim()).filter(Boolean);
                      // Replace full list by toggling diff
                      const current = new Set(adv.subtitleLangs);
                      const next = new Set(langs);
                      for (const l of current) if (!next.has(l)) toggleSubtitleLang(l);
                      for (const l of next) if (!current.has(l)) toggleSubtitleLang(l);
                    }}
                  />
                  <label class="toggle">
                    <input
                      type="checkbox"
                      checked={adv.autoGenSubs}
                      on:change={(e) => setAutoGenSubs((e.target as HTMLInputElement).checked)}
                    />
                    <span>Include auto-generated subtitles</span>
                  </label>
                {/if}
              {/if}
            {/if}
          </div>

          <!-- SponsorBlock -->
          <div class="section">
            <div class="label">SponsorBlock</div>
            <div class="sb-modes">
              {#each sponsorblockModes as m (m.id)}
                <button
                  class="preset"
                  class:active={adv.sponsorblock === m.id}
                  on:click={() => setSponsorblock(m.id)}
                >
                  <span class="preset-label">{m.label}</span>
                  <span class="preset-note">{m.note}</span>
                </button>
              {/each}
            </div>
          </div>

          <!-- Cookies -->
          <div class="section">
            <div class="label">Cookies</div>
            <div class="cookies-source">
              {#each cookiesSources as c (c.id)}
                <button
                  class="pill-btn"
                  class:active={adv.cookiesSource === c.id}
                  on:click={() => updateAdvanced({ cookiesSource: c.id })}
                >{c.label}</button>
              {/each}
            </div>
            {#if adv.cookiesSource === 'browser'}
              <select
                class="input small"
                value={adv.cookiesBrowser}
                on:change={(e) => updateAdvanced({ cookiesBrowser: (e.target as HTMLSelectElement).value })}
              >
                <option value="">Pick a browser…</option>
                {#each KNOWN_BROWSERS as b (b.id)}
                  <option value={b.id}>{b.label}</option>
                {/each}
              </select>
              <p class="muted small">
                yt-dlp reads cookies from the named browser's profile. The browser
                should be closed or at least logged into the target site.
              </p>
            {:else if adv.cookiesSource === 'file'}
              <div class="file-row">
                <input
                  class="input small"
                  type="text"
                  readonly
                  value={adv.cookiesFile || 'No file selected'}
                />
                <button class="btn small" on:click={pickCookiesFile}>Browse…</button>
              </div>
            {/if}
          </div>

          <!-- Network -->
          <div class="section">
            <div class="label">Network</div>
            <div class="grid-3">
              <label class="field">
                <span class="field-label">Rate limit</span>
                <input
                  class="input small"
                  type="text"
                  placeholder="e.g. 2M, 500K"
                  value={adv.rateLimit}
                  on:change={(e) => updateAdvanced({ rateLimit: (e.target as HTMLInputElement).value })}
                />
              </label>
              <label class="field">
                <span class="field-label">Retries</span>
                <input
                  class="input small"
                  type="number"
                  min="0"
                  max="99"
                  value={adv.retries}
                  on:change={(e) => {
                    const n = parseInt((e.target as HTMLInputElement).value, 10);
                    updateAdvanced({ retries: Number.isNaN(n) ? 10 : Math.max(0, Math.min(99, n)) });
                  }}
                />
              </label>
              <label class="field">
                <span class="field-label">Fragment retries</span>
                <input
                  class="input small"
                  type="number"
                  min="0"
                  max="99"
                  value={adv.fragmentRetries}
                  on:change={(e) => {
                    const n = parseInt((e.target as HTMLInputElement).value, 10);
                    updateAdvanced({ fragmentRetries: Number.isNaN(n) ? 10 : Math.max(0, Math.min(99, n)) });
                  }}
                />
              </label>
            </div>
          </div>

          <!-- Output -->
          <div class="section">
            <div class="label">Output template</div>
            <div class="template-row">
              <input
                class="input small mono"
                type="text"
                value={adv.outputTemplate}
                on:change={(e) => updateAdvanced({ outputTemplate: (e.target as HTMLInputElement).value })}
              />
              <button class="btn small btn-ghost" on:click={resetTemplate} title="Reset to default">↺</button>
            </div>
            <div class="template-presets">
              {#each templatePresets as tp (tp.template)}
                <button
                  class="pill-btn small-pill"
                  class:active={adv.outputTemplate === tp.template}
                  on:click={() => updateAdvanced({ outputTemplate: tp.template })}
                  title={tp.template}
                >{tp.label}</button>
              {/each}
            </div>
            <div class="preview-row">
              <span class="preview-label">Preview:</span>
              <span class="preview-value">{previewTemplate(adv.outputTemplate, single, state.preset)}</span>
            </div>

            <div class="sub-sublabel">If the file already exists</div>
            <div class="conflict-modes">
              {#each conflictModes as m (m.id)}
                <button
                  class="preset"
                  class:active={adv.conflictMode === m.id}
                  on:click={() => updateAdvanced({ conflictMode: m.id })}
                >
                  <span class="preset-label">{m.label}</span>
                  <span class="preset-note">{m.note}</span>
                </button>
              {/each}
            </div>
          </div>

          <!-- Embed extras -->
          <div class="section">
            <div class="label">Embed extras</div>
            <div class="embed-toggles">
              <label class="toggle">
                <input
                  type="checkbox"
                  checked={adv.embedThumbnail}
                  on:change={(e) => updateAdvanced({ embedThumbnail: (e.target as HTMLInputElement).checked })}
                />
                <span>Thumbnail</span>
              </label>
              <label class="toggle">
                <input
                  type="checkbox"
                  checked={adv.embedMetadata}
                  on:change={(e) => updateAdvanced({ embedMetadata: (e.target as HTMLInputElement).checked })}
                />
                <span>Metadata</span>
              </label>
              <label class="toggle">
                <input
                  type="checkbox"
                  checked={adv.embedChapters}
                  on:change={(e) => updateAdvanced({ embedChapters: (e.target as HTMLInputElement).checked })}
                />
                <span>Chapters</span>
              </label>
            </div>
          </div>
        </div>
      {/if}

      <div class="primary-action">
        {#if single}
          <button class="btn btn-primary" on:click={handleAddSingle} disabled={!state.outputDir}>
            Add to queue
          </button>
        {:else if playlist}
          <button
            class="btn btn-primary"
            on:click={handleAddPlaylist}
            disabled={!state.outputDir || selectedIdx.size === 0}
          >
            Add {selectedIdx.size} to queue
          </button>
        {/if}
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

  .error-inline .hint-text {
    display: block;
    margin-bottom: 6px;
    color: var(--fg-muted);
  }

  .error-inline .hint-actions {
    margin-top: 8px;
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

  .pill {
    padding: 2px 8px;
    border-radius: 4px;
    background: var(--accent-muted);
    color: var(--accent);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.3px;
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

  .playlist-picker {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .picker-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }

  .picker-controls {
    display: flex;
    gap: 6px;
  }

  .range-row {
    display: flex;
    gap: 8px;
  }

  .range-row .input {
    flex: 1;
  }

  .entries {
    max-height: 300px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 4px;
    background: var(--surface-2);
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12.5px;
    transition: background-color 120ms ease;
  }

  .entry:hover {
    background: var(--surface-3);
  }

  .entry.selected {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .entry input[type='checkbox'] {
    flex-shrink: 0;
    margin: 0;
    accent-color: var(--accent);
  }

  .entry-idx {
    min-width: 28px;
    font-variant-numeric: tabular-nums;
    color: var(--fg-muted);
    font-size: 11.5px;
    text-align: right;
  }

  .entry-thumb {
    width: 60px;
    height: 34px;
    object-fit: cover;
    border-radius: 4px;
    background: var(--surface-3);
    flex-shrink: 0;
  }

  .entry-thumb-placeholder {
    display: block;
  }

  .entry-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-duration {
    font-variant-numeric: tabular-nums;
    font-size: 11.5px;
    flex-shrink: 0;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .preset-select-row {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .preset-select-row .input {
    flex: 1;
  }

  .archive-pill {
    flex-shrink: 0;
    padding: 4px 8px;
    border-radius: 4px;
    background: var(--accent-muted);
    color: var(--accent);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.3px;
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

  .formats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  .sb-modes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
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

  .advanced-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 4px;
    background: none;
    border: none;
    color: var(--fg-muted);
    font-size: 12.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.4px;
    cursor: pointer;
    width: fit-content;
    border-radius: 6px;
    transition: color 120ms ease;
  }

  .advanced-toggle:hover {
    color: var(--fg);
  }

  .chevron {
    display: inline-block;
    transition: transform 140ms ease;
    font-size: 10px;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .advanced {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sub-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .sub-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sub-label {
    font-size: 11px;
    color: var(--fg-muted);
    text-transform: uppercase;
    letter-spacing: 0.3px;
    font-weight: 600;
  }

  .sub-sublabel {
    font-size: 11px;
    color: var(--fg-muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    margin-top: 4px;
  }

  .input.small {
    font-size: 12.5px;
    padding: 6px 10px;
  }

  .btn.small {
    font-size: 12px;
    padding: 6px 12px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    padding: 4px 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 12px;
    font-family: 'Consolas', 'Menlo', monospace;
    transition: background-color 120ms ease, border-color 120ms ease, color 120ms ease;
  }

  .chip:hover {
    border-color: var(--border-strong);
  }

  .chip.active {
    background: var(--accent-muted);
    border-color: var(--accent);
    color: var(--accent);
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    cursor: pointer;
    margin-top: 4px;
  }

  .toggle input {
    margin: 0;
    accent-color: var(--accent);
  }

  .small {
    font-size: 12px;
  }

  .muted.small {
    font-size: 12px;
    margin: 0;
  }

  .pill-btn {
    padding: 6px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 500;
    transition: background-color 120ms ease, border-color 120ms ease, color 120ms ease;
  }

  .pill-btn:hover:not(:disabled) {
    border-color: var(--border-strong);
  }

  .pill-btn.active {
    background: var(--accent-muted);
    border-color: var(--accent);
    color: var(--accent);
  }

  .pill-btn.small-pill {
    padding: 4px 10px;
    font-size: 11.5px;
  }

  .cookies-source {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .file-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .file-row .input {
    flex: 1;
    font-family: 'Consolas', 'Menlo', monospace;
  }

  .grid-3 {
    display: grid;
    grid-template-columns: 2fr 1fr 1fr;
    gap: 10px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-label {
    font-size: 11px;
    color: var(--fg-muted);
    text-transform: uppercase;
    letter-spacing: 0.3px;
    font-weight: 600;
  }

  .template-row {
    display: flex;
    gap: 8px;
  }

  .template-row .input {
    flex: 1;
  }

  .input.mono {
    font-family: 'Consolas', 'Menlo', monospace;
  }

  .template-presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .preview-row {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 8px 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    font-size: 12px;
  }

  .preview-label {
    color: var(--fg-muted);
    font-weight: 600;
    text-transform: uppercase;
    font-size: 10.5px;
    letter-spacing: 0.4px;
    flex-shrink: 0;
  }

  .preview-value {
    font-family: 'Consolas', 'Menlo', monospace;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .conflict-modes {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  .embed-toggles {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
  }

  .primary-action {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 4px;
  }

  .fmt-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .clip-suggest {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 10px;
    padding: 8px 12px;
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-2));
    border: 1px solid color-mix(in srgb, var(--accent) 25%, transparent);
    border-radius: 8px;
    font-size: 12.5px;
  }

  .clip-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .fmt-pill {
    margin-left: 8px;
    padding: 1px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--accent) 16%, var(--surface-2));
    color: var(--accent);
    font-size: 11px;
    font-weight: 600;
  }

  .fmt-note {
    margin: 0;
  }

  .fmt-table-wrap {
    max-height: 320px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  .fmt-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  .fmt-table th {
    position: sticky;
    top: 0;
    background: var(--surface-2);
    text-align: left;
    padding: 7px 10px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.4px;
    color: var(--fg-muted);
    border-bottom: 1px solid var(--border);
  }

  .fmt-table td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  .fmt-table tbody tr {
    cursor: pointer;
  }

  .fmt-table tbody tr:hover {
    background: var(--surface-2);
  }

  .fmt-table tbody tr.selected {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .fmt-table tbody tr:last-child td {
    border-bottom: none;
  }

  .num {
    font-variant-numeric: tabular-nums;
  }
</style>
