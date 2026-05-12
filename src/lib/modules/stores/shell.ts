import { writable } from 'svelte/store';

export const showTerminal = writable<boolean>(false);
export const showPreview = writable<boolean>(false);
export const terminalLayout = writable<'tab'>('tab');

export const TERMINAL_SENTINEL_PREFIX = '__terminal__';
export const TERMINAL_WORKSPACE_PATH = `${TERMINAL_SENTINEL_PREFIX}workspace`;
export const PREVIEW_PATH = '__preview__';
export const DIAGRAM_PREFIX = '__diagram__:';

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

export function terminalPath(): string {
  return TERMINAL_WORKSPACE_PATH;
}

export interface TerminalSessionInfo {
  id: number;
  name: string;
}

export const terminalSessions = writable<TerminalSessionInfo[]>([]);
export const createTerminalSignal = writable<number>(0);
export const killTerminalSignal = writable<number | 'all' | null>(null);
export const splitTerminalSignal = writable<{ count: number; direction: 'right' | 'bottom' }>({ count: 0, direction: 'right' });
export const collapseTerminalSplitsSignal = writable<number>(0);

export const openDiagramSearchSignal = writable<number>(0);
export const openDiagrams = writable<string[]>([]);
