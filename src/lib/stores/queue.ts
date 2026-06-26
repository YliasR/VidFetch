import { writable, get, derived } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { LazyStore } from '@tauri-apps/plugin-store';
import { ipc } from '$lib/ipc';
import type {
  DownloadLogEvent,
  DownloadOptions,
  DownloadProgressEvent,
  DownloadStatusEvent,
  DownloadStatusKind,
} from '$lib/types';
import { addHistoryEntry, initHistory } from './history';
import { initNotifications, notifyJobDone } from './notifications';

export type QueueItemStatus =
  | 'queued'
  | 'starting'
  | 'downloading'
  | 'paused'
  | 'postprocess'
  | 'done'
  | 'error'
  | 'canceled';

export interface QueueItemDisplay {
  title: string;
  thumbnail: string | null;
  uploader: string | null;
  duration: number | null;
}

export interface QueueLogLine {
  line: string;
  stream: 'stdout' | 'stderr';
}

const MAX_LOG_LINES_PER_ITEM = 800;

/** Items enqueued together from one playlist probe share a group. */
export interface QueueItemGroup {
  id: string;
  title: string;
}

export interface QueueItem {
  id: string;
  rustId: string | null;
  options: DownloadOptions;
  display: QueueItemDisplay;
  status: QueueItemStatus;
  downloaded: number;
  total: number | null;
  speed: number | null;
  eta: number | null;
  message: string | null;
  addedAt: number;
  group: QueueItemGroup | null;
  /** Final on-disk path once done — drives "open folder" / "send to editor". */
  filePath: string | null;
}

interface QueueState {
  items: QueueItem[];
  concurrency: number;
  logs: Record<string, QueueLogLine[]>;
}

const initial: QueueState = {
  items: [],
  concurrency: 2,
  logs: {},
};

export const queueStore = writable<QueueState>(initial);

export const activeCount = derived(queueStore, ($q) =>
  $q.items.filter(
    (i) =>
      i.status === 'queued' ||
      i.status === 'starting' ||
      i.status === 'downloading' ||
      i.status === 'paused' ||
      i.status === 'postprocess'
  ).length
);

let listenersInstalled = false;
let unlistenStatus: UnlistenFn | null = null;
let unlistenProgress: UnlistenFn | null = null;
let unlistenLog: UnlistenFn | null = null;

const PERSIST_FILE = 'settings.json';
const PERSIST_KEY = 'queue';
const persisted = new LazyStore(PERSIST_FILE);

interface PersistedQueue {
  items: QueueItem[];
  concurrency: number;
}

/**
 * Persist the queue snapshot. Called on structural mutations (add / remove /
 * status change / reorder) but never on progress ticks. Logs and live rust
 * job ids are intentionally not persisted — they don't survive the process.
 */
async function saveQueue(): Promise<void> {
  try {
    const s = get(queueStore);
    const items = s.items.map((i) => ({ ...i, rustId: null }));
    await persisted.set(PERSIST_KEY, { items, concurrency: s.concurrency } satisfies PersistedQueue);
    await persisted.save();
  } catch (err) {
    console.warn('[queue] save failed', err);
  }
}

async function loadQueue(): Promise<void> {
  try {
    const saved = await persisted.get<PersistedQueue>(PERSIST_KEY);
    if (!saved || !Array.isArray(saved.items)) return;
    // Anything that was in flight died with the previous process: requeue it.
    const items: QueueItem[] = saved.items.map((i) => {
      const base = { ...i, rustId: null, group: i.group ?? null, filePath: i.filePath ?? null };
      const terminal = i.status === 'done' || i.status === 'error' || i.status === 'canceled';
      return terminal
        ? base
        : {
            ...base,
            status: 'queued' as QueueItemStatus,
            downloaded: 0,
            total: null,
            speed: null,
            eta: null,
            message: null,
          };
    });
    const concurrency =
      typeof saved.concurrency === 'number'
        ? Math.max(1, Math.min(5, Math.floor(saved.concurrency)))
        : initial.concurrency;
    queueStore.update((s) => ({ ...s, items, concurrency }));
  } catch (err) {
    console.warn('[queue] load failed', err);
  }
}

