// ── Appearance mode (IDE chrome) ─────────────────────────────────
export type AppearanceMode = 'system' | 'light' | 'dark';

// ── Editor themes (CodeMirror) ───────────────────────────────────
export const EDITOR_THEMES = [
  'one-dark',
  'dracula',
  'github-dark',
  'tokyo-night',
  'nord',
  'catppuccin-mocha',
  'rose-pine',
  'plum-dark',
  'github-light',
  'catppuccin-latte',
  'solarized-light',
  'plum-light',
] as const;

export type EditorThemeId = (typeof EDITOR_THEMES)[number];

export const EDITOR_THEME_LABELS: Record<EditorThemeId, string> = {
  'one-dark': 'One Dark',
  'dracula': 'Dracula',
  'github-dark': 'GitHub Dark',
  'tokyo-night': 'Tokyo Night',
  'nord': 'Nord',
  'catppuccin-mocha': 'Catppuccin Mocha',
  'rose-pine': 'Rosé Pine',
  'plum-dark': 'Plum Dark',
  'github-light': 'GitHub Light',
  'catppuccin-latte': 'Catppuccin Latte',
  'solarized-light': 'Solarized Light',
  'plum-light': 'Plum Light',
};

/** Returns true if the given editor theme is a light theme */
export function isLightEditorTheme(id: EditorThemeId): boolean {
  return id === 'github-light' || id === 'catppuccin-latte' || id === 'solarized-light' || id === 'plum-light';
}
