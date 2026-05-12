export * from './files';
export * from './ai';
export * from './git';
export * from './settings';
export * from './shell';
export * from './ui';
export { THEMES, getTheme, type ThemeColors, type ThemePreset } from '../themes';

// ── Cross-window settings sync ───────────────────────────────────
import { autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers, terminalFontSize, hiddenPatterns } from './settings';
import { currentThemeId, uiFontSize, uiDensity } from './ui';
import { maxRecentProjects, maxTabs } from './files';
import { apiKey, openaiApiKey, anthropicApiKey, aiProvider, aiModel, type AiProvider } from './ai';

const SETTINGS_SYNC: Record<string, { set: (v: string | null) => void }> = {
  'leo-autosave':            { set: v => autosaveEnabled.set(v !== 'false') },
  'leo-autosave-delay':      { set: v => autosaveDelay.set(parseInt(v || '1000', 10)) },
  'leo-editor-font-size':    { set: v => editorFontSize.set(parseInt(v || '13', 10)) },
  'leo-editor-tab-size':     { set: v => editorTabSize.set(parseInt(v || '2', 10)) },
  'leo-editor-word-wrap':    { set: v => editorWordWrap.set(v === 'true') },
  'leo-editor-line-numbers': { set: v => editorLineNumbers.set(v !== 'false') },
  'leo-terminal-font-size':  { set: v => terminalFontSize.set(parseInt(v || '13', 10)) },
  'leo-theme':               { set: v => currentThemeId.set(v || 'catppuccin-mocha') },
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
