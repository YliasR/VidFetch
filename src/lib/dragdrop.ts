/**
 * Window-level drag-and-drop: drop a URL (or a text file full of URLs)
 * anywhere on the app to fetch or enqueue it.
 *
 * Two delivery paths:
 * - OS file drops arrive through Tauri's webview drag-drop event (paths).
 * - Plain text/link drags arrive through DOM drop events where the
 *   platform lets them through.
 */
import { writable, get } from 'svelte/store';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { ipc } from '$lib/ipc';
import { downloadStore, probe } from '$lib/stores/download';
import { addToQueue, newGroupId } from '$lib/stores/queue';
import { currentView } from '$lib/stores/nav';
import type { DownloadOptions } from '$lib/types';

const URL_RE = /^https?:\/\/\S+$/i;
const MAX_DROPPED_URLS = 50;
const TEXT_FILE_EXTS = new Set(['txt', 'text', 'url', 'list']);

/** True while something is being dragged over the window. */
export const dropHover = writable(false);

/** Transient feedback line shown after a drop ("Queued 12 links"). */
export const dropMessage = writable<string | null>(null);

let installed = false;
let unlistenDragDrop: UnlistenFn | null = null;
let messageTimer: ReturnType<typeof setTimeout> | null = null;

function flashMessage(msg: string): void {
  dropMessage.set(msg);
  if (messageTimer) clearTimeout(messageTimer);
  messageTimer = setTimeout(() => dropMessage.set(null), 4000);
}

/** Build DownloadOptions from the current Download-view settings. */
function optionsFor(url: string): DownloadOptions {
  const s = get(downloadStore);
  const adv = s.advanced;
  return {
    url,
    outputDir: s.outputDir,
    preset: s.preset,
    subtitleLangs: adv.subtitleLangs,
    subtitleMode: adv.subtitleMode,
    autoGenSubs: adv.autoGenSubs,
    sponsorblock: adv.sponsorblock,
    cookiesSource: adv.cookiesSource,
    cookiesBrowser: adv.cookiesBrowser || null,
    cookiesFile: adv.cookiesFile || null,
    rateLimit: adv.rateLimit || null,
    retries: adv.retries,
    fragmentRetries: adv.fragmentRetries,
    outputTemplate: adv.outputTemplate || null,
    conflictMode: adv.conflictMode,
    embedThumbnail: adv.embedThumbnail,
    embedMetadata: adv.embedMetadata,
    embedChapters: adv.embedChapters,
    outputFormat: adv.outputFormat,
  };
}

function extractUrls(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => URL_RE.test(l));
}

/** Parse a Windows `.url` internet shortcut (`URL=https://…`). */
function extractShortcutUrl(text: string): string[] {
  const m = text.match(/^URL=(https?:\/\/\S+)$/im);
  return m ? [m[1]] : [];
}

async function urlsFromPaths(paths: string[]): Promise<string[]> {
  const urls: string[] = [];
  for (const path of paths) {
    const ext = path.split('.').pop()?.toLowerCase() ?? '';
    if (!TEXT_FILE_EXTS.has(ext)) continue;
    try {
      const text = await ipc.readDroppedText(path);
      urls.push(...(ext === 'url' ? extractShortcutUrl(text) : extractUrls(text)));
    } catch (err) {
      console.warn('[dragdrop] failed to read dropped file', path, err);
    }
  }
  return urls;
}

async function enqueueUrls(urls: string[]): Promise<void> {
  const unique = [...new Set(urls)].slice(0, MAX_DROPPED_URLS);
  if (unique.length === 0) {
    flashMessage('No links found in that drop.');
    return;
  }

  // A single URL goes through the normal probe flow so the user can
  // review formats and options before adding.
  if (unique.length === 1) {
    currentView.set('download');
    await probe(unique[0]);
    return;
  }

  if (!get(downloadStore).outputDir) {
    flashMessage('Set an output folder on the Download view first.');
    return;
  }

  currentView.set('queue');
  flashMessage(`Probing ${unique.length} links…`);
  let queued = 0;
  let failed = 0;

  for (const url of unique) {
    try {
      const result = await ipc.probeUrl(url);
      if (result.kind === 'single') {
        addToQueue({
          options: optionsFor(url),
          display: {
            title: result.info.title,
            thumbnail: result.info.thumbnail,
            uploader: result.info.uploader,
            duration: result.info.duration,
          },
        });
        queued += 1;
      } else {
        const group = { id: newGroupId(), title: result.info.title };
        for (const entry of result.info.entries) {
          addToQueue({
            options: optionsFor(entry.url),
            display: {
              title: entry.title,
              thumbnail: entry.thumbnail,
              uploader: entry.uploader,
              duration: entry.duration,
            },
            group: result.info.entries.length > 1 ? group : null,
          });
          queued += 1;
        }
      }
    } catch (err) {
      failed += 1;
      console.warn('[dragdrop] probe failed for', url, err);
    }
  }

  flashMessage(
    failed > 0 ? `Queued ${queued} links (${failed} failed)` : `Queued ${queued} links`
  );
}

function domText(e: DragEvent): string {
  return (
    e.dataTransfer?.getData('text/uri-list') ||
    e.dataTransfer?.getData('text/plain') ||
    ''
  );
}

export async function initDragDrop(): Promise<void> {
  if (installed) return;
  installed = true;

  // File drops (native, via Tauri).
  unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
    const { type } = event.payload;
    if (type === 'enter' || type === 'over') {
      dropHover.set(true);
    } else if (type === 'leave') {
      dropHover.set(false);
    } else if (type === 'drop') {
      dropHover.set(false);
      void urlsFromPaths(event.payload.paths).then((urls) => {
        // A file drop with no recognizable text files is just ignored.
        if (urls.length > 0) return enqueueUrls(urls);
      });
    }
  });

  // Text/link drags (DOM path).
  window.addEventListener('dragover', (e) => {
    e.preventDefault();
  });
  window.addEventListener('drop', (e) => {
    e.preventDefault();
    dropHover.set(false);
    const text = domText(e);
    if (!text) return;
    const urls = extractUrls(text);
    if (urls.length > 0) void enqueueUrls(urls);
  });
}

export function disposeDragDrop(): void {
  if (unlistenDragDrop) {
    unlistenDragDrop();
    unlistenDragDrop = null;
  }
  installed = false;
}
