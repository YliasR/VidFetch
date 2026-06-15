mod commands;
mod paths;
mod state;
mod ytdlp;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            app.manage(AppState::default());
            paths::ensure_app_dirs(&app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ytdlp::check_binaries,
            commands::ytdlp::install_ytdlp,
            commands::ytdlp::install_ffmpeg,
            commands::ytdlp::get_versions,
            commands::probe::probe_url,
            commands::download::start_download,
            commands::download::cancel_download,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::files::read_dropped_text,
            commands::edit::probe_media,
            commands::edit::export_gif,
            commands::edit::append_to_gif,
            commands::edit::list_keyframes,
            commands::edit::thumbnail_at,
            commands::edit::trim_video,
            commands::edit::trim_multi,
            commands::edit::cancel_export,
            commands::updater::check_app_update,
            commands::updater::install_app_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
