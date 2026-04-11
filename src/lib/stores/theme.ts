import { writable, get } from 'svelte/store';
import { LazyStore } from '@tauri-apps/plugin-store';

export type Theme = 'dark' | 'light' | 'fox';

interface ThemeState {
  current: Theme;
  foxUnlocked: boolean;
}

const STORE_FILE = 'settings.json';
const KEY = 'theme';

const persisted = new LazyStore(STORE_FILE);

const defaultState: ThemeState = {
  current: systemPrefersDark() ? 'dark' : 'light',
  foxUnlocked: false,
};

export const themeState = writable<ThemeState>(defaultState);

function systemPrefersDark(): boolean {
  if (typeof window === 'undefined') return true;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function applyTheme(theme: Theme) {
  document.documentElement.setAttribute('data-theme', theme);
}

export async function initTheme(): Promise<void> {
  try {
    const saved = await persisted.get<ThemeState>(KEY);
    if (saved && typeof saved === 'object') {
      themeState.set({ ...defaultState, ...saved });
    }
  } catch (err) {
    console.warn('[theme] failed to load persisted state', err);
  }

  applyTheme(get(themeState).current);
  themeState.subscribe((s) => applyTheme(s.current));

  installKonamiListener();
}

export async function setTheme(theme: Theme): Promise<void> {
  themeState.update((s) => ({ ...s, current: theme }));
  await save();
}

export async function cycleTheme(): Promise<void> {
  const s = get(themeState);
  const order: Theme[] = s.foxUnlocked ? ['dark', 'light', 'fox'] : ['dark', 'light'];
  const next = order[(order.indexOf(s.current) + 1) % order.length];
  await setTheme(next);
}

async function save(): Promise<void> {
  try {
    await persisted.set(KEY, get(themeState));
    await persisted.save();
  } catch (err) {
    console.warn('[theme] failed to persist state', err);
  }
}

/* Konami: ↑↑↓↓←→←→BA */
const KONAMI = [
  'ArrowUp',
  'ArrowUp',
  'ArrowDown',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'ArrowLeft',
  'ArrowRight',
  'KeyB',
  'KeyA',
];

function installKonamiListener() {
  let idx = 0;
  window.addEventListener('keydown', (e) => {
    if (e.code === KONAMI[idx]) {
      idx += 1;
      if (idx === KONAMI.length) {
        idx = 0;
        void unlockFox();
      }
    } else {
      idx = e.code === KONAMI[0] ? 1 : 0;
    }
  });
}

async function unlockFox(): Promise<void> {
  const s = get(themeState);
  if (!s.foxUnlocked) {
    themeState.set({ current: 'fox', foxUnlocked: true });
    await save();
    flashUnlockToast();
  } else {
    await setTheme('fox');
  }
}

function flashUnlockToast() {
  const toast = document.createElement('div');
  toast.textContent = '🦊 fox mode unlocked :3';
  toast.style.cssText = `
    position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%);
    padding: 12px 20px; border-radius: 12px;
    background: var(--accent); color: var(--accent-fg);
    font-weight: 600; box-shadow: var(--shadow-lg);
    z-index: 9999; pointer-events: none;
    opacity: 0; transition: opacity 200ms ease, transform 400ms cubic-bezier(0.2, 0.9, 0.3, 1.4);
  `;
  document.body.appendChild(toast);
  requestAnimationFrame(() => {
    toast.style.opacity = '1';
    toast.style.transform = 'translateX(-50%) translateY(-6px)';
  });
  setTimeout(() => {
    toast.style.opacity = '0';
    setTimeout(() => toast.remove(), 300);
  }, 2400);
}
