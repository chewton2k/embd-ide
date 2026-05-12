import { writable } from 'svelte/store';
import { persistedNumber, persistedBool, persistedBoolDefaultTrue } from '../persisted';

export const autosaveEnabled = persistedBoolDefaultTrue('leo-autosave');
export const autosaveDelay = persistedNumber('leo-autosave-delay', 1000);
export const editorFontSize = persistedNumber('leo-editor-font-size', 13);
export const editorTabSize = persistedNumber('leo-editor-tab-size', 2);
export const editorWordWrap = persistedBool('leo-editor-word-wrap', false);
export const editorLineNumbers = persistedBoolDefaultTrue('leo-editor-line-numbers');
export const terminalFontSize = persistedNumber('leo-terminal-font-size', 13);

const DEFAULT_HIDDEN_PATTERNS = ['node_modules', 'target', '.git', '.vscode', '.DS_Store'];

function loadHiddenPatterns(): { pattern: string; enabled: boolean }[] {
  const stored = localStorage.getItem('leo-hidden-patterns');
  if (stored) { try { return JSON.parse(stored); } catch { /* fall through */ } }
  return DEFAULT_HIDDEN_PATTERNS.map(p => ({ pattern: p, enabled: true }));
}

export const hiddenPatterns = writable<{ pattern: string; enabled: boolean }[]>(loadHiddenPatterns());
hiddenPatterns.subscribe(patterns => {
  localStorage.setItem('leo-hidden-patterns', JSON.stringify(patterns));
});
