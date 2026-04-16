pub mod args;
pub mod installer;
pub mod runner;

use tokio::process::Command;

/// Hide the console window for child processes on Windows.
///
/// yt-dlp, ffmpeg, and ffprobe are console apps — without this flag every
/// spawn briefly flashes a cmd window, even for quick `--version` calls.
/// No-op on non-Windows platforms.
pub fn hide_console(#[allow(unused_variables)] cmd: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}
