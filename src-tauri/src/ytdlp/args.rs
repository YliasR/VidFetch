//! Translate a `DownloadOptions` struct from the frontend into a
//! concrete yt-dlp CLI argument vector.
//!
//! For 0.1 alpha we support quick quality presets only. The full format
//! browser and advanced toggles (subs, SponsorBlock, cookies, …) land in
//! later phases and plug into this same builder.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    pub url: String,
    pub output_dir: String,
    pub preset: QualityPreset,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QualityPreset {
    Best,
    P1080,
    P720,
    AudioMp3,
    AudioOpus,
}

impl QualityPreset {
    fn format_selector(self) -> &'static str {
        match self {
            Self::Best => "bv*+ba/b",
            Self::P1080 => "bv*[height<=1080]+ba/b[height<=1080]",
            Self::P720 => "bv*[height<=720]+ba/b[height<=720]",
            // Audio variants use -x, so format selector just grabs the best audio.
            Self::AudioMp3 | Self::AudioOpus => "ba/b",
        }
    }

    fn is_audio_only(self) -> bool {
        matches!(self, Self::AudioMp3 | Self::AudioOpus)
    }

    fn audio_codec(self) -> Option<&'static str> {
        match self {
            Self::AudioMp3 => Some("mp3"),
            Self::AudioOpus => Some("opus"),
            _ => None,
        }
    }
}

/// The pipe-separated prefix yt-dlp uses for our parseable progress lines.
pub const PROGRESS_PREFIX: &str = "VFPROG";

/// Build the CLI args for a download. `ffmpeg_path` is the absolute path
/// to our bundled ffmpeg binary (directory is passed to `--ffmpeg-location`).
pub fn build_args(opts: &DownloadOptions, ffmpeg_path: &Path) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // Point yt-dlp at our bundled ffmpeg — otherwise it scans PATH.
    if let Some(parent) = ffmpeg_path.parent() {
        args.push("--ffmpeg-location".into());
        args.push(parent.display().to_string());
    }

    // Output template + directory.
    args.push("-P".into());
    args.push(opts.output_dir.clone());
    args.push("-o".into());
    args.push("%(title)s.%(ext)s".into());

    // Format selection.
    args.push("-f".into());
    args.push(opts.preset.format_selector().into());

    // Audio-only post-processing.
    if opts.preset.is_audio_only() {
        args.push("-x".into());
        if let Some(codec) = opts.preset.audio_codec() {
            args.push("--audio-format".into());
            args.push(codec.into());
            args.push("--audio-quality".into());
            args.push("0".into());
        }
    }

    // Progress streaming — one line per tick, pipe-delimited, easy to parse.
    args.push("--newline".into());
    args.push("--progress".into());
    args.push("--no-colors".into());
    args.push("--progress-template".into());
    args.push(format!(
        "download:{PROGRESS_PREFIX}|%(progress.downloaded_bytes)s|%(progress.total_bytes,progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s"
    ));

    // Quieter output — we still get progress lines via --progress.
    args.push("--no-warnings".into());
    args.push("--no-playlist".into());

    // The URL always goes last.
    args.push(opts.url.clone());

    args
}
