import { writable, get } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';
import type { DownloadOptions } from '$lib/types';

export interface HistoryEntry {
  id: string;
  title: string;
  thumbnail: string | null;
  uploader: string | null;
  url: string;
  outputDir: string;
  preset: string;
  outputFormat: string | null;
  sizeBytes: number | null;
  completedAt: number;
  options: DownloadOptions;
}

interface HistoryState {
  entries: HistoryEntry[];
}

const STORE_FILE = 'settings.json';
const KEY = 'history';
const MAX_ENTRIES = 500;

const persisted = new LazyStore(STORE_FILE);
const initial: HistoryState = { entries: [] };

export const historyStore = writable<HistoryState>(initial);

let loaded = false;

export async function initHistory(): Promise<void> {
  if (loaded) return;
  loaded = true;
  try {
    const saved = await persisted.get<HistoryState>(KEY);
    if (saved && Array.isArray(saved.entries)) {
      historyStore.set({ entries: saved.entries });
    }
  } catch (err) {
    console.warn('[history] load failed', err);
  }
}

function newEntryId(): string {
  return `h_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export async function addHistoryEntry(
  entry: Omit<HistoryEntry, 'id' | 'completedAt'> & { completedAt?: number }
): Promise<void> {
  const full: HistoryEntry = {
    id: newEntryId(),
    completedAt: entry.completedAt ?? Date.now(),
    ...entry,
  };
  historyStore.update((s) => ({
    entries: [full, ...s.entries].slice(0, MAX_ENTRIES),
  }));
  await save();
}

export async function removeHistoryEntry(id: string): Promise<void> {
  historyStore.update((s) => ({
    entries: s.entries.filter((e) => e.id !== id),
  }));
  await save();
}

export async function clearHistory(): Promise<void> {
  historyStore.set({ entries: [] });
  await save();
}

async function save(): Promise<void> {
  try {
    await persisted.set(KEY, get(historyStore));
    await persisted.save();
  } catch (err) {
    console.warn('[history] save failed', err);
  }
}
