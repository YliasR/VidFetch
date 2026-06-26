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
  /** Whether the file has at least one audio stream. */
  hasAudio: boolean;
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

export interface TrimRange {
  start: number | null;
  end: number | null;
}

export type MultiTrimMode = 'separate' | 'concat';

export interface MultiTrimOptions {
  inputPath: string;
  ranges: TrimRange[];
  mode: MultiTrimMode;
  outputPath: string;
  reencode: boolean;
}

export interface ConcatClipsOptions {
  inputPaths: string[];
  outputPath: string;
}

export interface ConcatPlan {
  /** 'copy' = fast concat demuxer; 'reencode' = normalize pass for mixed sources. */
  mode: 'copy' | 'reencode';
  /** Why a re-encode is needed, or null when copying. */
  reason: string | null;
}

export interface RemoveAudioOptions {
  inputPath: string;
  outputPath: string;
}

export type ReplaceAudioMode = 'replace' | 'mix';
export type ReplaceAudioAlign = 'trim' | 'loop';

export interface ReplaceAudioOptions {
  inputPath: string;
  audioPath: string;
  outputPath: string;
  mode: ReplaceAudioMode;
  align: ReplaceAudioAlign;
  /** Fade-in / fade-out length in seconds; 0 = none. */
  fadeIn: number;
  fadeOut: number;
}

export type AudioFormat = 'mp3' | 'opus' | 'flac';

export interface ExtractAudioOptions {
  inputPath: string;
  outputPath: string;
  format: AudioFormat;
}

export interface VolumeOptions {
  inputPath: string;
  outputPath: string;
  /** Gain in dB; negative quietens, positive boosts. */
  gainDb: number;
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
  thumbnailAt: (path: string, time: number, width?: number) =>
    invoke<string>('thumbnail_at', { path, time, width: width ?? null }),
  trimVideo: (options: TrimOptions) => invoke<string>('trim_video', { options }),
  trimMulti: (options: MultiTrimOptions) => invoke<string>('trim_multi', { options }),
  concatClips: (options: ConcatClipsOptions) =>
    invoke<string>('concat_clips', { options }),
  planConcat: (inputPaths: string[]) =>
    invoke<ConcatPlan>('plan_concat', { inputPaths }),
  removeAudio: (options: RemoveAudioOptions) =>
    invoke<string>('remove_audio', { options }),
  replaceAudio: (options: ReplaceAudioOptions) =>
    invoke<string>('replace_audio', { options }),
  extractAudio: (options: ExtractAudioOptions) =>
    invoke<string>('extract_audio', { options }),
  adjustVolume: (options: VolumeOptions) =>
    invoke<string>('adjust_volume', { options }),
  audioWaveform: (path: string, width?: number, height?: number) =>
    invoke<string>('audio_waveform', { path, width: width ?? null, height: height ?? null }),
  cancelExport: (id: string) => invoke<boolean>('cancel_export', { id }),

  checkAppUpdate: (channel: string) =>
    invoke<UpdateInfo | null>('check_app_update', { channel }),
  installAppUpdate: (channel: string) =>
    invoke<void>('install_app_update', { channel }),
};
