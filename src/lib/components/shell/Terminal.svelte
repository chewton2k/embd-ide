<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { Terminal as XTerm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { WebLinksAddon } from '@xterm/addon-web-links';
  import { open } from '@tauri-apps/plugin-shell';
  import {
    projectRoot, terminalFontSize, appearanceMode, showTerminal,
    activeFilePath, openFiles, terminalSessions, createTerminalSignal,
    killTerminalSignal, splitTerminalSignal, collapseTerminalSplitsSignal,
    isTerminalPath, terminalPath
  } from '../../modules/stores';
  import { get } from 'svelte/store';
  import { SplitSquareVertical, PanelBottom, Columns2, TerminalSquare } from 'lucide-svelte';
  import '@xterm/xterm/css/xterm.css';

  interface TerminalPane {
    id: number;
    sessionId: number;
    name: string;
    xterm: XTerm;
    fitAddon: FitAddon;
    unlisten: UnlistenFn;
    unlistenExit: UnlistenFn;
    resizeObserver: ResizeObserver | null;
  }

  let terminalRoot: HTMLDivElement;
  let panes = $state<TerminalPane[]>([]);
  let activePaneId = $state<number | null>(null);
  let splitDirection = $state<'right' | 'bottom'>('right');
  let contextMenu = $state<{ x: number; y: number } | null>(null);
  let opChain = Promise.resolve();

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY };
  }

  function closeContextMenu() { contextMenu = null; }

  function ctxAction(action: 'right' | 'bottom' | 'collapse') {
    contextMenu = null;
    if (action === 'collapse') enqueue(() => collapseToActivePane());
    else enqueue(() => splitTerminal(action));
  }

  function buildXtermTheme() {
    const mode = get(appearanceMode);
    const isDark = mode === 'dark' || (mode === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    if (isDark) {
      return {
        background: '#111111', foreground: '#e0e0e0', cursor: '#7aa2f7', selectionBackground: '#2e2e2e',
        black: '#3a3a3a', red: '#f7768e', green: '#73daca', yellow: '#e0af68',
        blue: '#7aa2f7', magenta: '#bb9af7', cyan: '#7dcfff', white: '#c0c0c0',
        brightBlack: '#555555', brightRed: '#f7768e', brightGreen: '#73daca', brightYellow: '#e0af68',
        brightBlue: '#7aa2f7', brightMagenta: '#bb9af7', brightCyan: '#7dcfff', brightWhite: '#e0e0e0',
      };
    }
    return {
      background: '#fafafa', foreground: '#24292e', cursor: '#0969da', selectionBackground: '#d8dee4',
      black: '#8b949e', red: '#cf222e', green: '#1a7f37', yellow: '#9a6700',
      blue: '#0969da', magenta: '#8250df', cyan: '#1b7c83', white: '#24292e',
      brightBlack: '#6e7781', brightRed: '#cf222e', brightGreen: '#1a7f37', brightYellow: '#9a6700',
      brightBlue: '#0969da', brightMagenta: '#8250df', brightCyan: '#1b7c83', brightWhite: '#24292e',
    };
  }

  function getPaneMount(id: number): HTMLDivElement | null {
    return terminalRoot?.querySelector(`[data-pane-terminal="${id}"]`) ?? null;
  }

  function fitPane(pane: TerminalPane) {
    const mount = getPaneMount(pane.id);
    if (!mount || mount.clientWidth === 0 || mount.clientHeight === 0) return;
    pane.fitAddon.fit();
  }

  function focusPane(id: number) {
    activePaneId = id;
    const pane = panes.find((entry) => entry.id === id);
    if (!pane) return;
    requestAnimationFrame(() => {
      fitPane(pane);
      pane.xterm.focus();
    });
  }

  function enqueue<T>(task: () => Promise<T>): Promise<T> {
    const next = opChain.then(task);
    opChain = next.then(() => undefined, () => undefined);
    return next;
  }

  async function createPane() {
    const cwd = get(projectRoot);

    const xterm = new XTerm({
      cursorBlink: true,
      fontSize: get(terminalFontSize),
      fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--font-mono').trim()
        || "ui-monospace, 'SF Mono', Menlo, Monaco, Consolas, monospace",
      theme: buildXtermTheme(),
    });

    const fitAddon = new FitAddon();
    xterm.loadAddon(fitAddon);
    xterm.loadAddon(new WebLinksAddon((_event, uri) => {
      if (uri.startsWith('http://') || uri.startsWith('https://')) open(uri);
    }));

    let sessionId: number;
    let name: string;
    let unlisten: UnlistenFn;
    let unlistenExit: UnlistenFn;

    try {
      const result = await invoke<{ id: number; pid: number | null }>('spawn_terminal', {
        cwd,
        rows: 24,
        cols: 80,
      });
      sessionId = result.id;
      name = result.pid ? `Terminal ${result.pid}` : `Terminal ${result.id}`;

      unlisten = await listen<string>(`terminal-output-${sessionId}`, (event) => {
        xterm.write(event.payload);
      });

      unlistenExit = await listen<void>(`terminal-exit-${sessionId}`, () => {
        enqueue(() => closePane(sessionId, false));
      });

      xterm.attachCustomKeyEventHandler((e: KeyboardEvent) => {
        if (e.type !== 'keydown') return true;
        if (e.metaKey && e.key === 'Backspace') { invoke('write_terminal', { id: sessionId, data: '\x15' }); return false; }
        if (e.metaKey && e.key === 'ArrowLeft') { invoke('write_terminal', { id: sessionId, data: '\x01' }); return false; }
        if (e.metaKey && e.key === 'ArrowRight') { invoke('write_terminal', { id: sessionId, data: '\x05' }); return false; }
        if (e.altKey && e.key === 'Backspace') { invoke('write_terminal', { id: sessionId, data: '\x17' }); return false; }
        return true;
      });

      xterm.onData((data) => { invoke('write_terminal', { id: sessionId, data }); });
      xterm.onResize(({ rows, cols }) => { invoke('resize_terminal', { id: sessionId, rows, cols }); });
    } catch (e) {
      xterm.writeln(`\r\nFailed to start terminal: ${e}`);
      xterm.writeln('Terminal requires Tauri runtime.');
      xterm.dispose();
      return;
    }

    const pane: TerminalPane = {
      id: sessionId,
      sessionId,
      name,
      xterm,
      fitAddon,
      unlisten,
      unlistenExit,
      resizeObserver: null,
    };

    panes = [...panes, pane];
    activePaneId = sessionId;
    showTerminal.set(true);
    activeFilePath.set(terminalPath());

    await tick();

    const mount = getPaneMount(pane.id);
    if (!mount) return;

    xterm.open(mount);
    fitPane(pane);
    await new Promise((resolve) => requestAnimationFrame(resolve));
    fitPane(pane);

    const resizeObserver = new ResizeObserver(() => {
      fitPane(pane);
    });
    resizeObserver.observe(mount);
    pane.resizeObserver = resizeObserver;

    requestAnimationFrame(() => xterm.focus());
  }

  async function closePane(paneId: number, killBackend = true) {
    const idx = panes.findIndex((entry) => entry.id === paneId);
    if (idx === -1) return;

    const pane = panes[idx];
    pane.unlisten();
    pane.unlistenExit();
    pane.resizeObserver?.disconnect();
    pane.xterm.dispose();

    if (killBackend) {
      try { await invoke('kill_terminal', { id: pane.sessionId }); } catch { /* ignore */ }
    }

    const remaining = panes.filter((entry) => entry.id !== paneId);
    panes = remaining;

    if (activePaneId === paneId) {
      activePaneId = remaining[idx]?.id ?? remaining[idx - 1]?.id ?? remaining[0]?.id ?? null;
    }

    if (remaining.length === 0) {
      showTerminal.set(false);
      if (isTerminalPath(get(activeFilePath))) {
        const files = get(openFiles);
        activeFilePath.set(files.at(-1)?.path ?? null);
      }
      return;
    }

    if (!remaining.some((entry) => entry.id === activePaneId)) {
      activePaneId = remaining[0].id;
    }

    if (activePaneId !== null) {
      focusPane(activePaneId);
    }
  }

  async function closeAllPanes() {
    for (const pane of [...panes]) {
      await closePane(pane.id);
    }
  }

  async function collapseToActivePane() {
    const keepId = activePaneId ?? panes[0]?.id ?? null;
    if (keepId === null || panes.length <= 1) return;
    for (const pane of [...panes]) {
      if (pane.id !== keepId) {
        await closePane(pane.id);
      }
    }
    focusPane(keepId);
  }

  async function ensureTerminalVisible() {
    showTerminal.set(true);
    activeFilePath.set(terminalPath());
    if (panes.length === 0) {
      await createPane();
      return;
    }
    if (activePaneId !== null) {
      focusPane(activePaneId);
    }
  }

  async function splitTerminal(direction: 'right' | 'bottom') {
    splitDirection = direction;
    if (panes.length === 0) {
      await createPane();
    }
    await createPane();
  }

  $effect(() => {
    terminalSessions.set(panes.map((pane) => ({
      id: pane.id,
      name: pane.name,
    })));
  });

  $effect(() => {
    const path = $activeFilePath;
    if (!isTerminalPath(path)) return;
    if (path !== terminalPath()) {
      activeFilePath.set(terminalPath());
      return;
    }
    if (activePaneId !== null) {
      const paneId = activePaneId;
      requestAnimationFrame(() => focusPane(paneId));
    }
  });

  let createCount = 0;
  $effect(() => {
    const sig = $createTerminalSignal;
    if (sig > createCount) {
      createCount = sig;
      enqueue(async () => {
        await ensureTerminalVisible();
      });
    }
  });

  let splitCount = 0;
  $effect(() => {
    const sig = $splitTerminalSignal;
    if (sig.count > splitCount) {
      splitCount = sig.count;
      enqueue(async () => {
        await splitTerminal(sig.direction);
      });
    }
  });

  let collapseCount = 0;
  $effect(() => {
    const sig = $collapseTerminalSplitsSignal;
    if (sig > collapseCount) {
      collapseCount = sig;
      enqueue(async () => {
        await collapseToActivePane();
      });
    }
  });

  $effect(() => {
    const target = $killTerminalSignal;
    if (target === null) return;
    killTerminalSignal.set(null);
    enqueue(async () => {
      if (target === 'all') {
        await closeAllPanes();
      } else {
        await closePane(target);
      }
    });
  });

  $effect(() => {
    if ($showTerminal && isTerminalPath($activeFilePath) && activePaneId !== null) {
      const paneId = activePaneId;
      requestAnimationFrame(() => focusPane(paneId));
    }
  });

  onMount(() => {
    const unsubFont = terminalFontSize.subscribe((size) => {
      for (const pane of panes) {
        pane.xterm.options.fontSize = size;
        fitPane(pane);
      }
    });

    const unsubTheme = appearanceMode.subscribe(() => {
      const theme = buildXtermTheme();
      for (const pane of panes) {
        pane.xterm.options.theme = theme;
      }
    });

    return () => {
      unsubFont();
      unsubTheme();
    };
  });

  onDestroy(() => {
    terminalSessions.set([]);
    createTerminalSignal.set(0);
    for (const pane of panes) {
      pane.unlisten();
      pane.unlistenExit();
      pane.resizeObserver?.disconnect();
      pane.xterm.dispose();
    }
  });
</script>

<div class="terminal-panel">
  <div class="terminal-content" class:split-right={splitDirection === 'right'} class:split-bottom={splitDirection === 'bottom'} bind:this={terminalRoot} oncontextmenu={handleContextMenu}>
    {#if panes.length === 0}
      <div class="terminal-placeholder">Open a terminal to start a session.</div>
    {:else}
      {#each panes as pane (pane.id)}
        <div
          class="terminal-pane"
          class:active={activePaneId === pane.id}
          role="button"
          tabindex="0"
          onclick={() => focusPane(pane.id)}
          onkeydown={(e) => e.key === 'Enter' && focusPane(pane.id)}
        >
          <div class="pane-body" data-pane-terminal={pane.id}></div>
        </div>
      {/each}
    {/if}
  </div>

  {#if contextMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="ctx-backdrop" onclick={closeContextMenu} oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}></div>
    <div class="ctx-menu" style="left:{contextMenu.x}px;top:{contextMenu.y}px">
      <button class="ctx-item" onclick={() => ctxAction('right')}>
        <Columns2 size={13} /> Split Right
      </button>
      <button class="ctx-item" onclick={() => ctxAction('bottom')}>
        <PanelBottom size={13} /> Split Bottom
      </button>
      {#if panes.length > 1}
        <button class="ctx-item" onclick={() => ctxAction('collapse')}>
          <SplitSquareVertical size={13} /> Collapse Splits
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .terminal-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-tertiary);
    position: relative;
  }

  .terminal-content {
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    gap: 1px;
    background: var(--border);
  }

  .terminal-content.split-right { flex-direction: row; }
  .terminal-content.split-bottom { flex-direction: column; }

  .terminal-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    flex: 1;
    overflow: hidden;
    background: var(--bg-tertiary);
  }

  .terminal-pane.active { }

  .pane-body {
    flex: 1;
    min-height: 0;
    padding: 4px 8px;
  }

  .pane-body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    padding: 4px;
    overflow: hidden;
  }

  .pane-body :global(.xterm),
  .pane-body :global(.xterm-viewport),
  .pane-body :global(.xterm-screen) {
    height: 100%;
  }

  .terminal-placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    color: var(--text-muted);
    font-size: 13px;
  }

  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
  }

  .ctx-menu {
    position: fixed;
    z-index: 1000;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 4px;
    min-width: 160px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.3);
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border-radius: 5px;
    font-size: 12px;
    color: var(--text-primary);
    cursor: pointer;
    transition: background 0.1s;
  }

  .ctx-item:hover {
    background: var(--bg-surface);
  }
</style>
