//! Spawns yt-dlp and streams its progress lines back to the frontend as
//! Tauri events.
//!
//! Events emitted (one per target id):
//! - `download://status`  — lifecycle changes (queued/downloading/postprocess/done/error/canceled)
//! - `download://progress` — per-tick byte counts, speed, ETA
//! - `download://log`     — raw stdout/stderr lines for the debug panel

use crate::paths;
use crate::state::{AppState, JobHandle};
use crate::ytdlp::args::{build_args, DownloadOptions, PROGRESS_PREFIX};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStatus {
    pub id: String,
    pub status: &'static str,
    pub message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: Option<f64>,
    pub eta: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadLog {
    pub id: String,
    pub line: String,
    pub stream: &'static str,
}

/// Spawn yt-dlp for the given options and return the generated job id.
/// The actual download runs as a background tokio task; consumers listen
/// for `download://*` events keyed by the returned id.
pub fn spawn_download(app: AppHandle, opts: DownloadOptions) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let ytdlp = paths::ytdlp_path(&app)?;
    let ffmpeg = paths::ffmpeg_path(&app)?;

    if !ytdlp.exists() {
        anyhow::bail!("yt-dlp binary missing at {}", ytdlp.display());
    }
    if !ffmpeg.exists() {
        anyhow::bail!("ffmpeg binary missing at {}", ffmpeg.display());
    }

    let args = build_args(&opts, &ffmpeg);

    let mut cmd = Command::new(&ytdlp);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    super::hide_console(&mut cmd);

    let id_for_task = id.clone();
    let app_for_task = app.clone();

    let task = tokio::spawn(async move {
        run_job(app_for_task, id_for_task, cmd).await;
    });

    // Track the task handle so we can drop/cancel it later.
    let state = app.state::<AppState>();
    let mut jobs = state.jobs.lock().unwrap();
    jobs.insert(
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

/// Spawn a download that will be converted to GIF after completion.
/// This handles both the yt-dlp download and the subsequent ffmpeg GIF conversion.
pub fn spawn_gif_download(app: AppHandle, opts: DownloadOptions) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let ytdlp = paths::ytdlp_path(&app)?;
    let ffmpeg = paths::ffmpeg_path(&app)?;

    if !ytdlp.exists() {
        anyhow::bail!("yt-dlp binary missing at {}", ytdlp.display());
    }
    if !ffmpeg.exists() {
        anyhow::bail!("ffmpeg binary missing at {}", ffmpeg.display());
    }

    // Build args for yt-dlp - we'll download as mp4 first
    let args = build_args(&opts, &ffmpeg);
    
    // For GIF output, we need to ensure we get a video file that we can convert
    // The args builder already handles this by forcing mp4 output format

    let mut cmd = Command::new(&ytdlp);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    super::hide_console(&mut cmd);

    let id_for_task = id.clone();
    let app_for_task = app.clone();
    let opts_for_task = opts.clone();

    let task = tokio::spawn(async move {
        run_gif_job(app_for_task, id_for_task, cmd, opts_for_task, ffmpeg).await;
    });

    // Track the task handle so we can drop/cancel it later.
    let state = app.state::<AppState>();
    let mut jobs = state.jobs.lock().unwrap();
    jobs.insert(
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

pub fn cancel(app: &AppHandle, id: &str) -> anyhow::Result<bool> {
    let state = app.state::<AppState>();
    let mut jobs = state.jobs.lock().unwrap();

    if let Some(handle) = jobs.remove(id) {
        #[cfg(windows)]
        if let Some(pid) = handle.child_id {
            kill_windows(pid);
        }
        handle.task.abort();
        emit_status(app, id, "canceled", None);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Suspend a running job's yt-dlp process. Only meaningful during the
/// download phase: postprocess children (ffmpeg) are separate processes
/// and are not suspended, so the UI only offers pause while downloading.
pub fn pause(app: &AppHandle, id: &str) -> anyhow::Result<bool> {
    let state = app.state::<AppState>();
    let mut jobs = state.jobs.lock().unwrap();
    let Some(handle) = jobs.get_mut(id) else {
        return Ok(false);
    };
    if handle.paused {
        return Ok(true);
    }
    let Some(pid) = handle.child_id else {
        anyhow::bail!("job has no running process yet");
    };
    suspend_process(pid)?;
    handle.paused = true;
    emit_status(app, id, "paused", None);
    Ok(true)
}

/// Resume a previously paused job.
pub fn resume(app: &AppHandle, id: &str) -> anyhow::Result<bool> {
    let state = app.state::<AppState>();
    let mut jobs = state.jobs.lock().unwrap();
    let Some(handle) = jobs.get_mut(id) else {
        return Ok(false);
    };
    if !handle.paused {
        return Ok(true);
    }
    let Some(pid) = handle.child_id else {
        anyhow::bail!("job has no running process");
    };
    resume_process(pid)?;
    handle.paused = false;
    emit_status(app, id, "downloading", None);
    Ok(true)
}

#[cfg(windows)]
fn suspend_process(pid: u32) -> anyhow::Result<()> {
    nt_process_op(pid, true)
}

#[cfg(windows)]
fn resume_process(pid: u32) -> anyhow::Result<()> {
    nt_process_op(pid, false)
}

#[cfg(windows)]
fn nt_process_op(pid: u32, suspend: bool) -> anyhow::Result<()> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SUSPEND_RESUME};

    // NtSuspendProcess/NtResumeProcess suspend every thread in one call;
    // they are stable ntdll exports despite not being in the Win32 API.
    #[link(name = "ntdll")]
    extern "system" {
        fn NtSuspendProcess(handle: HANDLE) -> i32;
        fn NtResumeProcess(handle: HANDLE) -> i32;
    }

    unsafe {
        let handle = OpenProcess(PROCESS_SUSPEND_RESUME, false, pid)
            .map_err(|e| anyhow::anyhow!("OpenProcess failed: {e}"))?;
        let status = if suspend {
            NtSuspendProcess(handle)
        } else {
            NtResumeProcess(handle)
        };
        let _ = CloseHandle(handle);
        if status != 0 {
            anyhow::bail!(
                "{} failed with NTSTATUS 0x{status:08X}",
                if suspend { "NtSuspendProcess" } else { "NtResumeProcess" }
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
fn suspend_process(pid: u32) -> anyhow::Result<()> {
    signal_process(pid, libc::SIGSTOP)
}

#[cfg(unix)]
fn resume_process(pid: u32) -> anyhow::Result<()> {
    signal_process(pid, libc::SIGCONT)
}

#[cfg(unix)]
fn signal_process(pid: u32, signal: i32) -> anyhow::Result<()> {
    let ret = unsafe { libc::kill(pid as i32, signal) };
    if ret != 0 {
        anyhow::bail!(
            "kill({pid}, {signal}) failed: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn kill_windows(pid: u32) {
    // Hard-kill the child by PID. Safer than signaling and saves us wiring
    // a dedicated channel from the runner task for v1.
    unsafe {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, TerminateProcess, PROCESS_TERMINATE,
        };
        if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
            let _ = TerminateProcess(h, 1);
            let _ = CloseHandle(h);
        }
    }
}

async fn run_job(app: AppHandle, id: String, mut cmd: Command) {
    emit_status(&app, &id, "downloading", None);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            emit_status(&app, &id, "error", Some(format!("spawn failed: {e}")));
            return;
        }
    };

    // Record the child PID so `cancel` can target the OS process directly.
    let pid = child.id();
    if let Some(pid) = pid {
        if let Some(handle) = app.state::<AppState>().jobs.lock().unwrap().get_mut(&id) {
            handle.child_id = Some(pid);
        }
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(stdout) = stdout {
        let app_c = app.clone();
        let id_c = id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                handle_line(&app_c, &id_c, &line, "stdout");
            }
        });
    }

    if let Some(stderr) = stderr {
        let app_c = app.clone();
        let id_c = id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                handle_line(&app_c, &id_c, &line, "stderr");
            }
        });
    }

    match child.wait().await {
        Ok(status) if status.success() => {
            emit_status(&app, &id, "done", None);
        }
        Ok(status) => {
            emit_status(
                &app,
                &id,
                "error",
                Some(format!("yt-dlp exited with {status}")),
            );
        }
        Err(e) => {
            emit_status(&app, &id, "error", Some(format!("wait failed: {e}")));
        }
    }

    // Remove the job from state once the task is done (unless cancel got there first).
    let state = app.state::<AppState>();
    state.jobs.lock().unwrap().remove(&id);
}

