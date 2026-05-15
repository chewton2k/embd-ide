/**
 * Search index for the Settings window.
 *
 * The sidebar search matches both tab-level entries AND individual settings
 * (e.g. "Inline completions", "Error lens", "Word wrap"). Each entry declares
 * which tab contains it and an `anchor` id that matches a `data-setting`
 * attribute in the corresponding section component. When a user picks a
 * result, the Settings window switches to that tab and scrolls + highlights
 * the target row.
 *
 * Keep `anchor` values stable — they act as keys across components and are
 * referenced in section markup via `data-setting="..."`.
 */

export type SettingsTabId = 'general' | 'terminal' | 'shortcuts' | 'models' | 'agents' | 'knowledge' | 'about';

export interface SettingsTab {
  id: SettingsTabId;
  label: string;
  /** Space-separated synonyms used by the fuzzy search. */
  keywords: string;
  /** SVG path data (for the 24×24 icon rendered in the sidebar). */
  icon: string;
}

export interface SettingsEntry {
  /** Unique anchor id — must match `data-setting` on the target row. */
  anchor: string;
  tab: SettingsTabId;
  /** Human-readable label shown in the search result. */
  label: string;
  /** Extra synonyms / searchable text. */
  keywords: string;
  /** Optional group name shown as a breadcrumb, e.g. "Editor" or "Autosave". */
  group?: string;
}

