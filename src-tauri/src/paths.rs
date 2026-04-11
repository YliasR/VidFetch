use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub fn app_data_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_local_data_dir()?)
}

pub fn bin_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app_data_dir(app)?.join("bin"))
}

pub fn ytdlp_path(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let name = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
    Ok(bin_dir(app)?.join(name))
}

pub fn ffmpeg_path(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    Ok(bin_dir(app)?.join(name))
}

pub fn ffprobe_path(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    Ok(bin_dir(app)?.join(name))
}

pub fn ensure_app_dirs(app: &AppHandle) -> anyhow::Result<()> {
    std::fs::create_dir_all(bin_dir(app)?)?;
    Ok(())
}
