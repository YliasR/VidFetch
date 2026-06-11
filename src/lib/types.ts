export interface VideoInfo {
  id: string;
  title: string;
  uploader: string | null;
  duration: number | null;
  thumbnail: string | null;
  extractor: string | null;
  webpageUrl: string | null;
  isLive: boolean | null;
  availableSubs: string[];
  availableAutoSubs: string[];
  formats: FormatInfo[];
}

export interface FormatInfo {
  formatId: string;
  ext: string;
  resolution: string | null;
  height: number | null;
  fps: number | null;
  vcodec: string | null;
  acodec: string | null;
  filesize: number | null;
  tbr: number | null;
  formatNote: string | null;
}

export interface PlaylistEntry {
  id: string;
  title: string;
  duration: number | null;
  thumbnail: string | null;
  uploader: string | null;
  url: string;
}

export interface PlaylistInfo {
  id: string;
  title: string;
  uploader: string | null;
  thumbnail: string | null;
  extractor: string | null;
  webpageUrl: string | null;
  count: number;
  entries: PlaylistEntry[];
}

export type ProbeResult =
  | { kind: 'single'; info: VideoInfo }
  | { kind: 'playlist'; info: PlaylistInfo };

export type QualityPreset = 'best' | 'p1080' | 'p720' | 'audio-mp3' | 'audio-opus';

export type SubtitleMode = 'none' | 'embed' | 'separate';

export type SponsorblockMode = 'off' | 'mark' | 'remove';

export type CookiesSource = 'none' | 'browser' | 'file';

export type ConflictMode = 'skip' | 'overwrite';

export type OutputFormat = 'auto' | 'mp4' | 'mkv' | 'webm';

export interface DownloadOptions {
  url: string;
  outputDir: string;
  preset: QualityPreset;

  subtitleLangs?: string[];
  subtitleMode?: SubtitleMode;
  autoGenSubs?: boolean;
  sponsorblock?: SponsorblockMode;

  cookiesSource?: CookiesSource;
  cookiesBrowser?: string | null;
  cookiesFile?: string | null;

  rateLimit?: string | null;
  retries?: number | null;
  fragmentRetries?: number | null;

  outputTemplate?: string | null;
  conflictMode?: ConflictMode;

  embedThumbnail?: boolean;
  embedMetadata?: boolean;
  embedChapters?: boolean;

  outputFormat?: OutputFormat;
  downloadArchive?: string | null;

  /** Exact -f selector from the format browser; overrides the preset. */
  formatSelector?: string | null;
}

export type DownloadStatusKind =
  | 'queued'
  | 'downloading'
  | 'paused'
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
