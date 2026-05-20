import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { openFiles, activeFilePath, maxRecentProjects, expandedDirsStore } from '../explorer/files';
import { projectRoot } from '../git/git';
import { terminalTabs, showTerminal } from '../terminal/shell';
import { log } from '../logging';

export interface SessionFile {
  path: string;
  pinned: boolean;
}

export interface SessionData {
  open_files: SessionFile[];
  active_file: string | null;
  terminal_count: number;
  terminal_visible: boolean;
  expanded_dirs: string[];
}

export interface RecentProject {
  path: string;
  name: string;
  last_opened: number;
  session: SessionData;
}

export async function getRecentProjects(): Promise<RecentProject[]> {
  return invoke<RecentProject[]>('get_recent_projects');
}

export async function findRecentProject(path: string): Promise<RecentProject | null> {
  const projects = await getRecentProjects();
  return projects.find(p => p.path === path) ?? null;
}

export async function removeRecentProject(path: string): Promise<void> {
  return invoke('remove_recent_project', { projectPath: path });
}

export function buildSessionData(): SessionData {
  const files = get(openFiles);
  const active = get(activeFilePath);
  const tabs = get(terminalTabs);
  const termVisible = get(showTerminal);
  const expanded = get(expandedDirsStore);
  return {
    open_files: files.map(f => ({ path: f.path, pinned: f.pinned })),
    active_file: active,
    terminal_count: tabs.length,
    terminal_visible: termVisible,
    expanded_dirs: [...expanded],
  };
}

let debounceTimer: ReturnType<typeof setTimeout> | null = null;

export function scheduleSaveSession(): void {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    debounceTimer = null;
    const root = get(projectRoot);
    if (!root) return;
    const session = buildSessionData();
    const maxRecent = get(maxRecentProjects);
    invoke('save_session', { projectPath: root, session, maxRecent }).catch((e) => log.error('Failed to save session', e));
  }, 750);
}

/** Immediate save that returns a promise. Clears any pending debounce. */
export async function saveSessionNow(projectPath: string): Promise<void> {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  const session = buildSessionData();
  const maxRecent = get(maxRecentProjects);
  await invoke('save_session', { projectPath, session, maxRecent });
}
