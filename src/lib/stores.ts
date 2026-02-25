import { writable, derived, get } from 'svelte/store';

export interface OpenFile {
  path: string;
  name: string;
  content: string;
  modified: boolean;
  pinned: boolean;
  version: number;
}

export const openFiles = writable<OpenFile[]>([]);
export const activeFilePath = writable<string | null>(null);

export const activeFile = derived(activeFilePath, ($path) => $path);

export const activeFileModified = derived(
  [openFiles, activeFilePath],
  ([$files, $path]) => {
    if (!$path) return false;
    const file = $files.find(f => f.path === $path);
    return file?.modified ?? false;
  }
);

const MAX_TABS = 9;

export function addFile(path: string, name: string) {
  openFiles.update(files => {
    if (files.find(f => f.path === path)) {
      activeFilePath.set(path);
      return files;
    }
    activeFilePath.set(path);
    let updated = [...files, {
      path,
      name,
      content: '',
      modified: false,
      pinned: false,
      version: 0
    }];
    // Drop the oldest unmodified tab when over the limit
    while (updated.length > MAX_TABS) {
      const oldest = updated.find(
        f => !f.pinned && !f.modified && f.path !== path
      );

      if (!oldest) break;

      updated = updated.filter(f => f.path !== oldest.path);
    }

    return updated;
  });
}

export function updateFileContent(path: string, content: string) {
  openFiles.update(files =>
    files.map(f => f.path === path ? { ...f, content, modified: true } : f)
  );
}

type FileRenameCallback = (oldPath: string, newPath: string) => void;
const fileRenameCallbacks: FileRenameCallback[] = [];

export function registerFileRenameCallback(cb: FileRenameCallback): () => void {
  fileRenameCallbacks.push(cb);
  return () => {
    const idx = fileRenameCallbacks.indexOf(cb);
    if (idx >= 0) fileRenameCallbacks.splice(idx, 1);
  };
}

export function renameOpenFile(oldPath: string, newPath: string, newName: string) {
  openFiles.update(files =>
    files.map(f => f.path === oldPath ? { ...f, path: newPath, name: newName } : f)
  );
  activeFilePath.update(current => current === oldPath ? newPath : current);
  for (const cb of fileRenameCallbacks) cb(oldPath, newPath);
}

// Pinned tabs 
export function togglePin(path: string) {
  openFiles.update(files =>
    files.map(f =>
      f.path === path ? { ...f, pinned: !f.pinned } : f
    )
  );
}

export const pinnedFiles = derived(openFiles, files =>
  files.filter(f => f.pinned)
);

export const unpinnedFiles = derived(openFiles, files =>
  files.filter(f => !f.pinned)
);

export function closeFile(path: string) {
  openFiles.update(files => {
    const file = files.find(f => f.path === path);
    if (file?.pinned) return files; // 🚫 don’t close pinned

    const newFiles = files.filter(f => f.path !== path);

    activeFilePath.update(current =>
      current === path
        ? newFiles.at(-1)?.path ?? null
        : current
    );

    return newFiles;
  });
}

export function nextTab() {
  const files = get(openFiles);
  const ordered = [...files.filter(f => f.pinned), ...files.filter(f => !f.pinned)];
  if (ordered.length === 0) return;
  const currentPath = get(activeFilePath);
  const idx = ordered.findIndex(f => f.path === currentPath);
  const next = ordered[(idx + 1) % ordered.length];
  activeFilePath.set(next.path);
}

export function prevTab() {
  const files = get(openFiles);
  const ordered = [...files.filter(f => f.pinned), ...files.filter(f => !f.pinned)];
  if (ordered.length === 0) return;
  const currentPath = get(activeFilePath);
  const idx = ordered.findIndex(f => f.path === currentPath);
  const prev = ordered[(idx - 1 + ordered.length) % ordered.length];
  activeFilePath.set(prev.path);
}

export function closeAllUnpinned() {
  openFiles.update(files => {
    const pinned = files.filter(f => f.pinned);
    activeFilePath.set(pinned.at(-1)?.path ?? null);
    return pinned;
  });
}

export function markFileSaved(path: string) {
  openFiles.update(files =>
    files.map(f => f.path === path ? { ...f, modified: false } : f)
  );
}

export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

