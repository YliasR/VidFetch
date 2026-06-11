# VidFetch Roadmap

Phases 1–8 plus the v1.1/v1.2 point releases shipped (current: `v1.2.0`). Remaining work is grouped below.
Each phase is its own checkpoint and gets an alpha tag.

---

## Phase 6a — Content options

Playlists, subtitles, SponsorBlock. The "what goes into the file" side of
yt-dlp configuration.

- [x] **Playlists**
  - [x] Probe command detects `_type == "playlist"` and returns a playlist
        structure with entries (id, title, duration, thumbnail, url).
  - [x] Frontend `ProbeResult` becomes a discriminated union
        (`single` vs `playlist`).
  - [x] New `PlaylistView` in DownloadView: checkbox list, select-all,
        range pattern (e.g. `1,3,5-7`), per-item thumbnails.
  - [x] "Add N to queue" enqueues each selected entry as its own queue item
        (clean per-item progress, fits existing scheduler).
- [x] **Subtitles**
  - [x] Probe exposes `availableSubs` + `availableAutoSubs` (lang codes).
  - [x] UI: multi-select chips, "embed vs separate file", "include
        auto-generated" toggle. Disabled for audio-only presets.
  - [x] Args: `--write-subs`, `--sub-langs <...>`, `--write-auto-subs`,
        `--embed-subs`.
- [x] **SponsorBlock**
  - [x] Off / Mark / Remove radio in advanced options.
  - [x] Args: `--sponsorblock-mark all` or `--sponsorblock-remove all`.

**Ship tag:** `v0.3.0-alpha`.

---

## Phase 6b — Plumbing options + in-app updater

The "how it runs" knobs, plus self-update so end users don't re-download
installers from GitHub by hand.

- [x] **Cookies** — dropdown: none / browser (chrome/firefox/edge/brave/…) /
      cookies.txt file path. Args: `--cookies-from-browser <b>` or
      `--cookies <file>`.
- [x] **Rate limit** — text input (e.g. `2M`). Arg: `--limit-rate`.
- [x] **Retries / fragment retries** — number inputs. Args: `--retries`,
      `--fragment-retries`.
- [x] **Output template editor** — textarea with preset buttons
      (`%(title)s.%(ext)s`, `%(uploader)s - %(title)s.%(ext)s`, …) and
      a live-preview line computed from the probed metadata.
- [x] **File conflicts** — radio: skip / overwrite / auto-number. Args:
      `--no-overwrites` / `--force-overwrites` / yt-dlp's default numbering.
- [x] **Embed extras** — toggles for thumbnail, metadata, chapters. Args:
      `--embed-thumbnail`, `--embed-metadata`, `--embed-chapters`.
