import { writable, get } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';
import { downloadDir } from '@tauri-apps/api/path';
import { ipc } from '$lib/ipc';
import type {
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
}

interface State {
  probe: ProbeState;
  preset: QualityPreset;
  outputDir: string;
  advanced: AdvancedOptions;
}

const SETTINGS_FILE = 'settings.json';
const OUTPUT_DIR_KEY = 'outputDir';
const PRESET_KEY = 'preset';
const ADVANCED_KEY = 'advanced';

const defaultAdvanced: AdvancedOptions = {
  subtitleLangs: [],
  subtitleMode: 'none',
  autoGenSubs: false,
  sponsorblock: 'off',
};

const initial: State = {
  probe: { phase: 'idle', url: '', result: null, error: null },
  preset: 'best',
  outputDir: '',
  advanced: { ...defaultAdvanced },
};

export const downloadStore = writable<State>(initial);

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

export async function setSubtitleMode(mode: SubtitleMode): Promise<void> {
  downloadStore.update((s) => ({ ...s, advanced: { ...s.advanced, subtitleMode: mode } }));
  await saveAdvanced();
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
  downloadStore.update((s) => ({ ...s, advanced: { ...s.advanced, autoGenSubs: v } }));
  await saveAdvanced();
}

export async function setSponsorblock(mode: SponsorblockMode): Promise<void> {
  downloadStore.update((s) => ({ ...s, advanced: { ...s.advanced, sponsorblock: mode } }));
  await saveAdvanced();
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

export function singleInfo(state: State): VideoInfo | null {
  return state.probe.result?.kind === 'single' ? state.probe.result.info : null;
}

export function playlistInfo(state: State): PlaylistInfo | null {
  return state.probe.result?.kind === 'playlist' ? state.probe.result.info : null;
}
