import { writable, derived, get } from 'svelte/store';
import { persistedNumber } from '../session/persisted';

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
    return $files.find(f => f.path === $path)?.modified ?? false;
  }
);

export const maxRecentProjects = persistedNumber('leo-max-recent-projects', 3);
export const maxTabs = persistedNumber('leo-max-tabs', 9);

export const pinnedFiles = derived(openFiles, files => files.filter(f => f.pinned));
export const unpinnedFiles = derived(openFiles, files => files.filter(f => !f.pinned));

export function addFile(path: string, name: string) {
  openFiles.update(files => {
    if (files.find(f => f.path === path)) { activeFilePath.set(path); return files; }
    activeFilePath.set(path);
    let updated = [...files, { path, name, content: '', modified: false, pinned: false, version: 0 }];
    const limit = get(maxTabs);
    while (updated.length > limit) {
      const oldest = updated.find(f => !f.pinned && !f.modified && f.path !== path);
      if (!oldest) break;
      updated = updated.filter(f => f.path !== oldest.path);
    }
    return updated;
  });
}

/**
 * Non-reactive cache of in-memory file content for currently-open files.
 * Updated on every keystroke from the editor; consumers (AI chat, etc.)
 * read from here instead of the reactive `openFiles` store so that
 * typing doesn't fan out a re-render to every component subscribed to
 * `openFiles` (Tabs, GitPanel, ChatPanel, App $effects, derived stores).
 */
const fileContentCache = new Map<string, string>();

/** Read the latest in-memory content for a file (unsaved edits included). */
export function getFileContent(path: string): string | null {
  return fileContentCache.get(path) ?? null;
}

export function updateFileContent(path: string, content: string) {
  // Hot path: every keystroke calls this. Avoid touching the reactive
  // store unless the `modified` flag actually flips, otherwise every
  // keystroke would cascade re-renders into Tabs/GitPanel/ChatPanel/etc.
  fileContentCache.set(path, content);
  const files = get(openFiles);
  const file = files.find(f => f.path === path);
  if (file && !file.modified) {
    openFiles.update(fs => fs.map(f => f.path === path ? { ...f, modified: true } : f));
  }
}

type FileRenameCallback = (oldPath: string, newPath: string) => void;
const fileRenameCallbacks: FileRenameCallback[] = [];

export function registerFileRenameCallback(cb: FileRenameCallback): () => void {
  fileRenameCallbacks.push(cb);
  return () => { const idx = fileRenameCallbacks.indexOf(cb); if (idx >= 0) fileRenameCallbacks.splice(idx, 1); };
}

export function renameOpenFile(oldPath: string, newPath: string, newName: string) {
  const cached = fileContentCache.get(oldPath);
  if (cached !== undefined) {
    fileContentCache.delete(oldPath);
    fileContentCache.set(newPath, cached);
  }
  openFiles.update(files => files.map(f => f.path === oldPath ? { ...f, path: newPath, name: newName } : f));
  activeFilePath.update(current => current === oldPath ? newPath : current);
  for (const cb of fileRenameCallbacks) cb(oldPath, newPath);
}

export function togglePin(path: string) {
  openFiles.update(files => files.map(f => f.path === path ? { ...f, pinned: !f.pinned } : f));
}

export function closeFile(path: string) {
  openFiles.update(files => {
    if (files.find(f => f.path === path)?.pinned) return files;
    const newFiles = files.filter(f => f.path !== path);
    fileContentCache.delete(path);
    activeFilePath.update(current => current === path ? newFiles.at(-1)?.path ?? null : current);
    return newFiles;
  });
}

export function nextTab() {
  const files = get(openFiles);
  const ordered = [...files.filter(f => f.pinned), ...files.filter(f => !f.pinned)];
  if (!ordered.length) return;
  const idx = ordered.findIndex(f => f.path === get(activeFilePath));
  activeFilePath.set(ordered[(idx + 1) % ordered.length].path);
}

export function prevTab() {
  const files = get(openFiles);
  const ordered = [...files.filter(f => f.pinned), ...files.filter(f => !f.pinned)];
  if (!ordered.length) return;
  const idx = ordered.findIndex(f => f.path === get(activeFilePath));
  activeFilePath.set(ordered[(idx - 1 + ordered.length) % ordered.length].path);
}

export function closeAllUnpinned() {
  openFiles.update(files => {
    const pinned = files.filter(f => f.pinned);
    const pinnedPaths = new Set(pinned.map(f => f.path));
    for (const key of fileContentCache.keys()) {
      if (!pinnedPaths.has(key)) fileContentCache.delete(key);
    }
    activeFilePath.set(pinned.at(-1)?.path ?? null);
    return pinned;
  });
}

export function markFileSaved(path: string) {
  openFiles.update(files => files.map(f => f.path === path ? { ...f, modified: false } : f));
}

export function reloadFileContent(path: string, content: string) {
  fileContentCache.set(path, content);
  openFiles.update(files => files.map(f => f.path === path ? { ...f, content, modified: false, version: f.version + 1 } : f));
}
