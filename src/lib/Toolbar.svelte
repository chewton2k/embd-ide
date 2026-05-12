<script lang="ts">
  import Tabs from './Tabs.svelte';
  import { PanelLeft, SplitSquareVertical, Search, MessageSquareText, GitBranch, CheckSquare, Settings2, SidebarOpen, SidebarClose } from 'lucide-svelte';
  import { showTerminal, showSettings, autosaveEnabled, showChat, showGit, gitBranch, triggerSearchInFile, toggleChatPanel, toggleGitPanel, activeFilePath, terminalSessions, splitTerminalSignal, collapseTerminalSplitsSignal, terminalPath } from './stores';

  let splitMenuOpen = $state(false);
  let splitMenuPos = $state<{ top: number; left: number } | null>(null);
  let splitBtnEl: HTMLDivElement | undefined = $state();

  let splitActive = $derived($showTerminal && $terminalSessions.length > 1);

  function handleSplitBtn() {
    if (splitActive) {
      collapseTerminalSplitsSignal.update(n => n + 1);
      splitMenuOpen = false;
    } else {
      if (!splitMenuOpen) {
        const rect = splitBtnEl?.getBoundingClientRect();
        if (rect) splitMenuPos = { top: rect.bottom + 2, left: rect.left };
      }
      splitMenuOpen = !splitMenuOpen;
    }
  }

  function activateSplit(dir: 'bottom' | 'right') {
    $showTerminal = true;
    $activeFilePath = terminalPath();
    splitTerminalSignal.update(({ count }) => ({ count: count + 1, direction: dir === 'bottom' ? 'bottom' : 'right' }));
    splitMenuOpen = false;
  }

  function handleDocumentClick(e: MouseEvent) {
    if (splitMenuOpen && splitBtnEl && !splitBtnEl.contains(e.target as Node)) {
      splitMenuOpen = false;
    }
  }

  let { sidebarVisible, onToggleSidebar }: {
    sidebarVisible: boolean;
    onToggleSidebar: () => void;
  } = $props();

  function triggerSearch() {
    triggerSearchInFile.update(n => n + 1);
  }

  const viewerExts = new Set(['png','jpg','jpeg','gif','webp','bmp','ico','svg','pdf','mp4','webm','mov','mp3','wav','ogg','flac']);
  let searchEnabled = $derived.by(() => {
    const path = $activeFilePath;
    if (!path) return false;
    const ext = path.split('.').pop()?.toLowerCase() ?? '';
    return !viewerExts.has(ext);
  });
</script>

<svelte:document onclick={handleDocumentClick} />

<div class="toolbar">
  <button
    type="button"
    class="toolbar-btn sidebar-toggle"
    class:active={sidebarVisible}
    onclick={onToggleSidebar}
    title="Toggle sidebar"
    aria-label="Toggle sidebar"
    aria-pressed={sidebarVisible}
  >
    {#if sidebarVisible}
      <SidebarOpen size={14} />
    {:else}
      <SidebarClose size={14} />
    {/if}
  </button>

  <div class="split-btn-container" bind:this={splitBtnEl}>
    <button
      type="button"
      class="toolbar-btn split-btn"
      class:active={splitActive}
      onclick={handleSplitBtn}
      title={splitActive ? 'Close split' : 'Split terminal'}
      aria-label={splitActive ? 'Close split' : 'Split terminal'}
      aria-pressed={splitActive}
    >
      <SplitSquareVertical size={14} />
    </button>

    {#if splitMenuOpen && splitMenuPos}
      <div class="split-menu" role="menu" style="top: {splitMenuPos.top}px; left: {splitMenuPos.left}px;">
        <button class="split-menu-item" role="menuitem" onclick={() => activateSplit('right')}>
          <SplitSquareVertical size={12} />
          Split Right
        </button>
        <button class="split-menu-item" role="menuitem" onclick={() => activateSplit('bottom')}>
          <PanelLeft size={12} style="transform: rotate(-90deg);" />
          Split Below
        </button>
      </div>
    {/if}
  </div>

  <div class="tabs-wrapper">
    <Tabs />
  </div>

  <div class="toolbar-right">
    <button type="button" class="toolbar-search-btn" class:disabled={!searchEnabled} onclick={triggerSearch} disabled={!searchEnabled} title="Search in file (Cmd/Ctrl+F)" aria-label="Search in file">
      <Search size={13} />
      <span>Search in file</span>
    </button>
    <button
      type="button"
      class="toolbar-btn"
      class:active={$showChat}
      onclick={toggleChatPanel}
      title="Toggle AI chat (Ctrl+L)"
      aria-label="Toggle AI chat"
      aria-pressed={$showChat}
    >
      <MessageSquareText size={14} />
    </button>

    <button
      type="button"
      class="toolbar-btn"
      class:active={$showGit}
      onclick={toggleGitPanel}
      title="Toggle source control (Ctrl+G)"
      aria-label="Toggle source control"
      aria-pressed={$showGit}
    >
      <GitBranch size={14} />
      {#if $gitBranch}
        <span class="branch-label">{$gitBranch}</span>
      {/if}
    </button>

    <button
      type="button"
      class="toolbar-btn autosave-btn"
      class:active={$autosaveEnabled}
      onclick={() => autosaveEnabled.update(v => !v)}
      title="Toggle autosave"
      aria-label="Toggle autosave"
      aria-pressed={$autosaveEnabled}
    >
      <CheckSquare size={14} />
    </button>

    <button
      type="button"
      class="toolbar-btn gear-btn"
      class:active={$showSettings}
      onclick={() => showSettings.update(v => !v)}
      title="Settings"
      aria-label="Settings"
      aria-pressed={$showSettings}
    >
      <Settings2 size={14} />
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    height: var(--density-tabs-height, 36px);
    overflow: hidden;
    flex-shrink: 0;
  }

  .tabs-wrapper {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
  }

  /* Remove the tabs-bar's own bottom border — the toolbar provides it */
  .tabs-wrapper :global(.tabs-bar) {
    border-bottom: none;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 8px;
    flex-shrink: 0;
    height: 100%;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 11px;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }

  .toolbar-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .toolbar-btn.active {
    color: var(--accent);
  }

  .sidebar-toggle {
    padding: 4px 8px;
    margin-left: 4px;
    border-radius: 0;
    height: 100%;
  }

  .split-btn-container {
    position: relative;
    display: flex;
    align-items: center;
    padding: 0 2px;
    border-right: 1px solid var(--border);
    height: 100%;
  }

  .split-btn {
    padding: 4px 6px;
  }

  .split-menu {
    position: fixed;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px;
    z-index: 200;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 130px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .split-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-secondary);
    cursor: pointer;
    text-align: left;
    white-space: nowrap;
  }

  .split-menu-item:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .toolbar-search-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.1s, color 0.1s;
    margin-right: 6px;
  }

  .toolbar-search-btn:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .toolbar-search-btn.disabled {
    opacity: 0.4;
    cursor: not-allowed;
    pointer-events: none;
  }

  .branch-label {
    font-size: 11px;
    max-width: 80px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

</style>