/// Run a GIF download job: first download with yt-dlp, then convert to GIF with ffmpeg.
async fn run_gif_job(
    app: AppHandle,
    id: String,
    mut cmd: Command,
    opts: DownloadOptions,
    ffmpeg_path: PathBuf,
) {
    emit_status(&app, &id, "downloading", None);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            emit_status(&app, &id, "error", Some(format!("spawn failed: {e}")));
            return;
        }
    };

    // Record the child PID so `cancel` can target the OS process directly.
    let pid = child.id();
    if let Some(pid) = pid {
        if let Some(handle) = app.state::<AppState>().jobs.lock().unwrap().get_mut(&id) {
            handle.child_id = Some(pid);
        }
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Track if we should continue with GIF conversion
    let mut should_convert = true;

    if let Some(stdout) = stdout {
        let app_c = app.clone();
        let id_c = id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                handle_line(&app_c, &id_c, &line, "stdout");
            }
        });
    }

    if let Some(stderr) = stderr {
        let app_c = app.clone();
        let id_c = id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                handle_line(&app_c, &id_c, &line, "stderr");
            }
        });
    }

    // Wait for yt-dlp to complete
    match child.wait().await {
        Ok(status) if status.success() => {
            // yt-dlp succeeded, now convert to GIF
            emit_status(&app, &id, "postprocess", None);
            
            // Find the downloaded file and convert it
            if let Err(e) = convert_to_gif(&app, &id, &opts, &ffmpeg_path).await {
                emit_status(&app, &id, "error", Some(format!("GIF conversion failed: {e}")));
                should_convert = false;
            }
        }
        Ok(status) => {
            emit_status(
                &app,
                &id,
                "error",
                Some(format!("yt-dlp exited with {status}")),
            );
            should_convert = false;
        }
        Err(e) => {
            emit_status(&app, &id, "error", Some(format!("wait failed: {e}")));
            should_convert = false;
        }
    }

    if should_convert {
        emit_status(&app, &id, "done", None);
    }

    // Remove the job from state once the task is done (unless cancel got there first).
    let state = app.state::<AppState>();
    state.jobs.lock().unwrap().remove(&id);
}

