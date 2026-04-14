//! Translate a `DownloadOptions` struct from the frontend into a
//! concrete yt-dlp CLI argument vector.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    pub url: String,
    pub output_dir: String,
    pub preset: QualityPreset,

    #[serde(default)]
    pub subtitle_langs: Vec<String>,
    #[serde(default)]
    pub subtitle_mode: SubtitleMode,
    #[serde(default)]
    pub auto_gen_subs: bool,
    #[serde(default)]
    pub sponsorblock: SponsorblockMode,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SubtitleMode {
    #[default]
    None,
    Embed,
    Separate,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SponsorblockMode {
    #[default]
    Off,
    Mark,
    Remove,
}

/// The pipe-separated prefix yt-dlp uses for our parseable progress lines.
pub const PROGRESS_PREFIX: &str = "VFPROG";

pub fn build_args(opts: &DownloadOptions, ffmpeg_path: &Path) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // Bundled ffmpeg — skip the PATH scan.
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

    // Subtitles — skip for audio-only (no video container to embed into).
    if !opts.preset.is_audio_only()
        && opts.subtitle_mode != SubtitleMode::None
        && !opts.subtitle_langs.is_empty()
    {
        args.push("--write-subs".into());
        if opts.auto_gen_subs {
            args.push("--write-auto-subs".into());
        }
        args.push("--sub-langs".into());
        args.push(opts.subtitle_langs.join(","));
        if opts.subtitle_mode == SubtitleMode::Embed {
            args.push("--embed-subs".into());
        }
    }

    // SponsorBlock.
    match opts.sponsorblock {
        SponsorblockMode::Off => {}
        SponsorblockMode::Mark => {
            args.push("--sponsorblock-mark".into());
            args.push("all".into());
        }
        SponsorblockMode::Remove => {
            args.push("--sponsorblock-remove".into());
            args.push("all".into());
        }
    }

    // Progress streaming.
    args.push("--newline".into());
    args.push("--progress".into());
    args.push("--no-colors".into());
    args.push("--progress-template".into());
    args.push(format!(
        "download:{PROGRESS_PREFIX}|%(progress.downloaded_bytes)s|%(progress.total_bytes,progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s"
    ));

    args.push("--no-warnings".into());
    // Downloads always target a single item — playlists are exploded into
    // individual queue entries at the frontend.
    args.push("--no-playlist".into());

    args.push(opts.url.clone());

    args
}
