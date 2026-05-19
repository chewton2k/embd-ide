<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open, ask } from '@tauri-apps/plugin-dialog';
  import { watch, type UnwatchFn } from '@tauri-apps/plugin-fs';
  import { startDrag } from '@crabnebula/tauri-plugin-drag';
  import Icon from '@iconify/svelte';
  import { FolderOpen, Folder, ChevronRight, Link2 } from 'lucide-svelte';
  import { projectRoot, hiddenPatterns, renameOpenFile, fileTreeRefreshTrigger, closeAllUnpinned, sharedGitStatus, sharedGitRemoteStatus, gitBranch, addFile, togglePin, activeFilePath, fileTreeNavTarget, openDiagrams, diagramPath, showPreview, createFileSignal, createFolderSignal } from '../../modules';
  import { saveSessionNow, findRecentProject } from '../../modules/session';
  import { beginGitBranchRequest, getLatestGitBranchRequestId, updateGitBranch } from '../../modules/git/branchUpdate';
  import { log } from '../../modules/logging';
  import { exists } from '@tauri-apps/plugin-fs';
  import Button from '../ui/button/Button.svelte';
  import { getFileIconName } from '../../modules/explorer';

  function isValidName(name: string): boolean {
    return name.length > 0 && !/[\/\\]/.test(name) && name !== '..' && name !== '.';
  }

  interface FileEntry {
    name: string;
    path: string;
    is_dir: boolean;
    /** True when the entry is a symbolic link. The tree shows a Link2
        badge for these entries; symlinked directories are listed but
        not expanded. */
    is_symlink?: boolean;
    children: FileEntry[] | null;
  }

  let { onFileSelect, onSearchFiles, onOpenFolder: onOpenFolderProp }: { onFileSelect: (path: string, name: string) => void; onSearchFiles?: () => void; onOpenFolder?: (fn: (path: string, restoreSession?: boolean) => Promise<void>) => void } = $props();
  let files = $state<FileEntry[]>([]);
  let expandedDirs = $state<Set<string>>(new Set());
  let rootPath = $state<string | null>(null);
  let selectedPath = $state<string | null>(null);
  let selectedPaths = $state<Set<string>>(new Set());

  // New file/folder creation state
  let creating = $state<'file' | 'folder' | null>(null);
  let createParentPath = $state<string | null>(null);
  let newName = $state('');
  let newNameInput: HTMLInputElement | undefined = $state();
  let createError = $state<string | null>(null);

  // Context menu state
  let contextMenu = $state<{ x: number; y: number; path: string; isDir: boolean } | null>(null);
  let contextMenuEl = $state<HTMLDivElement | undefined>();

  $effect(() => {
    if (contextMenuEl && contextMenu) {
      const rect = contextMenuEl.getBoundingClientRect();
      const viewH = window.innerHeight;
      const viewW = window.innerWidth;
      if (rect.bottom > viewH) {
        contextMenu.y = Math.max(4, contextMenu.y - (rect.bottom - viewH) - 8);
      }
      if (rect.right > viewW) {
        contextMenu.x = Math.max(4, contextMenu.x - (rect.right - viewW) - 8);
      }
    }
  });

  $effect(() => {
    const target = $fileTreeNavTarget;
    if (!target) return;
    const normTarget = target.replace(/\\/g, '/');
    const normRoot = rootPath ? rootPath.replace(/\\/g, '/') : null;
    if (normRoot && normTarget.startsWith(normRoot)) {
      const rel = normTarget.slice(normRoot.length).replace(/^\//, '');
      const parts = rel.split('/').filter(Boolean);
      let current = normRoot;
      const toExpand: string[] = [];
      for (let i = 0; i < parts.length - 1; i++) {
        current += '/' + parts[i];
        toExpand.push(current);
      }
      // untrack prevents expandedDirs from becoming a tracked dependency of this effect
      untrack(() => {
        for (const dir of toExpand) expandedDirs.add(dir);
        if (toExpand.length > 0) expandedDirs = new Set(expandedDirs);
      });
    }
    selectedPath = target;
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-path="${CSS.escape(target)}"]`);
      el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      fileTreeNavTarget.set(null);
    });
  });

  // React to toolbar create-file/folder signals
  let lastCreateFile = 0;
  $effect(() => {
    const v = $createFileSignal;
    if (v > lastCreateFile) { lastCreateFile = v; startCreate('file'); }
  });
  let lastCreateFolder = 0;
  $effect(() => {
    const v = $createFolderSignal;
    if (v > lastCreateFolder) { lastCreateFolder = v; startCreate('folder'); }
  });

  // Clipboard for copy/paste files
  let clipboardPaths = $state<string[]>([]);

  // Rename state
  let renamingPath = $state<string | null>(null);
  let renameValue = $state('');
  let renameInput: HTMLInputElement | undefined = $state();

  // Undo/redo stack for file operations
  interface FileOp {
    type: 'move' | 'rename';
    // For move: sources moved, destDir they went to, and their original parents
    sources?: string[];
    destDir?: string;
    originalParents?: string[];
    // For rename: old and new path
    oldPath?: string;
    newPath?: string;
  }
  let undoStack = $state<FileOp[]>([]);
  let redoStack = $state<FileOp[]>([]);

  async function undoLastOp() {
    if (undoStack.length === 0) return;
    const op = undoStack.pop()!;
    undoStack = [...undoStack];

    if (op.type === 'move' && op.sources && op.destDir && op.originalParents) {
      // Move each file back to its original parent
      for (let i = 0; i < op.sources.length; i++) {
        const name = op.sources[i].split('/').pop()!;
        const currentPath = `${op.destDir}/${name}`;
        const originalParent = op.originalParents[i];
        try {
          await invoke('move_entries', { sources: [currentPath], destDir: originalParent });
        } catch (e) {
          log.error('Undo move failed', e);
        }
      }
      // Push reverse op to redo stack
      redoStack = [...redoStack, op];
      await refreshTree();
    } else if (op.type === 'rename' && op.oldPath && op.newPath) {
      try {
        await invoke('rename_entry', { oldPath: op.newPath, newPath: op.oldPath });
        redoStack = [...redoStack, op];
      } catch (e) {
        log.error('Undo rename failed', e);
      }
      await refreshTree();
    }
  }

  async function redoLastOp() {
    if (redoStack.length === 0) return;
    const op = redoStack.pop()!;
    redoStack = [...redoStack];

    if (op.type === 'move' && op.sources && op.destDir) {
      try {
        await invoke('move_entries', { sources: op.sources, destDir: op.destDir });
        undoStack = [...undoStack, op];
      } catch (e) {
        log.error('Redo move failed', e);
      }
      await refreshTree();
    } else if (op.type === 'rename' && op.oldPath && op.newPath) {
      try {
        await invoke('rename_entry', { oldPath: op.oldPath, newPath: op.newPath });
        undoStack = [...undoStack, op];
      } catch (e) {
        log.error('Redo rename failed', e);
      }
      await refreshTree();
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    // Don't intercept undo/redo when focused on text inputs (e.g. code editor, rename input)
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA') return;
    // Also skip if inside a contenteditable or the editor area
    if ((e.target as HTMLElement)?.closest?.('.cm-editor, [contenteditable]')) return;

    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key === 'z' && !e.shiftKey) {
      if (undoStack.length === 0) return; // Let other handlers deal with it
      e.preventDefault();
      undoLastOp();
    } else if (mod && e.key === 'z' && e.shiftKey) {
      if (redoStack.length === 0) return;
      e.preventDefault();
      redoLastOp();
    }
  }

  // Drag-and-drop state (using mouse events — HTML5 DnD is intercepted by Tauri's native webview)
  let draggedPaths = $state<string[]>([]);
  let dropTargetPath = $state<string | null>(null);
  let dragExpandTimer: ReturnType<typeof setTimeout> | null = null;
  let isDragging = $state(false);
  let dragStartPos = { x: 0, y: 0 };
  let dragPending = false; // mousedown happened, waiting for enough movement to start drag
  let dragPendingEntry: FileEntry | null = null;
  let dragGhost: HTMLElement | null = null;

  // External file drop state (files dragged from OS/Finder)
  let externalDropPaths = $state<string[]>([]);
  let isExternalDrag = $state(false);
  let unlistenDragEnter: UnlistenFn | null = null;
  let unlistenDragOver: UnlistenFn | null = null;
  let unlistenDragDrop: UnlistenFn | null = null;
  let unlistenDragLeave: UnlistenFn | null = null;

  // Git status: absolute path -> status code (M=modified, A=staged new, S=staged modified, D=deleted, U=untracked)
  let gitFileStatus = $state<Map<string, string>>(new Map());
  // Derived: folder path -> "highest priority" status of any child
  let gitFolderStatus = $state<Map<string, string>>(new Map());
  // Remote git status: files changed on upstream tracking branch
  let gitRemoteFileStatus = $state<Map<string, string>>(new Map());
  let gitRemoteFolderStatus = $state<Map<string, string>>(new Map());

  // Gitignored paths (files and directories)
  let gitIgnoredPaths = $state<Set<string>>(new Set());

  function recordsEqual(a: Record<string, string>, b: Record<string, string>): boolean {
    const keysA = Object.keys(a);
    if (keysA.length !== Object.keys(b).length) return false;
    for (const k of keysA) { if (a[k] !== b[k]) return false; }
    return true;
  }

  async function fetchGitStatus(preserveBranchOnNull: boolean = true) {
    const requestRoot = rootPath;
    if (!requestRoot) return;
    const branchRequestId = beginGitBranchRequest();
    let nextSharedStatus: Record<string, string> = {};
    let newFileStatus: Map<string, string>;
    let newFolderStatus: Map<string, string>;
    let newIgnored: Set<string>;
    try {
      const status = await invoke<Record<string, string>>('get_git_status', { path: requestRoot });
      nextSharedStatus = status;
      newFileStatus = new Map(Object.entries(status));
      // Compute folder statuses
      const folders = new Map<string, string>();
      const priority: Record<string, number> = { M: 3, U: 2, A: 1, S: 1, D: 2 };
      for (const [filePath, code] of newFileStatus) {
        let dir = filePath.substring(0, filePath.lastIndexOf('/'));
        while (dir.length >= requestRoot.length) {
          const existing = folders.get(dir);
          if (!existing || (priority[code] ?? 0) > (priority[existing] ?? 0)) {
            folders.set(dir, code);
          }
          dir = dir.substring(0, dir.lastIndexOf('/'));
        }
      }
      newFolderStatus = folders;
    } catch (_) {
      newFileStatus = new Map();
      newFolderStatus = new Map();
    }
    // Fetch remote status (files changed on upstream after git fetch)
    let nextSharedRemoteStatus: Record<string, string> = {};
    let newRemoteFileStatus: Map<string, string>;
    let newRemoteFolderStatus: Map<string, string>;
    try {
      const remoteStatus = await invoke<Record<string, string>>('get_git_remote_status', { path: requestRoot });
      nextSharedRemoteStatus = remoteStatus;
      newRemoteFileStatus = new Map(Object.entries(remoteStatus));
      // Compute folder propagation for remote status
      const remoteFolders = new Map<string, string>();
      const remotePriority: Record<string, number> = { M: 3, A: 2, D: 1 };
      for (const [filePath, code] of newRemoteFileStatus) {
        let dir = filePath.substring(0, filePath.lastIndexOf('/'));
        while (dir.length >= requestRoot.length) {
          const existing = remoteFolders.get(dir);
          if (!existing || (remotePriority[code] ?? 0) > (remotePriority[existing] ?? 0)) {
            remoteFolders.set(dir, code);
          }
          dir = dir.substring(0, dir.lastIndexOf('/'));
        }
      }
      newRemoteFolderStatus = remoteFolders;
    } catch (_) {
      newRemoteFileStatus = new Map();
      newRemoteFolderStatus = new Map();
    }
    // Fetch gitignored paths
    try {
      const ignored = await invoke<string[]>('get_git_ignored', { path: requestRoot });
      newIgnored = new Set(ignored);
    } catch (_) {
      newIgnored = new Set();
    }
    let branch: string | null = null;
    try {
      branch = await invoke<string | null>('get_git_branch', { path: requestRoot });
    } catch (_) {
      // Keep the last known branch on transient errors (e.g. git lock)
    }
    if (rootPath !== requestRoot) return;
    // Batch all reactive updates together to avoid multiple re-renders
    if (!recordsEqual(nextSharedStatus, $sharedGitStatus)) {
      sharedGitStatus.set(nextSharedStatus);
    }
    if (!recordsEqual(nextSharedRemoteStatus, $sharedGitRemoteStatus)) {
      sharedGitRemoteStatus.set(nextSharedRemoteStatus);
    }
    gitFileStatus = newFileStatus;
    gitFolderStatus = newFolderStatus;
    gitRemoteFileStatus = newRemoteFileStatus;
    gitRemoteFolderStatus = newRemoteFolderStatus;
    gitIgnoredPaths = newIgnored;
    rebuildIgnoredPrefixes();
    updateGitBranch({
      branch,
      preserveOnNull: preserveBranchOnNull,
      requestProjectRoot: requestRoot,
      activeProjectRoot: rootPath,
      requestId: branchRequestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
  }

  // Pre-computed set of ignored directory prefixes (with trailing /) for O(depth) lookup
  let gitIgnoredPrefixes = $state<string[]>([]);

  function rebuildIgnoredPrefixes() {
    const prefixes: string[] = [];
    for (const p of gitIgnoredPaths) {
      prefixes.push(p + '/');
    }
    gitIgnoredPrefixes = prefixes;
  }

  function isGitIgnored(path: string): boolean {
    if (gitIgnoredPaths.has(path)) return true;
    // Walk up the path checking each ancestor against the prefix list
    for (const prefix of gitIgnoredPrefixes) {
      if (path.startsWith(prefix)) return true;
    }
    return false;
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

  function getGitRemoteStatusColor(path: string, isDir: boolean): string | null {
    const code = isDir ? gitRemoteFolderStatus.get(path) : gitRemoteFileStatus.get(path);
    if (!code) return null;
    return 'var(--git-remote, #94e2d5)'; // teal — distinct from all local status colors
  }

  // Git status polling (git operations only modify .git/ which the file watcher doesn't cover)
  let gitPollInterval: ReturnType<typeof setInterval> | null = null;

  function startGitPolling() {
    stopGitPolling();
    gitPollInterval = setInterval(() => fetchGitStatus(), 3000);
  }

  function stopGitPolling() {
    if (gitPollInterval) {
      clearInterval(gitPollInterval);
      gitPollInterval = null;
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

  let refreshInProgress = false;

  async function refreshTree() {
    if (!rootPath) return;
    if (refreshInProgress) return; // Prevent overlapping refreshes
    refreshInProgress = true;
    try {
      const newFiles = await invoke<FileEntry[]>('read_dir_tree', { path: rootPath, depth: 1 });
      // Re-expand previously expanded dirs
      for (const dir of expandedDirs) {
        const entry = findEntry(newFiles, dir);
        if (entry) {
          try {
            const children = await invoke<FileEntry[]>('read_dir_tree', { path: entry.path, depth: 1 });
            entry.children = children;
          } catch (_) { /* dir may have been deleted */ }
        }
      }
      files = newFiles;
      await fetchGitStatus();
    } finally {
      refreshInProgress = false;
    }
  }

  async function openFolderByPath(path: string, restoreSession: boolean = true) {
    // Save current session before switching
    if (rootPath) {
      try {
        await saveSessionNow(rootPath);
      } catch (e) {
        log.error('Failed to save session before folder switch', e);
      }
    }
    rootPath = path;
    const canonical = await invoke<string>('set_project_root', { path: rootPath });
    rootPath = canonical;
    projectRoot.set(canonical);
    closeAllUnpinned();
    openDiagrams.set([]);
    showPreview.set(false);
    expandedDirs = new Set([rootPath]);
    await loadDirectory(rootPath);
    await fetchGitStatus(false);
    startWatching(rootPath);
    startGitPolling();

    // Restore saved session if this folder is in recent projects
    if (restoreSession) {
      try {
        const project = await findRecentProject(path);
        if (project && project.session.open_files.length > 0) {
          for (const file of project.session.open_files) {
            try {
              const fileExists = await exists(file.path);
              if (!fileExists) continue;
            } catch { continue; } // Skip files blocked by scope permissions
            const name = file.path.split(/[/\\]/).pop() || file.path;
            addFile(file.path, name);
            if (file.pinned) togglePin(file.path);
          }
          if (project.session.active_file) {
            try {
              const activeExists = await exists(project.session.active_file);
              if (activeExists) {
                activeFilePath.set(project.session.active_file);
              }
            } catch { /* scope permission — skip */ }
          }
        }
      } catch (e) {
        log.error('Failed to restore session', e);
      }
    }
  }

  async function openFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      try {
        await openFolderByPath(selected as string);
      } catch (e) {
        const msg = String(e);
        if (msg.includes('scope') || msg.includes('not allowed')) {
          log.error(`Cannot open folder: path "${selected}" is outside the allowed filesystem scope`);
        } else {
          log.error('Failed to open folder', e);
        }
      }
    }
  }

  async function loadDirectory(path: string) {
    try {
      files = await invoke<FileEntry[]>('read_dir_tree', { path, depth: 1 });
    } catch (e) {
      log.error('Failed to read directory', e);
    }
  }

  async function toggleDir(entry: FileEntry) {
    if (expandedDirs.has(entry.path)) {
      // If collapsing the folder that has an active creation, cancel it
      if (creating && createParentPath === entry.path) {
        cancelCreate();
      }
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
        log.error('Failed to expand', e);
      }
    }
  }

  function handleFileClick(entry: FileEntry, e: MouseEvent) {
    // If creating, cancel/confirm and consume the click — don't open the file
    if (creating) {
      if (newName.trim() && isValidName(newName.trim())) {
        confirmCreate();
      } else {
        cancelCreate();
      }
      return;
    }

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

  function startCreate(type: 'file' | 'folder', targetPath?: string) {
    // Cancel any existing creation
    if (creating) cancelCreate();

    creating = type;
    newName = '';
    createError = null;

    // Determine parent directory
    if (targetPath) {
      // Explicit target passed (from context menu or signal) — use directly
      createParentPath = targetPath;
    } else if (selectedPath && selectedPath !== rootPath) {
      const entry = findEntry(files, selectedPath);
      if (entry?.is_dir) {
        createParentPath = entry.path;
      } else if (selectedPath.includes('/')) {
        createParentPath = selectedPath.substring(0, selectedPath.lastIndexOf('/'));
      } else {
        createParentPath = rootPath;
      }
    } else {
      createParentPath = rootPath;
    }

    // Expand the parent folder so the inline input is visible
    if (createParentPath) {
      expandedDirs.add(createParentPath);
      expandedDirs = new Set(expandedDirs);
    }
    if (rootPath) {
      expandedDirs.add(rootPath);
      expandedDirs = new Set(expandedDirs);
    }

    // Wait for DOM to update then focus
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        newNameInput?.focus();
      });
    });
  }

  async function confirmCreate() {
    const name = newName.trim();
    createError = null;

    if (!name || !rootPath || !createParentPath) {
      cancelCreate();
      return;
    }

    // Validation
    if (!isValidName(name)) {
      createError = 'Invalid name';
      return;
    }
    if (name.includes('/') || name.includes('\\')) {
      createError = 'Name cannot contain path separators';
      return;
    }
    if (name.startsWith('.') && name.length === 1) {
      createError = 'Invalid name';
      return;
    }

    // Check for duplicates in parent
    const parentEntry = createParentPath === rootPath
      ? { children: files }
      : findEntry(files, createParentPath);
    const siblings = parentEntry?.children ?? files;
    const duplicate = siblings.some(e => e.name.toLowerCase() === name.toLowerCase());
    if (duplicate) {
      createError = `"${name}" already exists`;
      return;
    }

    const fullPath = `${createParentPath}/${name}`;
    const type = creating;

    try {
      if (type === 'file') {
        await invoke('create_file', { path: fullPath });
        onFileSelect(fullPath, name);
      } else {
        await invoke('create_folder', { path: fullPath });
        expandedDirs.add(fullPath);
        expandedDirs = new Set(expandedDirs);
      }

      // Keep parent expanded
      let dir = createParentPath;
      while (dir && dir.length > (rootPath?.length ?? 0)) {
        expandedDirs.add(dir);
        dir = dir.substring(0, dir.lastIndexOf('/'));
      }
      expandedDirs = new Set(expandedDirs);

      await refreshTree();
      selectedPath = fullPath;
      selectedPaths = new Set([fullPath]);
    } catch (e) {
      createError = `Failed: ${e}`;
      return;
    }

    creating = null;
    createParentPath = null;
    newName = '';
    createError = null;
  }

  function cancelCreate() {
    creating = null;
    createParentPath = null;
    newName = '';
    createError = null;
  }

  function handleCreateBlur() {
    setTimeout(() => {
      if (!creating) return;
      if (newName.trim() && isValidName(newName.trim())) {
        confirmCreate();
      } else {
        cancelCreate();
      }
    }, 100);
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
    const confirmed = await ask(
      `Delete ${paths.length} item${paths.length !== 1 ? 's' : ''}? This cannot be undone.`,
      { title: 'Confirm Delete', kind: 'warning' }
    );
    if (!confirmed) return;
    try {
      await invoke('delete_entries', { paths });
    } catch (e) {
      log.error('Failed to delete', e);
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
      log.error('Failed to paste', e);
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

  async function duplicateEntry(path: string) {
    closeContextMenu();
    try {
      await invoke('duplicate_entry', { path });
    } catch (e) {
      log.error('Failed to duplicate', e);
    }
    await refreshTree();
  }

  async function revealInFileManager(path: string) {
    closeContextMenu();
    try {
      await invoke('reveal_in_file_manager', { path });
    } catch (e) {
      log.error('Failed to reveal in file manager', e);
    }
  }

  function openDiagram(path: string) {
    closeContextMenu();
    openDiagrams.update(d => d.includes(path) ? d : [...d, path]);
    activeFilePath.set(diagramPath(path));
  }

  async function addToGitignore(path: string) {
    closeContextMenu();
    const root = $projectRoot;
    if (!root) return;
    const relativePath = path.startsWith(root) ? path.slice(root.length + 1) : path;
    const gitignorePath = `${root}/.gitignore`;
    try {
      let content = '';
      try {
        content = await invoke<string>('read_file_content', { path: gitignorePath });
      } catch { /* file doesn't exist yet */ }
      const lines = content.split('\n');
      if (lines.some(l => l.trim() === relativePath)) return; // already in .gitignore
      const newContent = content.endsWith('\n') || content === ''
        ? `${content}${relativePath}\n`
        : `${content}\n${relativePath}\n`;
      await invoke('write_file_content', { path: gitignorePath, content: newContent });
    } catch (e) {
      log.error('Failed to add to .gitignore', e);
    }
  }

  async function confirmRename() {
    if (!renamingPath || !renameValue.trim()) {
      cancelRename();
      return;
    }
    if (!isValidName(renameValue.trim())) {
      log.error('Invalid name: must not contain / or \\ or be . or ..');
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
        undoStack = [...undoStack, { type: 'rename', oldPath, newPath }];
        redoStack = [];
      } catch (e) {
        log.error('Failed to rename', e);
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

  // Drag-and-drop helpers
  function isDescendant(parentPath: string, childPath: string): boolean {
    return childPath.startsWith(parentPath + '/');
  }

  function getParentDir(path: string): string {
    return path.substring(0, path.lastIndexOf('/'));
  }

  async function moveEntries(paths: string[], destDir: string) {
    // Record original parent dirs for undo
    const originalParents = paths.map(p => getParentDir(p));
    try {
      await invoke('move_entries', { sources: paths, destDir });
      // Push to undo stack
      undoStack = [...undoStack, { type: 'move', sources: paths, destDir, originalParents }];
      redoStack = []; // Clear redo on new action
    } catch (e) {
      log.error('Failed to move', e);
    }
    await refreshTree();
  }

  function handleDragMouseDown(entry: FileEntry, e: MouseEvent) {
    // Don't initiate drag from inputs or during rename
    if ((e.target as HTMLElement).tagName === 'INPUT') return;
    if (renamingPath === entry.path) return;
    if (e.button !== 0) return; // left click only

    dragPending = true;
    dragPendingEntry = entry;
    dragStartPos = { x: e.clientX, y: e.clientY };
    e.preventDefault(); // Prevent text selection from starting
    window.addEventListener('mousemove', handleGlobalMouseMove);
  }

  let dragRafId: number | null = null;

  function handleGlobalMouseMove(e: MouseEvent) {
    // Fast path: if not dragging or pending, skip immediately
    if (!dragPending && !isDragging) return;

    if (dragPending && dragPendingEntry) {
      const dx = e.clientX - dragStartPos.x;
      const dy = e.clientY - dragStartPos.y;
      // Require 5px of movement to start a drag (avoid accidental drags on click)
      if (Math.abs(dx) > 5 || Math.abs(dy) > 5) {
        startInternalDrag(dragPendingEntry, e);
        dragPending = false;
        dragPendingEntry = null;
      }
      return;
    }

    if (!isDragging) return;

    // Ghost element moves immediately (cheap, no DOM queries)
    if (dragGhost) {
      dragGhost.style.left = `${e.clientX + 12}px`;
      dragGhost.style.top = `${e.clientY - 8}px`;
    }

    // Throttle the expensive DOM hit-testing to once per animation frame
    if (dragRafId) return;
    const clientX = e.clientX;
    const clientY = e.clientY;
    dragRafId = requestAnimationFrame(() => {
      dragRafId = null;
      if (!isDragging) return;
      handleDragHitTest(clientX, clientY);
    });
  }

  function handleDragHitTest(clientX: number, clientY: number) {
    // Detect mouse near window edge — hand off to OS drag
    const edgeThreshold = 10;
    const nearEdge =
      clientX <= edgeThreshold ||
      clientY <= edgeThreshold ||
      clientX >= window.innerWidth - edgeThreshold ||
      clientY >= window.innerHeight - edgeThreshold;

    if (nearEdge && draggedPaths.length > 0) {
      const paths = [...draggedPaths];
      endDrag();
      startDrag({ item: paths, icon: '' }).catch(() => {});
      return;
    }

    // Find which tree-item we're hovering over
    const el = document.elementFromPoint(clientX, clientY);
    if (!el) {
      updateDropTarget(null);
      return;
    }

    const treeItem = el.closest('.tree-item') as HTMLElement | null;
    const treeContent = el.closest('.tree-content') as HTMLElement | null;

    if (treeItem) {
      const path = treeItem.dataset.path;
      const isDir = treeItem.dataset.isdir === 'true';
      if (path) {
        const targetDir = isDir ? path : getParentDir(path);
        // Validate: not dropping onto self or descendants
        for (const dp of draggedPaths) {
          if (dp === targetDir || isDescendant(dp, targetDir)) {
            updateDropTarget(null);
            return;
          }
        }
        updateDropTarget(targetDir);

        // Auto-expand collapsed folder on hover
        if (isDir && !expandedDirs.has(path)) {
          if (dragExpandTimer) clearTimeout(dragExpandTimer);
          dragExpandTimer = setTimeout(() => {
            const entry = findEntry(files, path);
            if (entry) toggleDir(entry);
          }, 600);
        }
      }
    } else if (treeContent && rootPath) {
      // Hovering on empty area — target is root
      updateDropTarget(rootPath);
    } else {
      updateDropTarget(null);
    }
  }

  function updateDropTarget(path: string | null) {
    if (dropTargetPath === path) return;
    dropTargetPath = path;
    if (dragExpandTimer && !path) {
      clearTimeout(dragExpandTimer);
      dragExpandTimer = null;
    }
  }

  function startInternalDrag(entry: FileEntry, e: MouseEvent) {
    isDragging = true;

    // Set dragged paths
    if (selectedPaths.has(entry.path) && selectedPaths.size > 1) {
      draggedPaths = [...selectedPaths];
    } else {
      draggedPaths = [entry.path];
    }

    // Create ghost element
    dragGhost = document.createElement('div');
    dragGhost.className = 'drag-ghost';
    const count = draggedPaths.length;
    const name = entry.name;
    dragGhost.textContent = count > 1 ? `${name} (+${count - 1})` : name;
    dragGhost.style.left = `${e.clientX + 12}px`;
    dragGhost.style.top = `${e.clientY - 8}px`;
    document.body.appendChild(dragGhost);

    // Prevent text selection while dragging
    document.body.style.userSelect = 'none';
    window.getSelection()?.removeAllRanges();
  }

  function handleGlobalMouseUp(e: MouseEvent) {
    if (dragPending) {
      // Didn't move enough to start a drag — it was just a click
      dragPending = false;
      dragPendingEntry = null;
      return;
    }

    if (!isDragging) return;

    if (dropTargetPath && draggedPaths.length > 0) {
      // Validate again
      let valid = true;
      for (const dp of draggedPaths) {
        if (dp === dropTargetPath || isDescendant(dp, dropTargetPath)) {
          valid = false;
          break;
        }
        if (getParentDir(dp) === dropTargetPath && draggedPaths.length === 1) {
          valid = false;
          break;
        }
      }
      if (valid) {
        moveEntries(draggedPaths, dropTargetPath);
      }
    }

    endDrag();
  }

  function endDrag() {
    isDragging = false;
    draggedPaths = [];
    dropTargetPath = null;
    dragPending = false;
    dragPendingEntry = null;
    window.removeEventListener('mousemove', handleGlobalMouseMove);
    if (dragGhost) {
      dragGhost.remove();
      dragGhost = null;
    }
    if (dragExpandTimer) {
      clearTimeout(dragExpandTimer);
      dragExpandTimer = null;
    }
    if (dragRafId) {
      cancelAnimationFrame(dragRafId);
      dragRafId = null;
    }
    document.body.style.userSelect = '';
  }

  // External file drop handling (files from OS/Finder)
  async function setupExternalDropListeners() {
    unlistenDragEnter = await listen<{ paths: string[]; position: { x: number; y: number } }>('tauri://drag-enter', (event) => {
      if (!rootPath) return;
      externalDropPaths = event.payload.paths;
      isExternalDrag = true;
      // Default drop target to root
      dropTargetPath = rootPath;
    });

    unlistenDragOver = await listen<{ position: { x: number; y: number } }>('tauri://drag-over', (event) => {
      if (!isExternalDrag || !rootPath) return;
      const { x, y } = event.payload.position;
      // Hit-test to find which folder we're hovering
      const el = document.elementFromPoint(x, y);
      if (!el) {
        dropTargetPath = rootPath;
        return;
      }
      const treeItem = el.closest('.tree-item') as HTMLElement | null;
      if (treeItem) {
        const path = treeItem.dataset.path;
        const isDir = treeItem.dataset.isdir === 'true';
        if (path) {
          dropTargetPath = isDir ? path : getParentDir(path);
          // Auto-expand collapsed folder
          if (isDir && !expandedDirs.has(path)) {
            if (dragExpandTimer) clearTimeout(dragExpandTimer);
            dragExpandTimer = setTimeout(() => {
              const entry = findEntry(files, path);
              if (entry) toggleDir(entry);
            }, 600);
          }
          return;
        }
      }
      dropTargetPath = rootPath;
    });

    unlistenDragDrop = await listen<{ paths: string[]; position: { x: number; y: number } }>('tauri://drag-drop', async (event) => {
      if (!rootPath) return;
      const destDir = dropTargetPath || rootPath;
      const paths = event.payload.paths;
      if (paths.length > 0) {
        try {
          await invoke('import_external_files', { sources: paths, destDir });
        } catch (e) {
          log.error('Failed to import external files', e);
        }
        await refreshTree();
      }
      isExternalDrag = false;
      externalDropPaths = [];
      dropTargetPath = null;
      if (dragExpandTimer) {
        clearTimeout(dragExpandTimer);
        dragExpandTimer = null;
      }
    });

    unlistenDragLeave = await listen('tauri://drag-leave', () => {
      isExternalDrag = false;
      externalDropPaths = [];
      dropTargetPath = null;
      if (dragExpandTimer) {
        clearTimeout(dragExpandTimer);
        dragExpandTimer = null;
      }
    });
  }

  function teardownExternalDropListeners() {
    unlistenDragEnter?.();
    unlistenDragOver?.();
    unlistenDragDrop?.();
    unlistenDragLeave?.();
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

  let unsubTreeRefresh: (() => void) | null = null;
  let refreshFlash = $state(false);

  onMount(() => {
    window.addEventListener('mouseup', handleGlobalMouseUp);
    window.addEventListener('keydown', handleKeyDown);
    setupExternalDropListeners();

    let first = true;
    unsubTreeRefresh = fileTreeRefreshTrigger.subscribe(() => {
      if (first) { first = false; return; }
      refreshFlash = true;
      setTimeout(() => { refreshFlash = false; }, 1000);
      refreshTree();
    });

    // Expose openFolderByPath to parent
    onOpenFolderProp?.(openFolderByPath);
  });

  onDestroy(() => {
    stopWatching();
    stopGitPolling();
    if (watchDebounce) clearTimeout(watchDebounce);
    if (dragExpandTimer) clearTimeout(dragExpandTimer);
    endDrag();
    teardownExternalDropListeners();
    if (unsubTreeRefresh) unsubTreeRefresh();
    window.removeEventListener('mousemove', handleGlobalMouseMove);
    window.removeEventListener('mouseup', handleGlobalMouseUp);
    window.removeEventListener('keydown', handleKeyDown);
  });
</script>

<div class="file-tree" class:refresh-flash={refreshFlash}>
  {#if !rootPath}
    <div class="no-folder">
      <FolderOpen class="no-folder-icon" />
      <Button variant="default" size="md" class="open-folder-btn" onclick={openFolder}>Open Folder</Button>
      <p>Open a project to begin</p>
    </div>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="tree-content"
      onclick={(e) => {
        if (e.target === e.currentTarget) {
          if (creating) cancelCreate();
          selectedPath = null;
          selectedPaths = new Set();
        }
      }}
      class:drop-target={dropTargetPath === rootPath}
    >
      <!-- Root folder displayed as first tree item -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="tree-item"
        class:selected={selectedPath === rootPath}
        data-path={rootPath}
        onclick={() => { toggleDir({ name: rootPath!.split('/').pop()!, path: rootPath!, is_dir: true, children: files }); selectedPath = rootPath; selectedPaths = new Set([rootPath!]); }}
        oncontextmenu={(e) => handleContextMenu(e, { name: rootPath!.split('/').pop()!, path: rootPath!, is_dir: true, children: files })}
      >
        <span class="chevron" class:expanded={expandedDirs.has(rootPath!)}>
          <ChevronRight size={10} />
        </span>
        {#if expandedDirs.has(rootPath!)}
          <FolderOpen class="icon dir-icon" />
        {:else}
          <Folder class="icon dir-icon" />
        {/if}
        <span class="tree-item-name">{rootPath.split('/').pop()}</span>
      </div>
      {#if expandedDirs.has(rootPath!)}
        {#if creating && createParentPath === rootPath}
          {@render createInputRow(1)}
        {/if}
        {#each files.filter(e => !isHidden(e.name)) as entry}
          {@render fileNode(entry, 1)}
        {/each}
      {/if}
    </div>
  {/if}
</div>

{#if contextMenu}
  <div
    class="context-overlay"
    role="presentation"
    onclick={closeContextMenu}
    onkeydown={(e) => e.key === 'Escape' && closeContextMenu()}
    oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}
  ></div>
  <div class="context-menu" bind:this={contextMenuEl} style="left: {contextMenu.x}px; top: {contextMenu.y}px">
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
      <button class="context-item" onclick={() => {
        const path = contextMenu!.path;
        const isDir = contextMenu!.isDir;
        closeContextMenu();
        // Use the folder itself, or the parent of a file
        const target = isDir ? path : path.substring(0, path.lastIndexOf('/'));
        startCreate('file', target);
      }}>
        New File
      </button>
      <button class="context-item" onclick={() => {
        const path = contextMenu!.path;
        const isDir = contextMenu!.isDir;
        closeContextMenu();
        const target = isDir ? path : path.substring(0, path.lastIndexOf('/'));
        startCreate('folder', target);
      }}>
        New Folder
      </button>
      <div class="context-separator"></div>
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
      <button class="context-item" onclick={() => duplicateEntry(contextMenu!.path)}>
        Duplicate
      </button>
      <div class="context-separator"></div>
      <button class="context-item" onclick={() => startRename(contextMenu!.path)}>
        Rename
      </button>
      <button class="context-item" onclick={() => revealInFileManager(contextMenu!.path)}>
        Reveal in File Manager
      </button>
      <div class="context-separator"></div>
      {#if !contextMenu!.isDir}
        <button class="context-item" onclick={() => openDiagram(contextMenu!.path)}>
          Show Diagram
        </button>
      {/if}
      <button class="context-item" onclick={() => addToGitignore(contextMenu!.path)}>
        Add to .gitignore
      </button>
      <div class="context-separator"></div>
      <button class="context-item danger" onclick={deleteSelected}>
        Delete
      </button>
    {/if}
  </div>
{/if}

{#snippet fileNode(entry: FileEntry, depth: number)}
  <div
    class="tree-item"
    class:is-dir={entry.is_dir}
    class:selected={selectedPath === entry.path || selectedPaths.has(entry.path)}
    class:multi-selected={selectedPaths.has(entry.path)}
    class:dragging={draggedPaths.includes(entry.path)}
    class:drop-target={dropTargetPath === entry.path && entry.is_dir}
    class:drop-target-child={dropTargetPath !== null && getParentDir(entry.path) === dropTargetPath}
    class:git-ignored={isGitIgnored(entry.path)}
    style="padding-left: {8 + depth * 8}px"
    data-path={entry.path}
    data-isdir={entry.is_dir}
    role="treeitem"
    tabindex="0"
    aria-selected={selectedPath === entry.path || selectedPaths.has(entry.path)}
    onclick={(e) => handleFileClick(entry, e)}
    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleFileClick(entry, e as unknown as MouseEvent); } }}
    oncontextmenu={(e) => handleContextMenu(e, entry)}
    onmousedown={(e) => handleDragMouseDown(entry, e)}
  >
    {#if entry.is_dir}
      <span class="chevron" class:expanded={expandedDirs.has(entry.path)}>
        <ChevronRight size={10} />
      </span>
      {#if expandedDirs.has(entry.path)}
        <FolderOpen class="icon dir-icon" />
      {:else}
        <Folder class="icon dir-icon" />
      {/if}
    {:else}
      <span class="file-indent"></span>
      <Icon class="icon file-icon-svg" icon={getFileIconName(entry.name)} />
    {/if}
    {#if renamingPath === entry.path}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:this={renameInput}
        bind:value={renameValue}
        class="rename-input"
        autocapitalize="off"
        autocomplete="off"
        spellcheck="false"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => {
          if (e.key === 'Enter') confirmRename();
          if (e.key === 'Escape') cancelRename();
        }}
        onblur={confirmRename}
      />
    {:else}
      {@const gitColor = getGitStatusColor(entry.path, entry.is_dir)}
      {@const remoteColor = getGitRemoteStatusColor(entry.path, entry.is_dir)}
      {@const nameColor = gitColor || remoteColor}
      <span class="file-name" class:dir-name={entry.is_dir} style={nameColor ? `color: ${nameColor}` : ''}>{entry.name}</span>
      {#if entry.is_symlink}
        <span class="symlink-badge" title="Symbolic link" aria-label="Symbolic link">
          <Link2 size={10} />
        </span>
      {/if}
      {#if !entry.is_dir && gitRemoteFileStatus.has(entry.path)}
        <span class="git-badge remote-badge" style="color: {remoteColor}">↓{gitRemoteFileStatus.get(entry.path)}</span>
      {/if}
      {#if entry.is_dir && gitRemoteFolderStatus.has(entry.path)}
        <span class="git-badge remote-badge" style="color: {remoteColor}">↓{gitRemoteFolderStatus.get(entry.path)}</span>
      {/if}
      {#if !entry.is_dir && gitFileStatus.has(entry.path)}
        <span class="git-badge" style="color: {gitColor}">{gitFileStatus.get(entry.path)}</span>
      {/if}
    {/if}
  </div>

  {#if entry.is_dir && expandedDirs.has(entry.path) && entry.children}
    {#if creating && createParentPath === entry.path}
      {@render createInputRow(depth + 1)}
    {/if}
    {#each entry.children.filter(c => !isHidden(c.name)) as child}
      {@render fileNode(child, depth + 1)}
    {/each}
  {/if}
{/snippet}

{#snippet createInputRow(depth: number)}
  <div class="tree-item create-row" style="padding-left: {8 + depth * 8}px">
    <span class="file-indent"></span>
    {#if creating === 'folder'}
      <Folder class="icon dir-icon" />
    {:else}
      <Icon class="icon file-icon-svg" icon={newName ? getFileIconName(newName) : 'vscode-icons:default-file'} />
    {/if}
    <input
      bind:this={newNameInput}
      bind:value={newName}
      class="create-input"
      autocapitalize="off"
      autocomplete="off"
      spellcheck="false"
      placeholder=""
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => {
        if (e.key === 'Enter') { e.preventDefault(); confirmCreate(); }
        if (e.key === 'Escape') { e.preventDefault(); cancelCreate(); }
      }}
      onblur={handleCreateBlur}
    />
    {#if createError}
      <span class="create-error">{createError}</span>
    {/if}
  </div>
{/snippet}

<style>
  .file-tree {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    overflow-x: hidden;
    padding-bottom: 12px;
  }

  .file-tree.refresh-flash {
    animation: tree-flash 1s ease-out;
  }
  @keyframes tree-flash {
    0% { opacity: 1; }
    40% { opacity: 0.15; }
    100% { opacity: 1; }
  }

  /* Empty state */
  .no-folder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 16px;
    gap: 10px;
    text-align: center;
    color: var(--text-muted);
    font-size: 12px;
  }

  :global(.no-folder-icon) {
    width: 42px;
    height: 42px;
    color: var(--accent);
    opacity: 0.82;
    margin-bottom: 4px;
  }

  :global(.open-folder-btn) {
    letter-spacing: 0.3px;
  }


  /* Tree content */
  .tree-content {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  /* Tree items */
  .tree-item {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: var(--density-tree-padding, 2px 8px);
    width: 100%;
    text-align: left;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    border-radius: 3px;
    margin: 0 3px;
    width: calc(100% - 6px);
    /* Only transition cosmetic properties — `transition: all` was animating
       every state class change (drag, drop-target, selected) causing
       cascading repaints during scrolling/dragging. */
    transition: background-color 0.1s ease, color 0.1s ease;
    cursor: default;
    user-select: none;
  }

  .tree-item.selected {
    font-weight: 600;
    border-left: 2px solid var(--settings-icon, #B34B3C);
    border-radius: 0;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .tree-item:hover {
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-surface) 40%);
    color: var(--text-primary);
  }

  .tree-item.selected:hover {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
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
  :global(.icon) {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  :global(.dir-icon) {
    color: var(--text-secondary);
    opacity: 0.84;
  }

  .file-indent {
    width: 10px;
    flex-shrink: 0;
  }

  :global(.file-icon-svg) {
    opacity: 0.96;
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
    flex-shrink: 0;
    opacity: 0.8;
  }

  .git-badge:first-of-type {
    margin-left: auto;
  }

  .remote-badge {
    margin-right: 2px;
  }

  /* Symbolic-link indicator: a small Link2 icon to the right of the
     filename (before any git badges). Stays visible at every theme. */
  .symlink-badge {
    display: inline-flex;
    align-items: center;
    color: var(--text-muted);
    flex-shrink: 0;
    margin-left: 4px;
    opacity: 0.75;
  }

  /* Create input */
  .create-row {
    position: relative;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .create-row .create-input {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    font-weight: 500;
    padding: 1px 4px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 3px;
    outline: none;
    font-family: inherit;
  }
  .create-error {
    position: absolute;
    top: 100%;
    left: 8px;
    font-size: 10px;
    color: var(--error);
    white-space: nowrap;
    padding: 2px 6px;
    background: var(--bg-secondary);
    border: 1px solid var(--error);
    border-radius: 3px;
    z-index: 10;
  }

  /* Multi-select */
  .tree-item.multi-selected {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  /* Drag-and-drop */
  .tree-item.dragging {
    opacity: 0.4;
  }

  .tree-item.drop-target {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    outline: 1px dashed var(--accent);
    outline-offset: -1px;
    border-radius: 3px;
  }

  .tree-item.drop-target-child {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .tree-item.git-ignored {
    opacity: 0.5;
  }

  .tree-content.drop-target {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  :global(.drag-ghost) {
    position: fixed;
    z-index: 10000;
    pointer-events: none;
    background: var(--bg-secondary, #1e1e2e);
    color: var(--text-primary, #cdd6f4);
    border: 1px solid var(--accent, #89b4fa);
    border-radius: 4px;
    padding: 3px 10px;
    font-size: 11px;
    font-family: inherit;
    white-space: nowrap;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    opacity: 0.92;
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
