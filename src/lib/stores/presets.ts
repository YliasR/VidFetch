import { writable, get } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';
import type { AdvancedOptions } from './download';
import type { QualityPreset } from '$lib/types';

export interface SavedPreset {
  id: string;
  name: string;
  preset: QualityPreset;
  advanced: AdvancedOptions;
  archiveEnabled: boolean;
  archivePath: string;
  createdAt: number;
  updatedAt: number;
}

interface PresetsState {
  presets: SavedPreset[];
  activePresetId: string | null;
}

const STORE_FILE = 'settings.json';
const KEY = 'presets';

const persisted = new LazyStore(STORE_FILE);
const initial: PresetsState = { presets: [], activePresetId: null };

export const presetsStore = writable<PresetsState>(initial);

let loaded = false;

function newId(): string {
  return `p_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export async function initPresets(): Promise<void> {
  if (loaded) return;
  loaded = true;
  try {
    const saved = await persisted.get<PresetsState>(KEY);
    if (saved && Array.isArray(saved.presets)) {
      presetsStore.set({
        presets: saved.presets,
        activePresetId: saved.activePresetId ?? null,
      });
    }
  } catch (err) {
    console.warn('[presets] load failed', err);
  }
}

export async function savePreset(input: {
  id?: string;
  name: string;
  preset: QualityPreset;
  advanced: AdvancedOptions;
  archiveEnabled?: boolean;
  archivePath?: string;
}): Promise<string> {
  const name = input.name.trim();
  if (!name) throw new Error('Preset name is required');

  const now = Date.now();
  let id = input.id ?? newId();
  presetsStore.update((s) => {
    const existing = s.presets.find((p) => p.id === input.id);
    const next: SavedPreset = {
      id,
      name,
      preset: input.preset,
      advanced: structuredClone(input.advanced),
      archiveEnabled: input.archiveEnabled ?? existing?.archiveEnabled ?? false,
      archivePath: input.archivePath ?? existing?.archivePath ?? '',
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
    };
    const presets = existing
      ? s.presets.map((p) => (p.id === existing.id ? next : p))
      : [next, ...s.presets];
    return { ...s, presets };
  });
  await save();
  return id;
}

export async function deletePreset(id: string): Promise<void> {
  presetsStore.update((s) => ({
    presets: s.presets.filter((p) => p.id !== id),
    activePresetId: s.activePresetId === id ? null : s.activePresetId,
  }));
  await save();
}

export async function setActivePreset(id: string | null): Promise<void> {
  presetsStore.update((s) => ({ ...s, activePresetId: id }));
  await save();
}

export async function updatePresetArchive(
  id: string,
  patch: { archiveEnabled?: boolean; archivePath?: string }
): Promise<void> {
  presetsStore.update((s) => ({
    ...s,
    presets: s.presets.map((p) =>
      p.id === id ? { ...p, ...patch, updatedAt: Date.now() } : p
    ),
  }));
  await save();
}

async function save(): Promise<void> {
  try {
    await persisted.set(KEY, get(presetsStore));
    await persisted.save();
  } catch (err) {
    console.warn('[presets] save failed', err);
  }
}
