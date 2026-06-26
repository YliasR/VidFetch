import { writable } from 'svelte/store';
import { currentView } from './nav';

/**
 * A local file path waiting to be loaded into the Edit tab. Set by
 * `sendToEditor` (from the Queue / History views) and consumed by EditView,
 * which loads it as the source clip and then clears this back to null.
 */
export const pendingEditFile = writable<string | null>(null);

/** Hand a finished file off to the Edit tab and switch to it. */
export function sendToEditor(path: string): void {
  pendingEditFile.set(path);
  currentView.set('edit');
}
