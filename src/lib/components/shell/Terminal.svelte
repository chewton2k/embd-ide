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
    activeFilePath, openFiles,
    terminalSessions, terminalTabs, activeTerminalTabId,
    createTerminalSignal, killTerminalSignal,
    splitTerminalSignal, collapseTerminalSplitsSignal,
    isTerminalPath, terminalPath, terminalTabIdFromPath, allocateTerminalTabId,
    terminalMode,
    type TerminalTabInfo,
  } from '../../modules';
  import { get } from 'svelte/store';
  import { SplitSquareVertical, PanelBottom, Columns2 } from 'lucide-svelte';
  import '@xterm/xterm/css/xterm.css';

  // ── Types ────────────────────────────────────────────────────────

  interface TerminalPane {
    /** Backend PTY / session id. Also used as the DOM pane key. */
    id: number;
    sessionId: number;
    /** Which terminal tab this pane belongs to. */
    tabId: number;
    name: string;
    xterm: XTerm;
    fitAddon: FitAddon;
    unlisten: UnlistenFn;
    unlistenExit: UnlistenFn;
    resizeObserver: ResizeObserver | null;
    mounted: boolean;
  }

  type Rect = { top: number; left: number; width: number; height: number };

  type SplitNode =
    | { type: 'leaf'; paneId: number }
    | { type: 'split'; direction: 'horizontal' | 'vertical'; children: [SplitNode, SplitNode] };

  // ── State ────────────────────────────────────────────────────────

  let terminalRoot: HTMLDivElement;
  let panes = $state<TerminalPane[]>([]);

  /** Per-tab split tree. Empty tabs have no entry. */
  let splitTrees = $state<Record<number, SplitNode | null>>({});
  /** Per-tab active pane id. Updated on click/focus/split/close. */
  let activePaneByTab = $state<Record<number, number | null>>({});

  let contextMenu = $state<{ x: number; y: number; paneId: number | null; tabId: number } | null>(null);
  let contextMenuEl = $state<HTMLDivElement | undefined>();

  /** Serialize pane-mutation ops so rapid split/close clicks don't interleave. */
  let opChain = Promise.resolve();

  // ── Derived views for the active tab ─────────────────────────────

  const currentTabId = $derived($activeTerminalTabId);
  const currentPanes = $derived(
    currentTabId == null ? [] : panes.filter(p => p.tabId === currentTabId)
  );
  const currentSplitTree = $derived<SplitNode | null>(
    currentTabId == null ? null : (splitTrees[currentTabId] ?? null)
  );
  const currentActivePaneId = $derived<number | null>(
    currentTabId == null ? null : (activePaneByTab[currentTabId] ?? null)
  );

  /** Rect layout for the ACTIVE tab's panes (the only ones that matter for
   *  layout — other tabs are CSS-hidden so their xterm stays alive). */
  const paneRects = $derived(computePaneRects(currentSplitTree));

  // ── Split tree helpers (pure functions) ──────────────────────────

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

  // ── Per-tab state mutators ───────────────────────────────────────

  function setSplitTree(tabId: number, tree: SplitNode | null) {
    splitTrees = { ...splitTrees, [tabId]: tree };
  }
  function setActivePane(tabId: number, paneId: number | null) {
    activePaneByTab = { ...activePaneByTab, [tabId]: paneId };
  }
  function removeTabState(tabId: number) {
    const { [tabId]: _tree, ...restTrees } = splitTrees;
    splitTrees = restTrees;
    const { [tabId]: _active, ...restActive } = activePaneByTab;
    activePaneByTab = restActive;
  }

  // ── Context-menu positioning ─────────────────────────────────────

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
    if (currentTabId == null) return;
    const target = e.target as HTMLElement;
    const paneEl = target.closest('[data-pane-terminal]');
    const paneId = paneEl ? Number(paneEl.getAttribute('data-pane-terminal')) : currentActivePaneId;
    contextMenu = { x: e.clientX, y: e.clientY, paneId, tabId: currentTabId };
  }

  function closeContextMenu() { contextMenu = null; }

  function ctxAction(action: 'right' | 'bottom' | 'collapse' | 'close') {
    const paneId = contextMenu?.paneId;
    const tabId = contextMenu?.tabId ?? currentTabId;
    contextMenu = null;
    if (tabId == null) return;
    if (action === 'collapse') enqueue(() => collapseToActivePane(tabId));
    else if (action === 'close' && paneId != null) enqueue(() => closePane(paneId));
    else if (action === 'right' || action === 'bottom') {
      if (paneId != null) setActivePane(tabId, paneId);
      enqueue(() => splitTerminal(action, tabId));
    }
  }

  // ── xterm theme ──────────────────────────────────────────────────

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

  // ── Pane helpers ─────────────────────────────────────────────────

  function getPaneMount(id: number): HTMLDivElement | null {
    return terminalRoot?.querySelector(`[data-pane-terminal="${id}"]`) ?? null;
  }

  /**
   * Route the editor's `activeFilePath` to a terminal sentinel so the
   * legacy in-tab terminal slot becomes the visible pane.
   *
   * In panel mode (`terminalMode === 'panel'`) the editor's focus is
   * independent of the docked terminal panel — the user keeps the same
   * file visible above the panel — so we skip the write. The panel
   * drives focus via `activeTerminalTabId` instead.
   */
  function routeActiveFileToTerminal(tabId: number) {
    if (get(terminalMode) === 'panel') return;
    activeFilePath.set(terminalPath(tabId));
  }

  function fitPane(pane: TerminalPane) {
    if (!pane.mounted) return;
    const mount = getPaneMount(pane.id);
    if (!mount || mount.clientWidth === 0 || mount.clientHeight === 0) return;
    try { pane.fitAddon.fit(); } catch { /* Legitimate: xterm may not be attached to DOM yet */ }
  }

  function focusPane(id: number) {
    const pane = panes.find((entry) => entry.id === id);
    if (!pane) return;
    setActivePane(pane.tabId, id);
    // Focus also implies making that tab the active terminal tab.
    if (get(activeTerminalTabId) !== pane.tabId) {
      activeTerminalTabId.set(pane.tabId);
    }
    requestAnimationFrame(() => {
      fitPane(pane);
      if (pane.mounted) {
        try { pane.xterm.focus(); } catch { /* Legitimate: terminal may not be visible */ }
      }
    });
  }

  function enqueue<T>(task: () => Promise<T>): Promise<T> {
    const next = opChain.then(task);
    opChain = next.then(() => undefined, () => undefined);
    return next;
  }

  // ── Tab management ───────────────────────────────────────────────

  /** Create a brand-new terminal tab and seed it with a single pane. */
  async function createTab(): Promise<number | null> {
    const tabId = allocateTerminalTabId();
    terminalTabs.update(tabs => [...tabs, buildTabLabel(tabs, tabId)]);
    setSplitTree(tabId, null);
    setActivePane(tabId, null);
    activeTerminalTabId.set(tabId);
    showTerminal.set(true);
    routeActiveFileToTerminal(tabId);
    const pane = await createPane({ tabId });
    if (!pane) {
      // Backend refused to spawn — roll back the tab so the UI doesn't
      // show an empty placeholder tab forever.
      terminalTabs.update(tabs => tabs.filter(t => t.id !== tabId));
      removeTabState(tabId);
      if (get(activeTerminalTabId) === tabId) {
        const remaining = get(terminalTabs);
        activeTerminalTabId.set(remaining[0]?.id ?? null);
      }
      return null;
    }
    return tabId;
  }

  function buildTabLabel(existing: TerminalTabInfo[], id: number): TerminalTabInfo {
    // Use the next available sequential number (fills gaps left by closed tabs).
    const taken = new Set(existing.map(t => {
      const m = t.name.match(/^Terminal (\d+)$/);
      return m ? parseInt(m[1], 10) : 0;
    }));
    let n = 1;
    while (taken.has(n)) n++;
    return { id, name: `Terminal ${n}` };
  }

  async function focusTab(tabId: number) {
    activeTerminalTabId.set(tabId);
    showTerminal.set(true);
    routeActiveFileToTerminal(tabId);
    const activeInTab = activePaneByTab[tabId];
    if (activeInTab != null) {
      requestAnimationFrame(() => focusPane(activeInTab));
    } else {
      // Focus the first pane in this tab if any.
      const firstPane = panes.find(p => p.tabId === tabId);
      if (firstPane) {
        setActivePane(tabId, firstPane.id);
        requestAnimationFrame(() => focusPane(firstPane.id));
      }
    }
  }

  async function closeTab(tabId: number) {
    // Close all panes in this tab; the per-pane close handler handles the
    // tab removal when the last pane goes away.
    const tabPanes = panes.filter(p => p.tabId === tabId);
    for (const pane of tabPanes) {
      await closePane(pane.id);
    }
  }

  async function closeAllTabs() {
    for (const pane of [...panes]) {
      await closePane(pane.id);
    }
  }

  // ── Pane lifecycle ───────────────────────────────────────────────

  /**
   * Create a pane. Pass `tabId` to add to a specific tab, plus an optional
   * `splitFrom` pane + `direction` to split in-place.
   */
  async function createPane(target: {
    tabId: number;
    splitFrom?: number;
    direction?: 'horizontal' | 'vertical';
  }): Promise<TerminalPane | null> {
    const cwd = get(projectRoot);
    const { tabId } = target;

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

        // App-level shortcuts (tab navigation, panel toggles, file
        // search, settings, etc.) are intercepted upstream by a
        // capture-phase listener on `window` in App.svelte
        // (`handleKeydownCapture`). That listener fires before this
        // handler, calls preventDefault + stopImmediatePropagation,
        // and dispatches the action — so by the time we get here the
        // event is guaranteed to NOT be an app shortcut. We therefore
        // only deal with terminal-specific keystrokes that map to PTY
        // control codes. Returning false means xterm doesn't process
        // the key; we've already dispatched the equivalent control
        // sequence ourselves via `write_terminal`.
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
      tabId,
      name,
      xterm,
      fitAddon,
      unlisten,
      unlistenExit,
      resizeObserver: null,
      mounted: false,
    };

    panes = [...panes, pane];

    // Update the split tree for this tab.
    const currentTree = splitTrees[tabId] ?? null;
    if (target.splitFrom && currentTree && findLeaf(currentTree, target.splitFrom) && target.direction) {
      setSplitTree(tabId, replaceLeaf(currentTree, target.splitFrom, {
        type: 'split',
        direction: target.direction,
        children: [
          { type: 'leaf', paneId: target.splitFrom },
          { type: 'leaf', paneId: sessionId },
        ],
      }));
    } else if (!currentTree) {
      setSplitTree(tabId, { type: 'leaf', paneId: sessionId });
    }

    showTerminal.set(true);
    if (get(activeTerminalTabId) !== tabId) activeTerminalTabId.set(tabId);
    if (!isTerminalPath(get(activeFilePath)) || terminalTabIdFromPath(get(activeFilePath)) !== tabId) {
      routeActiveFileToTerminal(tabId);
    }

    // Wait for Svelte to render the mount div for this new pane.
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

    setActivePane(tabId, sessionId);
    requestAnimationFrame(() => {
      if (pane.mounted) {
        try { pane.xterm.focus(); } catch { /* Legitimate: terminal may not be visible */ }
      }
    });

    return pane;
  }

  async function closePane(paneId: number, killBackend = true) {
    const idx = panes.findIndex((entry) => entry.id === paneId);
    if (idx === -1) return;

    const pane = panes[idx];
    const tabId = pane.tabId;

    pane.unlisten();
    pane.unlistenExit();
    pane.resizeObserver?.disconnect();
    pane.xterm.dispose();

    if (killBackend) {
      try { await invoke('kill_terminal', { id: pane.sessionId }); } catch { /* Legitimate: session may already be dead */ }
    }

    const remaining = panes.filter((entry) => entry.id !== paneId);
    panes = remaining;

    const newTree = removeLeaf(splitTrees[tabId] ?? null, paneId);
    setSplitTree(tabId, newTree);

    const tabPanes = remaining.filter(p => p.tabId === tabId);
    if (tabPanes.length === 0) {
      // Tab is now empty — remove it entirely.
      removeTabState(tabId);
      terminalTabs.update(tabs => tabs.filter(t => t.id !== tabId));
      if (remaining.length === 0) {
        showTerminal.set(false);
        if (isTerminalPath(get(activeFilePath))) {
          const files = get(openFiles);
          activeFilePath.set(files.at(-1)?.path ?? null);
        }
        activeTerminalTabId.set(null);
      } else {
        // Switch to another tab.
        const remainingTabs = get(terminalTabs);
        const nextTabId = remainingTabs[0]?.id ?? null;
        activeTerminalTabId.set(nextTabId);
        if (nextTabId != null && isTerminalPath(get(activeFilePath))) {
          activeFilePath.set(terminalPath(nextTabId));
        }
      }
    } else {
      // Other panes remain in this tab — pick a new active pane.
      const prevActive = activePaneByTab[tabId];
      if (prevActive === paneId) {
        setActivePane(tabId, tabPanes[0].id);
      }
      // Refit remaining panes after layout change.
      await tick();
      await new Promise((r) => requestAnimationFrame(r));
      for (const p of tabPanes) fitPane(p);
      const current = activePaneByTab[tabId];
      if (current != null) focusPane(current);
    }
  }

  // ── Split / collapse (within a tab) ──────────────────────────────

  async function splitTerminal(direction: 'right' | 'bottom', tabId: number) {
    const tabPanes = panes.filter(p => p.tabId === tabId);
    if (tabPanes.length === 0) {
      // Creating a split in an empty tab just spawns a pane.
      await createPane({ tabId });
      return;
    }
    const targetId = activePaneByTab[tabId] ?? tabPanes[0]?.id;
    if (targetId == null) return;
    const dir = direction === 'right' ? 'horizontal' : 'vertical';
    await createPane({ tabId, splitFrom: targetId, direction: dir });

    await tick();
    await new Promise((r) => requestAnimationFrame(r));
    for (const p of tabPanes) fitPane(p);
  }

  async function collapseToActivePane(tabId: number) {
    const tabPanes = panes.filter(p => p.tabId === tabId);
    const keepId = activePaneByTab[tabId] ?? tabPanes[0]?.id ?? null;
    if (keepId == null || tabPanes.length <= 1) return;
    for (const pane of tabPanes) {
      if (pane.id !== keepId) {
        await closePane(pane.id);
      }
    }
    focusPane(keepId);
  }

  // ── External signal handlers ─────────────────────────────────────

  // Mirror the panes list to the global `terminalSessions` store. This is
  // how the agent loop and other components see what terminals exist.
  $effect(() => {
    terminalSessions.set(panes.map(pane => ({
      id: pane.id,
      tabId: pane.tabId,
      name: pane.name,
    })));
  });

  // Keep `activeTerminalTabId` in sync with `activeFilePath` when the user
  // clicks a terminal tab in the top bar.
  $effect(() => {
    const path = $activeFilePath;
    if (!isTerminalPath(path)) return;
    const tabIdFromPath = terminalTabIdFromPath(path);
    if (tabIdFromPath != null && tabIdFromPath !== get(activeTerminalTabId)) {
      activeTerminalTabId.set(tabIdFromPath);
    }
    // Re-focus the active pane for this tab.
    const current = tabIdFromPath != null ? activePaneByTab[tabIdFromPath] : null;
    if (current != null) {
      requestAnimationFrame(() => focusPane(current));
    }
  });

  // createTerminalSignal: create a new tab if requested, or ensure ≥1 tab exists.
  let createCount = 0;
  $effect(() => {
    const sig = $createTerminalSignal;
    if (sig.count <= createCount) return;
    createCount = sig.count;
    enqueue(async () => {
      if (sig.forceNew) {
        await createTab();
        return;
      }
      // Toggle/ensure behavior: if no tabs exist, create one; otherwise focus.
      const tabs = get(terminalTabs);
      if (tabs.length === 0) {
        await createTab();
      } else {
        const id = get(activeTerminalTabId) ?? tabs[0].id;
        focusTab(id);
      }
    });
  });

  let splitCount = 0;
  $effect(() => {
    const sig = $splitTerminalSignal;
    if (sig.count <= splitCount) return;
    splitCount = sig.count;
    enqueue(async () => {
      let tabId = get(activeTerminalTabId);
      if (tabId == null) {
        // No tab exists — create one + split immediately doesn't make sense,
        // so fall back to creating a single pane tab.
        await createTab();
        return;
      }
      await splitTerminal(sig.direction, tabId);
    });
  });

  let collapseCount = 0;
  $effect(() => {
    const sig = $collapseTerminalSplitsSignal;
    if (sig <= collapseCount) return;
    collapseCount = sig;
    const tabId = get(activeTerminalTabId);
    if (tabId == null) return;
    if (panes.filter(p => p.tabId === tabId).length > 1) {
      enqueue(async () => { await collapseToActivePane(tabId); });
    }
  });

  // killTerminalSignal: handle 'pane', 'tab', 'all' variants.
  $effect(() => {
    const target = $killTerminalSignal;
    if (target === null) return;
    killTerminalSignal.set(null);
    enqueue(async () => {
      if (target.kind === 'all') {
        await closeAllTabs();
      } else if (target.kind === 'tab') {
        await closeTab(target.id);
      } else {
        await closePane(target.id);
      }
    });
  });

  // Refit all panes when rects change (e.g. container resize or active tab switch).
  $effect(() => {
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    paneRects;
    currentTabId;
    requestAnimationFrame(() => {
      for (const p of currentPanes) fitPane(p);
    });
  });

  // Auto-focus the active pane when the terminal becomes visible. In tab
  // mode we key off `isTerminalPath($activeFilePath)` (the user just
  // clicked a terminal tab). In panel mode the editor's path is
  // independent, so we instead focus whenever the panel is shown and has
  // an active pane — this mirrors VSCode's behavior when you toggle the
  // bottom panel.
  $effect(() => {
    const inTerminalTab = $terminalMode === 'tab' && isTerminalPath($activeFilePath);
    const panelShown = $terminalMode === 'panel';
    if ($showTerminal && (inTerminalTab || panelShown) && currentActivePaneId !== null) {
      const paneId = currentActivePaneId;
      requestAnimationFrame(() => focusPane(paneId));
    }
  });

  // ── Lifecycle ────────────────────────────────────────────────────

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
    // Kill backend PTY processes immediately. Doing this here (rather
    // than via the async killTerminalSignal flow) avoids a race where
    // deferred close operations from this component fire AFTER a
    // fresh Terminal instance has been mounted and stomp on its
    // shared-store state (e.g. flipping showTerminal back to false
    // and hiding the user's new terminal).
    for (const pane of panes) {
      // Fire-and-forget: onDestroy can't await, but the backend kill
      // is independent of this component's lifecycle.
      invoke('kill_terminal', { id: pane.sessionId }).catch(() => { /* PTY may already be dead */ });
      pane.unlisten();
      pane.unlistenExit();
      pane.resizeObserver?.disconnect();
      pane.xterm.dispose();
    }
    terminalSessions.set([]);
    terminalTabs.set([]);
    activeTerminalTabId.set(null);
    createTerminalSignal.set({ count: 0, forceNew: false });
  });
