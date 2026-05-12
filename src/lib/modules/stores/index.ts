export * from './files';
export * from './ai';
export * from './git';
export * from './settings';
export * from './shell';
export * from './ui';
export * from './pendingEdits';
export * from './aiHistory';
export { EDITOR_THEMES, EDITOR_THEME_LABELS, isLightEditorTheme, type AppearanceMode, type EditorThemeId } from '../themes';

// ── Cross-window settings sync ───────────────────────────────────
import { autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers, editorShowErrorLens, terminalFontSize, hiddenPatterns } from './settings';
import { appearanceMode, editorTheme, uiFontSize, uiDensity } from './ui';
import { maxRecentProjects, maxTabs } from './files';
import { aiProvider, aiModel, type AiProvider } from './ai';
import type { AppearanceMode, EditorThemeId } from '../themes';

const SETTINGS_SYNC: Record<string, { set: (v: string | null) => void }> = {
  'leo-autosave':            { set: v => autosaveEnabled.set(v !== 'false') },
  'leo-autosave-delay':      { set: v => autosaveDelay.set(parseInt(v || '1000', 10)) },
  'leo-editor-font-size':    { set: v => editorFontSize.set(parseInt(v || '13', 10)) },
  'leo-editor-tab-size':     { set: v => editorTabSize.set(parseInt(v || '2', 10)) },
  'leo-editor-word-wrap':    { set: v => editorWordWrap.set(v === 'true') },
  'leo-editor-line-numbers': { set: v => editorLineNumbers.set(v !== 'false') },
  'leo-editor-show-error-lens': { set: v => editorShowErrorLens.set(v !== 'false') },
  'leo-terminal-font-size':  { set: v => terminalFontSize.set(parseInt(v || '13', 10)) },
  'leo-appearance':          { set: v => appearanceMode.set((v as AppearanceMode) || 'system') },
  'leo-editor-theme':        { set: v => editorTheme.set((v as EditorThemeId) || 'one-dark') },
  'leo-ui-font-size':        { set: v => uiFontSize.set(parseInt(v || '13', 10)) },
  'leo-ui-density':          { set: v => uiDensity.set((v as 'compact' | 'comfortable') || 'comfortable') },
  'leo-hidden-patterns':     { set: v => { try { hiddenPatterns.set(JSON.parse(v || '[]')); } catch { /* ignore */ } } },
  'leo-max-recent-projects': { set: v => maxRecentProjects.set(parseInt(v || '3', 10)) },
  'leo-max-tabs':            { set: v => maxTabs.set(parseInt(v || '9', 10)) },
  'leo-ai-provider':         { set: v => aiProvider.set((v as AiProvider) || 'openrouter') },
  'leo-ai-model':            { set: v => aiModel.set(v || 'openrouter/auto') },
};

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (!e.key) return;
    const entry = SETTINGS_SYNC[e.key];
    if (entry) entry.set(e.newValue);
  });
}