export const SETTINGS_TABS: SettingsTab[] = [
  { id: 'general',   label: 'General',   keywords: 'appearance theme editor font tab autosave density hidden patterns preview word wrap line numbers error lens vim',           icon: 'M4 21v-7 M4 10V3 M12 21v-9 M12 8V3 M20 21v-5 M20 12V3 M1 14h6 M9 8h6 M17 16h6' },
  { id: 'terminal',  label: 'Terminal',  keywords: 'shell xterm bash zsh fish layout panel tab font size profile',                                                            icon: 'M3 5h18v14H3z M7 10l3 2-3 2 M13 14h4' },
  { id: 'shortcuts', label: 'Shortcuts', keywords: 'keyboard keybindings hotkeys shortcut',                                                                                   icon: 'M2 8h20v8H2z M6 12h.01 M10 12h.01 M14 12h.01 M18 12h.01' },
  { id: 'models',    label: 'Models',    keywords: 'ai api key openrouter openai anthropic provider claude gpt llm default model',                                             icon: 'M12 2v4 M12 18v4 M4.93 4.93l2.83 2.83 M16.24 16.24l2.83 2.83 M2 12h4 M18 12h4 M4.93 19.07l2.83-2.83 M16.24 7.76l2.83-2.83 M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z' },
  { id: 'agents',    label: 'Agents',    keywords: 'assistant chat inline completions ghost text autocomplete auto-approve agent',                                             icon: 'M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M3 21v-1a6 6 0 0 1 6-6 6 6 0 0 1 6 6v1 M17 8a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z M14 14a4 4 0 0 1 8 0' },
  { id: 'knowledge', label: 'Knowledge', keywords: 'database sqlite storage index brain memory graph conversations',                                                            icon: 'M12 2L2 7l10 5 10-5-10-5Z M2 17l10 5 10-5 M2 12l10 5 10-5' },
  { id: 'about',     label: 'About',     keywords: 'version info tauri',                                                                                                       icon: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z M12 8v4 M12 16h.01' },
];

export const SETTINGS_ENTRIES: SettingsEntry[] = [
  // ── General › Appearance / Theme / Interface ──
  { anchor: 'appearance',            tab: 'general', group: 'Appearance', label: 'Appearance',            keywords: 'theme system light dark mode color scheme' },
  { anchor: 'editor-theme',          tab: 'general', group: 'Editor theme', label: 'Editor theme',        keywords: 'syntax highlight color scheme one dark dracula github tokyo night nord catppuccin rose pine solarized' },
  { anchor: 'ui-font-size',          tab: 'general', group: 'Interface', label: 'UI font size',           keywords: 'text size chrome sidebar toolbar' },
  { anchor: 'ui-density',            tab: 'general', group: 'Interface', label: 'Density',                keywords: 'compact comfortable spacing padding' },

  // ── General › Editor ──
  { anchor: 'editor-font-size',      tab: 'general', group: 'Editor', label: 'Editor font size',         keywords: 'text size code monospace' },
  { anchor: 'editor-tab-size',       tab: 'general', group: 'Editor', label: 'Tab size',                 keywords: 'indent spaces 2 4' },
  { anchor: 'editor-word-wrap',      tab: 'general', group: 'Editor', label: 'Word wrap',                keywords: 'wrap lines soft wrapping long lines' },
  { anchor: 'editor-line-numbers',   tab: 'general', group: 'Editor', label: 'Line numbers',             keywords: 'gutter row numbers' },
  { anchor: 'editor-error-lens',     tab: 'general', group: 'Editor', label: 'Error lens',               keywords: 'inline errors diagnostics syntax warnings squiggles' },
  { anchor: 'editor-vim-mode',      tab: 'general', group: 'Editor', label: 'Vim mode',                 keywords: 'vim vi keybindings modal hjkl normal insert visual' },

  // ── Terminal ──
  { anchor: 'terminal-mode',         tab: 'terminal', group: 'Layout', label: 'Terminal layout',         keywords: 'tab panel bottom docked vscode zed xcode placement position surface' },
  { anchor: 'terminal-font-size',    tab: 'terminal', group: 'Typography', label: 'Terminal font size',  keywords: 'text size shell xterm' },

  // ── General › Preview ──
  { anchor: 'preview-default-url',   tab: 'general', group: 'Preview', label: 'Preview default URL',     keywords: 'localhost web browser dev server iframe' },

  // ── General › Autosave ──
  { anchor: 'autosave-enabled',      tab: 'general', group: 'Autosave', label: 'Autosave',               keywords: 'save automatic auto save on change' },
  { anchor: 'autosave-delay',        tab: 'general', group: 'Autosave', label: 'Autosave delay',         keywords: 'save interval debounce timeout' },

  // ── General › Files / Tabs ──
  { anchor: 'max-recent-projects',   tab: 'general', group: 'Files', label: 'Max recent projects',       keywords: 'history recent folders' },
  { anchor: 'max-tabs',              tab: 'general', group: 'Files', label: 'Max open tabs',             keywords: 'tab limit open files' },
  { anchor: 'hidden-patterns',       tab: 'general', group: 'Files', label: 'Hidden files / patterns',   keywords: 'ignore exclude node_modules .git dotfiles filter filetree' },

  // ── General › Import/Export ──
  { anchor: 'export-import-settings', tab: 'general', group: 'Settings', label: 'Export / import settings', keywords: 'backup restore json config' },

  // ── Models ──
  { anchor: 'default-model',         tab: 'models', group: 'AI', label: 'Default model',                 keywords: 'llm ai claude gpt gemini grok deepseek llama auto openrouter openai anthropic' },
  { anchor: 'openrouter-api-key',    tab: 'models', group: 'API keys', label: 'OpenRouter API key',      keywords: 'sk-or openrouter credentials secret' },
  { anchor: 'openai-api-key',        tab: 'models', group: 'API keys', label: 'OpenAI API key',          keywords: 'sk- openai gpt credentials secret' },
  { anchor: 'anthropic-api-key',     tab: 'models', group: 'API keys', label: 'Anthropic API key',       keywords: 'sk-ant anthropic claude credentials secret' },

  // ── Agents ──
  { anchor: 'inline-completions',    tab: 'agents', group: 'Inline completions', label: 'Inline completions',       keywords: 'ghost text autocomplete suggestions copilot tab complete' },
  { anchor: 'inline-completion-delay', tab: 'agents', group: 'Inline completions', label: 'Inline completion delay', keywords: 'ghost text trigger debounce timeout' },
  { anchor: 'agent-max-steps',       tab: 'agents', group: 'Agent mode', label: 'Agent max steps',        keywords: 'iterations loop limit tool calls' },
  { anchor: 'agent-auto-approve',    tab: 'agents', group: 'Agent mode', label: 'Auto-approve edits',     keywords: 'agent yolo approval confirm changes' },

  // ── Knowledge ──
  { anchor: 'knowledge-index',       tab: 'knowledge', group: 'Knowledge', label: 'Knowledge index',      keywords: 'reindex database files project memory' },
  { anchor: 'knowledge-conversations', tab: 'knowledge', group: 'Knowledge', label: 'Conversations',      keywords: 'chat history memory clear delete' },

  // ── Shortcuts (one-entry) ──
  { anchor: 'shortcuts',             tab: 'shortcuts', group: 'Shortcuts', label: 'Keyboard shortcuts',   keywords: 'keybindings hotkeys cmd ctrl' },

  // ── About (one-entry) ──
  { anchor: 'about',                 tab: 'about', group: 'About', label: 'About leo',                   keywords: 'version info tauri' },
];

/** Case-insensitive token match. Each whitespace-separated token in the query
 *  must appear somewhere in the searchable haystack. Empty query → no filter. */
export function matchesQuery(haystack: string, query: string): boolean {
  const tokens = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return true;
  const hay = haystack.toLowerCase();
  return tokens.every(t => hay.includes(t));
}

export interface SettingsSearchResult {
  entry: SettingsEntry;
  tab: SettingsTab;
}

/** Filter the settings registry by a query, returning both matching entries
 *  and (separately) matching tabs. Tabs whose label matches are prepended,
 *  but individual setting matches are the main attraction. */
export function searchSettings(query: string): SettingsSearchResult[] {
  const q = query.trim();
  if (!q) return [];
  const tabById = new Map(SETTINGS_TABS.map(t => [t.id, t]));
  const results: SettingsSearchResult[] = [];

  for (const entry of SETTINGS_ENTRIES) {
    const tab = tabById.get(entry.tab);
    if (!tab) continue;
    const hay = `${entry.label} ${entry.keywords} ${entry.group ?? ''} ${tab.label}`;
    if (matchesQuery(hay, q)) results.push({ entry, tab });
  }
  return results;
}
