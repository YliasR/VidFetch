//! yt-dlp + ffmpeg installer.
//!
//! - yt-dlp: fetches the standalone build for the current platform from the
//!   official GitHub releases API and drops it into `<app_local_data>/bin/`.
//! - ffmpeg: downloads a static build for the current platform, extracts the
//!   archive in memory, and writes only the ffmpeg + ffprobe binaries to the
//!   same bin dir. Sources: BtbN (Windows/Linux), martin-riedl.de (macOS).
//!
//! Progress is streamed to the frontend via the `installer://progress`
//! event so the first-run wizard can render a proper progress bar.

use crate::paths;
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};

#[cfg(any(windows, target_os = "macos"))]
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

const USER_AGENT: &str = concat!("VidFetch/", env!("CARGO_PKG_VERSION"));

const YTDLP_LATEST_API: &str =
    "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";

// BtbN's builds are the variant officially recommended by yt-dlp and are
// hosted on GitHub's CDN, which is dramatically faster than gyan.dev for
// most users. The `latest` tag is a rolling nightly of ffmpeg master.
#[cfg(windows)]
const FFMPEG_WINDOWS_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

#[cfg(target_os = "linux")]
const FFMPEG_LINUX_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz";

// martin-riedl.de ships current, signed macOS builds for both architectures
// and is one of the sources listed on ffmpeg.org. The redirect endpoint is
// load-balanced and one mirror intermittently 404s; `get_with_retry`
// papers over that.
#[cfg(target_os = "macos")]
const FFMPEG_MACOS_BASE: &str = "https://ffmpeg.martin-riedl.de/redirect/latest/macos";

#[cfg(target_os = "macos")]
const FFMPEG_MACOS_ARCH: &str = if cfg!(target_arch = "aarch64") {
    "arm64"
} else {
    "amd64"
};

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
        // Standalone PyInstaller build — no system Python required.
        "yt-dlp_linux"
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

    set_executable(&dest)?;

    emit_phase(app, target, "done", None, "yt-dlp installed");
    Ok(dest)
}

pub async fn read_ytdlp_version(app: &AppHandle) -> Result<String> {
    let bin = paths::ytdlp_path(app)?;
    if !bin.exists() {
        anyhow::bail!("yt-dlp not installed");
    }
    let mut cmd = Command::new(&bin);
    cmd.arg("--version");
    super::hide_console(&mut cmd);
    let out = cmd.output().await?;
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

    emit_phase(app, target, "fetching", None, "Fetching ffmpeg build");

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    paths::ensure_app_dirs(app)?;

    #[cfg(windows)]
    install_ffmpeg_windows(app, target, &client).await?;
    #[cfg(target_os = "macos")]
    install_ffmpeg_macos(app, target, &client).await?;
    #[cfg(target_os = "linux")]
    install_ffmpeg_linux(app, target, &client).await?;

    let ffmpeg_dest = paths::ffmpeg_path(app)?;
    let ffprobe_dest = paths::ffprobe_path(app)?;

    if !ffmpeg_dest.exists() || !ffprobe_dest.exists() {
        anyhow::bail!("ffmpeg extraction finished but binaries are missing on disk");
    }

    set_executable(&ffmpeg_dest)?;
    set_executable(&ffprobe_dest)?;

    emit_phase(app, target, "done", None, "ffmpeg installed");
    Ok(ffmpeg_dest)
}

#[cfg(windows)]
async fn install_ffmpeg_windows(
    app: &AppHandle,
    target: &'static str,
    client: &reqwest::Client,
) -> Result<()> {
    // Download the whole zip into memory (~40 MB) — simpler than streaming
    // to disk and back, and memory pressure is negligible on desktops.
    let bytes = download_to_memory(
        app,
        target,
        client,
        FFMPEG_WINDOWS_URL,
        Some("Downloading ffmpeg".into()),
    )
    .await?;

    emit_phase(app, target, "extracting", None, "Extracting ffmpeg binaries");

    let dest_dir = paths::bin_dir(app)?;
    extract_zip_binaries(bytes, dest_dir, &["ffmpeg.exe", "ffprobe.exe"]).await
}

