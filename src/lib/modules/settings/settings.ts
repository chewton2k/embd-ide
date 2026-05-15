import { writable } from 'svelte/store';
import { persistedString, persistedNumber, persistedBool, persistedBoolDefaultTrue } from '../session/persisted';

export const autosaveEnabled = persistedBoolDefaultTrue('leo-autosave');
export const autosaveDelay = persistedNumber('leo-autosave-delay', 1000);
export const editorFontSize = persistedNumber('leo-editor-font-size', 13);
export const editorTabSize = persistedNumber('leo-editor-tab-size', 2);
export const editorWordWrap = persistedBool('leo-editor-word-wrap', false);
export const editorLineNumbers = persistedBoolDefaultTrue('leo-editor-line-numbers');
export const editorShowErrorLens = persistedBoolDefaultTrue('leo-editor-show-error-lens');
export const editorVimMode = persistedBool('leo-editor-vim-mode', false);
export const terminalFontSize = persistedNumber('leo-terminal-font-size', 13);
export const previewUrl = persistedString('leo-preview-url', 'http://localhost:3000');

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

// AI preferences
export const ghostTextEnabled = persistedBool('leo-ghost-text-enabled', true);
export const ghostTextDelay = persistedNumber('leo-ghost-text-delay', 450);
export const agentMaxStepsConfig = persistedNumber('leo-agent-max-steps', 10);
export const agentAutoApproveConfig = persistedBool('leo-agent-auto-approve', false);
