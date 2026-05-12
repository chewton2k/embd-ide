import { writable, get, derived } from 'svelte/store';

export const showTerminal = writable<boolean>(false);
export const showPreview = writable<boolean>(false);
export const terminalLayout = writable<'tab'>('tab');

export const TERMINAL_SENTINEL_PREFIX = '__terminal__';
export const PREVIEW_PATH = '__preview__';
export const DIAGRAM_PREFIX = '__diagram__:';

// Legacy single-terminal path, kept so stored sessions that reference it
// still parse as "a terminal path" via `isTerminalPath`. New code should
// route through `terminalPath(tabId)`.
export const TERMINAL_WORKSPACE_PATH = `${TERMINAL_SENTINEL_PREFIX}workspace`;

export function isTerminalPath(path: string | null): boolean {
  return !!path?.startsWith(TERMINAL_SENTINEL_PREFIX);
}

export function isPreviewPath(path: string | null): boolean {
  return path === PREVIEW_PATH;
}

export function isDiagramPath(path: string | null): boolean {
  return !!path?.startsWith(DIAGRAM_PREFIX);
}

export function diagramPath(filePath: string): string {
  return `${DIAGRAM_PREFIX}${filePath}`;
}

export function getDiagramFilePath(path: string): string {
  return path.slice(DIAGRAM_PREFIX.length);
}

/** Build the routing path for a specific terminal tab. */
export function terminalPath(tabId: number): string {
  return `${TERMINAL_SENTINEL_PREFIX}${tabId}`;
}

/** Parse a terminal-tab id out of a routing path. Returns null when the path
 *  doesn't reference a terminal, or when the id part isn't a valid number
 *  (which covers the legacy `__terminal__workspace` sentinel). */
export function terminalTabIdFromPath(path: string | null): number | null {
  if (!path || !path.startsWith(TERMINAL_SENTINEL_PREFIX)) return null;
  const n = parseInt(path.slice(TERMINAL_SENTINEL_PREFIX.length), 10);
  return Number.isNaN(n) ? null : n;
}

// ── Pane-level session info (one entry per PTY) ────────────────────

export interface TerminalSessionInfo {
  /** Backend PTY session id (also pane id). */
  id: number;
  /** Which tab this pane belongs to. */
  tabId: number;
  name: string;
}

export const terminalSessions = writable<TerminalSessionInfo[]>([]);

// ── Tab-level state ────────────────────────────────────────────────

export interface TerminalTabInfo {
  id: number;
  name: string;
}

/** Ordered list of open terminal tabs. Each tab holds 1..N panes. */
export const terminalTabs = writable<TerminalTabInfo[]>([]);

/** The last-focused terminal tab id. Persists even while the user views a
 *  file tab, so Ctrl+` can restore focus to the same terminal. */
export const activeTerminalTabId = writable<number | null>(null);

// Monotonic counter for tab ids (separate from pane ids, which are chosen by
// the backend). Kept in a closure so tests or reloads restart at 1.
let nextTerminalTabId = 1;
export function allocateTerminalTabId(): number {
  const id = nextTerminalTabId++;
  return id;
}

/** Count panes belonging to a tab — used for UI state like "can collapse?". */
export const panesInActiveTab = derived(
  [terminalSessions, activeTerminalTabId],
  ([$sessions, $activeId]) => {
    if ($activeId == null) return 0;
    return $sessions.filter(s => s.tabId === $activeId).length;
  }
);

// ── Signals ────────────────────────────────────────────────────────

/**
 * Bumped each time the UI wants the Terminal component to ensure at least
 * one terminal tab is open and visible. The Terminal component handles
 * "create a new tab" vs "focus the existing one" based on its own state.
 */
export const createTerminalSignal = writable<{
  count: number;
  /** When true, always create a NEW tab; when false, only create if none exist. */
  forceNew: boolean;
}>({ count: 0, forceNew: false });

/** Kill a specific pane by id, all panes in a tab, or everything. */
export const killTerminalSignal = writable<
  | { kind: 'pane'; id: number }
  | { kind: 'tab'; id: number }
  | { kind: 'all' }
  | null
>(null);

export const splitTerminalSignal = writable<{
  count: number;
  direction: 'right' | 'bottom';
}>({ count: 0, direction: 'right' });

export const collapseTerminalSplitsSignal = writable<number>(0);

// ── Diagram tabs (unrelated — kept here for historical reasons) ────

export const openDiagramSearchSignal = writable<number>(0);
export const openDiagrams = writable<string[]>([]);

// ── Helpers for callers that don't want to read stores directly ────

/** Returns the id of the terminal tab to focus when the user asks for "the
 *  terminal": prefers the last-active tab, falls back to the first tab, or
 *  null when none are open. */
export function preferredTerminalTabId(): number | null {
  const active = get(activeTerminalTabId);
  if (active != null) {
    const exists = get(terminalTabs).some(t => t.id === active);
    if (exists) return active;
  }
  const tabs = get(terminalTabs);
  return tabs.length > 0 ? tabs[0].id : null;
}
