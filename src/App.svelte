<script lang="ts">
  import FileTree from './lib/FileTree.svelte';
  import Editor from './lib/Editor.svelte';
  import FileViewer from './lib/FileViewer.svelte';
  import JSONViewer from './lib/JSONViewer.svelte';
  import MergeEditor from './lib/MergeEditor.svelte';
  import Toolbar from './lib/Toolbar.svelte';
  import Terminal from './lib/Terminal.svelte';
  import ChatPanel from './lib/ChatPanel.svelte';
  import GitPanel from './lib/GitPanel.svelte';
  import Settings from './lib/Settings.svelte';
  import FileSearch from './lib/FileSearch.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { exists } from '@tauri-apps/plugin-fs';
  import { openFiles, activeFile, activeFilePath, activeFileModified, addFile, autosaveEnabled, projectRoot, gitBranch, showSettings, showTerminal, isTerminalPath, terminalSessions, createTerminalSignal, currentThemeId, getTheme, uiFontSize, uiDensity, apiKey, sharedGitStatus, nextTab, prevTab, showChat, showGit, toggleChatPanel, toggleGitPanel, fileTreeNavTarget, terminalPath, openFileSearchSignal } from './lib/stores';
  import { getRecentProjects, removeRecentProject, scheduleSaveSession, saveSessionNow, type RecentProject } from './lib/session';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';

  const viewerExts = new Set([
    'png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'ico', 'svg',
    'pdf', 'mp4', 'webm', 'mov', 'mp3', 'wav', 'ogg', 'flac',
  ]);

  function isViewerFile(path: string): boolean {
    const ext = path.split('.').pop()?.toLowerCase() || '';
    return viewerExts.has(ext);
  }

  function isJsonFile(path: string): boolean {
    return path.toLowerCase().endsWith('.json');
  }

  let recentProjects = $state<RecentProject[]>([]);
  let showAllRecent = $state(false);
  let openFolderByPath: ((path: string, restoreSession?: boolean) => Promise<void>) | null = null;

  function handleOpenFolder(fn: (path: string, restoreSession?: boolean) => Promise<void>) {
    openFolderByPath = fn;
  }

  async function openRecentProject(project: RecentProject) {
    if (!openFolderByPath) return;
    const folderExists = await exists(project.path);
    if (!folderExists) {
      await removeRecentProject(project.path);
      recentProjects = recentProjects.filter(p => p.path !== project.path);
      alert(`Project folder no longer exists:\n${project.path}`);
      return;
    }
    // Session restoration is handled inside openFolderByPath
    await openFolderByPath(project.path);
  }

  let showFileSearch = $state(false);
  let sidebarWidth = $state(220);
  let sidebarVisible = $state(true);

  function toggleSidebar() {
    sidebarVisible = !sidebarVisible;
  }

  let chatWidth = $state(320);
  let gitWidth = $state(360);

  // --- Drag resize logic ---
  let dragging = $state<'sidebar' | 'chat' | 'git' | null>(null);

  function startDrag(target: 'sidebar' | 'chat' | 'git') {
    return (e: MouseEvent) => {
      e.preventDefault();
      dragging = target;
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
      window.addEventListener('mousemove', onDrag);
      window.addEventListener('mouseup', stopDrag);
    };
  }

  function onDrag(e: MouseEvent) {
    if (dragging === 'sidebar') {
      sidebarWidth = Math.max(140, Math.min(500, e.clientX));
    } else if (dragging === 'chat') {
      chatWidth = Math.max(200, Math.min(600, window.innerWidth - e.clientX));
    } else if (dragging === 'git') {
      gitWidth = Math.max(260, Math.min(600, window.innerWidth - e.clientX));
    }
  }

  function stopDrag() {
    dragging = null;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    window.removeEventListener('mousemove', onDrag);
    window.removeEventListener('mouseup', stopDrag);
  }

  function toggleTerminal() {
    const sessions = get(terminalSessions);
    if (!$showTerminal || sessions.length === 0) {
      $showTerminal = true;
      createTerminalSignal.update(n => n + 1);
    } else if (!isTerminalPath($activeFilePath)) {
      activeFilePath.set(terminalPath());
    }
  }

  let isClosing = false;

  let breadcrumbSegments = $derived.by(() => {
    const path = $activeFilePath;
    const root = $projectRoot;
    if (!path) return [];
    const normPath = path.replace(/\\/g, '/');
    const normRoot = root ? root.replace(/\\/g, '/') : null;
    if (normRoot && normPath.startsWith(normRoot + '/')) {
      const rel = normPath.slice(normRoot.length + 1);
      const relParts = rel.split('/');
      const rootName = normRoot.split('/').pop() || normRoot;
      return [
        { name: rootName, path: normRoot },
        ...relParts.map((part, i) => ({
          name: part,
          path: normRoot + '/' + relParts.slice(0, i + 1).join('/')
        }))
      ];
    }
    return [{ name: normPath.split('/').pop() || path, path: normPath }];
  });

  function navigateBreadcrumb(path: string) {
    if (!sidebarVisible) toggleSidebar();
    fileTreeNavTarget.set(path);
  }

  onMount(async () => {
    // Sync stored API key to backend on startup
    const storedKey = localStorage.getItem('embd-api-key');
    if (storedKey) {
      invoke('set_api_key', { key: storedKey }).catch(() => {});
    }
    // Load recent projects
    try {
      recentProjects = await getRecentProjects();
    } catch { /* ignore */ }
    // Save session on window close — await the save before destroying
    const appWindow = getCurrentWindow();
    await appWindow.onCloseRequested(async (event) => {
      if (isClosing) return;
      isClosing = true;
      event.preventDefault();
      const root = get(projectRoot);
      if (root) {
        try {
          await Promise.race([
            saveSessionNow(root),
            new Promise((_, reject) =>
              setTimeout(() => reject(new Error('Save timeout')), 5000)
            ),
          ]);
        } catch (e) {
          console.error('Failed to save session on close:', e);
        }
      }
      await appWindow.destroy();
    });
  });

  // Apply theme colors to CSS custom properties
  $effect(() => {
    const theme = getTheme($currentThemeId);
    const c = theme.colors;
    const root = document.documentElement;
    root.style.setProperty('--bg-primary', c.bgPrimary);
    root.style.setProperty('--bg-secondary', c.bgSecondary);
    root.style.setProperty('--bg-tertiary', c.bgTertiary);
    root.style.setProperty('--bg-surface', c.bgSurface);
    root.style.setProperty('--text-primary', c.textPrimary);
    root.style.setProperty('--text-secondary', c.textSecondary);
    root.style.setProperty('--text-muted', c.textMuted);
    root.style.setProperty('--accent', c.accent);
    root.style.setProperty('--accent-hover', c.accentHover);
    root.style.setProperty('--border', c.border);
    root.style.setProperty('--success', c.success);
    root.style.setProperty('--warning', c.warning);
    root.style.setProperty('--error', c.error);
    root.style.setProperty('--git-graph-accent', c.gitGraphAccent || c.accent);
    root.style.setProperty('--diff-add', c.diffAdd || c.success);
    root.style.setProperty('--diff-del', c.diffDel || c.error);
    root.style.setProperty('--git-notification', c.gitNotification || c.success);
    root.style.setProperty('--tab-active', c.bgPrimary);
    root.style.setProperty('--tab-inactive', c.bgSecondary);
  });

  // Apply UI font size
  $effect(() => {
    document.documentElement.style.fontSize = $uiFontSize + 'px';
  });

  // Apply UI density
  $effect(() => {
    document.documentElement.dataset.density = $uiDensity;
  });

  // Auto-save session when open files or active file changes
  $effect(() => {
    // Subscribe to reactive stores
    const _ = $openFiles;
    const __ = $activeFile;
    scheduleSaveSession();
  });

  // Refresh recent projects when returning to the welcome screen
  $effect(() => {
    if ($activeFile === null) {
      showAllRecent = false;
      getRecentProjects().then(p => recentProjects = p).catch(() => {});
    }
  });

  $effect(() => {
    if ($openFileSearchSignal > 0) {
      showFileSearch = true;
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === '`') {
      e.preventDefault();
      toggleTerminal();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'l') {
      e.preventDefault();
      toggleChatPanel();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'g') {
      e.preventDefault();
      toggleGitPanel();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'o') {
      e.preventDefault();
      showFileSearch = !showFileSearch;
    }
    // Tab navigation: Ctrl/Cmd+Shift+] or Ctrl/Cmd+Tab → next tab
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === ']') {
      e.preventDefault();
      nextTab();
    }
    // Tab navigation: Ctrl/Cmd+Shift+[ or Ctrl/Cmd+Shift+Tab → prev tab
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === '[') {
      e.preventDefault();
      prevTab();
    }
    // Ctrl/Cmd+Tab → next tab, Ctrl/Cmd+Shift+Tab → prev tab
    if ((e.metaKey || e.ctrlKey) && e.key === 'Tab') {
      e.preventDefault();
      if (e.shiftKey) prevTab();
      else nextTab();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="ide-layout">
  <Toolbar {sidebarVisible} onToggleSidebar={toggleSidebar} />
  <div class="ide-top">
    <div class="sidebar" class:hidden={!sidebarVisible} style="width: {sidebarWidth}px">
      <FileTree onFileSelect={(path, name) => addFile(path, name)} onSearchFiles={() => showFileSearch = true} onOpenFolder={handleOpenFolder} />
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="resize-handle resize-handle-col" class:hidden={!sidebarVisible} onmousedown={startDrag('sidebar')}></div>

    <div class="main-area">
      <div class="editor-col">
        <div class="editor-area" style="flex: 1; min-height: 0;">
          <div class="editor-container">
            <!-- Terminal tab slot: always rendered (visibility:hidden when unfocused) to keep PTY sessions alive -->
            {#if $showTerminal}
              <div class="terminal-tab-slot" class:focused={$showTerminal && isTerminalPath($activeFilePath)}>
                <Terminal />
              </div>
            {/if}
            <!-- File editor — hidden while a terminal tab is focused -->
            {#if !($showTerminal && isTerminalPath($activeFilePath))}
              {#if $activeFile && $sharedGitStatus[$activeFile] === 'C'}
                <MergeEditor filePath={$activeFile} />
              {:else if $activeFile && isJsonFile($activeFile)}
                <JSONViewer filePath={$activeFile} />
              {:else if $activeFile && isViewerFile($activeFile)}
                <FileViewer filePath={$activeFile} />
              {:else if $activeFile}
                <Editor filePath={$activeFile} />
              {:else}
                <div class="welcome">
                  {#if recentProjects.length > 0}
                    <div class="recent-projects">
                      <p style="font-size: 12px; margin-bottom: 8px; color: var(--text-secondary);">Recent Projects</p>
                      {#each (showAllRecent ? recentProjects : recentProjects.slice(0, 3)) as project}
                        <button class="recent-item" onclick={() => openRecentProject(project)}>
                          <span class="recent-name">{project.name}</span>
                          <span class="recent-path">{project.path}</span>
                        </button>
                      {/each}
                      {#if recentProjects.length > 3}
                        <button class="show-more-btn" onclick={() => showAllRecent = !showAllRecent}>
                          {showAllRecent ? 'Show less' : `Show more (${recentProjects.length - 3})`}
                        </button>
                      {/if}
                    </div>
                  {/if}
                  <p>Open a file from the sidebar to start editing</p>
                  <div class="shortcuts">
                    <div><kbd>Ctrl</kbd> + <kbd>`</kbd> Terminal</div>
                    <div><kbd>Ctrl</kbd> + <kbd>L</kbd> AI Chat</div>
                    <div><kbd>Cmd</kbd> + <kbd>O</kbd> Search Files</div>
                    <div><kbd>Cmd</kbd> + <kbd>F</kbd> Search Within Files</div>
                    <div><kbd>Cmd</kbd> + <kbd>G</kbd> Source Control</div>
                    <div><kbd>Ctrl</kbd> + <kbd>Tab</kbd> Next Tab</div>
                    <div><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Tab</kbd> Prev Tab</div>
                  </div>
                </div>
              {/if}
            {/if}
          </div>
        </div>

      </div>
    </div>

    {#if $showChat}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle resize-handle-col" onmousedown={startDrag('chat')}></div>
      <div class="chat-panel" style="width: {chatWidth}px">
        <div class="panel-header">
          <span>AI Chat</span>
          <button onclick={toggleChatPanel}>✕</button>
        </div>
        <ChatPanel />
      </div>
    {/if}

    {#if $showGit}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle resize-handle-col" onmousedown={startDrag('git')}></div>
      <div class="git-panel-container" style="width: {gitWidth}px">
        <div class="panel-header">
          <span>Source Control</span>
          <button onclick={toggleGitPanel}>✕</button>
        </div>
        <GitPanel />
      </div>
    {/if}

    {#if $showSettings}
      <Settings />
    {/if}

    {#if showFileSearch}
      <FileSearch onClose={() => showFileSearch = false} />
    {/if}
  </div>

  <div class="statusbar">
    <div class="statusbar-left">
      {#if breadcrumbSegments.length > 0}
        <div class="breadcrumb">
          {#each breadcrumbSegments as seg, i}
            <span class="breadcrumb-seg" role="button" tabindex="0" onclick={() => navigateBreadcrumb(seg.path)} onkeydown={(e) => e.key === 'Enter' && navigateBreadcrumb(seg.path)}>{seg.name}</span>
            {#if i < breadcrumbSegments.length - 1}
              <span class="breadcrumb-sep">›</span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
    <div class="statusbar-right">
      {#if $activeFile}
        <span class="save-indicator" class:saved={!$activeFileModified} class:unsaved={$activeFileModified}>
          {#if $activeFileModified}
            <svg viewBox="0 0 16 16" fill="currentColor" width="11" height="11">
              <circle cx="8" cy="8" r="5" />
            </svg>
            Unsaved
          {:else}
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11">
              <path d="M3 8.5l3.5 3.5 6.5-7" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            Saved
          {/if}
        </span>
      {/if}
    </div>
  </div>
</div>

<style>
  .ide-layout {
    display: grid;
    grid-template-rows: var(--density-tabs-height, 36px) 1fr var(--density-statusbar-height, 24px);
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .ide-top {
    display: flex;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }

  .sidebar {
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    flex-shrink: 0;
    min-width: 100px;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: row;
    min-width: 0;
    min-height: 0;
  }

  .editor-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .editor-area {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .editor-container {
    flex: 1;
    overflow: hidden;
    position: relative;
  }

  .terminal-tab-slot {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    visibility: hidden;
    pointer-events: none;
    z-index: 1;
  }

  .terminal-tab-slot.focused {
    visibility: visible;
    pointer-events: auto;
  }

  .welcome {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    gap: 12px;
  }

  .shortcuts {
    margin-top: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12px;
  }

  .recent-projects {
    display: flex;
    flex-direction: column;
    width: 340px;
    gap: 4px;
    margin-bottom: 12px;
    max-height: 240px;
    overflow-y: auto;
  }

  .recent-item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 12px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s;
  }

  .recent-item:hover {
    border-color: var(--accent);
  }

  .recent-name {
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 600;
  }

  .recent-path {
    color: var(--text-muted);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }

  .show-more-btn {
    font-size: 12px;
    color: var(--text-muted);
    padding: 6px 12px;
    border-radius: 6px;
    cursor: pointer;
    transition: color 0.15s;
  }

  .show-more-btn:hover {
    color: var(--accent);
  }

  .shortcuts kbd {
    background: var(--bg-surface);
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 11px;
  }

  /* Resize handles */
  .resize-handle {
    flex-shrink: 0;
    background: transparent;
    transition: background 0.15s;
    z-index: 10;
  }

  .resize-handle:hover,
  .resize-handle:active {
    background: var(--accent);
  }

  .resize-handle-col {
    width: 3px;
    cursor: col-resize;
  }

  .hidden {
    display: none !important;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 12px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    color: var(--text-secondary);
    flex-shrink: 0;
  }

  .panel-header button {
    font-size: 14px;
    color: var(--text-muted);
    padding: 2px 6px;
    border-radius: 3px;
  }

  .panel-header button:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .chat-panel {
    background: var(--bg-secondary);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex-shrink: 0;
    min-width: 150px;
  }

  .git-panel-container {
    background: var(--bg-secondary);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex-shrink: 0;
    min-width: 200px;
  }

  .statusbar {
    background: var(--accent);
    color: var(--bg-tertiary);
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 12px;
    font-size: 12px;
    font-weight: 500;
    min-width: 0;
    overflow: hidden;
    flex-shrink: 0;
  }

  .statusbar-left, .statusbar-right {
    display: flex;
    gap: 12px;
    align-items: center;
    min-width: 0;
    flex-shrink: 1;
    overflow: hidden;
  }

  .statusbar-left {
    flex: 1;
  }

  .statusbar-right {
    flex-shrink: 0;
  }

  .save-indicator {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 500;
    padding: 1px 6px;
    border-radius: 3px;
    transition: all 0.2s ease;
  }

  .save-indicator.saved {
    opacity: 0.85;
  }

  .save-indicator.unsaved {
    opacity: 1;
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    min-width: 0;
    overflow: hidden;
  }

  .breadcrumb-seg {
    background: color-mix(in srgb, currentColor 15%, transparent);
    padding: 1px 7px;
    border-radius: 10px;
    white-space: nowrap;
    font-size: 11px;
    font-weight: 500;
    max-width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
    color: inherit;
    cursor: pointer;
    transition: background 0.1s;
  }

  .breadcrumb-seg:hover {
    background: color-mix(in srgb, currentColor 30%, transparent);
  }

  .breadcrumb-sep {
    opacity: 0.55;
    font-size: 10px;
    flex-shrink: 0;
    color: inherit;
  }
</style>
