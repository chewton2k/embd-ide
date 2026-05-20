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

export const maxRecentProjects = persistedNumber('leo-max-recent-projects', 10);
export const maxTabs = persistedNumber('leo-max-tabs', 9);

/** Shared store for expanded directory paths in the file tree.
 *  Synced by FileTree.svelte; read by session.ts for persistence. */
export const expandedDirsStore = writable<Set<string>>(new Set());

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
 * Bounded LRU cache for in-memory file content.
 * Updated on every keystroke from the editor; consumers (AI chat, etc.)
 * read from here instead of the reactive `openFiles` store so that
 * typing doesn't fan out a re-render to every component subscribed to
 * `openFiles` (Tabs, GitPanel, App $effects, derived stores).
 */
class BoundedLru<K, V> {
  private map = new Map<K, V>();
  constructor(private cap: number) {}
  get(k: K): V | undefined {
    const v = this.map.get(k);
    if (v !== undefined) { this.map.delete(k); this.map.set(k, v); }
    return v;
  }
  set(k: K, v: V): void {
    if (this.map.has(k)) this.map.delete(k);
    this.map.set(k, v);
    while (this.map.size > this.cap) {
      const first = this.map.keys().next().value;
      if (first === undefined) break;
      this.map.delete(first);
    }
  }
  delete(k: K): void { this.map.delete(k); }
  has(k: K): boolean { return this.map.has(k); }
  keys(): IterableIterator<K> { return this.map.keys(); }
}

const fileContentCache = new BoundedLru<string, string>(50);

/** Read the latest in-memory content for a file (unsaved edits included). */
export function getFileContent(path: string): string | null {
  return fileContentCache.get(path) ?? null;
}

export function updateFileContent(path: string, content: string) {
  // Hot path: every keystroke calls this. Avoid touching the reactive
  // store unless the `modified` flag actually flips, otherwise every
  // keystroke would cascade re-renders into Tabs/GitPanel/etc.
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

/**
 * Patch a single file's fields in the openFiles store.
 * Skips the store update entirely if no field actually changed,
 * eliminating spurious reactive notifications.
 */
function patchFile(path: string, patchFn: (f: OpenFile) => Partial<OpenFile>): void {
  const files = get(openFiles);
  const idx = files.findIndex(f => f.path === path);
  if (idx === -1) return;
  const cur = files[idx];
  const patch = patchFn(cur);
  let changed = false;
  for (const k in patch) {
    if (cur[k as keyof OpenFile] !== patch[k as keyof OpenFile]) { changed = true; break; }
  }
  if (!changed) return;
  const next = files.slice();
  next[idx] = { ...cur, ...patch };
  openFiles.set(next);
}

export function togglePin(path: string) {
  patchFile(path, (f) => ({ pinned: !f.pinned }));
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
  patchFile(path, () => ({ modified: false }));
}

export function reloadFileContent(path: string, content: string) {
  fileContentCache.set(path, content);
  openFiles.update(files => files.map(f => f.path === path ? { ...f, content, modified: false, version: f.version + 1 } : f));
}
