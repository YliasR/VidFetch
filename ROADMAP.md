# VidFetch Roadmap

Phases 1–5 shipped in the `v0.1.0-alpha` / `v0.2.0-alpha` releases. Remaining
work is grouped below. Each phase is its own checkpoint and gets an alpha tag.

---

## Phase 6a — Content options (next up)

Playlists, subtitles, SponsorBlock. The "what goes into the file" side of
yt-dlp configuration.

- [ ] **Playlists**
  - [ ] Probe command detects `_type == "playlist"` and returns a playlist
        structure with entries (id, title, duration, thumbnail, url).
  - [ ] Frontend `ProbeResult` becomes a discriminated union
        (`single` vs `playlist`).
  - [ ] New `PlaylistView` in DownloadView: checkbox list, select-all,
        range pattern (e.g. `1,3,5-7`), per-item thumbnails.
  - [ ] "Add N to queue" enqueues each selected entry as its own queue item
        (clean per-item progress, fits existing scheduler).
- [ ] **Subtitles**
  - [ ] Probe exposes `availableSubs` + `availableAutoSubs` (lang codes).
  - [ ] UI: multi-select chips, "embed vs separate file", "include
        auto-generated" toggle. Disabled for audio-only presets.
  - [ ] Args: `--write-subs`, `--sub-langs <...>`, `--write-auto-subs`,
        `--embed-subs`.
- [ ] **SponsorBlock**
  - [ ] Off / Mark / Remove radio in advanced options.
  - [ ] Args: `--sponsorblock-mark all` or `--sponsorblock-remove all`.

**Ship tag:** `v0.3.0-alpha`.

---

## Phase 6b — Plumbing options

The "how it runs" knobs — less visual, more plumbing.

- [ ] **Cookies** — dropdown: none / browser (chrome/firefox/edge/brave/…) /
      cookies.txt file path. Args: `--cookies-from-browser <b>` or
      `--cookies <file>`.
- [ ] **Rate limit** — text input (e.g. `2M`). Arg: `--limit-rate`.
- [ ] **Retries / fragment retries** — number inputs. Args: `--retries`,
      `--fragment-retries`.
- [ ] **Output template editor** — textarea with preset buttons
      (`%(title)s.%(ext)s`, `%(uploader)s - %(title)s.%(ext)s`, …) and
      a live-preview line computed from the probed metadata.
- [ ] **File conflicts** — radio: skip / overwrite / auto-number. Args:
      `--no-overwrites` / `--force-overwrites` / yt-dlp's default numbering.
- [ ] **Embed extras** — toggles for thumbnail, metadata, chapters. Args:
      `--embed-thumbnail`, `--embed-metadata`, `--embed-chapters`.

**Ship tag:** `v0.4.0-alpha`.

---

## Phase 7 — Persistence & polish

- [ ] **History** — persisted list of completed downloads (title, path,
      timestamp, preset, size). Open folder / re-download actions.
- [ ] **Presets** — save current advanced config as a named preset, apply
      from a dropdown on the Download view.
- [ ] **Download archive** — `--download-archive` file stored per-preset
      so re-runs skip previously-fetched items.
- [ ] **Raw log panel** — collapsible per queue item, shows captured
      stdout/stderr from yt-dlp. Useful for debugging weird extractor
      failures.
- [ ] **Desktop notifications** on job completion (plugin-notification).
- [ ] **Error recovery UI** — surface known patterns ("yt-dlp outdated")
      with a one-click "Update now" action.
- [ ] **Fox theme unlock polish** — tail-wag animation on logo, `:3`
      toast, persistence verified.
- [ ] **Light theme** pass — audit all views for contrast / legibility.

**Ship tag:** `v0.5.0-alpha` (or beta if it feels ready).

---

## Phase 8 — Installer & release polish

- [ ] **Real app icon** — replace the placeholder icon set.
- [ ] **NSIS installer polish** — sidebar image, license page, "launch
      VidFetch" checkbox on finish.
- [ ] **Code signing** — document the cert flow even if we don't sign
      for v1 (SmartScreen warning is accepted for now).
- [ ] **Auto-update** — `tauri-plugin-updater` wired to GitHub Releases
      `latest.json`. Needs a signing keypair.
- [ ] **Clean-VM smoke test** — install on a fresh Windows 11 VM with no
      dev tools, confirm first-run wizard + a real download work.
- [ ] **README screenshots** — one per main view, fox theme teaser.

**Ship tag:** `v1.0.0` (drop the alpha).

---

## Longer-term / nice-to-haves

Not committed, just ideas:

- Multi-item playlist as a single queue group (collapsible) instead of N
  flat items.
- Pause/resume on running jobs (Windows `NtSuspendProcess` /
  `NtResumeProcess`; Unix `SIGSTOP` / `SIGCONT`).
- Format browser table (per-format codec/size/fps) for power users who
  want to pick manually.
- Cross-platform builds in CI (macOS, Linux — matrix expansion of
  `release.yml`).
- Clipboard auto-paste detection when a yt-dlp-able URL lands on the
  clipboard.
