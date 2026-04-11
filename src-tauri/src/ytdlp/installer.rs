//! yt-dlp and ffmpeg installer. Real implementation lands in Phase 2 —
//! this file currently exposes the public surface so the rest of the
//! backend compiles.

use std::path::PathBuf;
use tauri::AppHandle;

pub async fn install_ytdlp(_app: &AppHandle) -> anyhow::Result<PathBuf> {
    anyhow::bail!("install_ytdlp not yet implemented")
}

pub async fn install_ffmpeg(_app: &AppHandle) -> anyhow::Result<PathBuf> {
    anyhow::bail!("install_ffmpeg not yet implemented")
}

pub async fn read_ytdlp_version(_app: &AppHandle) -> anyhow::Result<String> {
    anyhow::bail!("read_ytdlp_version not yet implemented")
}

pub async fn read_ffmpeg_version(_app: &AppHandle) -> anyhow::Result<String> {
    anyhow::bail!("read_ffmpeg_version not yet implemented")
}
