//! Edit-tab commands: local media probing and video → GIF export.
//!
//! GIF export runs ffmpeg twice (palettegen, then paletteuse) so the
//! 256-color palette is derived from the actual clip instead of a generic
//! one. Progress is streamed back as Tauri events, mirroring the download
//! runner:
//! - `edit://status`   — lifecycle (queued/palette/encoding/done/error/canceled)
//! - `edit://progress` — percent through the encode pass
//! - `edit://log`      — raw ffmpeg stderr lines for debugging

use crate::paths;
use crate::state::{AppState, JobHandle};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GifExportOptions {
    pub input_path: String,
    pub output_path: String,
    /// Trim start in seconds; 0 / None means from the beginning.
    pub start: Option<f64>,
    /// Trim end in seconds; None means to the end of the source.
    pub end: Option<f64>,
    /// Output width in pixels (height follows aspect ratio). None = source width.
    pub width: Option<u32>,
    pub fps: f64,
    /// One of: sierra2_4a, floyd_steinberg, bayer, none.
    pub dither: String,
    /// GIF loop muxer flag: 0 = forever, -1 = play once, n = loop n extra
    /// times. None leaves ffmpeg's default (forever).
    pub loop_count: Option<i32>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GifAppendOptions {
    /// The base clip (typically an existing GIF) that gets extended.
    pub base_path: String,
    /// The clip whose range is appended to the base.
    pub clip_path: String,
    /// Trim of the appended clip in seconds; None = whole clip.
    pub clip_start: Option<f64>,
    pub clip_end: Option<f64>,
    /// "back" appends after the base, "front" before it.
    pub position: String,
    pub output_path: String,
    /// Output width in pixels; None = base width. Height follows the base
    /// aspect ratio and the appended clip is letterboxed to match.
    pub width: Option<u32>,
    pub fps: f64,
    pub dither: String,
    pub loop_count: Option<i32>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrimOptions {
    pub input_path: String,
    pub output_path: String,
    /// Start in seconds; 0 / None means from the beginning.
    pub start: Option<f64>,
    /// End in seconds; None means to the end of the source.
    pub end: Option<f64>,
    /// Re-encode instead of stream-copy. The frontend sets this when the
    /// start doesn't land on a keyframe (a `-c copy` cut would be inaccurate
    /// there). When false we copy packets for a lossless, near-instant trim.
    pub reencode: bool,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrimRange {
    pub start: Option<f64>,
    pub end: Option<f64>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MultiTrimOptions {
    pub input_path: String,
    pub ranges: Vec<TrimRange>,
    /// "separate" writes one file per range; "concat" joins them into one.
    pub mode: String,
    /// For "concat": the single output file. For "separate": a base path whose
    /// stem is suffixed per range (`clip.mp4` → `clip-1.mp4`, `clip-2.mp4`, …).
    pub output_path: String,
    pub reencode: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditStatus {
    id: String,
    status: &'static str,
    message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditProgress {
    id: String,
    /// 0.0–1.0 through the encode pass.
    fraction: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditLog {
    id: String,
    line: String,
}

/// Probe a local media file with ffprobe.
#[tauri::command]
pub async fn probe_media(app: AppHandle, path: String) -> Result<MediaInfo, String> {
    let ffprobe = paths::ffprobe_path(&app).map_err(|e| e.to_string())?;
    if !ffprobe.exists() {
        return Err("ffprobe not installed".into());
    }
    if !Path::new(&path).exists() {
        return Err(format!("file not found: {path}"));
    }

    let mut cmd = Command::new(&ffprobe);
    cmd.args([
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(&path)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    crate::ytdlp::hide_console(&mut cmd);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("failed to spawn ffprobe: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse ffprobe JSON: {e}"))?;

    let duration = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.parse::<f64>().ok());

    let video = json
        .get("streams")
        .and_then(|s| s.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|s| s.get("codec_type").and_then(|t| t.as_str()) == Some("video"))
        });

    let width = video
        .and_then(|v| v.get("width"))
        .and_then(|w| w.as_u64())
        .map(|w| w as u32);
    let height = video
        .and_then(|v| v.get("height"))
        .and_then(|h| h.as_u64())
        .map(|h| h as u32);
    let fps = video
        .and_then(|v| v.get("avg_frame_rate"))
        .and_then(|r| r.as_str())
        .and_then(parse_frame_rate);

    if video.is_none() {
        return Err("no video stream found in file".into());
    }

    Ok(MediaInfo {
        path,
        duration,
        width,
        height,
        fps,
    })
}

/// `avg_frame_rate` comes back as a ratio like "30000/1001" or "25/1".
fn parse_frame_rate(s: &str) -> Option<f64> {
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.trim().parse().ok()?;
    let den: f64 = den.trim().parse().ok()?;
    if den == 0.0 || num <= 0.0 {
        return None;
    }
    Some(num / den)
}

/// Start a GIF export and return the job id. Progress arrives via
/// `edit://*` events keyed by that id.
#[tauri::command]
pub async fn export_gif(app: AppHandle, options: GifExportOptions) -> Result<String, String> {
    let ffmpeg = paths::ffmpeg_path(&app).map_err(|e| e.to_string())?;
    if !ffmpeg.exists() {
        return Err("ffmpeg not installed".into());
    }
    if !Path::new(&options.input_path).exists() {
        return Err(format!("file not found: {}", options.input_path));
    }
    if !(1.0..=60.0).contains(&options.fps) {
        return Err("fps must be between 1 and 60".into());
    }
    if let (Some(start), Some(end)) = (options.start, options.end) {
        if end <= start {
            return Err("end must be after start".into());
        }
    }

    let id = Uuid::new_v4().to_string();
    let id_for_task = id.clone();
    let app_for_task = app.clone();

    let task = tokio::spawn(async move {
        run_export(app_for_task, id_for_task, ffmpeg, options).await;
    });

    let state = app.state::<AppState>();
    state.jobs.lock().unwrap().insert(
        id.clone(),
        JobHandle {
            task,
            child_id: None,
            paused: false,
        },
    );

    emit_status(&app, &id, "queued", None);
    Ok(id)
}

#[tauri::command]
pub async fn cancel_export(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let handle = state.jobs.lock().unwrap().remove(&id);
    if let Some(handle) = handle {
        #[cfg(windows)]
        if let Some(pid) = handle.child_id {
            crate::ytdlp::runner::kill_windows(pid);
        }
        handle.task.abort();
        emit_status(&app, &id, "canceled", None);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// List the keyframe (I-frame) timestamps of a video's first video stream,
/// in seconds, sorted ascending. The Trim view uses these to snap cut points
/// and to decide whether a `-c copy` (lossless) trim is possible: a copy is
/// only accurate when the start lands on a keyframe.
#[tauri::command]
pub async fn list_keyframes(app: AppHandle, path: String) -> Result<Vec<f64>, String> {
    let ffprobe = paths::ffprobe_path(&app).map_err(|e| e.to_string())?;
    if !ffprobe.exists() {
        return Err("ffprobe not installed".into());
    }
    if !Path::new(&path).exists() {
        return Err(format!("file not found: {path}"));
    }

    let mut cmd = Command::new(&ffprobe);
    cmd.args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-skip_frame",
        "nokey",
        "-show_entries",
        "frame=pts_time,best_effort_timestamp_time",
        "-print_format",
        "json",
    ])
    .arg(&path)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    crate::ytdlp::hide_console(&mut cmd);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("failed to spawn ffprobe: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse ffprobe JSON: {e}"))?;

    let mut times: Vec<f64> = json
        .get("frames")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|frame| {
                    // pts_time is usually present; fall back to the
                    // best-effort timestamp when a frame reports "N/A".
                    frame
                        .get("pts_time")
                        .or_else(|| frame.get("best_effort_timestamp_time"))
                        .and_then(|t| t.as_str())
                        .and_then(|t| t.parse::<f64>().ok())
                })
                .collect()
        })
        .unwrap_or_default();

    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup_by(|a, b| (*a - *b).abs() < 0.0005);
    Ok(times)
}

/// Grab a single frame at `time` seconds and return it as a JPEG data URI.
/// The Trim view lays these out along the timeline as a scrub strip so the
/// user can see what they're cutting. Kept lightweight: fast-seek, one frame,
/// scaled down, piped straight out of ffmpeg.
#[tauri::command]
pub async fn thumbnail_at(
    app: AppHandle,
    path: String,
    time: f64,
    width: Option<u32>,
) -> Result<String, String> {
    let ffmpeg = paths::ffmpeg_path(&app).map_err(|e| e.to_string())?;
    if !ffmpeg.exists() {
        return Err("ffmpeg not installed".into());
    }
    if !Path::new(&path).exists() {
        return Err(format!("file not found: {path}"));
    }
    let w = width.unwrap_or(240).max(16);

    let mut cmd = Command::new(&ffmpeg);
    cmd.args(["-v", "error"]);
    if time > 0.0 {
        cmd.args(["-ss", &format!("{time}")]);
    }
    cmd.arg("-i")
        .arg(&path)
        .args(["-frames:v", "1", "-an"])
        .arg("-vf")
        .arg(format!("scale={w}:-2"))
        .args(["-f", "image2pipe", "-vcodec", "mjpeg", "-q:v", "5", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::ytdlp::hide_console(&mut cmd);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("failed to spawn ffmpeg: {e}"))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(format!(
            "could not extract frame: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(format!("data:image/jpeg;base64,{}", base64_encode(&output.stdout)))
}

/// Minimal standard-alphabet base64 encoder. Inlined so a thumbnail data URI
/// needs no extra crate dependency.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Trim a single range out of a video. Returns a job id; progress arrives via
/// `edit://*`, same as the GIF commands. `reencode` selects between a lossless
/// stream-copy (`-c copy`) and a re-encode.
#[tauri::command]
pub async fn trim_video(app: AppHandle, options: TrimOptions) -> Result<String, String> {
    let ffmpeg = paths::ffmpeg_path(&app).map_err(|e| e.to_string())?;
    if !ffmpeg.exists() {
        return Err("ffmpeg not installed".into());
    }
    if !Path::new(&options.input_path).exists() {
        return Err(format!("file not found: {}", options.input_path));
    }
    if let (Some(start), Some(end)) = (options.start, options.end) {
        if end <= start {
            return Err("end must be after start".into());
        }
    }

    let id = Uuid::new_v4().to_string();
    let id_for_task = id.clone();
    let app_for_task = app.clone();

    let task = tokio::spawn(async move {
        run_trim(app_for_task, id_for_task, ffmpeg, options).await;
    });

    let state = app.state::<AppState>();
    state.jobs.lock().unwrap().insert(
        id.clone(),
        JobHandle {
            task,
            child_id: None,
            paused: false,
        },
    );

    emit_status(&app, &id, "queued", None);
    Ok(id)
}

/// Trim multiple ranges out of one video, written either as separate files
/// or concatenated into a single output. Returns a job id; progress via
/// `edit://*`.
#[tauri::command]
pub async fn trim_multi(app: AppHandle, options: MultiTrimOptions) -> Result<String, String> {
    let ffmpeg = paths::ffmpeg_path(&app).map_err(|e| e.to_string())?;
    if !ffmpeg.exists() {
        return Err("ffmpeg not installed".into());
    }
    if !Path::new(&options.input_path).exists() {
        return Err(format!("file not found: {}", options.input_path));
    }
    if options.ranges.is_empty() {
        return Err("no ranges selected".into());
    }
    if options.mode != "separate" && options.mode != "concat" {
        return Err("mode must be 'separate' or 'concat'".into());
    }
    for r in &options.ranges {
        if let (Some(start), Some(end)) = (r.start, r.end) {
            if end <= start {
                return Err("each range's end must be after its start".into());
            }
        }
    }

    let id = Uuid::new_v4().to_string();
    let id_for_task = id.clone();
    let app_for_task = app.clone();

    let task = tokio::spawn(async move {
        run_trim_multi(app_for_task, id_for_task, ffmpeg, options).await;
    });

    let state = app.state::<AppState>();
    state.jobs.lock().unwrap().insert(
        id.clone(),
        JobHandle {
            task,
            child_id: None,
            paused: false,
        },
    );

    emit_status(&app, &id, "queued", None);
    Ok(id)
}

/// Append a video/GIF range onto an existing GIF and re-encode the whole
/// thing as one GIF. Returns a job id; progress arrives via `edit://*`,
/// same as `export_gif`, so the frontend reuses one event path.
#[tauri::command]
pub async fn append_to_gif(app: AppHandle, options: GifAppendOptions) -> Result<String, String> {
    let ffmpeg = paths::ffmpeg_path(&app).map_err(|e| e.to_string())?;
    if !ffmpeg.exists() {
        return Err("ffmpeg not installed".into());
    }
    if !Path::new(&options.base_path).exists() {
        return Err(format!("file not found: {}", options.base_path));
    }
    if !Path::new(&options.clip_path).exists() {
        return Err(format!("file not found: {}", options.clip_path));
    }
    if !(1.0..=60.0).contains(&options.fps) {
        return Err("fps must be between 1 and 60".into());
    }
    if options.position != "front" && options.position != "back" {
        return Err("position must be 'front' or 'back'".into());
    }
    if let (Some(start), Some(end)) = (options.clip_start, options.clip_end) {
        if end <= start {
            return Err("clip end must be after start".into());
        }
    }

    let id = Uuid::new_v4().to_string();
    let id_for_task = id.clone();
    let app_for_task = app.clone();

    let task = tokio::spawn(async move {
        run_append(app_for_task, id_for_task, ffmpeg, options).await;
    });

    let state = app.state::<AppState>();
    state.jobs.lock().unwrap().insert(
        id.clone(),
        JobHandle {
            task,
            child_id: None,
            paused: false,
        },
    );

    emit_status(&app, &id, "queued", None);
    Ok(id)
}

async fn run_export(app: AppHandle, id: String, ffmpeg: PathBuf, opts: GifExportOptions) {
    // Clip duration drives the progress fraction during the encode pass.
    let clip_duration = match clip_duration(&app, &opts).await {
        Ok(d) => d,
        Err(e) => {
            finish(&app, &id, "error", Some(e));
            return;
        }
    };

    let palette = std::env::temp_dir().join(format!("vidfetch-palette-{id}.png"));
    let filters = build_filters(&opts);

    // Pass 1 — derive a 256-color palette from the clip itself.
    emit_status(&app, &id, "palette", None);
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y");
    push_trim_args(&mut cmd, &opts);
    cmd.arg("-i")
        .arg(&opts.input_path)
        .arg("-vf")
        .arg(format!("{filters},palettegen"))
        .arg(palette.as_os_str());
    if let Err(e) = run_pass(&app, &id, cmd, None).await {
        let _ = std::fs::remove_file(&palette);
        finish(&app, &id, "error", Some(format!("palette pass failed: {e}")));
        return;
    }

    // Pass 2 — encode the GIF against that palette, streaming progress.
    emit_status(&app, &id, "encoding", None);
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y");
    push_trim_args(&mut cmd, &opts);
    cmd.arg("-i")
        .arg(&opts.input_path)
        .arg("-i")
        .arg(palette.as_os_str())
        .arg("-lavfi")
        .arg(format!(
            "{filters} [x]; [x][1:v] paletteuse=dither={}",
            opts.dither
        ))
        .args(["-progress", "pipe:1", "-nostats"]);
    push_loop_arg(&mut cmd, opts.loop_count);
    cmd.arg(&opts.output_path);
    let result = run_pass(&app, &id, cmd, Some(clip_duration)).await;
    let _ = std::fs::remove_file(&palette);

    match result {
        Ok(()) => finish(&app, &id, "done", Some(opts.output_path.clone())),
        Err(e) => {
            // A canceled job already emitted its status and removed itself.
            let still_tracked = app
                .state::<AppState>()
                .jobs
                .lock()
                .unwrap()
                .contains_key(&id);
            if still_tracked {
                finish(&app, &id, "error", Some(format!("encode failed: {e}")));
            }
        }
    }
}

async fn run_append(app: AppHandle, id: String, ffmpeg: PathBuf, opts: GifAppendOptions) {
    // The base drives the output dimensions; both clips are normalized to
    // it so the concat filter (which needs matching resolution/SAR) is happy
    // and a single shared palette can cover the whole result.
    let base = match probe_media(app.clone(), opts.base_path.clone()).await {
        Ok(b) => b,
        Err(e) => {
            finish(&app, &id, "error", Some(format!("could not read base: {e}")));
            return;
        }
    };
    let (base_w, base_h) = match (base.width, base.height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => {
            finish(&app, &id, "error", Some("base file has no video dimensions".into()));
            return;
        }
    };
    let target_w = opts.width.unwrap_or(base_w).max(2);
    let target_h = (((target_w as f64) * (base_h as f64) / (base_w as f64)).round() as u32).max(2);

    // Total duration (base + appended range) for the encode progress bar.
    let clip_range = match (opts.clip_start, opts.clip_end) {
        (start, Some(end)) => Some((end - start.unwrap_or(0.0)).max(0.1)),
        (start, None) => media_duration(&app, &opts.clip_path)
            .await
            .map(|d| (d - start.unwrap_or(0.0)).max(0.1)),
    };
    let total = match (base.duration, clip_range) {
        (Some(b), Some(c)) => Some(b + c),
        _ => None,
    };

    let palette = std::env::temp_dir().join(format!("vidfetch-append-{id}.png"));
    let base_chain = scaled_input(0, "b", opts.fps, target_w, target_h);
    let clip_chain = scaled_input(1, "c", opts.fps, target_w, target_h);
    // "front" puts the appended clip first; "back" puts it after the base.
    let order = if opts.position == "front" { "[c][b]" } else { "[b][c]" };

    // Pass 1 — one palette derived from the whole concatenated stream.
    emit_status(&app, &id, "palette", None);
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y").arg("-i").arg(&opts.base_path);
    push_clip_trim(&mut cmd, &opts);
    cmd.arg("-i")
        .arg(&opts.clip_path)
        .arg("-filter_complex")
        .arg(format!(
            "{base_chain};{clip_chain};{order}concat=n=2:v=1:a=0[s];[s]palettegen[p]"
        ))
        .args(["-map", "[p]"])
        .arg(palette.as_os_str());
    if let Err(e) = run_pass(&app, &id, cmd, None).await {
        let _ = std::fs::remove_file(&palette);
        finish(&app, &id, "error", Some(format!("palette pass failed: {e}")));
        return;
    }

    // Pass 2 — encode the joined GIF against that palette.
    emit_status(&app, &id, "encoding", None);
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y").arg("-i").arg(&opts.base_path);
    push_clip_trim(&mut cmd, &opts);
    cmd.arg("-i")
        .arg(&opts.clip_path)
        .arg("-i")
        .arg(palette.as_os_str())
        .arg("-filter_complex")
        .arg(format!(
            "{base_chain};{clip_chain};{order}concat=n=2:v=1:a=0[s];[s][2:v]paletteuse=dither={}[out]",
            opts.dither
        ))
        .args(["-map", "[out]"])
        .args(["-progress", "pipe:1", "-nostats"]);
    push_loop_arg(&mut cmd, opts.loop_count);
    cmd.arg(&opts.output_path);
    let result = run_pass(&app, &id, cmd, total).await;
    let _ = std::fs::remove_file(&palette);

    match result {
        Ok(()) => finish(&app, &id, "done", Some(opts.output_path.clone())),
        Err(e) => {
            let still_tracked = app
                .state::<AppState>()
                .jobs
                .lock()
                .unwrap()
                .contains_key(&id);
            if still_tracked {
                finish(&app, &id, "error", Some(format!("encode failed: {e}")));
            }
        }
    }
}

async fn run_trim(app: AppHandle, id: String, ffmpeg: PathBuf, opts: TrimOptions) {
    // Range length drives the progress fraction.
    let start = opts.start.unwrap_or(0.0).max(0.0);
    let len = match opts.end {
        Some(end) => Some((end - start).max(0.1)),
        None => media_duration(&app, &opts.input_path)
            .await
            .map(|d| (d - start).max(0.1)),
    };

    emit_status(&app, &id, "encoding", None);
    let cmd = build_trim_cmd(
        &ffmpeg,
        &opts.input_path,
        start,
        opts.end,
        opts.reencode,
        Path::new(&opts.output_path),
        true,
    );

    let result = run_pass(&app, &id, cmd, len).await;
    match result {
        Ok(()) => finish(&app, &id, "done", Some(opts.output_path.clone())),
        Err(e) => {
            let still_tracked = app
                .state::<AppState>()
                .jobs
                .lock()
                .unwrap()
                .contains_key(&id);
            if still_tracked {
                finish(&app, &id, "error", Some(format!("trim failed: {e}")));
            }
        }
    }
}

/// Build an ffmpeg cut command for one range. `-ss` before `-i` fast-seeks;
/// on a re-encode it stays frame-accurate, and on a copy it lands on the
/// keyframe the frontend snapped the start to.
fn build_trim_cmd(
    ffmpeg: &Path,
    input: &str,
    start: f64,
    end: Option<f64>,
    reencode: bool,
    output: &Path,
    progress: bool,
) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-y");
    if start > 0.0 {
        cmd.args(["-ss", &format!("{start}")]);
    }
    cmd.arg("-i").arg(input);
    if let Some(end) = end {
        cmd.args(["-t", &format!("{}", (end - start).max(0.1))]);
    }
    if reencode {
        cmd.args([
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt",
            "yuv420p", "-c:a", "aac", "-b:a", "192k",
        ]);
    } else {
        // Stream-copy: no re-encode, so the cut is lossless and near-instant.
        cmd.args(["-c", "copy"]);
    }
    if progress {
        cmd.args(["-progress", "pipe:1", "-nostats"]);
    }
    cmd.arg(output);
    cmd
}

/// `clip.mp4` + index 1 → `clip-1.mp4`. Index is 1-based for humans.
fn indexed_output(base: &str, index: usize) -> String {
    match base.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{index}.{ext}"),
        None => format!("{base}-{index}"),
    }
}

async fn run_trim_multi(app: AppHandle, id: String, ffmpeg: PathBuf, opts: MultiTrimOptions) {
    let n = opts.ranges.len();
    let outputs: Vec<String> = if opts.mode == "separate" {
        (1..=n).map(|i| indexed_output(&opts.output_path, i)).collect()
    } else {
        // Concat: each range first goes to a temp segment, then we join them.
        let ext = opts.output_path.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
        (1..=n)
            .map(|i| {
                std::env::temp_dir()
                    .join(format!("vidfetch-seg-{id}-{i}.{ext}"))
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    };

    emit_status(&app, &id, "encoding", None);

    // Cut each range. Per-segment progress would jump backwards, so we report
    // a coarse fraction by completed segments instead.
    for (i, range) in opts.ranges.iter().enumerate() {
        let start = range.start.unwrap_or(0.0).max(0.0);
        let cmd = build_trim_cmd(
            &ffmpeg,
            &opts.input_path,
            start,
            range.end,
            opts.reencode,
            Path::new(&outputs[i]),
            false,
        );
        if let Err(e) = run_pass(&app, &id, cmd, None).await {
            if still_tracked(&app, &id) {
                finish(&app, &id, "error", Some(format!("range {} failed: {e}", i + 1)));
            }
            if opts.mode == "concat" {
                cleanup(&outputs);
            }
            return;
        }
        // Cutting is the bulk of the work; leave a little headroom for concat.
        let done = (i + 1) as f64 / n as f64;
        emit_progress(&app, &id, if opts.mode == "concat" { done * 0.9 } else { done });
    }

    if opts.mode == "separate" {
        finish(&app, &id, "done", Some(format!("{n} clips")));
        return;
    }

    // Join the temp segments with the concat demuxer (stream copy — the
    // segments are already in their final codec).
    let list = std::env::temp_dir().join(format!("vidfetch-concat-{id}.txt"));
    let body: String = outputs
        .iter()
        // The concat demuxer treats '\' as an escape; forward slashes are safe
        // on Windows too.
        .map(|p| format!("file '{}'\n", p.replace('\\', "/")))
        .collect();
    if let Err(e) = std::fs::write(&list, body) {
        finish(&app, &id, "error", Some(format!("could not write concat list: {e}")));
        cleanup(&outputs);
        return;
    }

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y")
        .args(["-f", "concat", "-safe", "0"])
        .arg("-i")
        .arg(&list)
        .args(["-c", "copy"])
        .args(["-progress", "pipe:1", "-nostats"])
        .arg(&opts.output_path);
    let result = run_pass(&app, &id, cmd, None).await;

    let _ = std::fs::remove_file(&list);
    cleanup(&outputs);

    match result {
        Ok(()) => finish(&app, &id, "done", Some(opts.output_path.clone())),
        Err(e) => {
            if still_tracked(&app, &id) {
                finish(&app, &id, "error", Some(format!("concat failed: {e}")));
            }
        }
    }
}

fn cleanup(paths: &[String]) {
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

fn still_tracked(app: &AppHandle, id: &str) -> bool {
    app.state::<AppState>().jobs.lock().unwrap().contains_key(id)
}

fn emit_progress(app: &AppHandle, id: &str, fraction: f64) {
    let _ = app.emit(
        "edit://progress",
        EditProgress {
            id: id.to_string(),
            fraction: fraction.clamp(0.0, 1.0),
        },
    );
}

/// Per-input filter chain: resample to `fps`, fit inside `w`x`h` keeping
/// aspect ratio, letterbox the rest, and normalize SAR + pixel format. concat
/// rejects segments whose resolution/SAR/format differ, and a GIF (pal8) won't
/// match a video (yuv420p) without the explicit `format`.
fn scaled_input(idx: usize, label: &str, fps: f64, w: u32, h: u32) -> String {
    format!(
        "[{idx}:v]fps={fps},scale={w}:{h}:force_original_aspect_ratio=decrease,\
         pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,format=rgb24[{label}]"
    )
}

/// Trim args for the appended clip. Mirrors `push_trim_args`: `-ss` before
/// the clip's `-i` seeks, so `-t` is the range length.
fn push_clip_trim(cmd: &mut Command, opts: &GifAppendOptions) {
    let start = opts.clip_start.unwrap_or(0.0).max(0.0);
    if start > 0.0 {
        cmd.args(["-ss", &format!("{start}")]);
    }
    if let Some(end) = opts.clip_end {
        cmd.args(["-t", &format!("{}", (end - start).max(0.1))]);
    }
}

/// ffprobe duration of a media file, or None if it can't be determined.
async fn media_duration(app: &AppHandle, path: &str) -> Option<f64> {
    probe_media(app.clone(), path.to_string())
        .await
        .ok()
        .and_then(|m| m.duration)
}

/// Duration of the exported range in seconds.
async fn clip_duration(app: &AppHandle, opts: &GifExportOptions) -> Result<f64, String> {
    let start = opts.start.unwrap_or(0.0).max(0.0);
    if let Some(end) = opts.end {
        return Ok((end - start).max(0.1));
    }
    let info = probe_media(app.clone(), opts.input_path.clone()).await?;
    let total = info.duration.ok_or("could not determine video duration")?;
    Ok((total - start).max(0.1))
}

/// Shared fps/scale filter prefix for both passes; identical filtering
/// before palettegen and paletteuse keeps the palette accurate.
fn build_filters(opts: &GifExportOptions) -> String {
    let mut filters = format!("fps={}", opts.fps);
    if let Some(width) = opts.width {
        filters.push_str(&format!(",scale={width}:-1:flags=lanczos"));
    }
    filters
}

/// GIF loop muxer flag (output option). 0 = forever, -1 = play once,
/// n = loop n extra times. None leaves ffmpeg's default.
fn push_loop_arg(cmd: &mut Command, loop_count: Option<i32>) {
    if let Some(n) = loop_count {
        cmd.args(["-loop", &n.to_string()]);
    }
}

fn push_trim_args(cmd: &mut Command, opts: &GifExportOptions) {
    let start = opts.start.unwrap_or(0.0).max(0.0);
    if start > 0.0 {
        cmd.args(["-ss", &format!("{start}")]);
    }
    if let Some(end) = opts.end {
        // -ss before -i seeks, so -t takes the range length, not the end time.
        cmd.args(["-t", &format!("{}", (end - start).max(0.1))]);
    }
}

/// Run one ffmpeg pass; when `total_secs` is set, parse `-progress pipe:1`
/// output from stdout and emit progress fractions.
async fn run_pass(
    app: &AppHandle,
    id: &str,
    mut cmd: Command,
    total_secs: Option<f64>,
) -> anyhow::Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::ytdlp::hide_console(&mut cmd);

    let mut child = cmd.spawn()?;

    if let Some(pid) = child.id() {
        if let Some(handle) = app.state::<AppState>().jobs.lock().unwrap().get_mut(id) {
            handle.child_id = Some(pid);
        }
    }

    if let (Some(stdout), Some(total)) = (child.stdout.take(), total_secs) {
        let app_c = app.clone();
        let id_c = id.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(us) = parse_out_time_us(&line) {
                    let fraction = (us as f64 / 1_000_000.0 / total).clamp(0.0, 1.0);
                    let _ = app_c.emit(
                        "edit://progress",
                        EditProgress {
                            id: id_c.clone(),
                            fraction,
                        },
                    );
                }
            }
        });
    }

    // Keep stderr for the log panel and for error reporting.
    let mut stderr_tail: Vec<String> = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app.emit(
                "edit://log",
                EditLog {
                    id: id.to_string(),
                    line: line.clone(),
                },
            );
            stderr_tail.push(line);
            if stderr_tail.len() > 8 {
                stderr_tail.remove(0);
            }
        }
    }

    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!("ffmpeg exited with {status}: {}", stderr_tail.join(" | "));
    }
    Ok(())
}

/// `-progress pipe:1` emits `out_time_us=<microseconds>` lines.
/// (`out_time_ms` is also microseconds — a long-standing ffmpeg quirk —
/// so accept it as a fallback with the same unit.)
fn parse_out_time_us(line: &str) -> Option<u64> {
    let value = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))?;
    value.trim().parse::<u64>().ok()
}

fn finish(app: &AppHandle, id: &str, status: &'static str, message: Option<String>) {
    emit_status(app, id, status, message);
    app.state::<AppState>().jobs.lock().unwrap().remove(id);
}

fn emit_status(app: &AppHandle, id: &str, status: &'static str, message: Option<String>) {
    let _ = app.emit(
        "edit://status",
        EditStatus {
            id: id.to_string(),
            status,
            message,
        },
    );
}