/// Convert the downloaded video file to GIF using ffmpeg.
async fn convert_to_gif(
    app: &AppHandle,
    id: &str,
    opts: &DownloadOptions,
    ffmpeg_path: &PathBuf,
) -> anyhow::Result<()> {
    // Parse the output template to figure out the downloaded filename
    // yt-dlp uses the template we provided, so we need to figure out what file was created
    let output_dir = Path::new(&opts.output_dir);
    
    // The merge step forces a .mp4, so find the most recently modified mp4 in
    // the output dir — that's the one yt-dlp just wrote. (Taking the first
    // entry would pick an arbitrary older file if the folder has others.)
    let mut downloaded_file: Option<PathBuf> = None;
    let mut newest: Option<std::time::SystemTime> = None;
    if output_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("mp4") {
                    let mtime = entry.metadata().and_then(|m| m.modified()).ok();
                    if mtime > newest {
                        newest = mtime;
                        downloaded_file = Some(path);
                    }
                }
            }
        }
    }
    
    let input_path = downloaded_file.ok_or_else(|| {
        anyhow::anyhow!("No downloaded video file found in {}", opts.output_dir)
    })?;
    
    // Create the output GIF path by changing the extension
    let gif_path = input_path.with_extension("gif");
    
    // Parse GIF options. Trim fields may be clock format ("1:30") or plain
    // seconds ("90"), matching what the Download view's inputs send.
    let start = opts.gif_start.as_deref().and_then(parse_time_secs);
    let end = opts.gif_end.as_deref().and_then(parse_time_secs);
    let width = opts.gif_width.unwrap_or(480);
    let fps = opts.gif_fps.unwrap_or(15.0);
    let dither = opts.gif_dither.as_ref().map(|s| s.as_str()).unwrap_or("sierra2_4a");
    
    // Validate fps range
    if !(1.0..=60.0).contains(&fps) {
        anyhow::bail!("fps must be between 1 and 60");
    }
    
    // Validate dither option
    let valid_dithers = ["sierra2_4a", "floyd_steinberg", "bayer", "none"];
    if !valid_dithers.contains(&dither) {
        anyhow::bail!("Invalid dither option: {}", dither);
    }
    
    // Run the two-pass GIF conversion
    let clip_duration = compute_clip_duration(ffmpeg_path, &input_path, start, end).await?;
    
    // Pass 1: Generate palette
    emit_status(app, id, "palette", None);
    let palette_path = std::env::temp_dir().join(format!("vidfetch-gif-palette-{}.png", id));
    
    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-y");
    if let Some(start) = start {
        if start > 0.0 {
            cmd.args(["-ss", &format!("{}", start)]);
        }
    }
    if let Some(end) = end {
        let start_val = start.unwrap_or(0.0);
        let duration = (end - start_val).max(0.1);
        cmd.args(["-t", &format!("{}", duration)]);
    }
    cmd.arg("-i").arg(&input_path);
    
    // Build filters for fps and scale
    let mut filters = format!("fps={}", fps);
    filters.push_str(&format!(",scale={}:-1:flags=lanczos", width));
    
    cmd.arg("-vf")
       .arg(format!("{},palettegen", filters));
    
    cmd.arg(&palette_path);
    cmd.stdin(Stdio::null())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped())
       .kill_on_drop(true);
    super::hide_console(&mut cmd);
    
    if let Err(e) = cmd.status().await {
        let _ = std::fs::remove_file(&palette_path);
        anyhow::bail!("Palette generation failed: {}", e);
    }
    
    // Pass 2: Encode GIF using palette
    emit_status(app, id, "encoding", None);
    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-y");
    if let Some(start) = start {
        if start > 0.0 {
            cmd.args(["-ss", &format!("{}", start)]);
        }
    }
    if let Some(end) = end {
        let start_val = start.unwrap_or(0.0);
        let duration = (end - start_val).max(0.1);
        cmd.args(["-t", &format!("{}", duration)]);
    }
    cmd.arg("-i").arg(&input_path);
    cmd.arg("-i").arg(&palette_path);
    cmd.arg("-lavfi")
       .arg(format!("fps={},scale={}:-1:flags=lanczos [x]; [x][1:v] paletteuse=dither={}", fps, width, dither));
    
    cmd.args(["-progress", "pipe:1", "-nostats"]);
    cmd.arg(&gif_path);
    
    cmd.stdin(Stdio::null())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped())
       .kill_on_drop(true);
    super::hide_console(&mut cmd);
    
    // Run the encode pass with progress tracking
    let mut child = cmd.spawn()?;
    
    // Track progress if we have a duration
    if let Some(total_secs) = clip_duration {
        let app_c = app.clone();
        let id_c = id.to_string();
        let stdout = child.stdout.take();
        
        if let Some(stdout) = stdout {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(us) = parse_out_time_us(&line) {
                        let fraction = (us as f64 / 1_000_000.0 / total_secs).clamp(0.0, 1.0);
                        emit_gif_progress(&app_c, &id_c, fraction);
                    }
                }
            });
        }
    }
    
    let status = child.wait().await?;
    let _ = std::fs::remove_file(&palette_path);
    
    if !status.success() {
        anyhow::bail!("GIF encoding failed with status: {}", status);
    }
    
    // Clean up the original mp4 file
    let _ = std::fs::remove_file(&input_path);
    
    Ok(())
}

