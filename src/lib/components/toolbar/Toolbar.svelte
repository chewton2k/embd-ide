<script lang="ts">
  /**
   * Tab bar — sits below the TitleBar. Hosts the open-file / terminal /
   * preview / diagram tabs (delegated to <Tabs />), plus a contextual
   * split / collapse control for the active terminal tab.
   *
   * This bar is intentionally NOT draggable — the user moves the window
   * from the title bar above. Global chrome (sidebar / git / settings /
   * search) lives there too.
   */
  import Tabs from '../tabs/Tabs.svelte';
  import { SplitSquareVertical, PanelLeft } from 'lucide-svelte';
  import {
    showTerminal, activeFilePath, panesInActiveTab,
    activeTerminalTabId, splitTerminalSignal, collapseTerminalSplitsSignal,
    terminalPath,
  } from '../../modules/stores';

  let splitMenuOpen = $state(false);
  let splitMenuPos = $state<{ top: number; left: number } | null>(null);
  let splitBtnEl: HTMLDivElement | undefined = $state();

  /**
   * The split button toggles between two intents:
   *  - When the active terminal tab has multiple panes, clicking
   *    collapses them down to the active one ("collapse splits").
   *  - Otherwise it opens a small menu offering Split-Right / Split-Down.
   */
  let splitActive = $derived($showTerminal && $panesInActiveTab > 1);

  function handleSplitBtn() {
    const canCollapse = $showTerminal && $panesInActiveTab > 1;
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
    const tabId = $activeTerminalTabId;
    if (tabId != null) {
      $activeFilePath = terminalPath(tabId);
    }
    splitTerminalSignal.update(({ count }) => ({
      count: count + 1,
      direction: dir === 'bottom' ? 'bottom' : 'right',
    }));
    splitMenuOpen = false;
  }

  function handleDocumentClick(e: MouseEvent) {
    if (splitMenuOpen && splitBtnEl && !splitBtnEl.contains(e.target as Node)) {
      splitMenuOpen = false;
    }
  }
</script>

<svelte:document onclick={handleDocumentClick} />

<div class="toolbar">
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
      <div
        class="split-menu"
        role="menu"
        style="top: {splitMenuPos.top}px; left: {splitMenuPos.left}px;"
      >
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
</style>
