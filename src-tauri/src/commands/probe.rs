use crate::paths;
use serde::Serialize;
use serde_json::Value;
use std::process::Stdio;
use tauri::AppHandle;
use tokio::process::Command;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProbeResult {
    Single { info: VideoInfo },
    Playlist { info: PlaylistInfo },
}

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
    pub available_subs: Vec<String>,
    pub available_auto_subs: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInfo {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub thumbnail: Option<String>,
    pub extractor: Option<String>,
    pub webpage_url: Option<String>,
    pub count: usize,
    pub entries: Vec<PlaylistEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
    pub url: String,
}

#[tauri::command]
pub async fn probe_url(app: AppHandle, url: String) -> Result<ProbeResult, String> {
    let ytdlp = paths::ytdlp_path(&app).map_err(|e| e.to_string())?;
    if !ytdlp.exists() {
        return Err("yt-dlp not installed".into());
    }

    let mut cmd = Command::new(&ytdlp);
    // --flat-playlist keeps playlist probing cheap; for single videos it's a no-op.
    cmd.arg("-J")
        .arg("--no-warnings")
        .arg("--flat-playlist")
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

    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse yt-dlp JSON: {e}"))?;

    if json.get("_type").and_then(|v| v.as_str()) == Some("playlist") {
        Ok(ProbeResult::Playlist {
            info: parse_playlist(&json),
        })
    } else {
        Ok(ProbeResult::Single {
            info: parse_video(&json),
        })
    }
}

fn parse_video(json: &Value) -> VideoInfo {
    VideoInfo {
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
        thumbnail: pick_thumbnail(json),
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
        available_subs: subtitle_langs(json, "subtitles"),
        available_auto_subs: subtitle_langs(json, "automatic_captions"),
    }
}

fn parse_playlist(json: &Value) -> PlaylistInfo {
    let entries_val = json.get("entries").and_then(|v| v.as_array());
    let entries: Vec<PlaylistEntry> = entries_val
        .map(|arr| arr.iter().filter_map(parse_entry).collect())
        .unwrap_or_default();
    let count = json
        .get("playlist_count")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(entries.len());

    PlaylistInfo {
        id: json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        title: json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(playlist)")
            .to_string(),
        uploader: json
            .get("uploader")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("channel").and_then(|v| v.as_str()))
            .map(String::from),
        thumbnail: pick_thumbnail(json),
        extractor: json
            .get("extractor_key")
            .or_else(|| json.get("extractor"))
            .and_then(|v| v.as_str())
            .map(String::from),
        webpage_url: json
            .get("webpage_url")
            .and_then(|v| v.as_str())
            .map(String::from),
        count,
        entries,
    }
}

fn parse_entry(v: &Value) -> Option<PlaylistEntry> {
    // Skip unplayable entries (privated/removed videos come back with null).
    if v.is_null() {
        return None;
    }
    let url = v
        .get("url")
        .and_then(|u| u.as_str())
        .or_else(|| v.get("webpage_url").and_then(|u| u.as_str()))?
        .to_string();
    Some(PlaylistEntry {
        id: v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        title: v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("(untitled)")
            .to_string(),
        duration: v.get("duration").and_then(|x| x.as_f64()),
        thumbnail: pick_thumbnail(v),
        uploader: v
            .get("uploader")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("channel").and_then(|x| x.as_str()))
            .map(String::from),
        url,
    })
}

fn pick_thumbnail(v: &Value) -> Option<String> {
    if let Some(t) = v.get("thumbnail").and_then(|x| x.as_str()) {
        return Some(t.to_string());
    }
    // Fall back to the last (usually highest-res) entry in `thumbnails`.
    v.get("thumbnails")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.last())
        .and_then(|last| last.get("url"))
        .and_then(|u| u.as_str())
        .map(String::from)
}

fn subtitle_langs(json: &Value, key: &str) -> Vec<String> {
    json.get(key)
        .and_then(|v| v.as_object())
        .map(|m| {
            let mut langs: Vec<String> = m.keys().cloned().collect();
            langs.sort();
            langs
        })
        .unwrap_or_default()
}