export const chatMessages = writable<ChatMessage[]>([]);
export const apiKey = writable<string>(localStorage.getItem('embd-api-key') || '');

apiKey.subscribe(key => {
  if (key) localStorage.setItem('embd-api-key', key);
  else localStorage.removeItem('embd-api-key');
});

export const projectRoot = writable<string | null>(null);

// Update a file's content from disk after external changes (e.g. git discard)
export function reloadFileContent(path: string, content: string) {
  openFiles.update(files =>
    files.map(f => f.path === path
      ? { ...f, content, modified: false, version: f.version + 1 }
      : f
    )
  );
}

// Bump to signal the file tree to refresh
export const fileTreeRefreshTrigger = writable<number>(0);
export function triggerFileTreeRefresh() {
  fileTreeRefreshTrigger.update(n => n + 1);
}
export const gitBranch = writable<string | null>(null);

// Shared git status data — written by FileTree's poll, read by GitPanel
// Maps absolute path -> status code (M, A, S, D, U)
export const sharedGitStatus = writable<Record<string, string>>({});
// Remote git status — files changed on upstream tracking branch (after git fetch)
export const sharedGitRemoteStatus = writable<Record<string, string>>({});

// Autosave settings
export const autosaveEnabled = writable<boolean>(
  localStorage.getItem('embd-autosave') !== 'false' // default on
);
export const autosaveDelay = writable<number>(
  parseInt(localStorage.getItem('embd-autosave-delay') || '1000', 10)
);

autosaveEnabled.subscribe(v => localStorage.setItem('embd-autosave', String(v)));
autosaveDelay.subscribe(v => localStorage.setItem('embd-autosave-delay', String(v)));

// Settings modal visibility
export const showSettings = writable<boolean>(false);

// Terminal panel visibility
export const showTerminal = writable<boolean>(true);

// Editor settings
export const editorFontSize = writable<number>(
  parseInt(localStorage.getItem('embd-editor-font-size') || '13', 10)
);
export const editorTabSize = writable<number>(
  parseInt(localStorage.getItem('embd-editor-tab-size') || '2', 10)
);
export const editorWordWrap = writable<boolean>(
  localStorage.getItem('embd-editor-word-wrap') === 'true'
);
export const editorLineNumbers = writable<boolean>(
  localStorage.getItem('embd-editor-line-numbers') !== 'false' // default on
);

editorFontSize.subscribe(v => localStorage.setItem('embd-editor-font-size', String(v)));
editorTabSize.subscribe(v => localStorage.setItem('embd-editor-tab-size', String(v)));
editorWordWrap.subscribe(v => localStorage.setItem('embd-editor-word-wrap', String(v)));
editorLineNumbers.subscribe(v => localStorage.setItem('embd-editor-line-numbers', String(v)));

// Terminal settings
export const terminalFontSize = writable<number>(
  parseInt(localStorage.getItem('embd-terminal-font-size') || '13', 10)
);

terminalFontSize.subscribe(v => localStorage.setItem('embd-terminal-font-size', String(v)));

// Theme system
export interface ThemeColors {
  bgPrimary: string;
  bgSecondary: string;
  bgTertiary: string;
  bgSurface: string;
  textPrimary: string;
  textSecondary: string;
  textMuted: string;
  accent: string;
  accentHover: string;
  border: string;
  success: string;
  warning: string;
  error: string;
  // Terminal colors
  termBg: string;
  termFg: string;
  termCursor: string;
  termSelection: string;
  termBlack: string;
  termRed: string;
  termGreen: string;
  termYellow: string;
  termBlue: string;
  termMagenta: string;
  termCyan: string;
  termWhite: string;
  termBrightBlack: string;
  termBrightWhite: string;
}

export interface ThemePreset {
  id: string;
  name: string;
  colors: ThemeColors;
}

