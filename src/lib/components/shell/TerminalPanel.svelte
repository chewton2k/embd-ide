<script lang="ts">
  /**
   * Docked bottom terminal panel (VSCode / Xcode / Zed style).
   *
   * Responsibilities:
   *  - Height control: drag the top resize handle (or arrow keys when the
   *    handle is focused) to resize; value persists via `terminalPanelHeight`.
   *  - Navigation: a built-in tab strip for the existing `terminalTabs` store.
   *    Switching tabs just moves `activeTerminalTabId` — it does NOT touch
   *    `activeFilePath` (the editor stays visible behind the panel).
   *  - Actions: new tab, split right / split bottom, collapse splits, close
   *    panel. All actions reuse the existing signals so there's no duplicate
   *    logic between this panel and the toolbar.
   *  - The actual terminal rendering is delegated to <Terminal /> unchanged;
   *    this component is purely a container.
   */
  import { TerminalSquare, Plus, X, SplitSquareVertical, SplitSquareHorizontal, Columns2, Trash2 } from 'lucide-svelte';
  import Terminal from './Terminal.svelte';
  import {
    showTerminal,
    terminalPanelHeight,
    terminalTabs,
    activeTerminalTabId,
    panesInActiveTab,
    createTerminalSignal,
    splitTerminalSignal,
    collapseTerminalSplitsSignal,
    killTerminalSignal,
  } from '../../modules/stores';

  // ── Resize: drag handle ──────────────────────────────────────────

  const MIN_HEIGHT = 120;
  /**
   * Minimum vertical breathing room for the editor above the panel.
   * The actual chrome rows (title bar + tab bar + status bar ≈ 92px)
   * also live in the viewport, so on tight windows we add a buffer
   * for editable content. Tuned so even on a 600px-tall window the
   * editor still gets a usable row.
   */
  const EDITOR_MIN = 220;

  function maxHeight(): number {
    // Clamp to 80vh as a safety net; additionally leave editor headroom.
    return Math.max(MIN_HEIGHT, Math.min(window.innerHeight * 0.8, window.innerHeight - EDITOR_MIN));
  }

  function clampHeight(h: number): number {
    return Math.max(MIN_HEIGHT, Math.min(maxHeight(), h));
  }

  let dragging = $state(false);

  function onResizeMouseDown(e: MouseEvent) {
    e.preventDefault();
    dragging = true;
    const startY = e.clientY;
    const startH = $terminalPanelHeight;
    document.body.style.cursor = 'row-resize';
    document.body.style.userSelect = 'none';

    const onMove = (ev: MouseEvent) => {
      // Dragging UP increases panel height; DOWN decreases.
      const delta = startY - ev.clientY;
      terminalPanelHeight.set(clampHeight(startH + delta));
    };
    const onUp = () => {
      dragging = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function onResizeKey(e: KeyboardEvent) {
    // Keyboard-driven resize: Arrow Up/Down to nudge, Home/End for extremes.
    const step = e.shiftKey ? 40 : 10;
    if (e.key === 'ArrowUp')       { e.preventDefault(); terminalPanelHeight.update(h => clampHeight(h + step)); }
    else if (e.key === 'ArrowDown'){ e.preventDefault(); terminalPanelHeight.update(h => clampHeight(h - step)); }
    else if (e.key === 'Home')     { e.preventDefault(); terminalPanelHeight.set(maxHeight()); }
    else if (e.key === 'End')      { e.preventDefault(); terminalPanelHeight.set(MIN_HEIGHT); }
  }

  // If the window shrinks below our panel, clamp. Without this the panel
  // can become taller than the window and occlude the editor entirely.
  function onWindowResize() {
    terminalPanelHeight.update(h => clampHeight(h));
  }

  // ── Tab actions ───────────────────────────────────────────────────

  function addTab() {
    // Force a brand-new tab (matches the `+` in the top Tabs bar's menu).
    $showTerminal = true;
    createTerminalSignal.update(s => ({ count: s.count + 1, forceNew: true }));
  }

  function closeTab(tabId: number, e: MouseEvent) {
    e.stopPropagation();
    killTerminalSignal.set({ kind: 'tab', id: tabId });
  }

  function selectTab(tabId: number) {
    activeTerminalTabId.set(tabId);
  }

  function splitRight() {
    splitTerminalSignal.update(({ count }) => ({ count: count + 1, direction: 'right' }));
  }
  function splitBottom() {
    splitTerminalSignal.update(({ count }) => ({ count: count + 1, direction: 'bottom' }));
  }
  function collapseSplits() {
    collapseTerminalSplitsSignal.update(n => n + 1);
  }
  function closePanel() {
    $showTerminal = false;
  }

  // ── A11y: keyboard navigation for the tab strip ───────────────────
  function onTabKey(e: KeyboardEvent, tabId: number) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      selectTab(tabId);
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault();
      const tabs = $terminalTabs;
      const idx = tabs.findIndex(t => t.id === tabId);
      if (idx === -1) return;
      const next = e.key === 'ArrowRight' ? (idx + 1) % tabs.length : (idx - 1 + tabs.length) % tabs.length;
      selectTab(tabs[next].id);
    }
  }

  // Derived: the active tab's label (for screen readers / tooltips).
  const activeTab = $derived($terminalTabs.find(t => t.id === $activeTerminalTabId) ?? null);
  const canCollapse = $derived($panesInActiveTab > 1);
