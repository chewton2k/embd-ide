import { writable, get, type Writable } from 'svelte/store';

export interface ShortcutDef {
  id: string;
  label: string;
  group: string;
  /** Default key combo string, e.g. "Meta+KeyK" or "" for unbound */
  defaultKeys: string;
}

export interface ShortcutBinding {
  id: string;
  keys: string; // current binding (may differ from default)
}

// ── Default shortcuts ────────────────────────────────────────────
// Keys use a normalized format: modifiers in order (Meta, Ctrl, Alt, Shift) + key code
// e.g. "Meta+KeyP", "Ctrl+Backquote", "Meta+Shift+KeyA"

export const DEFAULT_SHORTCUTS: ShortcutDef[] = [
  // File
  { id: 'file.search',       label: 'Open file search',       group: 'File',   defaultKeys: 'Meta+KeyO' },
  { id: 'file.save',         label: 'Save file',              group: 'File',   defaultKeys: 'Meta+KeyS' },
  { id: 'file.closeTab',     label: 'Close tab',              group: 'File',   defaultKeys: 'Meta+KeyW' },
  // Tabs
  { id: 'tabs.next',         label: 'Next tab',               group: 'Tabs',   defaultKeys: 'Ctrl+Tab' },
  { id: 'tabs.prev',         label: 'Previous tab',           group: 'Tabs',   defaultKeys: 'Ctrl+Shift+Tab' },
  { id: 'tabs.nextAlt',      label: 'Next tab (bracket)',     group: 'Tabs',   defaultKeys: 'Meta+Shift+BracketRight' },
  { id: 'tabs.prevAlt',      label: 'Previous tab (bracket)', group: 'Tabs',   defaultKeys: 'Meta+Shift+BracketLeft' },
  // View
  { id: 'view.toggleSidebar',  label: 'Toggle sidebar',       group: 'View',   defaultKeys: 'Meta+KeyB' },
  { id: 'view.toggleTerminal', label: 'Toggle terminal',      group: 'View',   defaultKeys: 'Ctrl+Backquote' },
  { id: 'view.toggleChat',     label: 'Toggle chat panel',    group: 'View',   defaultKeys: 'Meta+KeyL' },
  { id: 'view.toggleGit',      label: 'Toggle source control',group: 'View',   defaultKeys: 'Meta+KeyG' },
  { id: 'view.openSettings',   label: 'Open settings',        group: 'View',   defaultKeys: 'Meta+Comma' },
  // Editor
  { id: 'editor.find',       label: 'Find in file',           group: 'Editor', defaultKeys: 'Meta+KeyF' },
  { id: 'editor.replace',    label: 'Replace in file',        group: 'Editor', defaultKeys: 'Meta+Alt+KeyF' },
  { id: 'editor.goToLine',   label: 'Go to line',             group: 'Editor', defaultKeys: 'Ctrl+KeyG' },
];

const STORAGE_KEY = 'leo-keyboard-shortcuts';

// ── Persistence ──────────────────────────────────────────────────

function loadCustomBindings(): Record<string, string> {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return {};
  try { return JSON.parse(raw); } catch { return {}; }
}

function saveCustomBindings(bindings: Record<string, string>) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(bindings));
}

// ── Store ────────────────────────────────────────────────────────

/** Map of shortcut id → current key combo. Includes all defaults merged with user overrides. */
export const shortcutBindings: Writable<Record<string, string>> = writable(buildBindings());

function buildBindings(): Record<string, string> {
  const custom = loadCustomBindings();
  const result: Record<string, string> = {};
  for (const def of DEFAULT_SHORTCUTS) {
    result[def.id] = def.id in custom ? custom[def.id] : def.defaultKeys;
  }
  return result;
}

// ── Public API ───────────────────────────────────────────────────

/** Update a single shortcut binding. Pass '' to unbind. */
export function setShortcut(id: string, keys: string) {
  const custom = loadCustomBindings();
  const def = DEFAULT_SHORTCUTS.find(d => d.id === id);
  if (def && keys === def.defaultKeys) {
    delete custom[id]; // no need to store if it matches default
  } else {
    custom[id] = keys;
  }
  saveCustomBindings(custom);
  shortcutBindings.set(buildBindings());
}

/** Remove (unbind) a shortcut. */
export function removeShortcut(id: string) {
  setShortcut(id, '');
}

/** Reset a single shortcut to its default. */
export function resetShortcut(id: string) {
  const custom = loadCustomBindings();
  delete custom[id];
  saveCustomBindings(custom);
  shortcutBindings.set(buildBindings());
}

/** Reset all shortcuts to defaults. */
export function resetAllShortcuts() {
  localStorage.removeItem(STORAGE_KEY);
  shortcutBindings.set(buildBindings());
}

