import { writable, get } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

interface NotifPrefs {
  enabled: boolean;
}

const STORE_FILE = 'settings.json';
const KEY = 'notifications';
const defaultPrefs: NotifPrefs = { enabled: true };
const persisted = new LazyStore(STORE_FILE);

export const notifPrefs = writable<NotifPrefs>(defaultPrefs);

let permissionGranted: boolean | null = null;
let loaded = false;

export async function initNotifications(): Promise<void> {
  if (loaded) return;
  loaded = true;
  try {
    const saved = await persisted.get<NotifPrefs>(KEY);
    if (saved && typeof saved === 'object') {
      notifPrefs.set({ ...defaultPrefs, ...saved });
    }
  } catch (err) {
    console.warn('[notif] load failed', err);
  }
}

export async function setNotificationsEnabled(enabled: boolean): Promise<void> {
  notifPrefs.set({ enabled });
  try {
    await persisted.set(KEY, get(notifPrefs));
    await persisted.save();
  } catch (err) {
    console.warn('[notif] save failed', err);
  }
  if (enabled && permissionGranted !== true) {
    await ensurePermission();
  }
}

async function ensurePermission(): Promise<boolean> {
  if (permissionGranted !== null) return permissionGranted;
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const result = await requestPermission();
      granted = result === 'granted';
    }
    permissionGranted = granted;
  } catch (err) {
    console.warn('[notif] permission check failed', err);
    permissionGranted = false;
  }
  return permissionGranted;
}

export async function notifyJobDone(title: string, body: string): Promise<void> {
  if (!get(notifPrefs).enabled) return;
  const ok = await ensurePermission();
  if (!ok) return;
  try {
    sendNotification({ title, body });
  } catch (err) {
    console.warn('[notif] send failed', err);
  }
}
