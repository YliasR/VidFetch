import { writable, get } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';

export type UpdateChannel = 'stable' | 'nightly';

const STORE_FILE = 'settings.json';
const KEY = 'updateChannel';

const persisted = new LazyStore(STORE_FILE);

export const updateChannel = writable<UpdateChannel>('stable');

let loaded = false;

export async function initUpdateChannel(): Promise<void> {
  if (loaded) return;
  loaded = true;
  try {
    const saved = await persisted.get<UpdateChannel>(KEY);
    if (saved === 'stable' || saved === 'nightly') {
      updateChannel.set(saved);
    }
  } catch (err) {
    console.warn('[updates] channel load failed', err);
  }
}

export async function setUpdateChannel(channel: UpdateChannel): Promise<void> {
  updateChannel.set(channel);
  try {
    await persisted.set(KEY, channel);
    await persisted.save();
  } catch (err) {
    console.warn('[updates] channel save failed', err);
  }
}

export function currentChannel(): UpdateChannel {
  return get(updateChannel);
}
