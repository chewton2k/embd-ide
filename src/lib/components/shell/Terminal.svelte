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
    mounted: boolean;
  }

  type Rect = { top: number; left: number; width: number; height: number };

  let terminalRoot: HTMLDivElement;
  let panes = $state<TerminalPane[]>([]);
  let activePaneId = $state<number | null>(null);
  let contextMenu = $state<{ x: number; y: number; paneId: number | null } | null>(null);
  let contextMenuEl = $state<HTMLDivElement | undefined>();
  let opChain = Promise.resolve();

  // ── Split tree for nested tmux-style splits ──
  type SplitNode =
    | { type: 'leaf'; paneId: number }
    | { type: 'split'; direction: 'horizontal' | 'vertical'; children: [SplitNode, SplitNode] };
  let splitTree = $state<SplitNode | null>(null);

  function findLeaf(node: SplitNode | null, paneId: number): boolean {
    if (!node) return false;
    if (node.type === 'leaf') return node.paneId === paneId;
    return findLeaf(node.children[0], paneId) || findLeaf(node.children[1], paneId);
  }

  function replaceLeaf(node: SplitNode, targetId: number, replacement: SplitNode): SplitNode {
    if (node.type === 'leaf') return node.paneId === targetId ? replacement : node;
    return {
      type: 'split',
      direction: node.direction,
      children: [
        replaceLeaf(node.children[0], targetId, replacement),
        replaceLeaf(node.children[1], targetId, replacement),
      ],
    };
  }

  function removeLeaf(node: SplitNode | null, paneId: number): SplitNode | null {
    if (!node) return null;
    if (node.type === 'leaf') return node.paneId === paneId ? null : node;
    const left = removeLeaf(node.children[0], paneId);
    const right = removeLeaf(node.children[1], paneId);
    if (!left && !right) return null;
    if (!left) return right;
    if (!right) return left;
    return { type: 'split', direction: node.direction, children: [left, right] };
  }

  /** Compute percentage-based rects for each leaf pane from the split tree. */
  function computePaneRects(tree: SplitNode | null): Record<number, Rect> {
    const rects: Record<number, Rect> = {};
    function walk(node: SplitNode, top: number, left: number, width: number, height: number) {
      if (node.type === 'leaf') {
        rects[node.paneId] = { top, left, width, height };
        return;
      }
      if (node.direction === 'horizontal') {
        const w = width / 2;
        walk(node.children[0], top, left, w, height);
        walk(node.children[1], top, left + w, w, height);
      } else {
        const h = height / 2;
        walk(node.children[0], top, left, width, h);
        walk(node.children[1], top + h, left, width, h);
      }
    }
    if (tree) walk(tree, 0, 0, 100, 100);
    return rects;
  }

  let paneRects = $derived(computePaneRects(splitTree));

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

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    const target = e.target as HTMLElement;
    const paneEl = target.closest('[data-pane-terminal]');
    const paneId = paneEl ? Number(paneEl.getAttribute('data-pane-terminal')) : activePaneId;
    contextMenu = { x: e.clientX, y: e.clientY, paneId };
  }

  function closeContextMenu() { contextMenu = null; }

  function ctxAction(action: 'right' | 'bottom' | 'collapse' | 'close') {
    const paneId = contextMenu?.paneId;
    contextMenu = null;
    if (action === 'collapse') enqueue(() => collapseToActivePane());
    else if (action === 'close' && paneId != null) enqueue(() => closePane(paneId));
    else if (action === 'right' || action === 'bottom') {
      if (paneId != null) activePaneId = paneId;
      enqueue(() => splitTerminal(action));
    }
  }

  function buildXtermTheme() {
    const mode = get(appearanceMode);
    const isDark = mode === 'dark' || (mode === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    if (isDark) {
      return {
        background: '#131313', foreground: '#cccccc', cursor: '#7b9fc2', selectionBackground: '#2a3a4a',
        black: '#3b3b3b', red: '#f14c4c', green: '#4ec9b0', yellow: '#dcdcaa',
        blue: '#569cd6', magenta: '#c586c0', cyan: '#9cdcfe', white: '#cccccc',
        brightBlack: '#5a5a5a', brightRed: '#f14c4c', brightGreen: '#4ec9b0', brightYellow: '#dcdcaa',
        brightBlue: '#569cd6', brightMagenta: '#c586c0', brightCyan: '#9cdcfe', brightWhite: '#e0e0e0',
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
    if (!pane.mounted) return;
    const mount = getPaneMount(pane.id);
    if (!mount || mount.clientWidth === 0 || mount.clientHeight === 0) return;
    try { pane.fitAddon.fit(); } catch { /* ignore */ }
  }

  function focusPane(id: number) {
    activePaneId = id;
    const pane = panes.find((entry) => entry.id === id);
    if (!pane) return;
    requestAnimationFrame(() => {
      fitPane(pane);
      if (pane.mounted) {
        try { pane.xterm.focus(); } catch { /* ignore */ }
      }
    });
  }

  function enqueue<T>(task: () => Promise<T>): Promise<T> {
    const next = opChain.then(task);
    opChain = next.then(() => undefined, () => undefined);
    return next;
  }

  /**
   * Create a pane: spawn backend, build xterm, update tree, mount into DOM.
   * If `target` is provided, splits from that pane; otherwise creates standalone.
   */
  async function createPane(target?: { splitFrom: number; direction: 'horizontal' | 'vertical' }): Promise<TerminalPane | null> {
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
      return null;
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
      mounted: false,
    };

    // Update panes + split tree atomically so Svelte renders the mount div.
    panes = [...panes, pane];

    if (target && splitTree && findLeaf(splitTree, target.splitFrom)) {
      splitTree = replaceLeaf(splitTree, target.splitFrom, {
        type: 'split',
        direction: target.direction,
        children: [
          { type: 'leaf', paneId: target.splitFrom },
          { type: 'leaf', paneId: sessionId },
        ],
      });
    } else if (!splitTree) {
      splitTree = { type: 'leaf', paneId: sessionId };
    }

    showTerminal.set(true);
    activeFilePath.set(terminalPath());

    // Wait for Svelte to render the mount div.
    await tick();
    await new Promise((r) => requestAnimationFrame(r));

    const mount = getPaneMount(sessionId);
    if (mount) {
      xterm.open(mount);
      pane.mounted = true;
      fitPane(pane);
      await new Promise((r) => requestAnimationFrame(r));
      fitPane(pane);

      const resizeObserver = new ResizeObserver(() => fitPane(pane));
      resizeObserver.observe(mount);
      pane.resizeObserver = resizeObserver;
    }

    activePaneId = sessionId;
    requestAnimationFrame(() => {
      if (pane.mounted) {
        try { pane.xterm.focus(); } catch { /* ignore */ }
      }
    });

    return pane;
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
    splitTree = removeLeaf(splitTree, paneId);

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

    // Refit remaining panes after layout change.
    await tick();
    await new Promise((r) => requestAnimationFrame(r));
    for (const p of remaining) fitPane(p);

    if (activePaneId !== null) focusPane(activePaneId);
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
    if (panes.length === 0) {
      await createPane();
      return;
    }
    const targetId = activePaneId ?? panes[0]?.id;
    if (targetId == null) return;

    const dir = direction === 'right' ? 'horizontal' : 'vertical';
    await createPane({ splitFrom: targetId, direction: dir });

    // Refit all panes since existing ones shrank.
    await tick();
    await new Promise((r) => requestAnimationFrame(r));
    for (const p of panes) fitPane(p);
  }

  // Refit all panes when rects change (e.g. container resize).
  $effect(() => {
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    paneRects;
    requestAnimationFrame(() => {
      for (const p of panes) fitPane(p);
    });
  });

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
    // Auto-create a terminal pane when the component first mounts with none.
    if (panes.length === 0) {
      enqueue(() => createPane());
    }

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
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="terminal-content" role="application" bind:this={terminalRoot} oncontextmenu={handleContextMenu}>
    {#if panes.length === 0}
      <div class="terminal-placeholder">Open a terminal to start a session.</div>
    {:else}
      {#each panes as pane (pane.id)}
        {@const rect = paneRects[pane.id]}
        {#if rect}
          <div
            class="terminal-pane"
            class:active={activePaneId === pane.id}
            style="top:{rect.top}%;left:{rect.left}%;width:{rect.width}%;height:{rect.height}%"
            role="button"
            tabindex="0"
            onclick={() => focusPane(pane.id)}
            onkeydown={(e) => e.key === 'Enter' && focusPane(pane.id)}
          >
            <div class="pane-body" data-pane-terminal={pane.id}></div>
          </div>
        {/if}
      {/each}
    {/if}
  </div>

  {#if contextMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="ctx-backdrop" role="presentation" onclick={closeContextMenu} onkeydown={(e) => e.key === 'Escape' && closeContextMenu()} oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}></div>
    <div class="ctx-menu" bind:this={contextMenuEl} style="left:{contextMenu.x}px;top:{contextMenu.y}px">
      <button class="ctx-item" onclick={() => ctxAction('right')}>
        <Columns2 size={13} /> Split Right
      </button>
      <button class="ctx-item" onclick={() => ctxAction('bottom')}>
        <PanelBottom size={13} /> Split Bottom
      </button>
      {#if panes.length > 1}
        <div class="ctx-divider"></div>
        <button class="ctx-item" onclick={() => ctxAction('close')}>
          Close Terminal
        </button>
      {/if}
      {#if panes.length >= 2}
        <button class="ctx-item" onclick={() => ctxAction('collapse')}>
          <SplitSquareVertical size={13} /> Collapse All
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
    position: relative;
    background: var(--border);
    overflow: hidden;
  }

  .terminal-pane {
    position: absolute;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-tertiary);
    box-sizing: border-box;
    border: 0.5px solid var(--border);
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

  .ctx-divider {
    height: 1px;
    background: var(--border);
    margin: 3px 6px;
  }
</style>
