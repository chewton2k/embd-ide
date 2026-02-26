<script lang="ts">
  import FileTree from './lib/FileTree.svelte';
  import Editor from './lib/Editor.svelte';
  import FileViewer from './lib/FileViewer.svelte';
  import JSONViewer from './lib/JSONViewer.svelte';
  import MergeEditor from './lib/MergeEditor.svelte';
  import Tabs from './lib/Tabs.svelte';
  import Terminal from './lib/Terminal.svelte';
  import ChatPanel from './lib/ChatPanel.svelte';
  import GitPanel from './lib/GitPanel.svelte';
  import Settings from './lib/Settings.svelte';
  import FileSearch from './lib/FileSearch.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getVersion } from '@tauri-apps/api/app';
  import { openFiles, activeFile, activeFileModified, addFile, autosaveEnabled, projectRoot, gitBranch, showSettings, showTerminal, currentThemeId, getTheme, uiFontSize, uiDensity, apiKey, sharedGitStatus, nextTab, prevTab } from './lib/stores.ts';
  import { onMount } from 'svelte';

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

  let showChat = $state(false);
  let showGit = $state(false);
  let showFileSearch = $state(false);
  let sidebarWidth = $state(220);
  let terminalHeight = $state(220);
  let chatWidth = $state(320);
  let gitWidth = $state(360);

  // --- Drag resize logic ---
  let dragging = $state<'sidebar' | 'terminal' | 'chat' | 'git' | null>(null);

  function startDrag(target: 'sidebar' | 'terminal' | 'chat' | 'git') {
    return (e: MouseEvent) => {
      e.preventDefault();
      dragging = target;
      document.body.style.cursor = target === 'terminal' ? 'row-resize' : 'col-resize';
      document.body.style.userSelect = 'none';
      window.addEventListener('mousemove', onDrag);
      window.addEventListener('mouseup', stopDrag);
    };
  }

  function onDrag(e: MouseEvent) {
    if (dragging === 'sidebar') {
      sidebarWidth = Math.max(140, Math.min(500, e.clientX));
    } else if (dragging === 'terminal') {
      const sbHeight = parseInt(getComputedStyle(document.documentElement).getPropertyValue('--density-statusbar-height') || '24');
      const windowH = window.innerHeight - sbHeight;
      terminalHeight = Math.max(100, Math.min(windowH - 150, windowH - e.clientY));
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
    $showTerminal = !$showTerminal;
  }

  function toggleChat() {
    showChat = !showChat;
    if (showChat) showGit = false;
  }

  function toggleGit() {
    showGit = !showGit;
    if (showGit) showChat = false;
  }

  let appVersion = $state('');

  onMount(() => {
    getVersion().then(v => appVersion = v);
    // Sync stored API key to backend on startup
    const storedKey = localStorage.getItem('embd-api-key');
    if (storedKey) {
      invoke('set_api_key', { key: storedKey }).catch(() => {});
    }
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

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === '`') {
      e.preventDefault();
      toggleTerminal();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'l') {
      e.preventDefault();
      toggleChat();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'g') {
      e.preventDefault();
      toggleGit();
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
  <div class="ide-top">
    <div class="sidebar" style="width: {sidebarWidth}px">
      <FileTree onFileSelect={(path, name) => addFile(path, name)} onSearchFiles={() => showFileSearch = true} />
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="resize-handle resize-handle-col" onmousedown={startDrag('sidebar')}></div>

    <div class="main-area">
      <div class="editor-area" style="flex: 1; min-height: 0;">
        <Tabs />
        <div class="editor-container">
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
              <img src="/embd_logo.png" alt="embd" class="welcome-logo" />
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
        </div>
      </div>

      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle resize-handle-row" class:hidden={!$showTerminal} onmousedown={startDrag('terminal')}></div>
      <div class="bottom-panel" class:hidden={!$showTerminal} style="height: {terminalHeight}px;">
        <Terminal />
      </div>
    </div>

    {#if showChat}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle resize-handle-col" onmousedown={startDrag('chat')}></div>
      <div class="chat-panel" style="width: {chatWidth}px">
        <div class="panel-header">
          <span>AI Chat</span>
          <button onclick={toggleChat}>✕</button>
        </div>
        <ChatPanel />
      </div>
    {/if}

    {#if showGit}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle resize-handle-col" onmousedown={startDrag('git')}></div>
      <div class="git-panel-container" style="width: {gitWidth}px">
        <div class="panel-header">
          <span>Source Control</span>
          <button onclick={toggleGit}>✕</button>
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
      <button onclick={() => showSettings.update(v => !v)} class="statusbar-btn gear-btn" title="Settings">
        <svg viewBox="0 0 16 14" fill="currentColor" width="13" height="13">
          <path d="M8 1l1.3.8.8-.5 1 1-.5.8.5 1H12.5v1.4l-.8.5.2 1 .9.5-.3 1.2-1 .1-.3 1 .6.8-.7 1.1-1-.3-.7.8.1 1L8 13l-1.3-.8-.8.5-1-1 .5-.8-.5-1H3.5V8.5l.8-.5-.2-1-.9-.5.3-1.2 1-.1.3-1-.6-.8.7-1.1 1 .3.7-.8L6.5 2 8 1zm0 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z"/>
        </svg>
      </button>
      {#if $gitBranch}
        <button class="statusbar-btn statusbar-branch" onclick={toggleGit}>
          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
            <path d="M14.7 7.3L8.7 1.3a1 1 0 0 0-1.4 0L5.7 2.9l1.8 1.8A1.2 1.2 0 0 1 9 5.9v4.3a1.2 1.2 0 1 1-1-.1V6.1L6.3 7.8a1.2 1.2 0 1 1-.9-.5l1.8-1.8-1.8-1.8L1.3 7.3a1 1 0 0 0 0 1.4l6 6a1 1 0 0 0 1.4 0l6-6a1 1 0 0 0 0-1.4z"/>
          </svg>
          {$gitBranch}
        </button>
      {/if}
      <button onclick={toggleTerminal} class="statusbar-btn">
        {$showTerminal ? 'Hide' : 'Show'} Terminal 
      </button>
      <button onclick={toggleChat} class="statusbar-btn">
        | {showChat ? 'Hide' : 'Show'} AI 
      </button>
      <button onclick={toggleGit} class="statusbar-btn">
        | {showGit ? 'Hide' : 'Show'} Git 
      </button>
      <button onclick={() => autosaveEnabled.update(v => !v)} class="statusbar-btn autosave-btn">
        | {$autosaveEnabled ? 'ON Autosave' : 'OFF Autosave'} |
      </button>
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
      <span>embd v{appVersion}</span>
    </div>
  </div>
</div>

<style>
  .ide-layout {
    display: grid;
    grid-template-rows: 1fr var(--density-statusbar-height, 24px);
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

  .welcome-logo {
    height: 75px;
    width: auto;
    object-fit: contain;
    opacity: 0.7;
    border-radius: 18px;
  }

  .shortcuts {
    margin-top: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12px;
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

  .resize-handle-row {
    height: 3px;
    cursor: row-resize;
    width: 100%;
  }

  .bottom-panel {
    background: var(--bg-tertiary);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
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

  .statusbar-btn {
    color: var(--bg-tertiary);
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .statusbar-btn:hover {
    opacity: 0.8;
  }

  .autosave-btn {
    font-size: 11px;
    opacity: 0.9;
  }

  .gear-btn {
    display: flex;
    align-items: center;
    padding: 0 4px;
  }

  .statusbar-branch {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 500;
    padding-right: 8px;
    border-right: 1px solid color-mix(in srgb, var(--bg-tertiary) 40%, transparent);
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
</style>
