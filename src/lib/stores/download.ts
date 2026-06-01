import { writable, get } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';
import { downloadDir } from '@tauri-apps/api/path';
import { ipc } from '$lib/ipc';
import type {
  ConflictMode,
  CookiesSource,
  OutputFormat,
  PlaylistInfo,
  ProbeResult,
  QualityPreset,
  SponsorblockMode,
  SubtitleMode,
  VideoInfo,
} from '$lib/types';

interface ProbeState {
  phase: 'idle' | 'probing' | 'ready' | 'error';
  url: string;
  result: ProbeResult | null;
  error: string | null;
}

export interface AdvancedOptions {
  subtitleLangs: string[];
  subtitleMode: SubtitleMode;
  autoGenSubs: boolean;
  sponsorblock: SponsorblockMode;

  cookiesSource: CookiesSource;
  cookiesBrowser: string;
  cookiesFile: string;

  rateLimit: string;
  retries: number;
  fragmentRetries: number;

  outputTemplate: string;
  conflictMode: ConflictMode;

  embedThumbnail: boolean;
  embedMetadata: boolean;
  embedChapters: boolean;

  outputFormat: OutputFormat;
}

export interface DownloadState {
  probe: ProbeState;
  preset: QualityPreset;
  outputDir: string;
  advanced: AdvancedOptions;
}

const SETTINGS_FILE = 'settings.json';
const OUTPUT_DIR_KEY = 'outputDir';
const PRESET_KEY = 'preset';
const ADVANCED_KEY = 'advanced';

export const DEFAULT_OUTPUT_TEMPLATE = '%(title)s.%(ext)s';

export const KNOWN_BROWSERS: { id: string; label: string }[] = [
  { id: 'chrome', label: 'Chrome' },
  { id: 'firefox', label: 'Firefox' },
  { id: 'edge', label: 'Edge' },
  { id: 'brave', label: 'Brave' },
  { id: 'chromium', label: 'Chromium' },
  { id: 'opera', label: 'Opera' },
  { id: 'vivaldi', label: 'Vivaldi' },
  { id: 'safari', label: 'Safari' },
];

const defaultAdvanced: AdvancedOptions = {
  subtitleLangs: [],
  subtitleMode: 'none',
  autoGenSubs: false,
  sponsorblock: 'off',

  cookiesSource: 'none',
  cookiesBrowser: '',
  cookiesFile: '',

  rateLimit: '',
  retries: 10,
  fragmentRetries: 10,

  outputTemplate: DEFAULT_OUTPUT_TEMPLATE,
  conflictMode: 'skip',

  embedThumbnail: true,
  embedMetadata: true,
  embedChapters: false,

  outputFormat: 'auto',
};

const initial: DownloadState = {
  probe: { phase: 'idle', url: '', result: null, error: null },
  preset: 'best',
  outputDir: '',
  advanced: { ...defaultAdvanced },
};

export const downloadStore = writable<DownloadState>(initial);

const persisted = new LazyStore(SETTINGS_FILE);
let initialized = false;

export async function initDownload(): Promise<void> {
  if (initialized) return;
  initialized = true;

  try {
    const [savedDir, savedPreset, savedAdvanced, defaultDir] = await Promise.all([
      persisted.get<string>(OUTPUT_DIR_KEY).catch(() => null),
      persisted.get<QualityPreset>(PRESET_KEY).catch(() => null),
      persisted.get<Partial<AdvancedOptions>>(ADVANCED_KEY).catch(() => null),
      downloadDir().catch(() => ''),
    ]);

    downloadStore.update((s) => ({
      ...s,
      outputDir: savedDir || defaultDir || '',
      preset: savedPreset || s.preset,
      advanced: { ...defaultAdvanced, ...(savedAdvanced ?? {}) },
    }));
  } catch (err) {
    console.warn('[download] failed to hydrate settings', err);
  }
}

export async function setOutputDir(dir: string): Promise<void> {
  downloadStore.update((s) => ({ ...s, outputDir: dir }));
  await persisted.set(OUTPUT_DIR_KEY, dir);
  await persisted.save();
}

export async function setPreset(preset: QualityPreset): Promise<void> {
  downloadStore.update((s) => ({ ...s, preset }));
  await persisted.set(PRESET_KEY, preset);
  await persisted.save();
}

async function saveAdvanced(): Promise<void> {
  const adv = get(downloadStore).advanced;
  await persisted.set(ADVANCED_KEY, adv);
  await persisted.save();
}

export async function updateAdvanced(patch: Partial<AdvancedOptions>): Promise<void> {
  downloadStore.update((s) => ({ ...s, advanced: { ...s.advanced, ...patch } }));
  await saveAdvanced();
}

export async function applyDownloadConfig(config: {
  preset: QualityPreset;
  advanced: AdvancedOptions;
}): Promise<void> {
  downloadStore.update((s) => ({
    ...s,
    preset: config.preset,
    advanced: { ...defaultAdvanced, ...config.advanced },
  }));
  await persisted.set(PRESET_KEY, config.preset);
  await persisted.set(ADVANCED_KEY, get(downloadStore).advanced);
  await persisted.save();
}

export async function setSubtitleMode(mode: SubtitleMode): Promise<void> {
  await updateAdvanced({ subtitleMode: mode });
}

export async function toggleSubtitleLang(lang: string): Promise<void> {
  downloadStore.update((s) => {
    const langs = s.advanced.subtitleLangs.includes(lang)
      ? s.advanced.subtitleLangs.filter((l) => l !== lang)
      : [...s.advanced.subtitleLangs, lang];
    return { ...s, advanced: { ...s.advanced, subtitleLangs: langs } };
  });
  await saveAdvanced();
}

export async function setAutoGenSubs(v: boolean): Promise<void> {
  await updateAdvanced({ autoGenSubs: v });
}

export async function setSponsorblock(mode: SponsorblockMode): Promise<void> {
  await updateAdvanced({ sponsorblock: mode });
}

export async function probe(url: string): Promise<void> {
  const clean = url.trim();
  if (!clean) return;

  downloadStore.update((s) => ({
    ...s,
    probe: { phase: 'probing', url: clean, result: null, error: null },
  }));

  try {
    const result = await ipc.probeUrl(clean);
    downloadStore.update((s) => ({
      ...s,
      probe: { phase: 'ready', url: clean, result, error: null },
    }));
  } catch (err) {
    downloadStore.update((s) => ({
      ...s,
      probe: { phase: 'error', url: clean, result: null, error: String(err) },
    }));
  }
}

export function resetProbe(): void {
  downloadStore.update((s) => ({ ...s, probe: { ...initial.probe } }));
}

export function singleInfo(state: DownloadState): VideoInfo | null {
  return state.probe.result?.kind === 'single' ? state.probe.result.info : null;
}

export function playlistInfo(state: DownloadState): PlaylistInfo | null {
  return state.probe.result?.kind === 'playlist' ? state.probe.result.info : null;
}
