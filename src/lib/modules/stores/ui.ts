import { writable, get } from 'svelte/store';
import { persistedString, persistedNumber } from '../persisted';

export const showSettings = writable<boolean>(false);
export const showChat = writable<boolean>(false);
export const showGit = writable<boolean>(false);
export const triggerSearchInFile = writable<number>(0);
export const openFileSearchSignal = writable<number>(0);
export const openPreviewSignal = writable<number>(0);
export const fileTreeNavTarget = writable<string | null>(null);

export function toggleChatPanel() {
  const next = !get(showChat);
  showChat.set(next);
  if (next) showGit.set(false);
}

export function toggleGitPanel() {
  const next = !get(showGit);
  showGit.set(next);
  if (next) showChat.set(false);
}

// Theme
export const currentThemeId = persistedString('leo-theme', 'catppuccin-mocha');
export const uiFontSize = persistedNumber('leo-ui-font-size', 13);
export const uiDensity = persistedString('leo-ui-density', 'comfortable') as import('svelte/store').Writable<'compact' | 'comfortable'>;