export const THEMES: ThemePreset[] = [
  {
    id: 'catppuccin-mocha',
    name: 'Catppuccin Mocha',
    colors: {
      bgPrimary: '#1e1e2e', bgSecondary: '#181825', bgTertiary: '#11111b', bgSurface: '#313244',
      textPrimary: '#cdd6f4', textSecondary: '#a6adc8', textMuted: '#6c7086',
      accent: '#89b4fa', accentHover: '#74c7ec', border: '#45475a',
      success: '#a6e3a1', warning: '#f9e2af', error: '#f38ba8',
      termBg: '#11111b', termFg: '#cdd6f4', termCursor: '#89b4fa', termSelection: '#45475a',
      termBlack: '#45475a', termRed: '#f38ba8', termGreen: '#a6e3a1', termYellow: '#f9e2af',
      termBlue: '#89b4fa', termMagenta: '#cba6f7', termCyan: '#94e2d5', termWhite: '#bac2de',
      termBrightBlack: '#585b70', termBrightWhite: '#a6adc8',
    },
  },
  {
    id: 'catppuccin-frappe',
    name: 'Catppuccin Frappe',
    colors: {
      bgPrimary: '#303446', bgSecondary: '#292c3c', bgTertiary: '#232634', bgSurface: '#414559',
      textPrimary: '#c6d0f5', textSecondary: '#a5adce', textMuted: '#737994',
      accent: '#8caaee', accentHover: '#85c1dc', border: '#51576d',
      success: '#a6d189', warning: '#e5c890', error: '#e78284',
      termBg: '#232634', termFg: '#c6d0f5', termCursor: '#8caaee', termSelection: '#51576d',
      termBlack: '#51576d', termRed: '#e78284', termGreen: '#a6d189', termYellow: '#e5c890',
      termBlue: '#8caaee', termMagenta: '#ca9ee6', termCyan: '#81c8be', termWhite: '#b5bfe2',
      termBrightBlack: '#626880', termBrightWhite: '#a5adce',
    },
  },
  {
    id: 'tokyo-night',
    name: 'Tokyo Night',
    colors: {
      bgPrimary: '#1a1b26', bgSecondary: '#16161e', bgTertiary: '#13131a', bgSurface: '#292e42',
      textPrimary: '#c0caf5', textSecondary: '#a9b1d6', textMuted: '#565f89',
      accent: '#7aa2f7', accentHover: '#7dcfff', border: '#3b4261',
      success: '#9ece6a', warning: '#e0af68', error: '#f7768e',
      termBg: '#13131a', termFg: '#c0caf5', termCursor: '#7aa2f7', termSelection: '#3b4261',
      termBlack: '#414868', termRed: '#f7768e', termGreen: '#9ece6a', termYellow: '#e0af68',
      termBlue: '#7aa2f7', termMagenta: '#bb9af7', termCyan: '#7dcfff', termWhite: '#c0caf5',
      termBrightBlack: '#565f89', termBrightWhite: '#a9b1d6',
    },
  },
  {
    id: 'dracula',
    name: 'Dracula',
    colors: {
      bgPrimary: '#282a36', bgSecondary: '#21222c', bgTertiary: '#191a21', bgSurface: '#44475a',
      textPrimary: '#f8f8f2', textSecondary: '#d4d4d4', textMuted: '#6272a4',
      accent: '#bd93f9', accentHover: '#ff79c6', border: '#44475a',
      success: '#50fa7b', warning: '#f1fa8c', error: '#ff5555',
      termBg: '#191a21', termFg: '#f8f8f2', termCursor: '#bd93f9', termSelection: '#44475a',
      termBlack: '#44475a', termRed: '#ff5555', termGreen: '#50fa7b', termYellow: '#f1fa8c',
      termBlue: '#6272a4', termMagenta: '#bd93f9', termCyan: '#8be9fd', termWhite: '#f8f8f2',
      termBrightBlack: '#6272a4', termBrightWhite: '#f8f8f2',
    },
  },
  {
    id: 'github-dark',
    name: 'GitHub Dark',
    colors: {
      bgPrimary: '#0d1117', bgSecondary: '#010409', bgTertiary: '#010409', bgSurface: '#21262d',
      textPrimary: '#e6edf3', textSecondary: '#b1bac4', textMuted: '#6e7681',
      accent: '#58a6ff', accentHover: '#79c0ff', border: '#30363d',
      success: '#3fb950', warning: '#d29922', error: '#f85149',
      termBg: '#010409', termFg: '#e6edf3', termCursor: '#58a6ff', termSelection: '#30363d',
      termBlack: '#484f58', termRed: '#ff7b72', termGreen: '#3fb950', termYellow: '#d29922',
      termBlue: '#58a6ff', termMagenta: '#bc8cff', termCyan: '#39d2c0', termWhite: '#e6edf3',
      termBrightBlack: '#6e7681', termBrightWhite: '#b1bac4',
    },
  },
  {
    id: 'rose-pine',
    name: 'Rose Pine',
    colors: {
      bgPrimary: '#191724', bgSecondary: '#1f1d2e', bgTertiary: '#16141f', bgSurface: '#26233a',
      textPrimary: '#e0def4', textSecondary: '#908caa', textMuted: '#6e6a86',
      accent: '#c4a7e7', accentHover: '#ebbcba', border: '#403d52',
      success: '#31748f', warning: '#f6c177', error: '#eb6f92',
      termBg: '#16141f', termFg: '#e0def4', termCursor: '#c4a7e7', termSelection: '#403d52',
      termBlack: '#403d52', termRed: '#eb6f92', termGreen: '#31748f', termYellow: '#f6c177',
      termBlue: '#9ccfd8', termMagenta: '#c4a7e7', termCyan: '#9ccfd8', termWhite: '#e0def4',
      termBrightBlack: '#6e6a86', termBrightWhite: '#908caa',
    },
  },
  {
    id: 'nord',
    name: 'Nord',
    colors: {
      bgPrimary: '#2e3440', bgSecondary: '#2b303b', bgTertiary: '#242933', bgSurface: '#3b4252',
      textPrimary: '#eceff4', textSecondary: '#d8dee9', textMuted: '#616e88',
      accent: '#88c0d0', accentHover: '#81a1c1', border: '#434c5e',
      success: '#a3be8c', warning: '#ebcb8b', error: '#bf616a',
      termBg: '#242933', termFg: '#eceff4', termCursor: '#88c0d0', termSelection: '#434c5e',
      termBlack: '#3b4252', termRed: '#bf616a', termGreen: '#a3be8c', termYellow: '#ebcb8b',
      termBlue: '#81a1c1', termMagenta: '#b48ead', termCyan: '#88c0d0', termWhite: '#e5e9f0',
      termBrightBlack: '#4c566a', termBrightWhite: '#d8dee9',
    },
  },
  {
    id: 'catppuccin-latte',
    name: 'Catppuccin Latte',
    colors: {
      bgPrimary: '#eff1f5', bgSecondary: '#e6e9ef', bgTertiary: '#dce0e8', bgSurface: '#ccd0da',
      textPrimary: '#4c4f69', textSecondary: '#5c5f77', textMuted: '#8c8fa1',
      accent: '#1e66f5', accentHover: '#209fb5', border: '#bcc0cc',
      success: '#40a02b', warning: '#df8e1d', error: '#d20f39',
      termBg: '#dce0e8', termFg: '#4c4f69', termCursor: '#1e66f5', termSelection: '#bcc0cc',
      termBlack: '#9ca0b0', termRed: '#d20f39', termGreen: '#40a02b', termYellow: '#df8e1d',
      termBlue: '#1e66f5', termMagenta: '#8839ef', termCyan: '#179299', termWhite: '#4c4f69',
      termBrightBlack: '#8c8fa1', termBrightWhite: '#5c5f77',
    },
  },
  {
    id: 'one-dark',
    name: 'One Dark',
    colors: {
      bgPrimary: '#282c34', bgSecondary: '#21252b', bgTertiary: '#1b1f27', bgSurface: '#2c313c',
      textPrimary: '#abb2bf', textSecondary: '#9da5b4', textMuted: '#636d83',
      accent: '#61afef', accentHover: '#56b6c2', border: '#3e4452',
      success: '#98c379', warning: '#e5c07b', error: '#e06c75',
      termBg: '#1b1f27', termFg: '#abb2bf', termCursor: '#61afef', termSelection: '#3e4452',
      termBlack: '#3e4452', termRed: '#e06c75', termGreen: '#98c379', termYellow: '#e5c07b',
      termBlue: '#61afef', termMagenta: '#c678dd', termCyan: '#56b6c2', termWhite: '#abb2bf',
      termBrightBlack: '#636d83', termBrightWhite: '#9da5b4',
    },
  },
  {
    id: 'sandstone',
    name: 'Sandstone',
    colors: {
      bgPrimary: '#f5f0e8', bgSecondary: '#ece5d8', bgTertiary: '#e3dace', bgSurface: '#d9cfc0',
      textPrimary: '#3e3529', textSecondary: '#5c5143', textMuted: '#8c7f6e',
      accent: '#b07d3a', accentHover: '#c4913e', border: '#cdc2b0',
      success: '#5a8a3c', warning: '#c48c2a', error: '#c45040',
      termBg: '#e3dace', termFg: '#3e3529', termCursor: '#b07d3a', termSelection: '#cdc2b0',
      termBlack: '#8c7f6e', termRed: '#c45040', termGreen: '#5a8a3c', termYellow: '#c48c2a',
      termBlue: '#4a7a9b', termMagenta: '#8a5a9b', termCyan: '#4a8a7a', termWhite: '#3e3529',
      termBrightBlack: '#7a6e5e', termBrightWhite: '#5c5143',
    },
  },
  {
    id: 'midnight',
    name: 'Midnight',
    colors: {
      bgPrimary: '#0a0a0a', bgSecondary: '#060606', bgTertiary: '#000000', bgSurface: '#161616',
      textPrimary: '#d4d4d4', textSecondary: '#a0a0a0', textMuted: '#5a5a5a',
      accent: '#c0c0c0', accentHover: '#e0e0e0', border: '#222222',
      success: '#a0a0a0', warning: '#909090', error: '#808080',
      termBg: '#000000', termFg: '#d4d4d4', termCursor: '#c0c0c0', termSelection: '#222222',
      termBlack: '#333333', termRed: '#d4d4d4', termGreen: '#b0b0b0', termYellow: '#909090',
      termBlue: '#a0a0a0', termMagenta: '#c0c0c0', termCyan: '#b0b0b0', termWhite: '#d4d4d4',
      termBrightBlack: '#5a5a5a', termBrightWhite: '#a0a0a0',
    },
  },
  {
    id: 'ash',
    name: 'Ash',
    colors: {
      bgPrimary: '#2a2a2a', bgSecondary: '#242424', bgTertiary: '#1e1e1e', bgSurface: '#353535',
      textPrimary: '#d0d0d0', textSecondary: '#a8a8a8', textMuted: '#6e6e6e',
      accent: '#909090', accentHover: '#a8a8a8', border: '#404040',
      success: '#8c8c8c', warning: '#7a7a7a', error: '#686868',
      termBg: '#1e1e1e', termFg: '#d0d0d0', termCursor: '#909090', termSelection: '#404040',
      termBlack: '#404040', termRed: '#d0d0d0', termGreen: '#b0b0b0', termYellow: '#989898',
      termBlue: '#a0a0a0', termMagenta: '#b8b8b8', termCyan: '#a8a8a8', termWhite: '#d0d0d0',
      termBrightBlack: '#6e6e6e', termBrightWhite: '#a8a8a8',
    },
  },
];