</script>

<svelte:window on:resize={onWindowResize} />

<section
  class="terminal-panel"
  class:dragging
  class:hidden={!$showTerminal}
  style="height: {$terminalPanelHeight}px;"
  aria-label="Terminal panel"
>
  <!--
    Window-splitter pattern (https://www.w3.org/WAI/ARIA/apg/patterns/windowsplitter/).
    role="separator" + aria-valuenow + tabindex is the correct primitive,
    but Svelte's a11y lint treats role=separator as non-interactive.
  -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="resize-handle"
    role="separator"
    aria-orientation="horizontal"
    aria-label="Resize terminal panel"
    aria-valuenow={$terminalPanelHeight}
    aria-valuemin={MIN_HEIGHT}
    aria-valuemax={maxHeight()}
    tabindex="0"
    onmousedown={onResizeMouseDown}
    onkeydown={onResizeKey}
  ></div>

  <header class="panel-header">
    <div class="header-label">
      <TerminalSquare size={12} />
      <span>TERMINAL</span>
    </div>

    <div class="tab-strip" role="tablist" aria-label="Open terminal tabs">
      {#each $terminalTabs as tab (tab.id)}
        {@const isActive = tab.id === $activeTerminalTabId}
        <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
        <div
          class="panel-tab"
          class:active={isActive}
          role="tab"
          tabindex={isActive ? 0 : -1}
          aria-selected={isActive}
          aria-label="Terminal {tab.name}"
          title={tab.name}
          onclick={() => selectTab(tab.id)}
          onkeydown={(e) => onTabKey(e, tab.id)}
        >
          <TerminalSquare size={11} />
          <span class="panel-tab-name">{tab.name}</span>
          <button
            class="panel-tab-close"
            type="button"
            title="Close {tab.name}"
            aria-label="Close {tab.name}"
            onclick={(e) => closeTab(tab.id, e)}
          >
            <X size={10} />
          </button>
        </div>
      {/each}

      <button
        type="button"
        class="header-action"
        onclick={addTab}
        title="New terminal tab"
        aria-label="New terminal tab"
      >
        <Plus size={12} />
      </button>
    </div>

    <div class="header-actions" role="toolbar" aria-label="Terminal actions">
      <button
        type="button"
        class="header-action"
        onclick={splitRight}
        title="Split right"
        aria-label="Split terminal to the right"
      >
        <SplitSquareHorizontal size={12} />
      </button>
      <button
        type="button"
        class="header-action"
        onclick={splitBottom}
        title="Split down"
        aria-label="Split terminal downward"
      >
        <SplitSquareVertical size={12} />
      </button>
      {#if canCollapse}
        <button
          type="button"
          class="header-action"
          onclick={collapseSplits}
          title="Collapse splits"
          aria-label="Collapse splits in active terminal"
        >
          <Columns2 size={12} />
        </button>
      {/if}
      {#if activeTab}
        <button
          type="button"
          class="header-action danger"
          onclick={(e) => closeTab(activeTab.id, e)}
          title="Kill active terminal"
          aria-label="Kill active terminal"
        >
          <Trash2 size={12} />
        </button>
      {/if}
      <div class="divider" aria-hidden="true"></div>
      <button
        type="button"
        class="header-action"
        onclick={closePanel}
        title="Hide panel (Ctrl+`)"
        aria-label="Hide terminal panel"
      >
        <X size={13} />
      </button>
    </div>
  </header>

  <div class="panel-body">
    <Terminal />
  </div>
</section>

<style>
  .terminal-panel {
    display: flex;
    flex-direction: column;
    background: var(--bg-primary, var(--bg-tertiary));
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    position: relative;
    min-height: 0;
  }

  /* Kept in the DOM (so xterm keeps its state) but visually removed when
     collapsed. display:none is fine — xterm doesn't need layout while
     hidden, and the FitAddon is re-fit on next show. */
  .terminal-panel.hidden {
    display: none;
  }

  .resize-handle {
    position: absolute;
    top: -3px;
    left: 0;
    right: 0;
    height: 6px;
    cursor: row-resize;
    z-index: 2;
    background: transparent;
    transition: background 0.15s ease;
  }
  .resize-handle:hover,
  .resize-handle:focus-visible,
  .terminal-panel.dragging .resize-handle {
    background: var(--accent);
  }
  .resize-handle:focus-visible {
    outline: none;
  }

  .panel-header {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 30px;
    padding: 0 6px 0 12px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    font-size: 11px;
  }

  .header-label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--text-muted);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.6px;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  .tab-strip {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    scrollbar-width: none;
    flex: 1;
    min-width: 0;
  }
  .tab-strip::-webkit-scrollbar { display: none; height: 0; }

  .panel-tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px 3px 8px;
    border-radius: 14px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-secondary);
    background: color-mix(in srgb, var(--tab-inactive) 80%, transparent);
    border: 1px solid transparent;
    white-space: nowrap;
    max-width: 180px;
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
    user-select: none;
  }
  .panel-tab:hover {
    background: color-mix(in srgb, var(--bg-surface) 85%, transparent);
    color: var(--text-primary);
  }
  .panel-tab.active {
    background: var(--tab-active);
    color: var(--text-primary);
    border-color: var(--border);
  }
  .panel-tab:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: -2px;
  }

  .panel-tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .panel-tab-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0.6;
    transition: background 0.12s ease, color 0.12s ease, opacity 0.12s ease;
  }
  .panel-tab:hover .panel-tab-close,
  .panel-tab.active .panel-tab-close { opacity: 1; }
  .panel-tab-close:hover {
    background: color-mix(in srgb, var(--error, #f14c4c) 18%, transparent);
    color: var(--error, #f14c4c);
  }

  .header-actions {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .header-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    background: transparent;
    transition: background 0.12s ease, color 0.12s ease;
  }
  .header-action:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }
  .header-action:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: -2px;
  }
  .header-action.danger:hover {
    color: var(--error, #f14c4c);
    background: color-mix(in srgb, var(--error, #f14c4c) 12%, transparent);
  }

  .divider {
    width: 1px;
    height: 14px;
    background: var(--border);
    margin: 0 3px;
  }

  .panel-body {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  @media (prefers-reduced-motion: reduce) {
    .resize-handle,
    .panel-tab,
    .panel-tab-close,
    .header-action {
      transition: none;
    }
  }
</style>
