import { writable, get } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ipc, type BinariesStatus, type InstallProgress, type Versions } from '$lib/ipc';

export type BootPhase =
  | 'checking'
  | 'needsInstall'
  | 'installing'
  | 'ready'
  | 'error';

interface TargetProgress {
  phase: InstallProgress['phase'] | 'idle';
  downloaded: number;
  total: number | null;
  message: string | null;
}

interface State {
  boot: BootPhase;
  status: BinariesStatus | null;
  versions: Versions | null;
  error: string | null;
  progress: {
    ytdlp: TargetProgress;
    ffmpeg: TargetProgress;
  };
}

const idle: TargetProgress = {
  phase: 'idle',
  downloaded: 0,
  total: null,
  message: null,
};

const initial: State = {
  boot: 'checking',
  status: null,
  versions: null,
  error: null,
  progress: {
    ytdlp: { ...idle },
    ffmpeg: { ...idle },
  },
};

export const ytdlpStore = writable<State>(initial);

let unlistenProgress: UnlistenFn | null = null;

async function ensureProgressListener(): Promise<void> {
  if (unlistenProgress) return;
  unlistenProgress = await listen<InstallProgress>('installer://progress', (e) => {
    const p = e.payload;
    ytdlpStore.update((s) => ({
      ...s,
      progress: {
        ...s.progress,
        [p.target]: {
          phase: p.phase,
          downloaded: p.downloaded,
          total: p.total,
          message: p.message,
        },
      },
    }));
  });
}

export async function bootCheck(): Promise<void> {
  await ensureProgressListener();
  ytdlpStore.update((s) => ({ ...s, boot: 'checking', error: null }));

  try {
    const status = await ipc.checkBinaries();
    const ready = status.ytdlp && status.ffmpeg && status.ffprobe;

    if (ready) {
      const versions = await ipc.getVersions();
      ytdlpStore.update((s) => ({ ...s, status, versions, boot: 'ready' }));
    } else {
      ytdlpStore.update((s) => ({ ...s, status, boot: 'needsInstall' }));
    }
  } catch (err) {
    ytdlpStore.update((s) => ({
      ...s,
      boot: 'error',
      error: String(err),
    }));
  }
}

export async function runInstall(): Promise<void> {
  const state = get(ytdlpStore);
  const status = state.status;
  if (!status) return;

  ytdlpStore.update((s) => ({ ...s, boot: 'installing', error: null }));

  try {
    if (!status.ytdlp) {
      await ipc.installYtdlp();
    }
    if (!status.ffmpeg || !status.ffprobe) {
      await ipc.installFfmpeg();
    }

    const fresh = await ipc.checkBinaries();
    const versions = await ipc.getVersions();
    ytdlpStore.update((s) => ({
      ...s,
      status: fresh,
      versions,
      boot: 'ready',
    }));
  } catch (err) {
    ytdlpStore.update((s) => ({
      ...s,
      boot: 'error',
      error: String(err),
    }));
  }
}

export function resetError(): void {
  ytdlpStore.update((s) => ({ ...s, boot: 'needsInstall', error: null }));
}