export const currentThemeId = writable<string>(
  localStorage.getItem('embd-theme') || 'catppuccin-mocha'
);

currentThemeId.subscribe(v => localStorage.setItem('embd-theme', v));

export function getTheme(id: string): ThemePreset {
  return THEMES.find(t => t.id === id) || THEMES[0];
}

// UI font size
export const uiFontSize = writable<number>(
  parseInt(localStorage.getItem('embd-ui-font-size') || '13', 10)
);

uiFontSize.subscribe(v => localStorage.setItem('embd-ui-font-size', String(v)));

// UI density
export const uiDensity = writable<'compact' | 'comfortable'>(
  (localStorage.getItem('embd-ui-density') as 'compact' | 'comfortable') || 'comfortable'
);

uiDensity.subscribe(v => localStorage.setItem('embd-ui-density', v));

// Hidden file/folder patterns (persisted to localStorage)
const DEFAULT_HIDDEN_PATTERNS = ['node_modules', 'target', '.git', '.vscode', '.DS_Store'];

function loadHiddenPatterns(): { pattern: string; enabled: boolean }[] {
  const stored = localStorage.getItem('embd-hidden-patterns');
  if (stored) {
    try { return JSON.parse(stored); } catch { /* fall through */ }
  }
  return DEFAULT_HIDDEN_PATTERNS.map(p => ({ pattern: p, enabled: true }));
}

export const hiddenPatterns = writable<{ pattern: string; enabled: boolean }[]>(loadHiddenPatterns());

hiddenPatterns.subscribe(patterns => {
  localStorage.setItem('embd-hidden-patterns', JSON.stringify(patterns));
});