/** Find conflicts: returns shortcut ids that share the same key combo (excluding empty). */
export function findConflicts(keys: string, excludeId?: string): ShortcutDef[] {
  if (!keys) return [];
  const bindings = get(shortcutBindings);
  return DEFAULT_SHORTCUTS.filter(
    def => def.id !== excludeId && bindings[def.id] === keys
  );
}

/** Check if a shortcut has been modified from its default. */
export function isModified(id: string): boolean {
  const bindings = get(shortcutBindings);
  const def = DEFAULT_SHORTCUTS.find(d => d.id === id);
  if (!def) return false;
  return bindings[id] !== def.defaultKeys;
}

// ── Key event normalization ──────────────────────────────────────

const MODIFIER_CODES = new Set(['MetaLeft', 'MetaRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'ShiftLeft', 'ShiftRight']);

/** Convert a KeyboardEvent into our normalized key string format. */
export function normalizeKeyEvent(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.metaKey) parts.push('Meta');
  if (e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');

  // Ignore standalone modifier presses
  if (MODIFIER_CODES.has(e.code)) return '';

  parts.push(e.code);
  return parts.join('+');
}

/** Convert a KeyboardEvent to our normalized format for matching against bindings. */
export function eventMatchesBinding(e: KeyboardEvent, binding: string): boolean {
  if (!binding) return false;
  return normalizeKeyEvent(e) === binding;
}

// ── App-level shortcut catalog ───────────────────────────────────
//
// These are the shortcuts handled at the window level by App.svelte's
// `handleKeydown`. Editor (CodeMirror) and tab-system shortcuts like
// `editor.find` or `file.save` are deliberately excluded — they're owned
// by the focused control, not the app shell.
//
// Components that install their own keyboard handlers (e.g. xterm.js in
// the terminal) consult `isAppShortcutEvent` so matching events bubble
// up to App.svelte instead of being consumed locally. Keeping this list
// in one place is what prevents drift between the two consumers.
export const APP_LEVEL_SHORTCUT_IDS = [
  'tabs.next',
  'tabs.prev',
  'tabs.nextAlt',
  'tabs.prevAlt',
  'view.toggleTerminal',
  'view.toggleChat',
  'view.toggleGit',
  'view.toggleSidebar',
  'view.openSettings',
  'file.search',
] as const;

export type AppLevelShortcutId = (typeof APP_LEVEL_SHORTCUT_IDS)[number];

/**
 * Returns true if the given KeyboardEvent matches any app-level shortcut
 * binding currently in effect. Components that install their own
 * keydown handlers (e.g. xterm.js inside the terminal) call this to
 * decide whether to consume the event or let it bubble up to the
 * window-level handler.
 */
export function isAppShortcutEvent(e: KeyboardEvent): boolean {
  // Normalize once. `normalizeKeyEvent` returns '' for modifier-only
  // presses, so we can early-exit without touching the bindings store.
  const normalized = normalizeKeyEvent(e);
  if (!normalized) return false;
  const bindings = get(shortcutBindings);
  for (const id of APP_LEVEL_SHORTCUT_IDS) {
    if (bindings[id] === normalized) return true;
  }
  return false;
}

/** Format a normalized key string for display (e.g. "Meta+KeyP" → "⌘P"). */
export function formatKeysForDisplay(keys: string): string {
  if (!keys) return 'Unbound';
  return keys
    .replace(/Meta\+/g, '⌘')
    .replace(/Ctrl\+/g, '⌃')
    .replace(/Alt\+/g, '⌥')
    .replace(/Shift\+/g, '⇧')
    .replace(/Key([A-Z])/g, '$1')
    .replace(/Digit(\d)/g, '$1')
    .replace(/Backquote/g, '`')
    .replace(/BracketLeft/g, '[')
    .replace(/BracketRight/g, ']')
    .replace(/Comma/g, ',')
    .replace(/Period/g, '.')
    .replace(/Slash/g, '/')
    .replace(/Backslash/g, '\\')
    .replace(/Semicolon/g, ';')
    .replace(/Quote/g, "'")
    .replace(/Minus/g, '-')
    .replace(/Equal/g, '=')
    .replace(/Tab/g, 'Tab')
    .replace(/Space/g, 'Space')
    .replace(/Enter/g, '↵')
    .replace(/Escape/g, 'Esc')
    .replace(/Backspace/g, '⌫')
    .replace(/Delete/g, '⌦')
    .replace(/ArrowUp/g, '↑')
    .replace(/ArrowDown/g, '↓')
    .replace(/ArrowLeft/g, '←')
    .replace(/ArrowRight/g, '→');
}
