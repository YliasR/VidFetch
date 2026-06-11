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
        .args(["-progress", "pipe:1", "-nostats"])
        .arg(&opts.output_path);
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
