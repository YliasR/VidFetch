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
    pub formats: Vec<FormatInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatInfo {
    pub format_id: String,
    pub ext: String,
    pub resolution: Option<String>,
    pub height: Option<u64>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub filesize: Option<u64>,
    pub tbr: Option<f64>,
    pub format_note: Option<String>,
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
    crate::ytdlp::hide_console(&mut cmd);

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
        formats: parse_formats(json),
    }
}

fn parse_formats(json: &Value) -> Vec<FormatInfo> {
    let Some(arr) = json.get("formats").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|f| {
            let format_id = f.get("format_id").and_then(|v| v.as_str())?.to_string();
            let ext = f
                .get("ext")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let vcodec = codec_field(f, "vcodec");
            let acodec = codec_field(f, "acodec");
            // Storyboards and other non-media pseudo-formats have no codecs.
            if ext == "mhtml" || (vcodec.is_none() && acodec.is_none()) {
                return None;
            }
            Some(FormatInfo {
                format_id,
                ext,
                resolution: f
                    .get("resolution")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                height: f.get("height").and_then(|v| v.as_u64()),
                fps: f.get("fps").and_then(|v| v.as_f64()),
                vcodec,
                acodec,
                filesize: f
                    .get("filesize")
                    .and_then(|v| v.as_u64())
                    .or_else(|| f.get("filesize_approx").and_then(|v| v.as_u64())),
                tbr: f.get("tbr").and_then(|v| v.as_f64()),
                format_note: f
                    .get("format_note")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
        })
        .collect()
}

/// Normalize yt-dlp codec fields: the literal string "none" means absent.
fn codec_field(f: &Value, key: &str) -> Option<String> {
    f.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "none")
        .map(String::from)
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
