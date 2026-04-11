//! yt-dlp + ffmpeg installer.
//!
//! - yt-dlp: fetches the latest Windows asset from the official GitHub
//!   releases API and drops it into `<app_local_data>/bin/yt-dlp.exe`.
//! - ffmpeg: downloads the "essentials" build from gyan.dev, extracts the
//!   zip in memory, and writes only `ffmpeg.exe` + `ffprobe.exe` to the
//!   same bin dir.
//!
//! Progress is streamed to the frontend via the `installer://progress`
//! event so the first-run wizard can render a proper progress bar.

use crate::paths;
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

const USER_AGENT: &str = concat!("VidFetch/", env!("CARGO_PKG_VERSION"));

const YTDLP_LATEST_API: &str =
    "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";

// BtbN's builds are the variant officially recommended by yt-dlp and are
// hosted on GitHub's CDN, which is dramatically faster than gyan.dev for
// most users. The `latest` tag is a rolling nightly of ffmpeg master.
const FFMPEG_WINDOWS_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub target: &'static str, // "ytdlp" | "ffmpeg"
    pub phase: String,        // "fetching" | "downloading" | "extracting" | "done"
    pub downloaded: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}

impl InstallProgress {
    fn emit(&self, app: &AppHandle) {
        let _ = app.emit("installer://progress", self.clone());
    }
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/* ============================ yt-dlp ============================ */

pub async fn install_ytdlp(app: &AppHandle) -> Result<PathBuf> {
    let target = "ytdlp";
    emit_phase(app, target, "fetching", None, "Fetching latest release info");

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    let release: GhRelease = client
        .get(YTDLP_LATEST_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("fetching yt-dlp release metadata")?
        .error_for_status()?
        .json()
        .await
        .context("parsing yt-dlp release metadata")?;

    let asset_name = if cfg!(windows) {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else {
        "yt-dlp"
    };

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| anyhow!("yt-dlp release is missing asset '{asset_name}'"))?;

    let dest = paths::ytdlp_path(app)?;
    paths::ensure_app_dirs(app)?;

    download_to_file(
        app,
        target,
        &client,
        &asset.browser_download_url,
        &dest,
        Some(format!(
            "Downloading yt-dlp {}",
            release.tag_name.trim_start_matches('v')
        )),
    )
    .await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }

    emit_phase(app, target, "done", None, "yt-dlp installed");
    Ok(dest)
}

pub async fn read_ytdlp_version(app: &AppHandle) -> Result<String> {
    let bin = paths::ytdlp_path(app)?;
    if !bin.exists() {
        anyhow::bail!("yt-dlp not installed");
    }
    let out = Command::new(&bin).arg("--version").output().await?;
    if !out.status.success() {
        anyhow::bail!(
            "yt-dlp --version exited with {:?}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/* ============================ ffmpeg ============================ */

pub async fn install_ffmpeg(app: &AppHandle) -> Result<PathBuf> {
    let target = "ffmpeg";

    if !cfg!(windows) {
        anyhow::bail!("automatic ffmpeg install only supports Windows in v1");
    }

    emit_phase(
        app,
        target,
        "fetching",
        None,
        "Fetching ffmpeg essentials build",
    );

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    // Download the whole zip into memory (~40 MB) — simpler than streaming
    // to disk and back, and memory pressure is negligible on desktops.
    let bytes = download_to_memory(
        app,
        target,
        &client,
        FFMPEG_WINDOWS_URL,
        Some("Downloading ffmpeg".into()),
    )
    .await?;

    emit_phase(app, target, "extracting", None, "Extracting ffmpeg binaries");

    let dest_dir = paths::bin_dir(app)?;
    paths::ensure_app_dirs(app)?;

    let ffmpeg_dest = paths::ffmpeg_path(app)?;
    let ffprobe_dest = paths::ffprobe_path(app)?;

    // zip crate is sync — run the extract on a blocking thread
    let dest_dir_clone = dest_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let reader = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader)
            .context("opening ffmpeg zip archive")?;

        let wanted = ["ffmpeg.exe", "ffprobe.exe"];
        let mut found = 0usize;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = match file.enclosed_name() {
                Some(p) => p.to_path_buf(),
                None => continue,
            };

            let leaf = name
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();

            if !wanted.contains(&leaf.as_str()) {
                continue;
            }

            let out_path = dest_dir_clone.join(&leaf);
            let mut out = std::fs::File::create(&out_path)
                .with_context(|| format!("creating {}", out_path.display()))?;

            let mut buf = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buf)?;
            out.write_all(&buf)?;

            found += 1;
            if found == wanted.len() {
                break;
            }
        }

        if found < wanted.len() {
            anyhow::bail!("ffmpeg zip did not contain expected binaries");
        }
        Ok(())
    })
    .await
    .context("ffmpeg extract task panicked")??;

    if !ffmpeg_dest.exists() || !ffprobe_dest.exists() {
        anyhow::bail!("ffmpeg extraction finished but binaries are missing on disk");
    }

    emit_phase(app, target, "done", None, "ffmpeg installed");
    Ok(ffmpeg_dest)
}

