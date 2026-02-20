<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { Terminal as XTerm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { WebLinksAddon } from '@xterm/addon-web-links';
  import { open } from '@tauri-apps/plugin-shell';
  import { projectRoot, terminalFontSize, currentThemeId, getTheme, showTerminal } from './stores.ts';
  import { get } from 'svelte/store';
  import '@xterm/xterm/css/xterm.css';

  interface TerminalTab {
    id: number;         // local tab id
    sessionId: number;  // backend PTY session id
    name: string;
    xterm: XTerm;
    fitAddon: FitAddon;
    unlisten: UnlistenFn;
    unlistenExit: UnlistenFn;
    resizeObserver: ResizeObserver | null;
  }

  let terminalContainer: HTMLDivElement;
  let tabs = $state<TerminalTab[]>([]);
  let activeTabId = $state<number | null>(null);

  function buildXtermTheme() {
    const c = getTheme(get(currentThemeId)).colors;
    return {
      background: c.termBg,
      foreground: c.termFg,
      cursor: c.termCursor,
      selectionBackground: c.termSelection,
      black: c.termBlack,
      red: c.termRed,
      green: c.termGreen,
      yellow: c.termYellow,
      blue: c.termBlue,
      magenta: c.termMagenta,
      cyan: c.termCyan,
      white: c.termWhite,
      brightBlack: c.termBrightBlack,
      brightRed: c.termRed,
      brightGreen: c.termGreen,
      brightYellow: c.termYellow,
      brightBlue: c.termBlue,
      brightMagenta: c.termMagenta,
      brightCyan: c.termCyan,
      brightWhite: c.termBrightWhite,
    };
  }

  function getActiveTab(): TerminalTab | undefined {
    return tabs.find(t => t.id === activeTabId);
  }

  async function createTab() {
    const cwd = get(projectRoot);

    const xterm = new XTerm({
      cursorBlink: true,
      fontSize: get(terminalFontSize),
      fontFamily: "'SF Mono', 'Fira Code', 'Cascadia Code', monospace",
      theme: buildXtermTheme(),
    });

    const fitAddon = new FitAddon();
    xterm.loadAddon(fitAddon);
    xterm.loadAddon(new WebLinksAddon((_event, uri) => {
      if (uri.startsWith('http://') || uri.startsWith('https://')) {
        open(uri);
      }
    }));

    // Hide all other terminals, show this one
    hideAllTerminals();
    xterm.open(terminalContainer);
    // Fit first to get accurate dimensions, then spawn with correct size
    fitAddon.fit();
    await new Promise(r => requestAnimationFrame(r));
    fitAddon.fit();

    let sessionId: number;
    let name: string;
    let unlisten: UnlistenFn;
    let unlistenExit: UnlistenFn;

    try {
      const result = await invoke<{ id: number; pid: number | null }>('spawn_terminal', {
        cwd,
        rows: xterm.rows,
        cols: xterm.cols,
      });
      sessionId = result.id;
      name = result.pid ? `Terminal ${result.pid}` : `Terminal ${result.id}`;

      unlisten = await listen<string>(`terminal-output-${sessionId}`, (event) => {
        xterm.write(event.payload);
      });

      unlistenExit = await listen<void>(`terminal-exit-${sessionId}`, () => {
        killTab(sessionId);
      });

      // Send input directly to PTY
      xterm.onData((data) => {
        invoke('write_terminal', { id: sessionId, data });
      });

      xterm.onResize(({ rows, cols }) => {
        invoke('resize_terminal', { id: sessionId, rows, cols });
      });
    } catch (e) {
      xterm.writeln(`\r\nFailed to start terminal: ${e}`);
      xterm.writeln('Terminal requires Tauri runtime.');
      return;
    }

    const resizeObserver = new ResizeObserver(() => {
      if (activeTabId === sessionId) {
        fitAddon.fit();
      }
    });
    resizeObserver.observe(terminalContainer);

    const tab: TerminalTab = {
      id: sessionId,
      sessionId,
      name,
      xterm,
      fitAddon,
      unlisten,
      unlistenExit,
      resizeObserver,
    };

    tabs = [...tabs, tab];
    activeTabId = sessionId;
  }

  function hideAllTerminals() {
    for (const tab of tabs) {
      const el = tab.xterm.element;
      if (el) el.style.display = 'none';
    }
  }

  function switchTab(tabId: number) {
    if (activeTabId === tabId) return;
    hideAllTerminals();

    const tab = tabs.find(t => t.id === tabId);
    if (!tab) return;

    activeTabId = tabId;
    const el = tab.xterm.element;

    if (el && el.parentElement === terminalContainer) {
      // Already attached, just show it
      el.style.display = '';
    } else {
      // Re-attach (shouldn't normally happen)
      tab.xterm.open(terminalContainer);
    }

    // Refit after switching
    requestAnimationFrame(() => {
      tab.fitAddon.fit();
      tab.xterm.focus();
    });
  }

  async function killTab(tabId: number) {
    const idx = tabs.findIndex(t => t.id === tabId);
    if (idx === -1) return;

    const tab = tabs[idx];

    // Cleanup
    tab.unlisten();
    tab.unlistenExit();
    tab.resizeObserver?.disconnect();
    tab.xterm.dispose();
    try {
      await invoke('kill_terminal', { id: tab.sessionId });
    } catch (_) { /* ignore */ }

    const newTabs = tabs.filter(t => t.id !== tabId);
    tabs = newTabs;

    // Switch to another tab or clear
    if (activeTabId === tabId) {
      if (newTabs.length > 0) {
        const next = idx > 0 ? newTabs[idx - 1] : newTabs[0];
        switchTab(next.id);
      } else {
        activeTabId = null;
        showTerminal.set(false);
      }
    }
  }

  function handleCloseClick(e: MouseEvent, tabId: number) {
    e.stopPropagation();
    killTab(tabId);
  }

  // When project root changes, create a fresh first terminal
  let initializedForRoot: string | null = null;

  onMount(() => {
    const unsubFont = terminalFontSize.subscribe((size) => {
      for (const tab of tabs) {
        tab.xterm.options.fontSize = size;
        tab.fitAddon.fit();
      }
    });

    const unsubTheme = currentThemeId.subscribe(() => {
      const theme = buildXtermTheme();
      for (const tab of tabs) {
        tab.xterm.options.theme = theme;
      }
    });

    const unsub = projectRoot.subscribe((root) => {
      if (root !== initializedForRoot && terminalContainer) {
        initializedForRoot = root;
        // Kill all existing tabs and start fresh
        for (const tab of [...tabs]) {
          killTab(tab.id);
        }
        createTab();
      }
    });

    // Start a default terminal if no project root yet
    if (tabs.length === 0) {
      createTab();
    }

    return () => { unsub(); unsubFont(); unsubTheme(); };
  });

  onDestroy(() => {
    for (const tab of tabs) {
      tab.unlisten();
      tab.unlistenExit();
      tab.resizeObserver?.disconnect();
      tab.xterm.dispose();
    }
  });
</script>

<div class="terminal-panel">
  <div class="terminal-tabs">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="terminal-tab-list" onwheel={(e) => { e.preventDefault(); e.currentTarget.scrollLeft += e.deltaY; }}>
      {#each tabs as tab}
        <div
          class="terminal-tab"
          class:active={activeTabId === tab.id}
          role="tab"
          tabindex="0"
          onclick={() => switchTab(tab.id)}
          onkeydown={(e) => e.key === 'Enter' && switchTab(tab.id)}
        >
          <svg class="tab-icon" viewBox="0 0 16 16" fill="currentColor" width="11" height="11">
            <path d="M2 3a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V4a1 1 0 0 0-1-1H2zm1 2l3 2.5L3 10m4 0h4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span class="tab-name">{tab.name}</span>
          <button class="tab-close" onclick={(e) => handleCloseClick(e, tab.id)} title="Kill terminal">×</button>
        </div>
      {/each}
      <button class="new-tab-btn" onclick={createTab} title="New terminal">
        <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
          <path d="M8 3v10M3 8h10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      </button>
    </div>
  </div>
  <div class="terminal-content" bind:this={terminalContainer}></div>
</div>

<style>
  .terminal-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  .terminal-tabs {
    display: flex;
    align-items: center;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    height: 30px;
    flex-shrink: 0;
    padding: 0 4px;
    gap: 2px;
  }

  .terminal-tab-list {
    display: flex;
    align-items: center;
    overflow-x: auto;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }

  .terminal-tab-list::-webkit-scrollbar {
    height: 3px;
  }

  .terminal-tab-list::-webkit-scrollbar-track {
    background: transparent;
  }

  .terminal-tab-list::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 3px;
  }

  .terminal-tab {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    font-size: 11px;
    color: var(--text-muted);
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
    transition: all 0.1s;
  }

  .terminal-tab:hover {
    background: var(--bg-surface);
    color: var(--text-secondary);
  }

  .terminal-tab.active {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .tab-icon {
    flex-shrink: 0;
    opacity: 0.7;
  }

  .terminal-tab.active .tab-icon {
    opacity: 1;
    color: var(--accent);
  }

  .tab-name {
    max-width: 100px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tab-close {
    font-size: 14px;
    line-height: 1;
    color: var(--text-muted);
    padding: 0 2px;
    border-radius: 3px;
    opacity: 0;
    transition: opacity 0.1s;
  }

  .terminal-tab:hover .tab-close {
    opacity: 1;
  }

  .tab-close:hover {
    background: var(--bg-surface);
    color: var(--error);
  }

  .new-tab-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    color: var(--text-muted);
    flex-shrink: 0;
    transition: all 0.1s;
  }

  .new-tab-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .terminal-content {
    flex: 1;
    padding: 4px;
    overflow: hidden;
    position: relative;
  }

  .terminal-content :global(.xterm) {
    height: 100%;
  }
</style>