function newId(): string {
  return `q_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

function mapStatus(s: DownloadStatusKind): QueueItemStatus {
  return s;
}

export async function initQueue(): Promise<void> {
  if (listenersInstalled) return;
  listenersInstalled = true;

  await initHistory();
  await initNotifications();
  await loadQueue();

  unlistenStatus = await listen<DownloadStatusEvent>('download://status', (e) => {
    const payload = e.payload;
    let completed: QueueItem | null = null;
    let failed: QueueItem | null = null;
    let changed = false;

    queueStore.update((s) => ({
      ...s,
      items: s.items.map((item) => {
        if (item.rustId !== payload.id) return item;
        const next: QueueItem = {
          ...item,
          status: mapStatus(payload.status),
          message: payload.message ?? item.message,
          filePath: payload.filePath ?? item.filePath,
        };
        if (next.status !== item.status) changed = true;
        if (next.status === 'done' && item.status !== 'done') completed = next;
        if (next.status === 'error' && item.status !== 'error') failed = next;
        return next;
      }),
    }));

    if (completed) {
      void recordCompletion(completed);
    }
    if (failed) {
      const f = failed as QueueItem;
      void notifyJobDone('Download failed', f.display.title);
    }
    if (changed) {
      void saveQueue();
    }

    void tick();
  });

  unlistenProgress = await listen<DownloadProgressEvent>('download://progress', (e) => {
    const payload = e.payload;
    queueStore.update((s) => ({
      ...s,
      items: s.items.map((item) =>
        item.rustId === payload.id
          ? {
              ...item,
              downloaded: payload.downloaded,
              total: payload.total,
              speed: payload.speed,
              eta: payload.eta,
            }
          : item
      ),
    }));
  });

  unlistenLog = await listen<DownloadLogEvent>('download://log', (e) => {
    const payload = e.payload;
    queueStore.update((s) => {
      const item = s.items.find((i) => i.rustId === payload.id);
      if (!item) return s;
      const key = item.id;
      const prev = s.logs[key] ?? [];
      const next = prev.length >= MAX_LOG_LINES_PER_ITEM
        ? [...prev.slice(prev.length - MAX_LOG_LINES_PER_ITEM + 1), { line: payload.line, stream: payload.stream }]
        : [...prev, { line: payload.line, stream: payload.stream }];
      return { ...s, logs: { ...s.logs, [key]: next } };
    });
  });
}

async function recordCompletion(item: QueueItem): Promise<void> {
  try {
    await addHistoryEntry({
      title: item.display.title,
      thumbnail: item.display.thumbnail,
      uploader: item.display.uploader,
      url: item.options.url,
      outputDir: item.options.outputDir,
      preset: item.options.preset,
      outputFormat: item.options.outputFormat ?? null,
      sizeBytes: item.total ?? item.downloaded ?? null,
      filePath: item.filePath ?? null,
      options: item.options,
    });
  } catch (err) {
    console.warn('[queue] history record failed', err);
  }
  void notifyJobDone('Download complete', item.display.title);
}

export async function disposeQueue(): Promise<void> {
  if (unlistenStatus) {
    unlistenStatus();
    unlistenStatus = null;
  }
  if (unlistenProgress) {
    unlistenProgress();
    unlistenProgress = null;
  }
  if (unlistenLog) {
    unlistenLog();
    unlistenLog = null;
  }
  listenersInstalled = false;
}

export function addToQueue(params: {
  options: DownloadOptions;
  display: QueueItemDisplay;
  group?: QueueItemGroup | null;
}): string {
  const item: QueueItem = {
    id: newId(),
    rustId: null,
    options: params.options,
    display: params.display,
    status: 'queued',
    downloaded: 0,
    total: null,
    speed: null,
    eta: null,
    message: null,
    addedAt: Date.now(),
    group: params.group ?? null,
    filePath: null,
  };
  queueStore.update((s) => ({ ...s, items: [...s.items, item] }));
  void saveQueue();
  void tick();
  return item.id;
}

export function newGroupId(): string {
  return `g_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export function removeFromQueue(id: string): void {
  queueStore.update((s) => {
    const { [id]: _, ...rest } = s.logs;
    return {
      ...s,
      items: s.items.filter((i) => i.id !== id),
      logs: rest,
    };
  });
  void saveQueue();
}

export async function cancelItem(id: string): Promise<void> {
  const state = get(queueStore);
  const item = state.items.find((i) => i.id === id);
  if (!item) return;

  if (item.rustId) {
    try {
      await ipc.cancelDownload(item.rustId);
    } catch (err) {
      console.warn('[queue] cancel failed', err);
    }
    return;
  }

  queueStore.update((s) => ({
    ...s,
    items: s.items.map((i) =>
      i.id === id ? { ...i, status: 'canceled' as QueueItemStatus } : i
    ),
  }));
  void saveQueue();
  void tick();
}

function isPending(s: QueueItemStatus): boolean {
  return (
    s === 'queued' ||
    s === 'starting' ||
    s === 'downloading' ||
    s === 'paused' ||
    s === 'postprocess'
  );
}

/** Cancel every still-pending item that belongs to a playlist group. */
export async function cancelGroup(groupId: string): Promise<void> {
  const items = get(queueStore).items.filter((i) => i.group?.id === groupId);
  for (const item of items) {
    if (isPending(item.status)) {
      await cancelItem(item.id);
    }
  }
}

/** Remove a whole playlist group, canceling anything still running. */
export async function removeGroup(groupId: string): Promise<void> {
  await cancelGroup(groupId);
  queueStore.update((s) => {
    const removed = new Set(
      s.items.filter((i) => i.group?.id === groupId).map((i) => i.id)
    );
    const logs: Record<string, QueueLogLine[]> = {};
    for (const [k, v] of Object.entries(s.logs)) {
      if (!removed.has(k)) logs[k] = v;
    }
    return {
      ...s,
      items: s.items.filter((i) => !removed.has(i.id)),
      logs,
    };
  });
  void saveQueue();
}

/** Suspend the yt-dlp process of an actively downloading item. */
export async function pauseItem(id: string): Promise<void> {
  const item = get(queueStore).items.find((i) => i.id === id);
  if (!item?.rustId || item.status !== 'downloading') return;
  try {
    await ipc.pauseDownload(item.rustId);
  } catch (err) {
    console.warn('[queue] pause failed', err);
  }
}

/** Resume a paused item's yt-dlp process. */
export async function resumeItem(id: string): Promise<void> {
  const item = get(queueStore).items.find((i) => i.id === id);
  if (!item?.rustId || item.status !== 'paused') return;
  try {
    await ipc.resumeDownload(item.rustId);
  } catch (err) {
    console.warn('[queue] resume failed', err);
  }
}

/** Re-enqueue a failed or canceled item from scratch. */
export function retryItem(id: string): void {
  queueStore.update((s) => {
    const item = s.items.find((i) => i.id === id);
    if (!item || (item.status !== 'error' && item.status !== 'canceled')) return s;
    const { [id]: _, ...logs } = s.logs;
    return {
      ...s,
      items: s.items.map((i) =>
        i.id === id
          ? {
              ...i,
              status: 'queued' as QueueItemStatus,
              rustId: null,
              downloaded: 0,
              total: null,
              speed: null,
              eta: null,
              message: null,
            }
          : i
      ),
      logs,
    };
  });
  void saveQueue();
  void tick();
}

export function clearCompleted(): void {
  queueStore.update((s) => {
    const keep = s.items.filter(
      (i) => i.status !== 'done' && i.status !== 'canceled' && i.status !== 'error'
    );
    const keepIds = new Set(keep.map((i) => i.id));
    const logs: Record<string, QueueLogLine[]> = {};
    for (const [k, v] of Object.entries(s.logs)) {
      if (keepIds.has(k)) logs[k] = v;
    }
    return { ...s, items: keep, logs };
  });
  void saveQueue();
}

export function moveItem(id: string, direction: -1 | 1): void {
  queueStore.update((s) => {
    const idx = s.items.findIndex((i) => i.id === id);
    if (idx < 0) return s;
    const target = idx + direction;
    if (target < 0 || target >= s.items.length) return s;
    const items = [...s.items];
    [items[idx], items[target]] = [items[target], items[idx]];
    return { ...s, items };
  });
  void saveQueue();
  void tick();
}

export function setConcurrency(n: number): void {
  const clamped = Math.max(1, Math.min(5, Math.floor(n)));
  queueStore.update((s) => ({ ...s, concurrency: clamped }));
  void saveQueue();
  void tick();
}

async function tick(): Promise<void> {
  const state = get(queueStore);
  const running = state.items.filter(
    (i) =>
      i.status === 'starting' ||
      i.status === 'downloading' ||
      i.status === 'postprocess'
  ).length;
  if (running >= state.concurrency) return;

  const next = state.items.find((i) => i.status === 'queued' && i.rustId === null);
  if (!next) return;

  queueStore.update((s) => ({
    ...s,
    items: s.items.map((i) =>
      i.id === next.id ? { ...i, status: 'starting' as QueueItemStatus } : i
    ),
  }));

  try {
    const rustId = await ipc.startDownload(next.options);
    queueStore.update((s) => ({
      ...s,
      items: s.items.map((i) =>
        i.id === next.id ? { ...i, rustId, status: 'queued' as QueueItemStatus } : i
      ),
    }));
    void tick();
  } catch (err) {
    queueStore.update((s) => ({
      ...s,
      items: s.items.map((i) =>
        i.id === next.id
          ? { ...i, status: 'error' as QueueItemStatus, message: String(err) }
          : i
      ),
    }));
    void tick();
  }
}