#[cfg(target_os = "macos")]
async fn install_ffmpeg_macos(
    app: &AppHandle,
    target: &'static str,
    client: &reqwest::Client,
) -> Result<()> {
    // ffmpeg and ffprobe ship as separate single-binary zips.
    for bin in ["ffmpeg", "ffprobe"] {
        let url = format!("{FFMPEG_MACOS_BASE}/{FFMPEG_MACOS_ARCH}/release/{bin}.zip");
        let bytes = download_to_memory(
            app,
            target,
            client,
            &url,
            Some(format!("Downloading {bin}")),
        )
        .await?;

        emit_phase(app, target, "extracting", None, "Extracting ffmpeg binaries");

        let dest_dir = paths::bin_dir(app)?;
        extract_zip_binaries(bytes, dest_dir, &[bin]).await?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn install_ffmpeg_linux(
    app: &AppHandle,
    target: &'static str,
    client: &reqwest::Client,
) -> Result<()> {
    let bytes = download_to_memory(
        app,
        target,
        client,
        FFMPEG_LINUX_URL,
        Some("Downloading ffmpeg".into()),
    )
    .await?;

    emit_phase(app, target, "extracting", None, "Extracting ffmpeg binaries");

    let dest_dir = paths::bin_dir(app)?;

    tokio::task::spawn_blocking(move || -> Result<()> {
        let decoder = xz2::read::XzDecoder::new(Cursor::new(bytes));
        let mut archive = tar::Archive::new(decoder);

        let wanted = ["ffmpeg", "ffprobe"];
        let mut found = 0usize;

        for entry in archive.entries().context("reading ffmpeg tar archive")? {
            let mut entry = entry?;
            let leaf = entry
                .path()?
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();

            if !wanted.contains(&leaf.as_str()) || !entry.header().entry_type().is_file() {
                continue;
            }

            let out_path = dest_dir.join(&leaf);
            let mut out = std::fs::File::create(&out_path)
                .with_context(|| format!("creating {}", out_path.display()))?;
            std::io::copy(&mut entry, &mut out)?;

            found += 1;
            if found == wanted.len() {
                break;
            }
        }

        if found < wanted.len() {
            anyhow::bail!("ffmpeg archive did not contain expected binaries");
        }
        Ok(())
    })
    .await
    .context("ffmpeg extract task panicked")??;

    Ok(())
}

/// Extract the named binaries (matched by file name, any directory depth)
/// from a zip held in memory into `dest_dir`.
#[cfg(any(windows, target_os = "macos"))]
async fn extract_zip_binaries(
    bytes: Vec<u8>,
    dest_dir: PathBuf,
    wanted: &[&str],
) -> Result<()> {
    let wanted: Vec<String> = wanted.iter().map(|s| s.to_string()).collect();

    // zip crate is sync — run the extract on a blocking thread
    tokio::task::spawn_blocking(move || -> Result<()> {
        let reader = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader)
            .context("opening ffmpeg zip archive")?;

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

            if !wanted.iter().any(|w| w == &leaf) {
                continue;
            }

            let out_path = dest_dir.join(&leaf);
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

    Ok(())
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub async fn read_ffmpeg_version(app: &AppHandle) -> Result<String> {
    let bin = paths::ffmpeg_path(app)?;
    if !bin.exists() {
        anyhow::bail!("ffmpeg not installed");
    }
    let mut cmd = Command::new(&bin);
    cmd.arg("-version");
    super::hide_console(&mut cmd);
    let out = cmd.output().await?;
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

/// GET with a couple of retries. Mainly for the macOS ffmpeg mirror, where
/// one load-balanced backend intermittently returns 404, but transient
/// network errors on any platform benefit too.
async fn get_with_retry(client: &reqwest::Client, url: &str) -> Result<reqwest::Response> {
    const ATTEMPTS: u32 = 3;
    let mut last_err = anyhow!("unreachable");

    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        }
        match client.get(url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(resp) => return Ok(resp),
                Err(e) => last_err = anyhow!(e),
            },
            Err(e) => last_err = anyhow!(e),
        }
    }

    Err(last_err.context(format!("GET {url} failed after {ATTEMPTS} attempts")))
}

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

    let resp = get_with_retry(client, url).await?;

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
    let resp = get_with_retry(client, url).await?;

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
