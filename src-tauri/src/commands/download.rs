use crate::ytdlp::args::DownloadOptions;
use crate::ytdlp::runner;
use tauri::AppHandle;

#[tauri::command]
pub async fn start_download(app: AppHandle, options: DownloadOptions) -> Result<String, String> {
    runner::spawn_download(app, options).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_download(app: AppHandle, id: String) -> Result<bool, String> {
    runner::cancel(&app, &id).map_err(|e| e.to_string())
}
