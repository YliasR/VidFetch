export interface VideoInfo {
  id: string;
  title: string;
  uploader: string | null;
  duration: number | null;
  thumbnail: string | null;
  extractor: string | null;
  webpageUrl: string | null;
  isLive: boolean | null;
}

export type QualityPreset = 'best' | 'p1080' | 'p720' | 'audio-mp3' | 'audio-opus';

export interface DownloadOptions {
  url: string;
  outputDir: string;
  preset: QualityPreset;
}

export type DownloadStatusKind =
  | 'queued'
  | 'downloading'
  | 'postprocess'
  | 'done'
  | 'error'
  | 'canceled';

export interface DownloadStatusEvent {
  id: string;
  status: DownloadStatusKind;
  message: string | null;
}

export interface DownloadProgressEvent {
  id: string;
  downloaded: number;
  total: number | null;
  speed: number | null;
  eta: number | null;
}

export interface DownloadLogEvent {
  id: string;
  line: string;
  stream: 'stdout' | 'stderr';
}
