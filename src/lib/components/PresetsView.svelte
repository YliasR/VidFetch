<script lang="ts">
  import { onMount } from 'svelte';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import {
    presetsStore,
    initPresets,
    savePreset,
    deletePreset,
    updatePresetArchive,
    setActivePreset,
    type SavedPreset,
  } from '$lib/stores/presets';
  import {
    downloadStore,
    initDownload,
    applyDownloadConfig,
  } from '$lib/stores/download';
  import { currentView } from '$lib/stores/nav';

  let initialized = false;
  let newName = '';
  let error: string | null = null;

  onMount(async () => {
    if (initialized) return;
    initialized = true;
    await Promise.all([initPresets(), initDownload()]);
  });

  $: presets = $presetsStore.presets;
  $: current = $downloadStore;
  $: currentSummary = formatCurrent();

  async function saveCurrent() {
    error = null;
    try {
      await savePreset({
        name: newName,
        preset: current.preset,
        advanced: current.advanced,
      });
      newName = '';
    } catch (err) {
      error = String(err);
    }
  }

  async function overwritePreset(preset: SavedPreset) {
    await savePreset({
      id: preset.id,
      name: preset.name,
      preset: current.preset,
      advanced: current.advanced,
      archiveEnabled: preset.archiveEnabled,
      archivePath: preset.archivePath,
    });
  }

  async function applyPreset(preset: SavedPreset) {
    await setActivePreset(preset.id);
    await applyDownloadConfig({
      preset: preset.preset,
      advanced: preset.advanced,
    });
    currentView.set('download');
  }

  async function renamePreset(preset: SavedPreset, name: string) {
    await savePreset({
      id: preset.id,
      name,
      preset: preset.preset,
      advanced: preset.advanced,
      archiveEnabled: preset.archiveEnabled,
      archivePath: preset.archivePath,
    });
  }

  async function pickArchive(preset: SavedPreset) {
    const picked = await saveDialog({
      defaultPath: preset.archivePath || `${preset.name.toLowerCase().replace(/\s+/g, '-')}-archive.txt`,
      filters: [{ name: 'Download archive', extensions: ['txt'] }],
    });
    if (typeof picked === 'string' && picked) {
      await updatePresetArchive(preset.id, {
        archivePath: picked,
        archiveEnabled: true,
      });
    }
  }

  function formatPreset(preset: SavedPreset): string {
    const fmt = preset.advanced.outputFormat;
    const bits = [preset.preset, fmt && fmt !== 'auto' ? fmt : null].filter(Boolean);
    return bits.join(' / ');
  }

  function formatCurrent(): string {
    const fmt = current.advanced.outputFormat;
    const bits = [current.preset, fmt && fmt !== 'auto' ? fmt : null].filter(Boolean);
    return bits.join(' / ');
  }
</script>

<section class="view">
  <header class="title">
    <h2>Presets</h2>
    <p class="muted">Save reusable download configurations and optional archive files.</p>
  </header>

  <div class="card save-card">
    <div class="save-copy">
      <div class="label">Current Download Config</div>
      <div class="summary">{currentSummary}</div>
    </div>
    <div class="save-row">
      <input
        class="input"
        type="text"
        placeholder="Preset name"
        bind:value={newName}
        on:keydown={(e) => e.key === 'Enter' && saveCurrent()}
      />
      <button class="btn btn-primary" on:click={saveCurrent} disabled={!newName.trim()}>
        Save
      </button>
    </div>
    {#if error}
      <div class="error"><code>{error}</code></div>
    {/if}
  </div>

  {#if presets.length === 0}
    <div class="card empty">
      <p class="empty-title">No presets yet</p>
      <p class="muted">Configure Download options, name them here, then apply them later in one click.</p>
    </div>
  {:else}
    <div class="items">
      {#each presets as preset (preset.id)}
        <div class="card item">
          <div class="main">
            <input
              class="name-input"
              value={preset.name}
              on:change={(e) => renamePreset(preset, (e.target as HTMLInputElement).value)}
            />
            <div class="meta muted">
              <span class="preset-tag">{formatPreset(preset)}</span>
              {#if preset.advanced.sponsorblock !== 'off'}
                <span class="dot">.</span>
                <span>SponsorBlock {preset.advanced.sponsorblock}</span>
              {/if}
              {#if preset.advanced.subtitleMode !== 'none'}
                <span class="dot">.</span>
                <span>Subtitles {preset.advanced.subtitleMode}</span>
              {/if}
            </div>

            <div class="archive">
              <label class="toggle">
                <input
                  type="checkbox"
                  checked={preset.archiveEnabled}
                  on:change={(e) => updatePresetArchive(preset.id, { archiveEnabled: (e.target as HTMLInputElement).checked })}
                />
                <span>Download archive</span>
              </label>
              <div class="archive-row">
                <input
                  class="input small mono"
                  type="text"
                  value={preset.archivePath}
                  placeholder="No archive file selected"
                  on:change={(e) => updatePresetArchive(preset.id, { archivePath: (e.target as HTMLInputElement).value })}
                />
                <button class="btn small" on:click={() => pickArchive(preset)}>Browse</button>
              </div>
            </div>
          </div>

          <div class="actions">
            <button class="btn btn-primary" on:click={() => applyPreset(preset)}>Apply</button>
            <button class="btn" on:click={() => overwritePreset(preset)}>Update</button>
            <button class="btn btn-ghost danger" on:click={() => deletePreset(preset.id)}>Delete</button>
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
    margin: 0 0 4px 0;
  }

  .title p {
    margin: 0;
  }

  .save-card {
    display: grid;
    grid-template-columns: 1fr minmax(280px, 420px);
    gap: 16px;
    align-items: end;
  }

  .label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--fg-muted);
  }

  .summary {
    margin-top: 4px;
    font-size: 15px;
    font-weight: 600;
  }

  .save-row,
  .archive-row {
    display: flex;
    gap: 8px;
  }

  .save-row .input,
  .archive-row .input {
    flex: 1;
  }

  .error {
    grid-column: 1 / -1;
    padding: 10px 14px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    color: var(--danger);
    font-size: 12.5px;
  }

  .empty {
    padding: 32px;
    text-align: center;
  }

  .empty-title {
    margin: 0 0 6px 0;
    font-weight: 600;
  }

  .items {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .item {
    display: flex;
    gap: 16px;
    align-items: flex-start;
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .name-input {
    width: 100%;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--fg);
    font-size: 16px;
    font-weight: 650;
  }

  .name-input:focus {
    outline: none;
    color: var(--accent);
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

  .archive {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 4px;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    cursor: pointer;
  }

  .toggle input {
    margin: 0;
    accent-color: var(--accent);
  }

  .input.small {
    font-size: 12.5px;
    padding: 6px 10px;
  }

  .mono {
    font-family: 'Consolas', 'Menlo', monospace;
  }

  .btn.small {
    font-size: 12px;
    padding: 6px 12px;
  }

  .actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .danger {
    color: var(--danger);
  }

  @media (max-width: 820px) {
    .save-card,
    .item {
      display: flex;
      flex-direction: column;
    }

    .actions {
      width: 100%;
      justify-content: flex-end;
    }
  }
</style>
