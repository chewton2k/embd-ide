<script lang="ts">
  import { onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { watch, type UnwatchFn } from '@tauri-apps/plugin-fs';
  import { projectRoot, hiddenPatterns, renameOpenFile } from './stores.ts';

  interface FileEntry {
    name: string;
    path: string;
    is_dir: boolean;
    children: FileEntry[] | null;
  }

  let { onFileSelect }: { onFileSelect: (path: string, name: string) => void } = $props();
  let files = $state<FileEntry[]>([]);
  let expandedDirs = $state<Set<string>>(new Set());
  let rootPath = $state<string | null>(null);
  let selectedPath = $state<string | null>(null);
  let selectedPaths = $state<Set<string>>(new Set());

  // New file/folder creation state
  let creating = $state<'file' | 'folder' | null>(null);
  let newName = $state('');
  let newNameInput: HTMLInputElement | undefined = $state();

  // Context menu state
  let contextMenu = $state<{ x: number; y: number; path: string; isDir: boolean } | null>(null);

  // Clipboard for copy/paste files
  let clipboardPaths = $state<string[]>([]);

  // Rename state
  let renamingPath = $state<string | null>(null);
  let renameValue = $state('');
  let renameInput: HTMLInputElement | undefined = $state();

  // Git status: absolute path -> status code (M=modified, A=staged new, S=staged modified, D=deleted, U=untracked)
  let gitFileStatus = $state<Map<string, string>>(new Map());
  // Derived: folder path -> "highest priority" status of any child
  let gitFolderStatus = $state<Map<string, string>>(new Map());

  async function fetchGitStatus() {
    if (!rootPath) return;
    try {
      const status = await invoke<Record<string, string>>('get_git_status', { path: rootPath });
      gitFileStatus = new Map(Object.entries(status));
      // Compute folder statuses
      const folders = new Map<string, string>();
      const priority: Record<string, number> = { M: 3, U: 2, A: 1, S: 1, D: 2 };
      for (const [filePath, code] of gitFileStatus) {
        let dir = filePath.substring(0, filePath.lastIndexOf('/'));
        while (dir.length >= (rootPath?.length ?? 0)) {
          const existing = folders.get(dir);
          if (!existing || (priority[code] ?? 0) > (priority[existing] ?? 0)) {
            folders.set(dir, code);
          }
          dir = dir.substring(0, dir.lastIndexOf('/'));
        }
      }
      gitFolderStatus = folders;
    } catch (_) {
      gitFileStatus = new Map();
      gitFolderStatus = new Map();
    }
  }

  function getGitStatusColor(path: string, isDir: boolean): string | null {
    const code = isDir ? gitFolderStatus.get(path) : gitFileStatus.get(path);
    if (!code) return null;
    switch (code) {
      case 'M': return 'var(--git-modified, #e5c07b)';   // orange/yellow for modified
      case 'A': case 'S': return 'var(--git-staged, #61afef)';  // blue for staged
      case 'U': return 'var(--git-untracked, #98c379)';  // green for untracked
      case 'D': return 'var(--git-deleted, #e06c75)';    // red for deleted
      default: return null;
    }
  }

  // File watcher
  let unwatchFn: UnwatchFn | null = null;
  let watchDebounce: ReturnType<typeof setTimeout> | null = null;

  async function startWatching(path: string) {
    await stopWatching();
    unwatchFn = await watch(path, () => {
      // Debounce to avoid rapid-fire reloads
      if (watchDebounce) clearTimeout(watchDebounce);
      watchDebounce = setTimeout(() => refreshTree(), 300);
    }, { recursive: true });
  }

  async function stopWatching() {
    if (unwatchFn) {
      unwatchFn();
      unwatchFn = null;
    }
  }

  async function refreshTree() {
    if (!rootPath) return;
    await loadDirectory(rootPath);
    // Re-expand previously expanded dirs
    for (const dir of expandedDirs) {
      const entry = findEntry(files, dir);
      if (entry) {
        try {
          const children = await invoke<FileEntry[]>('read_dir_tree', { path: entry.path, depth: 1 });
          entry.children = children;
        } catch (_) { /* dir may have been deleted */ }
      }
    }
    files = [...files];
    await fetchGitStatus();
  }

  async function openFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      rootPath = selected as string;
      projectRoot.set(rootPath);
      expandedDirs = new Set();
      await loadDirectory(rootPath);
      await fetchGitStatus();
      startWatching(rootPath);
    }
  }

  async function loadDirectory(path: string) {
    try {
      files = await invoke<FileEntry[]>('read_dir_tree', { path, depth: 1 });
    } catch (e) {
      console.error('Failed to read directory:', e);
    }
  }

  async function toggleDir(entry: FileEntry) {
    if (expandedDirs.has(entry.path)) {
      expandedDirs.delete(entry.path);
      expandedDirs = new Set(expandedDirs);
    } else {
      try {
        const children = await invoke<FileEntry[]>('read_dir_tree', { path: entry.path, depth: 1 });
        entry.children = children;
        expandedDirs.add(entry.path);
        expandedDirs = new Set(expandedDirs);
        files = [...files];
      } catch (e) {
        console.error('Failed to expand:', e);
      }
    }
  }

  function handleFileClick(entry: FileEntry, e: MouseEvent) {
    if (e.metaKey || e.ctrlKey) {
      // Multi-select toggle
      const next = new Set(selectedPaths);
      if (next.has(entry.path)) {
        next.delete(entry.path);
      } else {
        next.add(entry.path);
      }
      selectedPaths = next;
      selectedPath = entry.path;
      return;
    }

    // Normal click — clear multi-select
    selectedPaths = new Set();
    selectedPath = entry.path;
    if (entry.is_dir) {
      toggleDir(entry);
    } else {
      onFileSelect(entry.path, entry.name);
    }
  }

  function startCreate(type: 'file' | 'folder') {
    creating = type;
    newName = '';
    // Focus input after render
    requestAnimationFrame(() => newNameInput?.focus());
  }

  async function confirmCreate() {
    if (!newName.trim() || !rootPath) return;
    // Determine parent directory: use selected dir, or parent of selected file, or root
    let parentDir = rootPath;
    if (selectedPath) {
      const selectedEntry = findEntry(files, selectedPath);
      if (selectedEntry?.is_dir) {
        parentDir = selectedEntry.path;
      } else if (selectedPath.includes('/')) {
        parentDir = selectedPath.substring(0, selectedPath.lastIndexOf('/'));
      }
    }
    const fullPath = `${parentDir}/${newName.trim()}`;
    try {
      if (creating === 'file') {
        await invoke('create_file', { path: fullPath });
        // Open the newly created file
        onFileSelect(fullPath, newName.trim());
      } else {
        await invoke('create_folder', { path: fullPath });
      }
      await refreshTree();
      selectedPath = fullPath;
    } catch (e) {
      console.error('Failed to create:', e);
    }
    creating = null;
    newName = '';
  }

  function cancelCreate() {
    creating = null;
    newName = '';
  }

  function handleContextMenu(e: MouseEvent, entry: FileEntry) {
    e.preventDefault();
    // If right-clicking on a non-selected item, select just that one
    if (!selectedPaths.has(entry.path)) {
      selectedPaths = new Set();
      selectedPath = entry.path;
    }
    contextMenu = { x: e.clientX, y: e.clientY, path: entry.path, isDir: entry.is_dir };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function deleteSelected() {
    const paths = selectedPaths.size > 0 ? [...selectedPaths] : (selectedPath ? [selectedPath] : []);
    if (paths.length === 0) return;
    closeContextMenu();
    try {
      await invoke('delete_entries', { paths });
    } catch (e) {
      console.error('Failed to delete:', e);
    }
    selectedPaths = new Set();
    selectedPath = null;
    await refreshTree();
  }

  function copyPath(path: string) {
    closeContextMenu();
    navigator.clipboard.writeText(path);
  }

  function copyRelativePath(path: string) {
    closeContextMenu();
    const rel = rootPath ? path.replace(rootPath + '/', '') : path;
    navigator.clipboard.writeText(rel);
  }

  function copyFiles() {
    const paths = selectedPaths.size > 0 ? [...selectedPaths] : (contextMenu?.path ? [contextMenu.path] : []);
    clipboardPaths = paths;
    closeContextMenu();
  }

  async function pasteFiles(destDir: string) {
    closeContextMenu();
    if (clipboardPaths.length === 0) return;
    try {
      await invoke('paste_entries', { sources: clipboardPaths, destDir });
    } catch (e) {
      console.error('Failed to paste:', e);
    }
    await refreshTree();
  }

  function startRename(path: string) {
    closeContextMenu();
    renamingPath = path;
    const name = path.split('/').pop() || '';
    renameValue = name;
    requestAnimationFrame(() => {
      if (renameInput) {
        renameInput.focus();
        // Select name without extension for files
        const dotIdx = name.lastIndexOf('.');
        if (dotIdx > 0) {
          renameInput.setSelectionRange(0, dotIdx);
        } else {
          renameInput.select();
        }
      }
    });
  }

  async function confirmRename() {
    if (!renamingPath || !renameValue.trim()) {
      cancelRename();
      return;
    }
    const oldPath = renamingPath;
    const parentDir = oldPath.substring(0, oldPath.lastIndexOf('/'));
    const newPath = `${parentDir}/${renameValue.trim()}`;
    if (newPath !== oldPath) {
      try {
        await invoke('rename_entry', { oldPath, newPath });
        renameOpenFile(oldPath, newPath, renameValue.trim());
      } catch (e) {
        console.error('Failed to rename:', e);
      }
    }
    renamingPath = null;
    renameValue = '';
    selectedPath = newPath;
    await refreshTree();
  }

  function cancelRename() {
    renamingPath = null;
    renameValue = '';
  }

  function findEntry(entries: FileEntry[], path: string): FileEntry | null {
    for (const entry of entries) {
      if (entry.path === path) return entry;
      if (entry.children) {
        const found = findEntry(entry.children, path);
        if (found) return found;
      }
    }
    return null;
  }

  function getFileColor(name: string): string {
    const ext = name.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'ts': case 'tsx': return '#3178c6';
      case 'js': case 'jsx': return '#f0db4f';
      case 'py': return '#3572A5';
      case 'rs': return '#dea584';
      case 'go': return '#00ADD8';
      case 'html': return '#e34c26';
      case 'svelte': return '#ff3e00';
      case 'vue': return '#42b883';
      case 'css': case 'scss': case 'less': return '#563d7c';
      case 'json': return '#a6e3a1';
      case 'md': case 'mdx': return '#83a598';
      case 'toml': case 'yaml': case 'yml': return '#8B8680';
      case 'lock': return '#585b70';
      case 'svg': return '#FFB13B';
      case 'png': case 'jpg': case 'jpeg': case 'gif': case 'webp': case 'ico': case 'bmp': return '#a78bfa';
      case 'pdf': return '#f38ba8';
      case 'sh': case 'bash': case 'zsh': return '#89b4fa';
      case 'env': return '#f9e2af';
      case 'gitignore': case 'gitmodules': return '#f38ba8';
      case 'dockerfile': return '#2496ED';
      default: return '#7f849c';
    }
  }

  // SVG icon paths for file types
  function getFileIconSvg(name: string): { path: string; viewBox: string } {
    const ext = name.split('.').pop()?.toLowerCase();
    const lname = name.toLowerCase();

    // Special filenames
    if (lname === 'dockerfile') return icons.docker;
    if (lname === '.env' || lname.startsWith('.env.')) return icons.env;
    if (lname === 'package.json') return icons.package;
    if (lname === 'cargo.toml') return icons.gear;

    switch (ext) {
      case 'ts': case 'tsx': return icons.typescript;
      case 'js': case 'jsx': return icons.javascript;
      case 'py': return icons.python;
      case 'rs': return icons.rust;
      case 'go': return icons.go;
      case 'html': return icons.html;
      case 'svelte': return icons.svelte;
      case 'vue': return icons.vue;
      case 'css': case 'scss': case 'less': return icons.css;
      case 'json': return icons.json;
      case 'md': case 'mdx': return icons.markdown;
      case 'toml': case 'yaml': case 'yml': return icons.config;
      case 'lock': return icons.lock;
      case 'svg': return icons.image;
      case 'png': case 'jpg': case 'jpeg': case 'gif': case 'webp': case 'ico': case 'bmp': return icons.image;
      case 'pdf': return icons.pdf;
      case 'sh': case 'bash': case 'zsh': return icons.terminal;
      case 'gitignore': case 'gitmodules': return icons.git;
      default: return icons.file;
    }
  }

  function isHidden(name: string): boolean {
    const patterns = $hiddenPatterns;
    return patterns.some(p => {
      if (!p.enabled) return false;
      const pat = p.pattern;
      // Simple glob: *.ext
      if (pat.startsWith('*.')) {
        return name.endsWith(pat.slice(1));
      }
      // Exact match
      return name === pat;
    });
  }

  const icons = {
    // Folder open/closed
    folderClosed: {
      path: 'M2 4a2 2 0 0 1 2-2h3.17a2 2 0 0 1 1.41.59l1.42 1.41H18a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4z',
      viewBox: '0 0 20 18',
    },
    folderOpen: {
      path: 'M2 4a2 2 0 0 1 2-2h3.17a2 2 0 0 1 1.41.59l1.42 1.41H18a2 2 0 0 1 2 2v1H8.5a2 2 0 0 0-1.8 1.12L3 15V4zM0 8.5l4.5-2.25A1 1 0 0 1 5 6h15v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2l-2-5.5z',
      viewBox: '0 0 20 18',
    },
    // Generic file
    file: {
      path: 'M4 1h8l4 4v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1zm7 0v4h4',
      viewBox: '0 0 16 18',
    },
    typescript: {
      path: 'M2 2h12v12H2V2zm5.5 3.5v1H6v4.5H5V6.5H3.5v-1h4zM9.5 6.2c-.4 0-.7.2-.7.5 0 .4.3.5.9.7.9.3 1.3.7 1.3 1.4 0 .9-.7 1.4-1.6 1.4-.7 0-1.2-.2-1.6-.6l.5-.6c.3.3.6.4 1 .4.4 0 .7-.2.7-.5 0-.3-.2-.5-.8-.7-1-.3-1.4-.7-1.4-1.5 0-.8.7-1.3 1.5-1.3.6 0 1.1.2 1.4.5l-.5.6c-.2-.2-.5-.3-.9-.3z',
      viewBox: '0 0 16 16',
    },
    javascript: {
      path: 'M2 2h12v12H2V2zm4.5 8.5c.3.3.6.5 1.1.5.5 0 .8-.2.8-.6 0-.4-.3-.5-.8-.7-.8-.3-1.3-.6-1.3-1.4 0-.8.6-1.3 1.4-1.3.6 0 1 .2 1.3.5l-.5.6c-.2-.2-.5-.3-.8-.3-.4 0-.6.2-.6.4 0 .3.3.5.8.6.9.3 1.3.7 1.3 1.4 0 .9-.7 1.5-1.7 1.5-.6 0-1.2-.2-1.6-.7l.6-.5zM11 7h-1v3.2c0 .6-.2.8-.5.8-.2 0-.3 0-.5-.2l-.4.7c.3.2.6.3 1 .3.9 0 1.4-.5 1.4-1.5V7z',
      viewBox: '0 0 16 16',
    },
    python: {
      path: 'M8 1C5.2 1 5.5 2.2 5.5 2.2V4h2.6v.8H4C4 4.8 1.7 4.5 1.7 8c0 3.5 2 3.4 2 3.4H5v-1.6s-.1-2 2-2h2.5s1.9 0 1.9-1.9V3.1S11.7 1 8 1zM6.3 2.4a.7.7 0 1 1 0 1.4.7.7 0 0 1 0-1.4zM8 15c2.8 0 2.5-1.2 2.5-1.2V12H7.9v-.8H12s2.3.3 2.3-3.2c0-3.5-2-3.4-2-3.4H11v1.6s.1 2-2 2H6.5s-1.9 0-1.9 1.9v2.8S4.3 15 8 15zm1.7-1.4a.7.7 0 1 1 0-1.4.7.7 0 0 1 0 1.4z',
      viewBox: '0 0 16 16',
    },
    rust: {
      path: 'M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zM5 6h2v4H5.5L5 11H4l.5-1H4V6h1zm4 0h2.5c.8 0 1.5.7 1.5 1.5S12.3 9 11.5 9H11v2H9V6z',
      viewBox: '0 0 16 16',
    },
    go: {
      path: 'M1 8a7 7 0 1 1 14 0A7 7 0 0 1 1 8zm4.5-1.5h5v1h-5v-1zm0 2h5v1h-5v-1z',
      viewBox: '0 0 16 16',
    },
    html: {
      path: 'M2 1l1.2 13L8 15l4.8-1L14 1H2zm9.5 4.5H6l.2 1.5h5l-.5 5.5-2.7.8-2.7-.8-.2-2h1.5l.1 1 1.3.3 1.3-.3.2-1.5H5.5L5 3.5h6l-.5 2z',
      viewBox: '0 0 16 16',
    },
    css: {
      path: 'M2 1l1.2 13L8 15l4.8-1L14 1H2zm9.2 4.5H6.3l.1 1.5h4.5l-.4 5-2.5.8-2.5-.8-.2-2H7l.1 1 .9.3.9-.3.1-1.5H5.7l-.4-5.5h5.5l-.6 1.5z',
      viewBox: '0 0 16 16',
    },
    svelte: {
      path: 'M10.6 1.6C9 .3 6.5.5 5.2 2.1L2.7 5.4c-.6.8-.9 1.7-.8 2.7.1.7.3 1.4.8 2-.3.5-.4 1-.4 1.6.1 1 .5 1.9 1.3 2.5 1.6 1.3 4 1.1 5.4-.5l2.5-3.3c.6-.8.9-1.7.8-2.7-.1-.7-.3-1.4-.8-2 .3-.5.4-1 .4-1.6-.1-1-.6-1.9-1.3-2.5z',
      viewBox: '0 0 14 16',
    },
    vue: {
      path: 'M8 12L2 2h3l3 5.5L11 2h3L8 12z',
      viewBox: '0 0 16 14',
    },
    json: {
      path: 'M4 2C2.9 2 2 2.9 2 4v2c0 .6-.4 1-1 1v2c.6 0 1 .4 1 1v2c0 1.1.9 2 2 2h1v-1.5H4.5c-.3 0-.5-.2-.5-.5V9.5c0-.5-.2-1-.6-1.3v-.4c.4-.3.6-.8.6-1.3V4c0-.3.2-.5.5-.5H5V2H4zm8 0h-1v1.5h.5c.3 0 .5.2.5.5v2.5c0 .5.2 1 .6 1.3v.4c-.4.3-.6.8-.6 1.3V12c0 .3-.2.5-.5.5H11V14h1c1.1 0 2-.9 2-2v-2c0-.6.4-1 1-1V7c-.6 0-1-.4-1-1V4c0-1.1-.9-2-2-2z',
      viewBox: '0 0 16 16',
    },
    markdown: {
      path: 'M1 3h14v10H1V3zm2.5 7.5V7l2 2.5L7.5 7v3.5h1.5L6.5 7 8 4.5h-1L5.5 7 4 4.5H3l1.5 2.5-1 3h1zm7.5-1l2-2v4h1.5V5.5l-2 2L10.5 5.5V9.5l2 1z',
      viewBox: '0 0 16 16',
    },
    config: {
      path: 'M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zM5 5h6v1H5V5zm0 2.5h6v1H5v-1zM5 10h4v1H5v-1z',
      viewBox: '0 0 16 16',
    },
    lock: {
      path: 'M11 7V5a3 3 0 0 0-6 0v2a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2zM8 12a1 1 0 1 1 0-2 1 1 0 0 1 0 2zm2-5H6V5a2 2 0 1 1 4 0v2z',
      viewBox: '0 0 16 16',
    },
    image: {
      path: 'M2 3a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V4a1 1 0 0 0-1-1H2zm3.5 2a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM2 12l3-4 2 2 3-4 4 6H2z',
      viewBox: '0 0 16 16',
    },
    pdf: {
      path: 'M4 1h5l4 4v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1zm4 0v4h4M5 9h1.5c.6 0 1-.4 1-1s-.4-1-1-1H5v4m4-4h1.2c.9 0 1.5.7 1.5 2s-.6 2-1.5 2H9V8',
      viewBox: '0 0 16 18',
    },
    terminal: {
      path: 'M2 3a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V4a1 1 0 0 0-1-1H2zm1 2l3 2.5L3 10m4 0h4',
      viewBox: '0 0 16 16',
    },
    git: {
      path: 'M14.7 7.3L8.7 1.3a1 1 0 0 0-1.4 0L5.7 2.9l1.8 1.8A1.2 1.2 0 0 1 9 5.9v4.3a1.2 1.2 0 1 1-1-.1V6.1L6.3 7.8a1.2 1.2 0 1 1-.9-.5l1.8-1.8-1.8-1.8L1.3 7.3a1 1 0 0 0 0 1.4l6 6a1 1 0 0 0 1.4 0l6-6a1 1 0 0 0 0-1.4z',
      viewBox: '0 0 16 16',
    },
    docker: {
      path: 'M1.5 8.5c.5-.3 1.5-.5 2.5-.2.2-1 .8-1.8 1.5-2.3l-.5-.8c0 0 0 0 0 0 .8-.5 2-.8 3-.5V3H6.5v2H5V3H3.5v2h-1v1.5H2V5H.5v2H0v1.5h1.5zM7 5.5h1.5V7H7V5.5zM5 5.5h1.5V7H5V5.5zM3 5.5h1.5V7H3V5.5zM5 3.5h1.5V5H5V3.5zM7 3.5h1.5V5H7V3.5z',
      viewBox: '0 0 16 12',
    },
    env: {
      path: 'M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zm0 2a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM5.5 12v-1c0-1.4 1.1-2.5 2.5-2.5s2.5 1.1 2.5 2.5v1h-5z',
      viewBox: '0 0 16 16',
    },
    package: {
      path: 'M8 1L1 5v6l7 4 7-4V5L8 1zM8 3l4.5 2.5L8 8 3.5 5.5 8 3zM2.5 6.5L7 9v4.5l-4.5-2.5v-4.5z',
      viewBox: '0 0 16 16',
    },
    gear: {
      path: 'M8 1l1.3.8.8-.5 1 1-.5.8.5 1H12.5v1.4l-.8.5.2 1 .9.5-.3 1.2-1 .1-.3 1 .6.8-.7 1.1-1-.3-.7.8.1 1L8 13l-1.3-.8-.8.5-1-1 .5-.8-.5-1H3.5V8.5l.8-.5-.2-1-.9-.5.3-1.2 1-.1.3-1-.6-.8.7-1.1 1 .3.7-.8L6.5 2 8 1zm0 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z',
      viewBox: '0 0 16 14',
    },
    // Open folder for the empty state
    folderPlus: {
      path: 'M4 2a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2H8.4L7 2.6A2 2 0 0 0 5.6 2H4zm4 4v2h2v1.5H8V12H6.5V9.5H4.5V8H6.5V5.5H8z',
      viewBox: '0 0 16 16',
    },
  };

  onDestroy(() => {
    stopWatching();
    if (watchDebounce) clearTimeout(watchDebounce);
  });
