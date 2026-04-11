use crate::paths;
use serde::Serialize;
use std::process::Stdio;
use tauri::AppHandle;
use tokio::process::Command;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub extractor: Option<String>,
    pub webpage_url: Option<String>,
    pub is_live: Option<bool>,
}

#[tauri::command]
pub async fn probe_url(app: AppHandle, url: String) -> Result<VideoInfo, String> {
    let ytdlp = paths::ytdlp_path(&app).map_err(|e| e.to_string())?;
    if !ytdlp.exists() {
        return Err("yt-dlp not installed".into());
    }

    let mut cmd = Command::new(&ytdlp);
    cmd.arg("-J")
        .arg("--no-warnings")
        .arg("--no-playlist")
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("failed to spawn yt-dlp: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "yt-dlp -J failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse yt-dlp JSON: {e}"))?;

    Ok(VideoInfo {
        id: json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        title: json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(untitled)")
            .to_string(),
        uploader: json
            .get("uploader")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("channel").and_then(|v| v.as_str()))
            .map(String::from),
        duration: json.get("duration").and_then(|v| v.as_f64()),
        thumbnail: json
            .get("thumbnail")
            .and_then(|v| v.as_str())
            .map(String::from),
        extractor: json
            .get("extractor_key")
            .or_else(|| json.get("extractor"))
            .and_then(|v| v.as_str())
            .map(String::from),
        webpage_url: json
            .get("webpage_url")
            .and_then(|v| v.as_str())
            .map(String::from),
        is_live: json.get("is_live").and_then(|v| v.as_bool()),
    })
}
