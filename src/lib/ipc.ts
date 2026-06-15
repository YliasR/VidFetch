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

export interface MediaInfo {
  path: string;
  duration: number | null;
  width: number | null;
  height: number | null;
  fps: number | null;
}

export type GifDither = 'sierra2_4a' | 'floyd_steinberg' | 'bayer' | 'none';

export interface GifExportOptions {
  inputPath: string;
  outputPath: string;
  start: number | null;
  end: number | null;
  width: number | null;
  fps: number;
  dither: GifDither;
  /** 0 = loop forever, -1 = play once, n = loop n extra times. */
  loopCount: number | null;
}

export type GifAppendPosition = 'front' | 'back';

export interface GifAppendOptions {
  basePath: string;
  clipPath: string;
  clipStart: number | null;
  clipEnd: number | null;
  position: GifAppendPosition;
  outputPath: string;
  width: number | null;
  fps: number;
  dither: GifDither;
  loopCount: number | null;
}

export interface TrimOptions {
  inputPath: string;
  outputPath: string;
  start: number | null;
  end: number | null;
  /** Re-encode instead of lossless stream-copy. */
  reencode: boolean;
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

  probeMedia: (path: string) => invoke<MediaInfo>('probe_media', { path }),
  exportGif: (options: GifExportOptions) => invoke<string>('export_gif', { options }),
  appendToGif: (options: GifAppendOptions) => invoke<string>('append_to_gif', { options }),
  listKeyframes: (path: string) => invoke<number[]>('list_keyframes', { path }),
  trimVideo: (options: TrimOptions) => invoke<string>('trim_video', { options }),
  cancelExport: (id: string) => invoke<boolean>('cancel_export', { id }),

  checkAppUpdate: (channel: string) =>
    invoke<UpdateInfo | null>('check_app_update', { channel }),
  installAppUpdate: (channel: string) =>
    invoke<void>('install_app_update', { channel }),
};
