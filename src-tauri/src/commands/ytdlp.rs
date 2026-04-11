use crate::paths;
use crate::ytdlp::installer;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
pub struct BinariesStatus {
    pub ytdlp: bool,
    pub ffmpeg: bool,
    pub ffprobe: bool,
}

#[derive(Serialize)]
pub struct Versions {
    pub ytdlp: Option<String>,
    pub ffmpeg: Option<String>,
}

#[tauri::command]
pub async fn check_binaries(app: AppHandle) -> Result<BinariesStatus, String> {
    let ytdlp = paths::ytdlp_path(&app).map_err(|e| e.to_string())?.exists();
    let ffmpeg = paths::ffmpeg_path(&app).map_err(|e| e.to_string())?.exists();
    let ffprobe = paths::ffprobe_path(&app).map_err(|e| e.to_string())?.exists();
    Ok(BinariesStatus { ytdlp, ffmpeg, ffprobe })
}

#[tauri::command]
pub async fn install_ytdlp(app: AppHandle) -> Result<String, String> {
    installer::install_ytdlp(&app)
        .await
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_ffmpeg(app: AppHandle) -> Result<String, String> {
    installer::install_ffmpeg(&app)
        .await
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_versions(app: AppHandle) -> Result<Versions, String> {
    Ok(Versions {
        ytdlp: installer::read_ytdlp_version(&app).await.ok(),
        ffmpeg: installer::read_ffmpeg_version(&app).await.ok(),
    })
}
