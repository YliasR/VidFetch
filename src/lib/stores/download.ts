import { writable, get } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { LazyStore } from '@tauri-apps/plugin-store';
import { downloadDir } from '@tauri-apps/api/path';
import { ipc } from '$lib/ipc';
import type {
  DownloadOptions,
  DownloadProgressEvent,
  DownloadStatusEvent,
  DownloadStatusKind,
  QualityPreset,
  VideoInfo,
} from '$lib/types';

interface ProbeState {
  phase: 'idle' | 'probing' | 'ready' | 'error';
  url: string;
  info: VideoInfo | null;
  error: string | null;
}

interface JobState {
  id: string | null;
  status: DownloadStatusKind | 'idle';
  downloaded: number;
  total: number | null;
  speed: number | null;
  eta: number | null;
  message: string | null;
  finalPath: string | null;
}

interface State {
  probe: ProbeState;
  job: JobState;
  preset: QualityPreset;
  outputDir: string;
}

const SETTINGS_FILE = 'settings.json';
const OUTPUT_DIR_KEY = 'outputDir';
const PRESET_KEY = 'preset';

const initial: State = {
  probe: { phase: 'idle', url: '', info: null, error: null },
  job: {
    id: null,
    status: 'idle',
    downloaded: 0,
    total: null,
    speed: null,
    eta: null,
    message: null,
    finalPath: null,
  },
  preset: 'best',
  outputDir: '',
};

export const downloadStore = writable<State>(initial);

const persisted = new LazyStore(SETTINGS_FILE);
let listenersInstalled = false;
let unlistenStatus: UnlistenFn | null = null;
let unlistenProgress: UnlistenFn | null = null;

export async function initDownload(): Promise<void> {
  // Hydrate persisted settings + default output dir.
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

  if (listenersInstalled) return;
  listenersInstalled = true;

  unlistenStatus = await listen<DownloadStatusEvent>('download://status', (e) => {
    const payload = e.payload;
    downloadStore.update((s) => {
      if (s.job.id !== payload.id) return s;
      return {
        ...s,
        job: {
          ...s.job,
          status: payload.status,
          message: payload.message,
        },
      };
    });
  });

  unlistenProgress = await listen<DownloadProgressEvent>('download://progress', (e) => {
    const payload = e.payload;
    downloadStore.update((s) => {
      if (s.job.id !== payload.id) return s;
      return {
        ...s,
        job: {
          ...s.job,
          downloaded: payload.downloaded,
          total: payload.total,
          speed: payload.speed,
          eta: payload.eta,
        },
      };
    });
  });
}

export async function disposeDownload(): Promise<void> {
  if (unlistenStatus) {
    unlistenStatus();
    unlistenStatus = null;
  }
  if (unlistenProgress) {
    unlistenProgress();
    unlistenProgress = null;
  }
  listenersInstalled = false;
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
    job: { ...initial.job }, // reset any previous job UI
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
      probe: {
        phase: 'error',
        url: clean,
        info: null,
        error: String(err),
      },
    }));
  }
}

export async function startDownload(): Promise<void> {
  const state = get(downloadStore);
  if (state.probe.phase !== 'ready' || !state.probe.info || !state.probe.url) return;
  if (!state.outputDir) {
    downloadStore.update((s) => ({
      ...s,
      job: {
        ...s.job,
        status: 'error',
        message: 'Pick an output folder first.',
      },
    }));
    return;
  }

  const options: DownloadOptions = {
    url: state.probe.url,
    outputDir: state.outputDir,
    preset: state.preset,
  };

  try {
    const id = await ipc.startDownload(options);
    downloadStore.update((s) => ({
      ...s,
      job: {
        id,
        status: 'queued',
        downloaded: 0,
        total: null,
        speed: null,
        eta: null,
        message: null,
        finalPath: null,
      },
    }));
  } catch (err) {
    downloadStore.update((s) => ({
      ...s,
      job: {
        ...s.job,
        status: 'error',
        message: String(err),
      },
    }));
  }
}

export async function cancelCurrent(): Promise<void> {
  const s = get(downloadStore);
  if (!s.job.id) return;
  try {
    await ipc.cancelDownload(s.job.id);
  } catch (err) {
    console.warn('[download] cancel failed', err);
  }
}

export function resetProbe(): void {
  downloadStore.update((s) => ({
    ...s,
    probe: { ...initial.probe },
    job: { ...initial.job },
  }));
}
