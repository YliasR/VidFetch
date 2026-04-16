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

    #[serde(default)]
    pub cookies_source: CookiesSource,
    #[serde(default)]
    pub cookies_browser: Option<String>,
    #[serde(default)]
    pub cookies_file: Option<String>,

    #[serde(default)]
    pub rate_limit: Option<String>,
    #[serde(default)]
    pub retries: Option<u32>,
    #[serde(default)]
    pub fragment_retries: Option<u32>,

    #[serde(default)]
    pub output_template: Option<String>,
    #[serde(default)]
    pub conflict_mode: ConflictMode,

    #[serde(default = "default_true")]
    pub embed_thumbnail: bool,
    #[serde(default = "default_true")]
    pub embed_metadata: bool,
    #[serde(default)]
    pub embed_chapters: bool,

    #[serde(default)]
    pub output_format: OutputFormat,
}

fn default_true() -> bool {
    true
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
    fn height_filter(self) -> &'static str {
        match self {
            Self::P1080 => "[height<=1080]",
            Self::P720 => "[height<=720]",
            _ => "",
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

/// Build the `-f` format selector, biasing codec choice so the MP4 and
/// WebM containers actually accept the streams yt-dlp picks.
fn build_format_selector(preset: QualityPreset, fmt: OutputFormat) -> String {
    if preset.is_audio_only() {
        return "ba/b".into();
    }
    let hf = preset.height_filter();
    match fmt {
        // H.264 + AAC → mp4-native, embeds in Discord / iOS / legacy players.
        OutputFormat::Mp4 => format!(
            "bv*{hf}[vcodec^=avc1]+ba[acodec^=mp4a]/bv*{hf}+ba/b{hf}"
        ),
        // VP9 + Opus → webm-native.
        OutputFormat::Webm => format!(
            "bv*{hf}[vcodec^=vp9]+ba[acodec^=opus]/bv*{hf}+ba/b{hf}"
        ),
        // Auto and MKV: just take the best available — MKV accepts anything.
        OutputFormat::Auto | OutputFormat::Mkv => format!("bv*{hf}+ba/b{hf}"),
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CookiesSource {
    #[default]
    None,
    Browser,
    File,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Let yt-dlp pick — whatever the source provides, merged as needed.
    #[default]
    Auto,
    /// MP4 container. Best for compatibility (Discord embeds, iOS, older players).
    Mp4,
    /// Matroska container. Accepts any codec combination without re-encoding.
    Mkv,
    /// WebM container. Smaller files, VP9/Opus-friendly, no Safari support.
    Webm,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictMode {
    /// Don't re-download files that already exist (yt-dlp default).
    #[default]
    Skip,
    /// Replace the existing file.
    Overwrite,
}

/// The pipe-separated prefix yt-dlp uses for our parseable progress lines.
pub const PROGRESS_PREFIX: &str = "VFPROG";

/// Default yt-dlp output template — mirrors what the frontend shows as the
/// "reset to default" value for the template editor.
pub const DEFAULT_OUTPUT_TEMPLATE: &str = "%(title)s.%(ext)s";

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
    let template = opts
        .output_template
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_OUTPUT_TEMPLATE);
    args.push(template.into());

    // File conflict behavior.
    match opts.conflict_mode {
        ConflictMode::Skip => args.push("--no-overwrites".into()),
        ConflictMode::Overwrite => args.push("--force-overwrites".into()),
    }

    // Format selection.
    args.push("-f".into());
    args.push(build_format_selector(opts.preset, opts.output_format));

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

    // Output container (only meaningful for video presets).
    if !opts.preset.is_audio_only() {
        match opts.output_format {
            OutputFormat::Auto => {}
            OutputFormat::Mp4 => {
                args.push("--merge-output-format".into());
                args.push("mp4".into());
            }
            OutputFormat::Mkv => {
                args.push("--merge-output-format".into());
                args.push("mkv".into());
            }
            OutputFormat::Webm => {
                args.push("--merge-output-format".into());
                args.push("webm".into());
            }
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

    // Cookies.
    match opts.cookies_source {
        CookiesSource::None => {}
        CookiesSource::Browser => {
            if let Some(browser) = opts.cookies_browser.as_deref().filter(|s| !s.is_empty()) {
                args.push("--cookies-from-browser".into());
                args.push(browser.into());
            }
        }
        CookiesSource::File => {
            if let Some(path) = opts.cookies_file.as_deref().filter(|s| !s.is_empty()) {
                args.push("--cookies".into());
                args.push(path.into());
            }
        }
    }

    // Network knobs.
    if let Some(rate) = opts.rate_limit.as_deref().filter(|s| !s.is_empty()) {
        args.push("--limit-rate".into());
        args.push(rate.into());
    }
    if let Some(n) = opts.retries {
        args.push("--retries".into());
        args.push(n.to_string());
    }
    if let Some(n) = opts.fragment_retries {
        args.push("--fragment-retries".into());
        args.push(n.to_string());
    }

    // Embeds — thumbnail/metadata need ffmpeg (we bundle it).
    if opts.embed_thumbnail {
        args.push("--embed-thumbnail".into());
    }
    if opts.embed_metadata {
        args.push("--embed-metadata".into());
    }
    if opts.embed_chapters {
        args.push("--embed-chapters".into());
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