- [x] **In-app updater** — `tauri-plugin-updater` pointed at a GitHub
      Releases `latest.json` manifest.
  - Generate a signing keypair with `tauri signer generate`.
  - Commit the public key to `tauri.conf.json > plugins.updater.pubkey`.
  - Store the private key + password as repo secrets
        (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
  - Extend `release.yml` to sign the NSIS bundle and publish
        `latest.json` alongside the installer.
  - Settings panel: "Check for updates" button + "Update now" prompt
        that downloads, verifies, and relaunches.
  - Optional "Auto-check on launch" toggle (default on).

**Ship tag:** `v0.4.0-alpha`.

---

## Phase 7a — History & logs

The "what just happened" surface. Pulls log capture out of dev-tools and
keeps a record after jobs finish.

- [x] **History** — persisted list of completed downloads (title, path,
      timestamp, preset, size). Open folder / re-download actions.
      Stored via `tauri-plugin-store`; trimmed to a sane cap.
- [x] **Raw log panel** — collapsible per queue item, shows captured
      stdout/stderr from yt-dlp (`download://log` is already emitted).
      Useful for debugging weird extractor failures.
- [x] **Desktop notifications** on job completion via
      `tauri-plugin-notification`. Toggle in settings, default on.

**Ship tag:** `v0.5.0-alpha`.

---

## Phase 7b - Presets & archive

Reusable configurations and skip-already-downloaded support.

- [x] **Presets** - save current advanced config as a named preset, apply
      from a dropdown on the Download view. CRUD UI in the Presets view.
- [x] **Download archive** - `--download-archive` file stored per-preset
      so re-runs skip previously-fetched items. Toggle on the preset.

**Ship tag:** `v0.6.0-alpha`.

---

## Phase 7c — Polish

The "feels finished" pass before the v1 push.

- [x] **Error recovery UI** — surface known patterns ("yt-dlp outdated",
      "ffmpeg missing", "HTTP 403") with a one-click action.
- [x] **Fox theme unlock polish** — tail-wag animation on logo, `:3`
      toast, persistence verified.
- [x] **Light theme** pass — audit all views for contrast / legibility.

**Ship tag:** `v0.7.0-alpha` (or beta if it feels ready).

---

## Phase 8a — Cross-platform builds (pulled forward from v1.3)

- [x] **macOS build** — universal .dmg (`macos-14`,
      `--target universal-apple-darwin`). First-run installer fetches the
      universal2 `yt-dlp_macos` plus per-arch ffmpeg/ffprobe from
      martin-riedl.de (signed builds listed on ffmpeg.org), with retry to
      ride out a flaky mirror.
- [x] **Linux build** — `ubuntu-22.04`, ships `.AppImage` and `.deb`.
      Installer fetches the standalone `yt-dlp_linux` plus ffmpeg/ffprobe
      from BtbN's linux64 tar.xz (extracted via `xz2` + `tar`).
- [ ] **macOS/Linux smoke test** — run the first-run wizard + a real
      download on each platform before tagging v1.0.
- [ ] **Platform-specific theming sanity** — audit macOS traffic-light
      offset, Linux window controls placement.

**Ship tag:** `v0.8.0-alpha`.

---

## Phase 8b — Installer & release polish

- [x] **Real app icon** — replace the placeholder icon set.
- [x] **NSIS installer polish** — sidebar + header art shipped; license
      page and "launch VidFetch" checkbox deferred past v1.
- [x] **Code signing** — document the cert flow even if we don't sign
      for v1 (Windows SmartScreen warning and macOS right-click-to-open
      for the unsigned app are accepted for now).
- [x] **Clean-VM smoke test** — install on a fresh Windows 11 VM with no
      dev tools, confirm first-run wizard + a real download work.
- [ ] **README screenshots** — one per main view, fox theme teaser.

**Ship tag:** `v1.0.0` (drop the alpha).

---

---

# Post-1.0 — point releases

Smaller, focused improvements that ride on top of the v1.0 foundation.

## v1.1 — Queue polish

- [x] **Playlist groups** — a set of entries added together from one
      playlist probe collapses into a single queue row that expands to
      show per-item progress. Bulk cancel / remove at the group level.
- [x] **Pause/resume on running jobs** — Windows
      `NtSuspendProcess` / `NtResumeProcess` via the `windows` crate;
      Unix `SIGSTOP` / `SIGCONT`. Exposed as a pause icon on active
      queue items.
- [x] **Queue persistence across restarts** — save pending + queued
      items to LazyStore on mutation; on boot, requeue as `queued`
      (running jobs die with the process, so they go back to queued).

**Ship tag:** shipped together with v1.2 as `v1.2.0`.

## v1.2 — Power-user UX

- [x] **Format browser** — full table of formats returned by
      `yt-dlp -J` (resolution, fps, codec, ext, size, bitrate). Click a
      row to pick that exact format instead of a preset. Preset stays
      the default; this is an opt-in "Advanced" toggle.
- [x] **Clipboard auto-paste** — watch the clipboard for a URL whose
      extractor yt-dlp recognizes; offer a one-click "Fetch the URL you
      just copied" button on the Download view. Uses
      `tauri-plugin-clipboard-manager`.
- [x] **Drag-and-drop URLs** — drop a text file or URL directly onto
      the window to enqueue.

**Ship tag:** `v1.2.0`.

# v2.0 and beyond — GIFs & video editing

This is the pivot from "yt-dlp GUI" to "yt-dlp + tiny video studio."
ffmpeg is already bundled, so no new binary deps — but the UX gets a
new top-level **Edit** tab alongside Download / Queue, with a
per-clip timeline view. Each v2.x release adds one slice of editing.

## v2.0 — GIF pipeline

- [ ] **Video → GIF export** — pick a downloaded video (or any local
      file), trim the range, tweak width / fps / dithering. Two-pass
      `ffmpeg -vf palettegen` then `-lavfi paletteuse` for clean
      palettes. Progress streamed the same way download progress is.
- [ ] **Import existing GIF** — drop a `.gif` into the Edit tab to
      load it as a source clip (treated the same as a video internally).
- [ ] **Re-edit imported GIF** — trim, resize, frame-drop to lower fps,
      change loop count, or re-optimize the palette.
- [ ] **Append video range to GIF** — take an existing GIF and tack a
      range from a video onto the end (or front) with a matched
      palette to avoid color-wash.

**Ship tag:** `v2.0.0`.

## v2.1 — Trim & cut

- [ ] **Single-file trim** — pick start/end on a video scrubber, write
      the cut. Use `-c copy` when both cuts land on keyframes, re-encode
      otherwise (show user a "lossless / re-encoded" badge).
- [ ] **Multi-range trim** — pick N ranges from a single source and
      export them as separate files, or concat them into one (hand-off
      to v2.2).
- [ ] **Scrub preview** — lightweight ffmpeg thumbnail generation
      along the timeline so the user isn't trimming blind.

**Ship tag:** `v2.1.0`.

## v2.2 — Multi-clip concat

- [ ] **Drag-and-drop clip list** — order a list of local files in the
      Edit tab, preview the sequence, write a single output.
- [ ] **Fast path** (same codec/resolution/fps) — concat demuxer, no
      re-encode.
- [ ] **Slow path** (mixed sources) — concat filter with an ffmpeg
      normalize pass so resolution/fps/audio-rate match.

**Ship tag:** `v2.2.0`.

## v2.3 — Audio ops

- [ ] **Remove audio** — passthrough encode with `-an`.
- [ ] **Replace audio** — pick a local audio file, align to video
      length (trim or loop), optional fade-in/out, choose mix vs.
      replace the existing track.
- [ ] **Extract audio** — rip the audio track out to mp3/opus/flac
      (shares the yt-dlp audio-preset code path).
- [ ] **Volume adjust** — simple dB slider with waveform preview.

**Ship tag:** `v2.3.0`.

## v2.4 — Transforms

- [ ] **Rotate / flip** — 90° rotations + horizontal/vertical flip.
- [ ] **Crop** — rectangle selector overlaid on a preview frame.
- [ ] **Scale** — resize to a target width/height or percentage,
      optionally locking aspect ratio.
- [ ] **Speed** — change playback speed with audio pitch preserved
      (`atempo` chain for audio, `setpts` for video).

**Ship tag:** `v2.4.0`.

---

# Unsorted / parking lot

Ideas without a phase yet — pull into one of the above when they
become concrete:

- Batch preset application: select N queue items and re-apply a
  different preset in one click.
- Per-site defaults (e.g. always Opus for SoundCloud, always 720p
  for Twitch VODs).
- "Download only new items" on a channel or playlist URL, powered by
  `--download-archive`.
- Keyboard shortcuts layer (queue nav, add from clipboard, open
  preferences).