</script>

<div class="file-tree">
  {#if !rootPath}
    <div class="no-folder">
      <svg class="no-folder-icon" viewBox={icons.folderPlus.viewBox} fill="currentColor">
        <path d={icons.folderPlus.path} />
      </svg>
      <button class="open-folder-btn" onclick={openFolder}>Open Folder</button>
      <p>Open a project to begin</p>
    </div>
  {:else}
    <div class="tree-header">
      <div class="tree-header-left">
        <svg class="header-folder-icon" viewBox={icons.folderOpen.viewBox} fill="var(--accent)">
          <path d={icons.folderOpen.path} />
        </svg>
        <span class="root-name" title={rootPath}>{rootPath.split('/').pop()}</span>
      </div>
      <div class="tree-header-actions">
        <button class="header-btn" title="New file" onclick={() => startCreate('file')}>
          <svg class="header-btn-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
            <path d="M9 1H4a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V5L9 1z" />
            <path d="M8 7v4M6 9h4" />
          </svg>
        </button>
        <button class="header-btn" title="New folder" onclick={() => startCreate('folder')}>
          <svg class="header-btn-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
            <path d="M2 3a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1H7.4L6 3.6A1 1 0 0 0 5.2 3H2z" />
            <path d="M8 7v4M6 9h4" />
          </svg>
        </button>
        <button class="header-btn" title="Open folder" onclick={openFolder}>
          <svg class="header-btn-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
            <path d="M2 3a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1H7.4L6 3.6A1 1 0 0 0 5.2 3H2z" />
            <path d="M5 9l2-2 2 2" />
          </svg>
        </button>
      </div>
    </div>
    {#if creating}
      <div class="create-input-row">
        <span class="create-label">{creating === 'file' ? '📄' : '📁'}</span>
        <input
          bind:this={newNameInput}
          bind:value={newName}
          class="create-input"
          placeholder={creating === 'file' ? 'filename.ext' : 'folder name'}
          onkeydown={(e) => {
            if (e.key === 'Enter') confirmCreate();
            if (e.key === 'Escape') cancelCreate();
          }}
          onblur={cancelCreate}
        />
      </div>
    {/if}
    <div class="tree-content">
      {#each files.filter(e => !isHidden(e.name)) as entry}
        {@render fileNode(entry, 0)}
      {/each}
    </div>
  {/if}
</div>

{#if contextMenu}
  <div class="context-overlay" onclick={closeContextMenu} oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}></div>
  <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px">
    {#if selectedPaths.size > 1}
      <button class="context-item" onclick={copyFiles}>
        Copy {selectedPaths.size} items
      </button>
      {#if clipboardPaths.length > 0}
        <button class="context-item" onclick={() => {
          const paths = [...selectedPaths];
          const last = paths[paths.length - 1];
          const entry = findEntry(files, last);
          const dir = entry?.is_dir ? last : last.substring(0, last.lastIndexOf('/'));
          pasteFiles(dir);
        }}>
          Paste
        </button>
      {/if}
      <div class="context-separator"></div>
      <button class="context-item danger" onclick={deleteSelected}>
        Delete {selectedPaths.size} items
      </button>
    {:else}
      <button class="context-item" onclick={() => copyPath(contextMenu!.path)}>
        Copy Path
      </button>
      <button class="context-item" onclick={() => copyRelativePath(contextMenu!.path)}>
        Copy Relative Path
      </button>
      <div class="context-separator"></div>
      <button class="context-item" onclick={copyFiles}>
        Copy
      </button>
      {#if clipboardPaths.length > 0 && contextMenu!.isDir}
        <button class="context-item" onclick={() => pasteFiles(contextMenu!.path)}>
          Paste
        </button>
      {/if}
      <div class="context-separator"></div>
      <button class="context-item" onclick={() => startRename(contextMenu!.path)}>
        Rename
      </button>
      <button class="context-item danger" onclick={deleteSelected}>
        Delete
      </button>
    {/if}
  </div>
{/if}

{#snippet fileNode(entry: FileEntry, depth: number)}
  <button
    class="tree-item"
    class:is-dir={entry.is_dir}
    class:selected={selectedPath === entry.path || selectedPaths.has(entry.path)}
    class:multi-selected={selectedPaths.has(entry.path)}
    style="padding-left: {8 + depth * 8}px"
    onclick={(e) => handleFileClick(entry, e)}
    oncontextmenu={(e) => handleContextMenu(e, entry)}
  >
    {#if entry.is_dir}
      <span class="chevron" class:expanded={expandedDirs.has(entry.path)}>
        <svg viewBox="0 0 16 16" fill="currentColor" width="10" height="10">
          <path d="M6 3l5 5-5 5V3z" />
        </svg>
      </span>
      <svg class="icon dir-icon" viewBox={expandedDirs.has(entry.path) ? icons.folderOpen.viewBox : icons.folderClosed.viewBox} fill="currentColor">
        <path d={expandedDirs.has(entry.path) ? icons.folderOpen.path : icons.folderClosed.path} />
      </svg>
    {:else}
      <span class="file-indent"></span>
      {@const fi = getFileIconSvg(entry.name)}
      <svg class="icon file-icon-svg" viewBox={fi.viewBox} fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" style="color: {getFileColor(entry.name)}">
        <path d={fi.path} />
      </svg>
    {/if}
    {#if renamingPath === entry.path}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:this={renameInput}
        bind:value={renameValue}
        class="rename-input"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => {
          if (e.key === 'Enter') confirmRename();
          if (e.key === 'Escape') cancelRename();
        }}
        onblur={confirmRename}
      />
    {:else}
      {@const gitColor = getGitStatusColor(entry.path, entry.is_dir)}
      <span class="file-name" class:dir-name={entry.is_dir} style={gitColor ? `color: ${gitColor}` : ''}>{entry.name}</span>
      {#if !entry.is_dir && gitFileStatus.has(entry.path)}
        <span class="git-badge" style="color: {gitColor}">{gitFileStatus.get(entry.path)}</span>
      {/if}
    {/if}
  </button>

  {#if entry.is_dir && expandedDirs.has(entry.path) && entry.children}
    {#each entry.children.filter(c => !isHidden(c.name)) as child}
      {@render fileNode(child, depth + 1)}
    {/each}
  {/if}
{/snippet}

<style>
  .file-tree {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding-bottom: 12px;
  }

  /* Empty state */
  .no-folder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px 16px;
    gap: 10px;
    text-align: center;
    color: var(--text-muted);
    font-size: 12px;
  }

  .no-folder-icon {
    width: 40px;
    height: 40px;
    color: var(--accent);
    opacity: 0.6;
    margin-bottom: 4px;
  }

  .open-folder-btn {
    background: var(--accent);
    color: var(--bg-tertiary);
    padding: 6px 20px;
    border-radius: 5px;
    font-weight: 600;
    font-size: 11px;
    letter-spacing: 0.3px;
    transition: background 0.15s;
  }

  .open-folder-btn:hover {
    background: var(--accent-hover);
  }

  /* Header */
  .tree-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 12px 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    margin-bottom: 4px;
  }

  .tree-header-left {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .header-folder-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .root-name {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    color: var(--text-secondary);
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tree-header-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  .header-btn {
    font-size: 11px;
    color: var(--text-muted);
    padding: 3px 5px;
    border-radius: 4px;
    transition: all 0.15s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .header-btn:hover {
    color: var(--text-primary);
    background: var(--bg-surface);
  }

  .header-btn-icon {
    width: 14px;
    height: 14px;
  }

  /* Tree content */
  .tree-content {
    display: flex;
    flex-direction: column;
  }

  /* Tree items */
  .tree-item {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: var(--density-tree-padding, 2px 8px);
    width: 100%;
    text-align: left;
    font-size: 11.5px;
    color: var(--text-secondary);
    border-radius: 3px;
    margin: 0 3px;
    width: calc(100% - 6px);
    transition: all 0.1s;
  }

  .tree-item:hover {
    background: color-mix(in srgb, var(--bg-surface) 60%, transparent);
    color: var(--text-primary);
  }

  .tree-item.selected {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text-primary);
  }

  /* Chevron */
  .chevron {
    color: var(--text-muted);
    width: 10px;
    height: 10px;
    flex-shrink: 0;
    transition: transform 0.15s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .chevron.expanded {
    transform: rotate(90deg);
  }

  /* Icons */
  .icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .dir-icon {
    color: var(--accent);
  }

  .file-indent {
    width: 10px;
    flex-shrink: 0;
  }

  .file-icon-svg {
    opacity: 0.9;
  }

  /* Names */
  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.3;
  }

  .dir-name {
    font-weight: 500;
    color: var(--text-primary);
  }

  .git-badge {
    font-size: 9px;
    font-weight: 700;
    margin-left: auto;
    flex-shrink: 0;
    opacity: 0.8;
  }

  /* Create input */
  .create-input-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }

  .create-label {
    font-size: 12px;
    flex-shrink: 0;
  }

  .create-input {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    padding: 3px 6px;
    background: var(--bg-tertiary);
    border: 1px solid var(--accent);
    color: var(--text-primary);
    border-radius: 3px;
    outline: none;
  }

  /* Multi-select */
  .tree-item.multi-selected {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  /* Inline rename */
  .rename-input {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    padding: 0 4px;
    height: 18px;
    background: var(--bg-tertiary);
    border: 1px solid var(--accent);
    color: var(--text-primary);
    border-radius: 3px;
    outline: none;
    font-family: inherit;
  }

  /* Context menu */
  .context-overlay {
    position: fixed;
    inset: 0;
    z-index: 999;
  }

  .context-menu {
    position: fixed;
    z-index: 1000;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px;
    min-width: 140px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  }

  .context-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 5px 10px;
    font-size: 11.5px;
    color: var(--text-secondary);
    border-radius: 4px;
    transition: all 0.1s;
  }

  .context-item:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .context-item.danger:hover {
    background: color-mix(in srgb, var(--error) 15%, transparent);
    color: var(--error);
  }

  .context-separator {
    height: 1px;
    background: var(--border);
    margin: 3px 6px;
  }
</style>
