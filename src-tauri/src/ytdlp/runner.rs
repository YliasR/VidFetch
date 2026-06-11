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
