import { invoke } from '@tauri-apps/api/core';
import type { DownloadOptions, ProbeResult } from './types';

export interface BinariesStatus {
  ytdlp: boolean;
  ffmpeg: boolean;
  ffprobe: boolean;
}

export interface Versions {
  ytdlp: string | null;
  ffmpeg: string | null;
}

export interface UpdateInfo {
  version: string;
  body: string | null;
  date: string | null;
}

export interface InstallProgress {
  target: 'ytdlp' | 'ffmpeg';
  phase: 'fetching' | 'downloading' | 'extracting' | 'done';
  downloaded: number;
  total: number | null;
  message: string | null;
}

export const ipc = {
  checkBinaries: () => invoke<BinariesStatus>('check_binaries'),
  installYtdlp: () => invoke<string>('install_ytdlp'),
  installFfmpeg: () => invoke<string>('install_ffmpeg'),
  getVersions: () => invoke<Versions>('get_versions'),

  probeUrl: (url: string) => invoke<ProbeResult>('probe_url', { url }),
  startDownload: (options: DownloadOptions) =>
    invoke<string>('start_download', { options }),
  cancelDownload: (id: string) => invoke<boolean>('cancel_download', { id }),
  pauseDownload: (id: string) => invoke<boolean>('pause_download', { id }),
  resumeDownload: (id: string) => invoke<boolean>('resume_download', { id }),
  readDroppedText: (path: string) => invoke<string>('read_dropped_text', { path }),

  checkAppUpdate: (channel: string) =>
    invoke<UpdateInfo | null>('check_app_update', { channel }),
  installAppUpdate: (channel: string) =>
    invoke<void>('install_app_update', { channel }),
};
