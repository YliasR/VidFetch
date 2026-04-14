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
};
