import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { openFiles, activeFilePath, projectRoot, maxRecentProjects } from './stores';

export interface SessionFile {
  path: string;
  pinned: boolean;
}

export interface SessionData {
  open_files: SessionFile[];
  active_file: string | null;
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

export async function removeRecentProject(path: string): Promise<void> {
  return invoke('remove_recent_project', { projectPath: path });
}

export function buildSessionData(): SessionData {
  const files = get(openFiles);
  const active = get(activeFilePath);
  return {
    open_files: files.map(f => ({ path: f.path, pinned: f.pinned })),
    active_file: active,
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
    invoke('save_session', { projectPath: root, session, maxRecent }).catch(console.error);
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
