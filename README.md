# VidFetch

A sleek, cross-platform desktop GUI for [yt-dlp](https://github.com/yt-dlp/yt-dlp) — paste a URL, pick a format, download. No terminal required.

Built with Tauri v2 + Svelte 5 + TypeScript. Rust handles the download pipeline; the UI is plain CSS with full theme control (dark, light, and a hidden one 🦊).

> **Status:** early alpha (`0.1.x`). Things work; things are also missing. See [Roadmap](#roadmap).

---

## Why

The yt-dlp CLI is the best video downloader on the internet, but "best" and "friendly" aren't always the same thing. VidFetch is a thin, pretty layer on top — you get yt-dlp's full extractor support, but the UX looks and feels like a real desktop app instead of a man page.

It also **manages yt-dlp for you**: on first launch it downloads the latest yt-dlp and a matching ffmpeg build into its own app-data folder, and updates them on demand from Settings. Nothing to install, nothing to `pip`.

## Features

### In `0.1` (alpha)

- Paste-and-download single videos
- Auto format/quality detection
- First-run wizard that fetches yt-dlp + ffmpeg automatically
- Real-time progress bars driven by `yt-dlp --progress-template`
- Dark and light themes, system-preference aware
- Output folder picker, persisted across launches
- Windows installer (NSIS)

### Planned (tracked in the project plan)

- [ ] Download queue with pause / resume / cancel and concurrency limit
- [ ] Full format browser (resolution, codec, bitrate, size)
- [ ] Playlist picker with per-item selection
- [ ] Subtitle download + embedding (multi-language)
- [ ] Thumbnail + metadata + chapter embedding
- [ ] SponsorBlock integration
- [ ] Cookies import (from browser or `cookies.txt`)
- [ ] Rate limit, retries, output template editor
- [ ] Download archive (skip already-downloaded)
- [ ] History with re-download and open-folder shortcuts
- [ ] Saved presets
- [ ] Raw-log debug panel per job
- [ ] Linux (AppImage + `.deb`) and macOS (`.dmg`) builds
- [ ] In-app auto-update via `tauri-plugin-updater`

## Install

### Windows

Grab the latest `VidFetch_x.y.z_x64-setup.exe` from the [Releases page](../../releases) and run it. The installer is not yet code-signed, so SmartScreen will show a warning on first run — click **More info → Run anyway**.

On first launch, the app's setup wizard downloads yt-dlp (~5 MB) and ffmpeg (~100 MB) from their official sources. Everything lives in `%APPDATA%\be.mystic.vidfetch\`; no files touched outside that folder and your chosen download directory.

### Linux / macOS

Not shipped in `0.1`. The codebase targets cross-platform from day one, so these builds are a packaging exercise rather than a rewrite — tracked in the roadmap.

## Build from source

You need:

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) stable (1.77+)
- The [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS (WebView2 on Windows — usually already installed on Windows 11)

```bash
git clone https://github.com/<you>/VidFetch.git
cd VidFetch
npm install
npm run tauri dev     # hot-reload dev build
npm run tauri build   # release build + NSIS installer in src-tauri/target/release/bundle/
```

## Project layout

```
VidFetch/
├── src/                  # Svelte frontend (TS)
│   ├── lib/components/   # UI components
│   ├── lib/stores/       # reactive state
│   ├── lib/ipc.ts        # typed wrappers around Tauri invoke()
│   └── styles/           # base + theme-{dark,light,fox}.css
├── src-tauri/            # Rust backend
│   └── src/
│       ├── commands/     # #[tauri::command] handlers
│       ├── ytdlp/        # installer + runner + arg builder
│       ├── paths.rs      # app data / bin dir resolution
│       └── state.rs      # shared app state
└── scripts/              # dev/build helper scripts
```

## Tech stack

| Layer         | Choice                                    | Why                                                                 |
| ------------- | ----------------------------------------- | ------------------------------------------------------------------- |
| Shell         | [Tauri v2](https://v2.tauri.app/)         | Small installers, Rust backend, uses the system webview             |
| Frontend      | [Svelte 5](https://svelte.dev/) + TS      | Minimal runtime, clean syntax, no unnecessary state lib             |
| Styling       | Plain CSS + custom properties             | Maximum theme flexibility, no framework churn                       |
| Downloader    | [yt-dlp](https://github.com/yt-dlp/yt-dlp)| Best-in-class extractor coverage                                    |
| Postprocessor | [ffmpeg](https://ffmpeg.org/) (BtbN build)| Required for merging, audio extraction, and metadata/thumb embedding|
| Packaging     | NSIS (via Tauri bundler)                  | Smaller, friendlier than MSI for a desktop app                      |

## Credits

VidFetch is a GUI — all the heavy lifting is done by upstream projects:

- **[yt-dlp](https://github.com/yt-dlp/yt-dlp)** — the actual downloader. None of this works without it.
- **[ffmpeg](https://ffmpeg.org/)** — format conversion and stream muxing. Windows builds by [BtbN](https://github.com/BtbN/FFmpeg-Builds).
- **[Tauri](https://v2.tauri.app/)** — the app shell framework.
- **[Svelte](https://svelte.dev/)** — the frontend framework.

## License

TBD — will be set before the first tagged release. Contributions welcome once the license is clarified.

---

<sub>Built with ❤️ — and a small fox hidden somewhere in the interface. Have fun finding it.</sub>
