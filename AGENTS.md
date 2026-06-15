# AGENTS.md — codebase guide for AI agents

## What this app is

**VidFetch** is a desktop GUI wrapper around **yt-dlp** (with bundled ffmpeg),
built with **Tauri 2** (Rust backend) + **Svelte 4 / TypeScript / Vite**
(frontend). It downloads videos/audio/playlists from any yt-dlp-supported
site, manages a download queue with pause/resume and persistence, and is
currently growing a second identity as a small video editor (v2 line:
GIF export shipped first, trim/concat/audio/transforms planned — see
`ROADMAP.md`).

Current stable: `v1.2.1`. v2 work ships in **nightly phases** (rolling
`nightly` prerelease tag) and will culminate in a single public `v2.0.0`.

Key runtime facts:

- yt-dlp / ffmpeg / ffprobe are **not** bundled in the installer; a
  first-run wizard downloads them into `<app_local_data>/bin/`
  (`be.mystic.vidfetch` identifier → `%LOCALAPPDATA%\be.mystic.vidfetch\bin`
  on Windows).
- Long-running work (downloads, GIF exports) runs as spawned child
  processes in the Rust backend; progress streams to the frontend as
  Tauri events (`download://*`, `edit://*`).
- Settings/queue/history persist via `tauri-plugin-store` (LazyStore JSON
  files in app data).
- In-app updater (`tauri-plugin-updater`) with two channels: Stable
  (GitHub `latest.json`) and Nightly (rolling `nightly` prerelease).

## Repo layout

```
ROADMAP.md           Phase-by-phase plan; keep checkboxes in sync with work
README.md            User-facing readme
index.html           Vite entry
package.json         npm scripts: dev / build / check (svelte-check) / tauri
vite.config.ts       Vite + Svelte config ($lib alias → src/lib)
svelte.config.js     vitePreprocess
branding/            SVG sources + build.py to render icons/banner/installer art
scripts/             gen_placeholder_icons.py (legacy placeholder icons)
dist/                Vite build output (generated, don't edit)
.github/workflows/
  release.yml        Tag-driven stable release: 3-platform Tauri build, signs
                     updater artifacts, publishes latest.json
  nightly.yml        Manual (workflow_dispatch, any branch) rolling prerelease:
                     deletes previous `nightly` release, stamps version as
                     <patch+1>-nightly.<date>.<sha>, builds + publishes
```

## Frontend (`src/`)

```
main.ts                       Mounts App.svelte, imports global styles
App.svelte                    Boot flow: BootSplash → Shell, or FirstRunWizard
                              when binaries are missing
styles/base.css               Global classes: .btn .btn-primary .btn-ghost
                              .btn-sm .input .card .muted — reuse these in views
styles/theme-{dark,light,fox}.css  Theme variables (--accent, --danger,
                              --success, --surface*, --fg*). Fox theme is an
                              easter-egg unlock.
```

`src/lib/`:

```
ipc.ts            Thin typed wrappers over invoke() for every Tauri command.
                  Add new commands here, not inline invoke() calls in views.
types.ts          Shared probe/download types mirroring Rust serde structs
                  (camelCase on the wire).
errors.ts         Pattern-matches yt-dlp failure output → known cause +
                  one-click fix suggestion (used by error recovery UI).
dragdrop.ts       Window-level drag-and-drop of URLs / URL-list text files.
```

`src/lib/stores/` (Svelte stores; most hydrate from LazyStore JSON):

```
nav.ts            currentView: 'download'|'queue'|'edit'|'history'|'presets'|'settings'
download.ts       Probe state + preset + outputDir + AdvancedOptions
                  (subs, SponsorBlock, cookies, rate limit, template, embeds)
queue.ts          Download queue: scheduling, progress events, playlist
                  groups, persistence across restarts
history.ts        Completed-download history (capped)
presets.ts        Saved download configs + per-preset download archive
notifications.ts  Desktop notification on job completion
theme.ts          Theme persistence + fox unlock
updates.ts        Update channel (stable/nightly) + check/install state
ytdlp.ts          Binary presence boot check + versions
```

`src/lib/components/` (one view per sidebar item, rendered by MainPane):

```
Shell.svelte           App chrome: Header + Sidebar + MainPane
Header.svelte          Title bar area
Sidebar.svelte         Nav buttons (Download/Queue/Edit/History/Presets/Settings)
MainPane.svelte        View switch on $currentView
DownloadView.svelte    URL probe, preset picker, advanced options, format
                       browser, playlist selection, add-to-queue
QueueView.svelte       Queue rows: progress, pause/resume/cancel, groups, logs
EditView.svelte        Edit tab shell with a GIF / Trim mode toggle. GIF mode:
                       video/GIF → GIF (source picker via Browse or drop,
                       trim/width/fps/dither, loop count, two-pass progress,
                       "Append a clip" panel). Trim mode renders TrimView.
TrimView.svelte        Trim & cut: source picker, scrub-preview thumbnail strip
                       (click a frame → set a range's start/end), N ranges with
                       separate-files / join export, lossless-copy vs re-encode
                       badge driven by list_keyframes, force-re-encode toggle.
                       Reuses the edit:// events + cancel_export.
HistoryView.svelte     Completed downloads, open-folder / re-download
PresetsView.svelte     Preset CRUD + archive file picker
SettingsView.svelte    Binaries, notifications, update channel + check
FirstRunWizard.svelte  Guided yt-dlp/ffmpeg install on first launch
BootSplash.svelte      Splash while checking binaries
ThemeSwitcher.svelte   Theme toggle
PlaceholderView.svelte Stub view (kept around for new tabs)
```

