use crate::ytdlp::args::{DownloadOptions, OutputFormat};
use crate::ytdlp::runner;
use tauri::AppHandle;

#[tauri::command]
pub async fn start_download(app: AppHandle, options: DownloadOptions) -> Result<String, String> {
    // Check if this is a GIF download - use special runner
    if options.output_format == OutputFormat::Gif {
        runner::spawn_gif_download(app, options).map_err(|e| e.to_string())
    } else {
        runner::spawn_download(app, options).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn cancel_download(app: AppHandle, id: String) -> Result<bool, String> {
    runner::cancel(&app, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_download(app: AppHandle, id: String) -> Result<bool, String> {
    runner::pause(&app, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_download(app: AppHandle, id: String) -> Result<bool, String> {
    runner::resume(&app, &id).map_err(|e| e.to_string())
}