/// Compute the duration of the clip to be encoded for progress tracking.
async fn compute_clip_duration(
    ffmpeg_path: &PathBuf,
    input_path: &PathBuf,
    start: Option<f64>,
    end: Option<f64>,
) -> anyhow::Result<Option<f64>> {
    if let (Some(start_val), Some(end_val)) = (start, end) {
        return Ok(Some((end_val - start_val).max(0.1)));
    }
    
    // Need to probe the input file to get its duration using std::process for simplicity
    let mut cmd = std::process::Command::new(ffmpeg_path);
    cmd.args([
        "-v", "error",
        "-print_format", "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(input_path)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());
    
    let output = cmd.output()?;
    if !output.status.success() {
        return Ok(None); // If probing fails, we'll just not show progress
    }
    
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let duration = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.parse::<f64>().ok());
    
    if let Some(duration) = duration {
        let start_val = start.unwrap_or(0.0);
        Ok(Some((duration - start_val).max(0.1)))
    } else {
        Ok(None)
    }
}

/// Parse a trim time that may be clock format ("1:30", "1:02:05.5") or plain
/// seconds ("90", "12.5"). Mirrors the Edit tab's `parseTime`. Empty → None.
fn parse_time_secs(s: &str) -> Option<f64> {
    let clean = s.trim();
    if clean.is_empty() {
        return None;
    }
    let mut total = 0.0_f64;
    for part in clean.split(':') {
        let v: f64 = part.trim().parse().ok()?;
        total = total * 60.0 + v;
    }
    Some(total)
}

/// Parse `out_time_us` from ffmpeg progress output.
fn parse_out_time_us(line: &str) -> Option<u64> {
    let value = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))?;
    value.trim().parse::<u64>().ok()
}

