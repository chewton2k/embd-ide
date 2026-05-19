<script lang="ts">
  /**
   * Top-level window title bar (VSCode / Zed / Xcode style).
   *
   * Responsibilities:
   *   1. Hosts the small set of always-visible global controls — sidebar
   *      toggle, terminal toggle, source-control toggle, quick-search,
   *      settings.
   *   2. Provides a draggable background so the user can move the window
   *      from any empty area, the same way a native title bar works.
   *   3. Reserves horizontal space for the macOS traffic-light buttons
   *      when the window is not fullscreen (the parent layout sets the
   *      `mac-traffic-lights` class on the layout root).
   *
   * Tabs intentionally live in a SEPARATE bar below this one. Crowding
   * tabs into the same row would leave the user with no draggable area
   * once a few tabs are open — that was the original bug report.
   */
  import {
    SidebarOpen, SidebarClose, GitBranch, Settings2, Search, TerminalSquare,
  } from 'lucide-svelte';
  import {
    showSettings, showGit, gitBranch, toggleGitPanel,
    showTerminal,
    openFileSearchSignal,
  } from '../../modules';
  import { toggleTerminal } from '../../modules/terminal';

  let { sidebarVisible, onToggleSidebar }: {
    sidebarVisible: boolean;
    onToggleSidebar: () => void;
  } = $props();

  function openFileSearch() {
    openFileSearchSignal.update(n => n + 1);
  }
</script>

<!--
  `data-tauri-drag-region` is Tauri v2's documented mechanism for
  marking an element as draggable for native window movement. The CSS
  `-webkit-app-region: drag` rule below is kept as a belt-and-suspenders
  fallback for non-macOS platforms; on macOS with `titleBarStyle:
  "Overlay"` only the data attribute is reliable.
-->
<header
  class="title-bar"
  data-tauri-drag-region
  aria-label="Window title bar"
>
  <!-- Left cluster — sidebar toggle plus a thin divider. -->
  <div class="cluster cluster-left">
    <button
      type="button"
      class="title-btn sidebar-toggle"
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
  </div>

  <!--
    Drag region between the two clusters. Tauri's runtime listens for
    mousedown on elements with `data-tauri-drag-region`; buttons inside
    the clusters don't carry the attribute, so click events on them
    fire normally.
  -->
  <div class="drag-spacer" data-tauri-drag-region aria-hidden="true"></div>

  <!-- Center — search bar -->
  <div class="cluster cluster-center">
    <div
      class="title-search"
      role="button"
      tabindex="0"
      aria-label="Quick open file"
      onclick={openFileSearch}
      onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && openFileSearch()}
      title="Quick open file (Cmd+O)"
    >
      <Search size={11} />
      <span>Search files…</span>
      <kbd>⌘O</kbd>
    </div>
  </div>

  <div class="drag-spacer" data-tauri-drag-region aria-hidden="true"></div>

  <!-- Right cluster — global toggles. -->
  <div class="cluster cluster-right">
    <button
      type="button"
      class="title-btn"
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
      class="title-btn settings-btn"
      class:active={$showSettings}
      onclick={() => showSettings.update(v => !v)}
      title="Settings"
      aria-label="Settings"
      aria-pressed={$showSettings}
    >
      <Settings2 size={14} />
    </button>
  </div>
</header>

<style>
  .title-bar {
    display: flex;
    align-items: center;
    height: var(--density-titlebar-height, 32px);
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    /*
     * Default drag region for the bar's background. Specific interactive
     * controls below opt OUT via `-webkit-app-region: no-drag`. This
     * pattern matches Electron and Tauri's recommended approach.
     */
    -webkit-app-region: drag;
    user-select: none;
    -webkit-user-select: none;
  }

  .cluster {
    display: flex;
    align-items: center;
    height: 100%;
    flex-shrink: 0;
    /* Clusters themselves stay no-drag so their padding doesn't trap
       attempted drags inside them; the spacer between them is the
       primary drag handle. */
    -webkit-app-region: no-drag;
  }

  .cluster-left {
    padding: 0 6px;
  }
  .cluster-right {
    padding: 0 8px;
    gap: 2px;
  }

  .cluster-center {
    display: flex;
    align-items: center;
    -webkit-app-region: no-drag;
  }

  .drag-spacer {
    flex: 1;
    height: 100%;
    /* Inherits the parent's drag region; nothing else to do here. */
  }

  .title-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 4px 8px;
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 11px;
    background: transparent;
    cursor: pointer;
    transition: color 0.1s ease, background 0.1s ease;
    -webkit-app-region: no-drag;
  }
  .title-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }
  .title-btn.active {
    color: var(--text-primary);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .title-btn:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: -2px;
  }

  .branch-label {
    font-size: 10.5px;
    max-width: 90px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title-search {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 3px 14px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 14px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.1s ease, color 0.1s ease;
    min-width: 340px;
    justify-content: space-between;
    -webkit-app-region: no-drag;
  }
  .title-search:hover {
    border-color: var(--text-muted);
    color: var(--text-secondary);
  }
  .title-search:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: -2px;
  }
  .title-search kbd {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--bg-surface);
    color: var(--text-primary);
    margin-left: 4px;
    font-family: var(--font-ui);
    font-weight: 700;
    letter-spacing: 0.4px;
  }

  .settings-btn {
    color: var(--settings-icon, #B34B3C);
  }
  .settings-btn:hover {
    color: var(--settings-icon, #B34B3C);
  }
</style>
