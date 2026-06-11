//! Channel-aware update checks. The config endpoint covers the stable
//! channel; nightly opt-in swaps the endpoint at runtime for the rolling
//! `nightly` prerelease.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

const STABLE_ENDPOINT: &str =
    "https://github.com/YliasR/VidFetch/releases/latest/download/latest.json";
const NIGHTLY_ENDPOINT: &str =
    "https://github.com/YliasR/VidFetch/releases/download/nightly/latest.json";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

fn endpoint_for(channel: &str) -> &'static str {
    if channel == "nightly" {
        NIGHTLY_ENDPOINT
    } else {
        STABLE_ENDPOINT
    }
}

async fn check_on_channel(
    app: &AppHandle,
    channel: &str,
) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let endpoint = endpoint_for(channel)
        .parse()
        .map_err(|e| format!("bad endpoint: {e}"))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    channel: String,
) -> Result<Option<UpdateInfo>, String> {
    let update = check_on_channel(&app, &channel).await?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version.clone(),
        body: u.body.clone(),
        date: u.date.map(|d| d.to_string()),
    }))
}

/// Download and install the update found on the given channel, streaming
/// progress as `updater://progress` events. The app must relaunch after.
#[tauri::command]
pub async fn install_app_update(app: AppHandle, channel: String) -> Result<(), String> {
    let Some(update) = check_on_channel(&app, &channel).await? else {
        return Err("no update available on this channel".into());
    };

    let progress_app = app.clone();
    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit(
                    "updater://progress",
                    UpdateProgress { downloaded, total },
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