/// Emit GIF conversion progress.
fn emit_gif_progress(app: &AppHandle, id: &str, fraction: f64) {
    // For now, we'll just emit a progress event that shows the conversion progress
    // We can use the same DownloadProgress struct but with different interpretation
    let _ = app.emit("download://progress", DownloadProgress {
        id: id.to_string(),
        downloaded: (fraction * 100.0) as u64,
        total: Some(100),
        speed: None,
        eta: None,
    });
}

fn handle_line(app: &AppHandle, id: &str, line: &str, stream: &'static str) {
    let _ = app.emit(
        "download://log",
        DownloadLog {
            id: id.to_string(),
            line: line.to_string(),
            stream,
        },
    );

    if let Some(rest) = line.strip_prefix(&format!("{PROGRESS_PREFIX}|")) {
        if let Some(progress) = parse_progress(id, rest) {
            let _ = app.emit("download://progress", progress);
        }
    }
}

fn parse_progress(id: &str, rest: &str) -> Option<DownloadProgress> {
    // Format: downloaded|total|speed|eta
    let mut parts = rest.split('|');
    let downloaded = parts.next()?;
    let total = parts.next()?;
    let speed = parts.next()?;
    let eta = parts.next()?;

    Some(DownloadProgress {
        id: id.to_string(),
        downloaded: parse_u64(downloaded).unwrap_or(0),
        total: parse_u64(total),
        speed: parse_f64(speed),
        eta: parse_u64(eta),
    })
}

fn parse_u64(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() || s == "NA" || s == "None" {
        return None;
    }
    s.parse::<f64>().ok().map(|v| v.max(0.0) as u64)
}

fn parse_f64(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s == "NA" || s == "None" {
        return None;
    }
    s.parse::<f64>().ok()
}

fn emit_status(app: &AppHandle, id: &str, status: &'static str, message: Option<String>) {
    let _ = app.emit(
        "download://status",
        DownloadStatus {
            id: id.to_string(),
            status,
            message,
        },
    );
}
