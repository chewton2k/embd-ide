<script lang="ts">
  import FileTree from './lib/components/filetree/FileTree.svelte';
  import { Sparkles, TerminalSquare } from 'lucide-svelte';
  import Editor from './lib/components/editor/Editor.svelte';
  import FileViewer from './lib/components/file-viewer/FileViewer.svelte';
  import JSONViewer from './lib/components/file-viewer/JSONViewer.svelte';
  import MergeEditor from './lib/components/merge/MergeEditor.svelte';
  import Toolbar from './lib/components/toolbar/Toolbar.svelte';
  import TitleBar from './lib/components/toolbar/TitleBar.svelte';
  import Terminal from './lib/components/shell/Terminal.svelte';
  import TerminalPanel from './lib/components/shell/TerminalPanel.svelte';
  import FloatingChat from './lib/components/ai/FloatingChat.svelte';
  import GitPanel from './lib/components/git/GitPanel.svelte';
  import FileSearch from './lib/components/filetree/FileSearch.svelte';
  import Preview from './lib/components/preview/Preview.svelte';
  import FileDiagram from './lib/components/diagram/FileDiagram.svelte';
  import Toast from './lib/components/Toast.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { exists } from '@tauri-apps/plugin-fs';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { openFiles, activeFile, activeFilePath, activeFileModified, addFile, autosaveEnabled, projectRoot, gitBranch, showSettings, showTerminal, showPreview, isTerminalPath, isPreviewPath, isDiagramPath, getDiagramFilePath, PREVIEW_PATH, terminalTabs, activeTerminalTabId, createTerminalSignal, appearanceMode, uiFontSize, uiDensity, apiKey, openaiApiKey, anthropicApiKey, sharedGitStatus, nextTab, prevTab, showChat, showGit, toggleChatPanel, toggleGitPanel, fileTreeNavTarget, terminalPath, openFileSearchSignal, openDiagramSearchSignal, openDiagrams, diagramPath, terminalMode, saveConversationNow, createFileSignal, createFolderSignal, breadcrumbSegmentsFor, createPanelResizer, type PanelTarget } from './lib/modules';
  import { getRecentProjects, removeRecentProject, scheduleSaveSession, saveSessionNow, type RecentProject } from './lib/modules/session';
  import { log } from './lib/modules/logging';
  import { isMac, isFullscreen, installWindowChromeWatchers, openSettingsWindow } from './lib/modules/ui';
  import { showToast } from './lib/modules/ui/toast';
  import { toggleTerminal } from './lib/modules/terminal';
  import { shortcutBindings, eventMatchesBinding, APP_LEVEL_SHORTCUT_IDS, type AppLevelShortcutId } from './lib/modules/shortcuts';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';

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

  let recentProjects = $state<RecentProject[]>([]);
  let showAllRecent = $state(false);
  let openFolderByPath: ((path: string, restoreSession?: boolean) => Promise<void>) | null = null;

  function handleOpenFolder(fn: (path: string, restoreSession?: boolean) => Promise<void>) {
    openFolderByPath = fn;
  }

  async function openRecentProject(project: RecentProject) {
    if (!openFolderByPath) return;
    const folderExists = await exists(project.path);
    if (!folderExists) {
      await removeRecentProject(project.path);
      recentProjects = recentProjects.filter(p => p.path !== project.path);
      showToast({ level: 'warn', message: `Project folder no longer exists: ${project.path}` });
      return;
    }
    await openFolderByPath(project.path);
  }

  async function openProjectFromToolbar(path: string) {
    if (!openFolderByPath) return;
    await openFolderByPath(path);
  }

  async function openFolderDialog() {
    const selected = await openDialog({ directory: true, multiple: false });
    if (selected && openFolderByPath) {
      await openFolderByPath(selected as string);
    }
  }

  let showFileSearch = $state(false);
  let showDiagramSearch = $state(false);
  let sidebarWidth = $state(220);
  let sidebarVisible = $state(true);

  function toggleSidebar() {
    sidebarVisible = !sidebarVisible;
  }

  let chatWidth = $state(320);
  let gitWidth = $state(360);

  // --- Drag resize logic ---
  // The state machine (rAF coalescing, body-cursor toggle, listener
  // lifecycle) lives in modules/layout/panelResize.ts so it's testable
  // independently. The component just owns the three width cells.
  let dragging = $state<PanelTarget | null>(null);
  const panelResizer = createPanelResizer({
    sidebar: { min: 140, max: 500 },
    chat:    { min: 200, max: 600 },
    git:     { min: 260, max: 600 },
    setSidebarWidth: (w) => { sidebarWidth = w; },
    setChatWidth:    (w) => { chatWidth = w; },
    setGitWidth:     (w) => { gitWidth = w; },
    onDragStateChange: (t) => { dragging = t; },
  });
  const startDrag = (t: PanelTarget) => panelResizer.startDrag(t);

  // toggleTerminal() lives in `./lib/modules/terminalActions` so it's
  // shared between App.svelte (Cmd+`), TitleBar.svelte's terminal
  // button, and the status-bar terminal button. See that module for
  // the mode-aware behavior.

  let isClosing = false;

  // openSettingsWindow lives in lib/modules/ui/windows so the same
  // helper can be invoked from the toolbar, the command palette, or
  // any other launch point without re-implementing the focus-or-spawn
  // dance. We just keep the reactive subscription to `showSettings`
  // here because it's a Svelte-level concern.

  // Treat `showSettings` as an "open settings" trigger. Reset it after
  // launching so the Toolbar's toggle reads as off again.
  showSettings.subscribe((v) => {
    if (v) {
      openSettingsWindow();
      showSettings.set(false);
    }
  });

  let breadcrumbSegments = $derived(
    breadcrumbSegmentsFor($activeFilePath, $projectRoot)
  );

  function navigateBreadcrumb(path: string) {
    if (!sidebarVisible) toggleSidebar();
    fileTreeNavTarget.set(path);
  }

  onMount(async () => {
    // Install the fullscreen / platform-chrome watchers as early as
    // possible so the toolbar can render with the right padding from
    // the very first frame. The teardown isn't captured because this
    // component lives for the entire app lifetime.
    void installWindowChromeWatchers().catch(() => {});

    // Load API keys from OS keychain into stores
    const providers = ['openrouter', 'openai', 'anthropic'] as const;
    const storeMap = { openrouter: apiKey, openai: openaiApiKey, anthropic: anthropicApiKey } as const;
    async function loadKeys() {
      for (const provider of providers) {
        try {
          const key: string = await invoke('get_provider_key', { provider });
          storeMap[provider].set(key || '');
        } catch { /* keychain unavailable or empty */ }
      }
    }
    await loadKeys();

    // Reload keys when window regains focus (e.g. after closing settings)
    const onFocus = () => { loadKeys(); };
    window.addEventListener('focus', onFocus);

    // Initialize knowledge store when project root is set
    const unsubRoot = projectRoot.subscribe((root) => {
      if (root) {
        invoke('knowledge_init', { projectRoot: root }).then(() => {
          invoke('knowledge_index', { projectRoot: root }).catch(() => {});
        }).catch((e) => { log.warn('knowledge_init failed', e); });
        // Also persist for settings window
        localStorage.setItem('leo-project-root', root);
      }
    });
    // Load recent projects
    try {
      recentProjects = await getRecentProjects();
    } catch { /* ignore */ }

    // Handle restored sessions: if the user's saved `activeFilePath` is
    // a terminal sentinel but their current layout is panel mode, move
    // the editor off the terminal path once on startup. The mode-change
    // $effect above handles live flips; this is the one-shot equivalent
    // for cold start.
    if (get(terminalMode) === 'panel' && isTerminalPath(get(activeFilePath))) {
      activeFilePath.set(get(openFiles).at(-1)?.path ?? null);
    }
    // Save session on window close — await the save before destroying
    const appWindow = getCurrentWindow();
    await appWindow.onCloseRequested(async (event) => {
      if (isClosing) return;
      isClosing = true;
      event.preventDefault();
      const root = get(projectRoot);
      if (root) {
        try {
          // Save the AI conversation, then the editor session.
          await saveConversationNow();
          // Save session
          await Promise.race([
            saveSessionNow(root),
            new Promise((_, reject) =>
              setTimeout(() => reject(new Error('Save timeout')), 5000)
            ),
          ]);
        } catch (e) {
          log.error('Failed to save session on close', e);
        }
      }
      await appWindow.destroy();
    });
  });

  // Apply appearance mode (system/light/dark)
  $effect(() => {
    const mode = $appearanceMode;
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    if (mode === 'light') root.classList.add('light');
    else if (mode === 'dark') root.classList.add('dark');
    else {
      // system: use OS preference
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      root.classList.add(prefersDark ? 'dark' : 'light');
    }
  });

  // Apply UI font size
  $effect(() => {
    document.documentElement.style.fontSize = $uiFontSize + 'px';
  });

  // Apply UI density
  $effect(() => {
    document.documentElement.dataset.density = $uiDensity;
  });

  // ── Terminal mode transitions ──────────────────────────────────
  //
  // When the user flips between 'tab' ↔ 'panel' we wipe the slate:
  // hide the terminal surface and redirect the editor off any
  // terminal sentinel. PTY processes are killed by Terminal.svelte's
  // own `onDestroy` when its container unmounts as a result of the
  // mode change — we deliberately do NOT dispatch a kill signal here
  // because that would queue async work into the about-to-be-destroyed
  // component, which can then race against (and stomp on) a freshly
  // mounted Terminal instance the user opens immediately afterwards.
  //
  // We also do NOT auto-spawn a new terminal in the new container —
  // doing so in tab mode would steal `activeFilePath` (Terminal.svelte
  // routes it to the new terminal on create) and cover whatever file
  // the user was editing. The user opens a terminal themselves via
  // the status-bar button / Ctrl+` / the "+" menu when they want one.
  let prevTerminalMode: 'tab' | 'panel' | null = null;
  $effect(() => {
    const mode = $terminalMode;
    if (prevTerminalMode !== null && prevTerminalMode !== mode) {
      showTerminal.set(false);
      if (isTerminalPath(get(activeFilePath))) {
        activeFilePath.set(get(openFiles).at(-1)?.path ?? null);
      }
    }
    prevTerminalMode = mode;
  });

  // Auto-save session when open files or active file changes
  $effect(() => {
    // Subscribe to reactive stores
    const _ = $openFiles;
    const __ = $activeFile;
    scheduleSaveSession();
  });

  // Refresh recent projects when returning to the welcome screen
  $effect(() => {
    if ($activeFile === null) {
      showAllRecent = false;
      getRecentProjects().then(p => recentProjects = p).catch(() => {});
    }
  });

  $effect(() => {
    if ($openFileSearchSignal > 0) {
      showFileSearch = true;
    }
  });

  $effect(() => {
    if ($openDiagramSearchSignal > 0) {
      showDiagramSearch = true;
    }
  });

  // Keep preview alive once opened
  $effect(() => {
    if (isPreviewPath($activeFilePath)) showPreview.set(true);
  });

  function handleKeydown(e: KeyboardEvent) {
    dispatchAppShortcut(e);
  }

  /**
   * Match `e` against the app-level shortcut bindings and run the
   * corresponding action. Returns true if the event was handled (caller
   * is responsible for preventing default / stopping propagation as
   * appropriate for the phase it's running in).
   *
   * Editor (CodeMirror) and tab-system shortcuts like `editor.find` or
   * `file.save` are deliberately excluded — they're owned by the
   * focused control, not the app shell. Adding a new app-level
   * shortcut means: add the id to APP_LEVEL_SHORTCUT_IDS in
   * shortcuts.ts and add the action below. TypeScript enforces that
   * the action map covers every id (`Record<AppLevelShortcutId, …>`).
   */
  function dispatchAppShortcut(e: KeyboardEvent): boolean {
    const bindings = $shortcutBindings;

    const actions: Record<AppLevelShortcutId, () => void> = {
      'view.toggleTerminal': () => toggleTerminal(),
      'view.toggleChat':     () => toggleChatPanel(),
      'view.toggleGit':      () => toggleGitPanel(),
      'view.toggleSidebar':  () => toggleSidebar(),
      'view.openSettings':   () => showSettings.set(true),
      'file.search':         () => { showFileSearch = !showFileSearch; },
      'tabs.nextAlt':        () => nextTab(),
      'tabs.prevAlt':        () => prevTab(),
      'tabs.next':           () => nextTab(),
      'tabs.prev':           () => prevTab(),
    };

    for (const id of APP_LEVEL_SHORTCUT_IDS) {
      const binding = bindings[id];
      if (binding && eventMatchesBinding(e, binding)) {
        e.preventDefault();
        actions[id]();
        return true;
      }
    }
    return false;
  }

  /**
   * Capture-phase keydown listener installed on `window`.
   *
   * xterm.js installs its own keydown listener on a hidden helper
   * textarea inside the terminal and, by default, processes most
   * keystrokes locally (writing them to the PTY and calling
   * preventDefault + stopPropagation on the way out). That means the
   * bubble-phase `<svelte:window onkeydown>` listener below never sees
   * the event when the terminal has focus — so without this capture
   * listener, opening a terminal silently disables every app-level
   * shortcut (Ctrl+Tab, Ctrl+`, Cmd+B, Cmd+L, Cmd+G, Cmd+Shift+], …).
   *
   * Why capture phase: events flow window → … → xterm-helper-textarea
   * during the capture phase. Listening at the window with
   * `{ capture: true }` puts us first in line, before xterm.js can
   * call preventDefault/stopPropagation. This is the same pattern used
   * by terax-ai's `useGlobalShortcuts`.
   *
   * Why scoped to terminal targets: many app shortcuts share key
   * combinations with editor / chat keymaps (Cmd+G = "find next" in
   * CodeMirror, Cmd+L = "select line", Cmd+Shift+] / [ = indent more
   * / less). When the editor is focused, the editor's keymap MUST win
   * — so we only intercept events whose target is inside the terminal
   * area. Anything else falls through to the editor's local handler
   * and (if it doesn't preventDefault) to the bubble-phase listener.
   *
   * Why stopImmediatePropagation: prevents the event from also being
   * delivered to the bubble-phase window handler later, which would
   * fire the action a second time.
   */
  function handleKeydownCapture(e: KeyboardEvent) {
    // Don't intercept events that are part of an active IME composition
    // sequence (e.g. CJK input). Today none of the app-level shortcuts
    // could overlap with composition because they all require a
    // non-letter modifier combo, but bare-letter shortcuts could be
    // added in the future and this guard makes that safe.
    if (e.isComposing) return;
    const target = e.target;
    if (!(target instanceof Element)) return;
    if (!target.closest('.terminal-content')) return;
    if (dispatchAppShortcut(e)) {
      e.stopImmediatePropagation();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydownCapture, { capture: true });
    return () => {
      window.removeEventListener('keydown', handleKeydownCapture, { capture: true });
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="ide-layout" class:mac-traffic-lights={isMac && !$isFullscreen}>
  <TitleBar {sidebarVisible} onToggleSidebar={toggleSidebar} />
  <Toolbar
    onOpenProject={openProjectFromToolbar}
    onOpenFolderDialog={openFolderDialog}
    onSearchFiles={() => { showFileSearch = !showFileSearch; }}
    onNewFile={() => createFileSignal.update(n => n + 1)}
    onNewFolder={() => createFolderSignal.update(n => n + 1)}
  />
  <div class="ide-top">
    <div class="sidebar" class:hidden={!sidebarVisible} style="width: {sidebarWidth}px">
      <FileTree onFileSelect={(path, name) => addFile(path, name)} onSearchFiles={() => showFileSearch = true} onOpenFolder={handleOpenFolder} />
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="resize-handle resize-handle-col" class:hidden={!sidebarVisible} onmousedown={startDrag('sidebar')}></div>

    <div class="main-area">
      <div class="editor-col">
        <div class="editor-area" style="flex: 1; min-height: 0;">
          <div class="editor-container">
            <!--
              Tab-mode terminal slot: visible only when the user has chosen
              the legacy in-tab terminal layout. In panel mode the terminal
              is rendered by <TerminalPanel /> below and this slot is absent.
              Kept in the DOM (visibility:hidden when unfocused) so PTY
              sessions survive tab switches within tab mode.
            -->
            {#if $terminalMode === 'tab' && $showTerminal}
              <div class="terminal-tab-slot" class:focused={$showTerminal && isTerminalPath($activeFilePath)}>
                <Terminal />
              </div>
            {/if}
            <!-- Preview tab: kept alive like terminal -->
            {#if $showPreview}
              <div class="terminal-tab-slot" class:focused={isPreviewPath($activeFilePath)}>
                <Preview />
              </div>
            {/if}
            <!-- Diagram tab -->
            {#if isDiagramPath($activeFilePath)}
              <div class="terminal-tab-slot focused">
                <FileDiagram filePath={getDiagramFilePath($activeFilePath ?? '')} />
              </div>
            {/if}
            <!-- File editor — hidden while a terminal or preview tab is focused -->
            {#if !($showTerminal && isTerminalPath($activeFilePath)) && !isPreviewPath($activeFilePath) && !isDiagramPath($activeFilePath)}
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
                  <p> [  W E L C O M E  ] </p>
                  {#if recentProjects.length > 0}
                    <div class="recent-projects">
                      <p style="font-size: 12px; margin-bottom: 8px; color: var(--text-secondary);">Recent Projects</p>
                      {#each (showAllRecent ? recentProjects : recentProjects.slice(0, 3)) as project}
                        <button class="recent-item" onclick={() => openRecentProject(project)}>
                          <span class="recent-name">{project.name}</span>
                          <span class="recent-path">{project.path}</span>
                        </button>
                      {/each}
                      {#if recentProjects.length > 3}
                        <button class="show-more-btn" onclick={() => showAllRecent = !showAllRecent}>
                          {showAllRecent ? 'Show less' : `Show more (${recentProjects.length - 3})`}
                        </button>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}
            {/if}
          </div>
        </div>

      </div>
    </div>

    {#if $showChat}
      <!-- Floating chat rendered below -->
    {/if}

    {#if $showGit}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle resize-handle-col" onmousedown={startDrag('git')}></div>
      <div class="git-panel-container" style="width: {gitWidth}px">
        <div class="panel-header">
          <span>Source Control</span>
          <button onclick={toggleGitPanel}>✕</button>
        </div>
        <GitPanel />
      </div>
    {/if}


    {#if showFileSearch}
      <FileSearch onClose={() => showFileSearch = false} />
    {/if}

    {#if showDiagramSearch}
      <FileSearch onClose={() => showDiagramSearch = false} onSelect={(relPath) => {
        const root = get(projectRoot);
        if (!root) return;
        const fullPath = `${root}/${relPath}`;
        openDiagrams.update(d => d.includes(fullPath) ? d : [...d, fullPath]);
        activeFilePath.set(diagramPath(fullPath));
      }} />
    {/if}
  </div>

  {#if $terminalMode === 'panel'}
    <!--
      Bottom-docked terminal panel (VSCode / Xcode / Zed style). The panel
      stays in the DOM while in panel mode so xterm + PTY state survives
      visibility toggles; the component CSS-hides itself when
      `$showTerminal` is false.
    -->
    <TerminalPanel />
  {/if}

  <div class="statusbar">
    <div class="statusbar-left">
      {#if isTerminalPath($activeFilePath) || isPreviewPath($activeFilePath)}
        <span class="breadcrumb-plain">{isTerminalPath($activeFilePath) ? 'Terminal' : 'Preview'}</span>
      {:else if isDiagramPath($activeFilePath)}
        <span class="breadcrumb-plain">Diagram: {getDiagramFilePath($activeFilePath ?? '').split('/').pop()}</span>
      {:else if breadcrumbSegments.length > 0}
        <div class="breadcrumb">
          {#each breadcrumbSegments as seg, i}
            <span class="breadcrumb-seg" role="button" tabindex="0" onclick={() => navigateBreadcrumb(seg.path)} onkeydown={(e) => e.key === 'Enter' && navigateBreadcrumb(seg.path)}>
              {#if i === 0}
                <span class="breadcrumb-dot"></span>
              {/if}
              {seg.name}
            </span>
            {#if i < breadcrumbSegments.length - 1}
              <span class="breadcrumb-sep">›</span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
    <div class="statusbar-right">
      {#if $terminalMode === 'panel'}
        <button
          class="statusbar-terminal-btn"
          class:active={$showTerminal}
          onclick={toggleTerminal}
          title="Toggle terminal (Ctrl+`)"
          aria-label="Toggle terminal panel"
          aria-pressed={$showTerminal}
        >
          <TerminalSquare size={12} />
        </button>
      {/if}
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
      <button class="statusbar-ai-btn" class:active={$showChat} onclick={() => showChat.update(v => !v)} title="Leo AI (Ctrl+L)">
        <Sparkles size={13} />
      </button>
    </div>
  </div>
</div>

<FloatingChat />
<Toast />

<style>
  .ide-layout {
    display: grid;
    /*
     * Row layout:
     *   1. toolbar / tab bar
     *   2. main content (minmax(0, 1fr) — absorbs remaining height, but is
     *      also allowed to shrink to zero so the terminal panel can take
     *      as much vertical space as the user drags it to)
     *   3. terminal panel (auto — collapses to 0 when not rendered)
     *   4. statusbar
     *
     * `minmax(0, 1fr)` instead of plain `1fr` is critical: a bare `1fr`
     * is really `minmax(auto, 1fr)`, which refuses to shrink below the
     * main content's min-content height. When the terminal panel gets
     * tall, that would push the statusbar out of the viewport (clipped
     * by `overflow: hidden` on this container) — observed as the
     * breadcrumbs row "moving" / disappearing.
     *
     * Each direct child is also pinned to an explicit `grid-row` below
     * (see selectors). That prevents a very subtle failure mode where
     * auto-flow would collapse positions when the panel row is unused:
     *   - In tab mode there is no TerminalPanel child, only 3 elements.
     *   - In panel mode with the panel hidden, its <section> has
     *     `display: none`, so it's excluded from grid placement.
     * Without explicit pinning, the statusbar would land in row 3
     * (the `auto` panel row), leaving row 4 as a 24px empty gap and
     * appearing "pushed up" off the bottom.
     */
    grid-template-rows:
      [titlebar] var(--density-titlebar-height, 32px)
      [toolbar]  var(--density-tabs-height, 36px)
      [main]     minmax(0, 1fr)
      [panel]    auto
      [status]   var(--density-statusbar-height, 24px);
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  /*
   * Pin every direct grid child to its intended row by named grid lines.
   * This makes the layout robust to conditional rendering and to any
   * component hiding itself via `display: none`. The named lines above
   * in `grid-template-rows` are what these selectors reference.
   */
  .ide-layout > :global(.title-bar)      { grid-row: titlebar; }
  .ide-layout > :global(.toolbar)        { grid-row: toolbar; }
  .ide-layout > .ide-top                 { grid-row: main; }
  .ide-layout > :global(.terminal-panel) { grid-row: panel; }
  .ide-layout > .statusbar               { grid-row: status; }

  /*
   * macOS title-bar integration.
   *
   * With `titleBarStyle: "Overlay"` set in tauri.conf.json, the system
   * traffic-light buttons (close / minimize / maximize) float over the
   * top-left of our content. Reserve horizontal space at the start of
   * the title bar so they don't collide with our first button.
   *
   * 78px = ~70px for the three traffic-light buttons plus an 8px gap.
   * Matches VSCode and Zed's macOS layouts.
   *
   * In fullscreen the system hides the traffic lights, so we drop the
   * padding to reclaim the full title-bar width — the
   * `.mac-traffic-lights` class is removed by App.svelte when
   * `$isFullscreen` flips true.
   *
   * The drag region itself lives entirely on .title-bar (set inside the
   * TitleBar component). We do NOT mark .toolbar (the tab bar below) as
   * draggable — tabs occupy almost all of its width, so the user
   * effectively wouldn't have any empty area to grab anyway, and
   * accidental drags from in-between-tabs gaps would be confusing.
   */
  .ide-layout.mac-traffic-lights > :global(.title-bar) {
    padding-left: 78px;
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
    flex-direction: row;
    min-width: 0;
    min-height: 0;
  }

  .editor-col {
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
    position: relative;
  }

  .terminal-tab-slot {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    visibility: hidden;
    pointer-events: none;
    z-index: 1;
  }

  .terminal-tab-slot.focused {
    visibility: visible;
    pointer-events: auto;
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

  .recent-projects {
    display: flex;
    flex-direction: column;
    width: 340px;
    gap: 4px;
    margin-bottom: 12px;
    max-height: 240px;
    overflow-y: auto;
  }

  .recent-item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 12px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s;
  }

  .recent-item:hover {
    border-color: var(--accent);
  }

  .recent-name {
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 600;
  }

  .recent-path {
    color: var(--text-muted);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }

  .show-more-btn {
    font-size: 12px;
    color: var(--text-muted);
    padding: 6px 12px;
    border-radius: 6px;
    cursor: pointer;
    transition: color 0.15s;
  }

  .show-more-btn:hover {
    color: var(--accent);
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
    color: var(--statusbar-text, #E8E2D5);
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

  .statusbar-ai-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: var(--statusbar-text, #E8E2D5);
    cursor: pointer;
    transition: background 0.15s;
    margin-left: 8px;
  }

  .statusbar-ai-btn:hover {
    background: var(--bg-surface);
    color: var(--statusbar-text, #E8E2D5);
  }

  .statusbar-ai-btn.active {
    color: var(--statusbar-text, #E8E2D5);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  /*
   * Bottom-right terminal toggle. Only rendered in panel mode; kept
   * close to the AI button so the two "utility toggles" cluster visually
   * on the right edge, matching VSCode/Zed conventions.
   */
  .statusbar-terminal-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 7px;
    height: 20px;
    border-radius: 4px;
    color: inherit;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    background: transparent;
    border: none;
    transition: background 0.15s ease;
  }
  .statusbar-terminal-btn:hover {
    background: color-mix(in srgb, currentColor 18%, transparent);
  }
  .statusbar-terminal-btn.active {
    background: color-mix(in srgb, currentColor 26%, transparent);
  }
  .statusbar-terminal-btn:focus-visible {
    outline: 2px solid currentColor;
    outline-offset: -2px;
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 10.5px;
    min-width: 0;
    overflow: hidden;
    padding-left: 8px;
  }

  .breadcrumb-seg {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 0;
    white-space: nowrap;
    font-size: 10.5px;
    font-weight: 400;
    color: inherit;
    cursor: pointer;
    transition: opacity 0.1s;
  }

  .breadcrumb-seg:hover {
    opacity: 0.7;
  }

  .breadcrumb-dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--settings-icon, #B34B3C);
    flex-shrink: 0;
  }

  .breadcrumb-sep {
    opacity: 0.75;
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
    color: inherit;
  }

  .breadcrumb-plain {
    font-size: 11.5px;
    color: var(--text-muted);
    padding: 0 8px;
  }
</style>