## Backend (`src-tauri/`)

```
tauri.conf.json        App config: version (stamped by CI for nightlies),
                       updater pubkey + endpoint, bundle settings
capabilities/*.json    Tauri 2 permission grants for the main window —
                       update this when using a new plugin API from JS
Cargo.toml             Rust deps (tokio, reqwest, windows/libc for
                       pause-resume, zip/xz2/tar for installer extraction)
src/main.rs            Entry → vidfetch_lib::run()
src/lib.rs             Builder: plugins, AppState, invoke_handler list —
                       register every new #[tauri::command] here
src/state.rs           AppState { jobs: Mutex<HashMap<id, JobHandle>> } —
                       shared by download and edit jobs (task handle,
                       child PID, paused flag)
src/paths.rs           app data / bin dir / yt-dlp / ffmpeg / ffprobe paths
```

`src/commands/` (Tauri command surface):

```
mod.rs        Module list
ytdlp.rs      check_binaries / install_ytdlp / install_ffmpeg / get_versions
probe.rs      probe_url: yt-dlp -J → discriminated Single/Playlist result
download.rs   start/cancel/pause/resume download (delegates to ytdlp::runner)
edit.rs       Edit tab: probe_media (ffprobe JSON), export_gif (two-pass
              palettegen → paletteuse with -progress pipe:1, loop flag),
              append_to_gif (concat two clips through one shared palette,
              letterboxed to the base's dimensions), list_keyframes (I-frame
              timestamps for lossless-cut decisions), trim_video (single-range
              cut, -c copy vs re-encode), trim_multi (N ranges → separate
              files or concat-demuxer join), thumbnail_at (single-frame JPEG
              data URI for the scrub strip; inlined base64), cancel_export.
              Events: edit://status, edit://progress, edit://log
files.rs      read_dropped_text (size-capped URL-list reads)
updater.rs    Channel-aware update check/install (stable vs nightly endpoint)
```

`src/ytdlp/`:

```
mod.rs        hide_console() — CREATE_NO_WINDOW for all child spawns on
              Windows; use it for every Command
args.rs       DownloadOptions → yt-dlp argv (incl. PROGRESS_PREFIX template
              for machine-readable progress lines)
runner.rs     Spawns yt-dlp, streams download://{status,progress,log} events,
              cancel (kill by PID on Windows), pause/resume
              (NtSuspendProcess / SIGSTOP)
installer.rs  Downloads platform-specific yt-dlp + ffmpeg static builds with
              progress events, extracts (zip/tar.xz) into bin dir
```

## Conventions

- **Events over polling:** background jobs emit `<area>://status|progress|log`
  events keyed by a UUID job id returned from the start command.
- **Serde casing:** Rust structs use `#[serde(rename_all = "camelCase")]`;
  TS types in `types.ts`/`ipc.ts` mirror them.
- **All child processes** go through `tokio::process::Command` +
  `hide_console()` + `kill_on_drop(true)`.
- **Styling:** use the global classes from `base.css` and theme CSS
  variables; views keep layout-specific styles in their own `<style>`.
- **Releases:** stable = tag `vX.Y.Z` → `release.yml`; nightly = run
  `nightly.yml` manually from any branch (version is auto-stamped).
- **Release notes are mandatory.** Every GitHub release page (stable *and*
  nightly) must get a real changelog: what's new, what changed, what's
  fixed — written for end users, not commit-message dumps. Do this without
  being asked; an empty or placeholder release body is a bug. For nightlies,
  edit the rolling `nightly` release body (`gh release edit nightly --notes`)
  after the workflow publishes, since the workflow only writes a generic
  template.
- **ROADMAP.md** is the source of truth for scope; tick checkboxes when a
  feature lands.

## Dev environment

- The main dev machine is **Windows 11** (PowerShell; MinGW/MSYS2 available).
  Assume Windows paths and test there first.
- A **macOS** machine and a **Linux** PC are also available for
  platform-specific testing — ask the user to run things there when a change
  touches platform-conditional code (`cfg(windows)` / `cfg(unix)`, installers,
  window chrome) rather than shipping it blind.

## Verification

- Backend: `cd src-tauri && cargo check`
- Frontend: `npm run check` (svelte-check) and `npm run build`
- Full app: `npm run tauri dev`

---

> **Note to the next agent:** this file was written from a session focused
> on the GIF export feature (Nightly 1). The per-file descriptions for the
> stores and views listed above are accurate at a one-line level but thin —
> if you work in one of them and notice missing or outdated detail (props,
> persistence keys, event flows), expand the relevant entry while you're
> there. Keep this file current.
