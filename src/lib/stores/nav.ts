import { writable } from 'svelte/store';

export type View = 'download' | 'queue' | 'history' | 'presets' | 'settings';

export const currentView = writable<View>('download');