</script>

<div class="terminal-panel">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="terminal-content" role="application" bind:this={terminalRoot} oncontextmenu={handleContextMenu}>
    {#if $terminalTabs.length === 0}
      <div class="terminal-placeholder">Open a terminal to start a session.</div>
    {/if}

    <!--
      We render each terminal tab as its own absolutely-positioned layer.
      Inactive layers are CSS-hidden (visibility: hidden; pointer-events: none)
      so their xterm DOM stays mounted and the PTY output keeps writing into
      their scrollback — switching tabs is then instant with no loss of state.
    -->
    {#each $terminalTabs as tab (tab.id)}
      {@const tabPanes = panes.filter(p => p.tabId === tab.id)}
      {@const isActive = tab.id === currentTabId}
      {@const rectsForTab = isActive ? paneRects : computePaneRects(splitTrees[tab.id] ?? null)}
      <div class="tab-layer" class:active={isActive}>
        {#each tabPanes as pane (pane.id)}
          {@const rect = rectsForTab[pane.id]}
          {#if rect}
            <div
              class="terminal-pane"
              class:active={isActive && currentActivePaneId === pane.id}
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
      </div>
    {/each}
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
      {#if contextMenu && panes.filter(p => p.tabId === contextMenu!.tabId).length > 1}
        <div class="ctx-divider"></div>
        <button class="ctx-item" onclick={() => ctxAction('close')}>
          Close Pane
        </button>
        <button class="ctx-item" onclick={() => ctxAction('collapse')}>
          <SplitSquareVertical size={13} /> Collapse Panes
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

  /* One absolutely-positioned layer per terminal tab. Only the active layer
     is visible — inactive layers keep their xterm DOM mounted so PTYs stay
     live and their scrollback isn't lost when switching.

     Uses opacity (not visibility) so that the parent .terminal-tab-slot's
     visibility:hidden cannot be overridden by children. CSS spec allows a
     child to set visibility:visible and punch through a hidden parent, but
     opacity on a child cannot override a parent's visibility:hidden. This
     keeps the slot/layer hiding hierarchy correct. */
  .tab-layer {
    position: absolute;
    inset: 0;
    opacity: 0;
    pointer-events: none;
  }
  .tab-layer.active {
    opacity: 1;
    pointer-events: auto;
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
