<script lang="ts">
  import Tabs from '../tabs/Tabs.svelte';
  import { PanelLeft, SplitSquareVertical, Search, MessageSquareText, GitBranch, Settings2, SidebarOpen, SidebarClose } from 'lucide-svelte';
  import { showTerminal, showSettings, showChat, showGit, gitBranch, toggleChatPanel, toggleGitPanel, activeFilePath, terminalSessions, splitTerminalSignal, collapseTerminalSplitsSignal, terminalPath, openFileSearchSignal } from '../../modules/stores';

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

  function openFileSearch() {
    openFileSearchSignal.update(n => n + 1);
  }
</script>

<svelte:document onclick={handleDocumentClick} />

<div class="toolbar">
  <button
    type="button"
    class="toolbar-btn sidebar-toggle"
    class:active={sidebarVisible}
    onclick={onToggleSidebar}
    title="Toggle Sidebar (Cmd+B)"
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
      title={splitActive ? 'Collapse terminal splits' : 'Split terminal pane'}
      aria-label={splitActive ? 'Collapse splits' : 'Split terminal'}
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
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="toolbar-search" onclick={openFileSearch} onkeydown={(e) => e.key === 'Enter' && openFileSearch()} title="Quick open file (Cmd+O)">
      <Search size={12} />
      <span>Search files...</span>
      <kbd>⌘O</kbd>
    </div>
    <button
      type="button"
      class="toolbar-btn"
      class:active={$showChat}
      onclick={toggleChatPanel}
      title="AI Chat (Ctrl+L)"
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
      title="Source Control (Cmd+G)"
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

  .tabs-wrapper :global(.tabs-bar) {
    border-bottom: none;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 10px;
    flex-shrink: 0;
    height: 100%;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 8px;
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
    color: var(--text-primary);
  }

  .sidebar-toggle {
    padding: 0 10px;
    margin: 0;
    border-radius: 0;
    height: 100%;
    border-right: 1px solid var(--border);
  }

  .split-btn-container {
    position: relative;
    display: flex;
    align-items: center;
    height: 100%;
    border-right: 1px solid var(--border);
  }

  .split-btn {
    padding: 0 10px;
    border-radius: 0;
    height: 100%;
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

  .toolbar-search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.1s, color 0.1s;
    margin-right: 8px;
    min-width: 140px;
  }
  .toolbar-search:hover {
    border-color: var(--text-muted);
    color: var(--text-secondary);
  }
  .toolbar-search kbd {
    font-size: 10px;
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--bg-surface);
    color: var(--text-muted);
    margin-left: auto;
  }
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
