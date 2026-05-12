<script lang="ts">
  import Tabs from '../tabs/Tabs.svelte';
  import { PanelLeft, SplitSquareVertical, Search, GitBranch, Settings2, SidebarOpen, SidebarClose } from 'lucide-svelte';
  import { showTerminal, showSettings, showGit, gitBranch, toggleGitPanel, activeFilePath, terminalSessions, splitTerminalSignal, collapseTerminalSplitsSignal, terminalPath, openFileSearchSignal } from '../../modules/stores';

  let splitMenuOpen = $state(false);
  let splitMenuPos = $state<{ top: number; left: number } | null>(null);
  let splitBtnEl: HTMLDivElement | undefined = $state();

  // Collapse is only available when there is more than one terminal pane open
  // in the terminal view; otherwise the button acts as a "split" trigger.
  let splitActive = $derived($showTerminal && $terminalSessions.length > 1);

  function handleSplitBtn() {
    // Re-check the session count at click time — `splitActive` is derived and
    // could briefly be stale during rapid pane open/close transitions.
    const canCollapse = $showTerminal && $terminalSessions.length > 1;
    if (canCollapse) {
      collapseTerminalSplitsSignal.update(n => n + 1);
      splitMenuOpen = false;
      return;
    }
    if (!splitMenuOpen) {
      const rect = splitBtnEl?.getBoundingClientRect();
      if (rect) splitMenuPos = { top: rect.bottom + 2, left: rect.left };
    }
    splitMenuOpen = !splitMenuOpen;
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
      class:active={$showGit}
      onclick={toggleGitPanel}
      title="Source Control (Cmd+G)"
      aria-label="Toggle source control"
      aria-pressed={$showGit}
    >
      <GitBranch size={15} />
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
      <Settings2 size={15} />
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
    gap: 2px;
    padding: 0 12px;
    flex-shrink: 0;
    height: 100%;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 6px 10px;
    border-radius: 6px;
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
    gap: 8px;
    padding: 5px 12px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 20px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.1s, color 0.1s;
    margin-right: 10px;
    min-width: 150px;
  }
  .toolbar-search:hover {
    border-color: var(--text-muted);
    color: var(--text-secondary);
  }
  .toolbar-search kbd {
    font-size: 11px;
    padding: 3px 7px;
    border-radius: 5px;
    background: var(--bg-surface);
    color: var(--text-primary);
    margin-left: auto;
    font-family: var(--font-ui);
    font-weight: 700;
    letter-spacing: 0.5px;
  }

  .branch-label {
    font-size: 11px;
    max-width: 80px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

</style>