pub async fn read_ffmpeg_version(app: &AppHandle) -> Result<String> {
    let bin = paths::ffmpeg_path(app)?;
    if !bin.exists() {
        anyhow::bail!("ffmpeg not installed");
    }
    let out = Command::new(&bin).arg("-version").output().await?;
    if !out.status.success() {
        anyhow::bail!("ffmpeg -version exited with {:?}", out.status);
    }
    let first_line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    // Typical format: "ffmpeg version 7.0.2-essentials_build-www.gyan.dev ..."
    let version = first_line
        .split_whitespace()
        .nth(2)
        .unwrap_or("unknown")
        .to_string();
    Ok(version)
}

/* ============================ helpers ============================ */

fn emit_phase(
    app: &AppHandle,
    target: &'static str,
    phase: &str,
    total: Option<u64>,
    message: &str,
) {
    InstallProgress {
        target,
        phase: phase.to_string(),
        downloaded: 0,
        total,
        message: Some(message.to_string()),
    }
    .emit(app);
}

async fn download_to_file(
    app: &AppHandle,
    target: &'static str,
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    message: Option<String>,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?;

    let total = resp.content_length();
    let mut stream = resp.bytes_stream();

    // Write to <dest>.part, then atomically rename.
    let tmp = dest.with_extension(format!(
        "{}part",
        dest.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()
    ));
    let mut file = std::fs::File::create(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;

    let mut downloaded: u64 = 0;
    let mut last_emitted: u64 = 0;

    InstallProgress {
        target,
        phase: "downloading".into(),
        downloaded: 0,
        total,
        message: message.clone(),
    }
    .emit(app);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("reading download stream")?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        // Throttle events to ~every 64 KiB to keep IPC chatter sane.
        if downloaded - last_emitted >= 64 * 1024 {
            last_emitted = downloaded;
            InstallProgress {
                target,
                phase: "downloading".into(),
                downloaded,
                total,
                message: message.clone(),
            }
            .emit(app);
        }
    }

    file.flush()?;
    drop(file);

    if dest.exists() {
        std::fs::remove_file(dest).ok();
    }
    std::fs::rename(&tmp, dest)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), dest.display()))?;

    Ok(())
}

async fn download_to_memory(
    app: &AppHandle,
    target: &'static str,
    client: &reqwest::Client,
    url: &str,
    message: Option<String>,
) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?;

    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(total.unwrap_or(0) as usize);

    let mut downloaded: u64 = 0;
    let mut last_emitted: u64 = 0;

    InstallProgress {
        target,
        phase: "downloading".into(),
        downloaded: 0,
        total,
        message: message.clone(),
    }
    .emit(app);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("reading download stream")?;
        buf.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;

        if downloaded - last_emitted >= 256 * 1024 {
            last_emitted = downloaded;
            InstallProgress {
                target,
                phase: "downloading".into(),
                downloaded,
                total,
                message: message.clone(),
            }
            .emit(app);
        }
    }

    Ok(buf)
}
