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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        // tauri-plugin-updater deferred to v1.1 — needs signing keys + release channel.
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
