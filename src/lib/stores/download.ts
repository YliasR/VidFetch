import { writable } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';
import { downloadDir } from '@tauri-apps/api/path';
import { ipc } from '$lib/ipc';
import type { QualityPreset, VideoInfo } from '$lib/types';

interface ProbeState {
  phase: 'idle' | 'probing' | 'ready' | 'error';
  url: string;
  info: VideoInfo | null;
  error: string | null;
}

interface State {
  probe: ProbeState;
  preset: QualityPreset;
  outputDir: string;
}

const SETTINGS_FILE = 'settings.json';
const OUTPUT_DIR_KEY = 'outputDir';
const PRESET_KEY = 'preset';

const initial: State = {
  probe: { phase: 'idle', url: '', info: null, error: null },
  preset: 'best',
  outputDir: '',
};

export const downloadStore = writable<State>(initial);

const persisted = new LazyStore(SETTINGS_FILE);
let initialized = false;

export async function initDownload(): Promise<void> {
  if (initialized) return;
  initialized = true;

  try {
    const [savedDir, savedPreset, defaultDir] = await Promise.all([
      persisted.get<string>(OUTPUT_DIR_KEY).catch(() => null),
      persisted.get<QualityPreset>(PRESET_KEY).catch(() => null),
      downloadDir().catch(() => ''),
    ]);

    downloadStore.update((s) => ({
      ...s,
      outputDir: savedDir || defaultDir || '',
      preset: savedPreset || s.preset,
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

export async function probe(url: string): Promise<void> {
  const clean = url.trim();
  if (!clean) return;

  downloadStore.update((s) => ({
    ...s,
    probe: { phase: 'probing', url: clean, info: null, error: null },
  }));

  try {
    const info = await ipc.probeUrl(clean);
    downloadStore.update((s) => ({
      ...s,
      probe: { phase: 'ready', url: clean, info, error: null },
    }));
  } catch (err) {
    downloadStore.update((s) => ({
      ...s,
      probe: { phase: 'error', url: clean, info: null, error: String(err) },
    }));
  }
}

export function resetProbe(): void {
  downloadStore.update((s) => ({ ...s, probe: { ...initial.probe } }));
}
