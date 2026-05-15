<script lang="ts">
  /**
   * Tab bar — sits below the TitleBar. Hosts the open-file / terminal /
   * preview / diagram tabs (delegated to <Tabs />), plus a contextual
   * split / collapse control for the active terminal tab.
   *
   * Also includes a Zed-style project switcher at the left: clicking the
   * project name opens a dropdown with a search bar, recent projects, and
   * an "Open Folder" button to switch the workspace root.
   */
  import Tabs from '../tabs/Tabs.svelte';
  import { SplitSquareVertical, PanelLeft, FolderOpen, ChevronDown, Search, FilePlus2, FolderPlus } from 'lucide-svelte';
  import {
    showTerminal, activeFilePath, panesInActiveTab,
    activeTerminalTabId, splitTerminalSignal, collapseTerminalSplitsSignal,
    terminalPath, projectRoot,
  } from '../../modules';
  import { getRecentProjects, removeRecentProject, type RecentProject } from '../../modules/session';
  import { exists } from '@tauri-apps/plugin-fs';

  interface Props {
    onOpenProject?: (path: string) => void;
    onOpenFolderDialog?: () => void;
    onSearchFiles?: () => void;
    onNewFile?: () => void;
    onNewFolder?: () => void;
  }
  let { onOpenProject, onOpenFolderDialog, onSearchFiles, onNewFile, onNewFolder }: Props = $props();

  // ── Split button ─────────────────────────────────────────────────

  let splitMenuOpen = $state(false);
  let splitMenuPos = $state<{ top: number; left: number } | null>(null);
  let splitBtnEl: HTMLDivElement | undefined = $state();

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

  // ── Project switcher dropdown ────────────────────────────────────

  let projectDropdownOpen = $state(false);
  let recentProjects = $state<RecentProject[]>([]);
  let searchQuery = $state('');
  let searchInputEl: HTMLInputElement | undefined = $state();
  let dropdownEl: HTMLDivElement | undefined = $state();

  const projectName = $derived($projectRoot?.split('/').pop() ?? 'No Project');

  const filteredProjects = $derived(
    searchQuery.trim()
      ? recentProjects.filter(p =>
          p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          p.path.toLowerCase().includes(searchQuery.toLowerCase())
        )
      : recentProjects
  );

  async function toggleProjectDropdown() {
    if (projectDropdownOpen) {
      projectDropdownOpen = false;
      return;
    }
    recentProjects = await getRecentProjects();
    searchQuery = '';
    projectDropdownOpen = true;
    requestAnimationFrame(() => searchInputEl?.focus());
  }

  async function selectProject(project: RecentProject) {
    const folderExists = await exists(project.path);
    if (!folderExists) {
      await removeRecentProject(project.path);
      recentProjects = recentProjects.filter(p => p.path !== project.path);
      return;
    }
    projectDropdownOpen = false;
    onOpenProject?.(project.path);
  }

  function openFolderFromDropdown() {
    projectDropdownOpen = false;
    onOpenFolderDialog?.();
  }

  // ── Click-outside handling ───────────────────────────────────────

  function handleDocumentClick(e: MouseEvent) {
    if (splitMenuOpen && splitBtnEl && !splitBtnEl.contains(e.target as Node)) {
      splitMenuOpen = false;
    }
    if (projectDropdownOpen && dropdownEl && !dropdownEl.contains(e.target as Node)) {
      const trigger = document.querySelector('.project-switcher');
      if (!trigger?.contains(e.target as Node)) {
        projectDropdownOpen = false;
      }
    }
  }
</script>

<svelte:document onclick={handleDocumentClick} />

<div class="toolbar">
  <!-- Project switcher (Zed-style) -->
  <button
    type="button"
    class="project-switcher"
    class:open={projectDropdownOpen}
    onclick={toggleProjectDropdown}
    title="Switch project"
    aria-expanded={projectDropdownOpen}
    aria-haspopup="true"
  >
    <FolderOpen size={13} />
    <span class="project-name">{projectName}</span>
    <ChevronDown size={11} class="chevron" />
  </button>

  {#if projectDropdownOpen}
    <div class="project-dropdown" bind:this={dropdownEl} role="menu">
      <div class="dropdown-search">
        <Search size={12} class="search-icon" />
        <input
          bind:this={searchInputEl}
          bind:value={searchQuery}
          type="text"
          class="dropdown-search-input"
          placeholder="Search projects..."
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
        />
      </div>
      <div class="dropdown-list">
        {#each filteredProjects as project}
          <button
            class="dropdown-item"
            class:active={project.path === $projectRoot}
            role="menuitem"
            onclick={() => selectProject(project)}
          >
            <span class="dropdown-item-name">{project.name}</span>
            <span class="dropdown-item-path">{project.path.replace(/^\/Users\/[^/]+/, '~')}</span>
          </button>
        {:else}
          <div class="dropdown-empty">
            {searchQuery ? 'No matching projects' : 'No recent projects'}
          </div>
        {/each}
      </div>
      <div class="dropdown-footer">
        <button class="dropdown-open-btn" onclick={openFolderFromDropdown}>
          <FolderOpen size={12} />
          Open Folder...
        </button>
      </div>
    </div>
  {/if}

  <!-- File actions -->
  {#if $projectRoot}
    <div class="file-actions">
      <button type="button" class="toolbar-btn" title="Search files (⌘O)" onclick={() => onSearchFiles?.()}>
        <Search size={13} />
      </button>
    </div>
  {/if}

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

  /* ── Project switcher ─────────────────────────────────────────── */

  .project-switcher {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    height: 100%;
    border-right: 1px solid var(--border);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }
  .project-switcher:hover, .project-switcher.open {
    background: var(--bg-surface);
    color: var(--text-primary);
  }
  .project-name {
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .project-switcher :global(.chevron) {
    opacity: 0.5;
    transition: transform 0.15s;
  }
  .project-switcher.open :global(.chevron) {
    transform: rotate(180deg);
  }

  .project-dropdown {
    position: fixed;
    top: calc(var(--density-titlebar-height, 32px) + var(--density-tabs-height, 36px));
    left: 0;
    width: 300px;
    max-height: 400px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
    z-index: 300;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    margin: 4px 0 0 4px;
  }

  .dropdown-search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .dropdown-search :global(.search-icon) {
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .dropdown-search-input {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    padding: 0;
  }
  .dropdown-search-input::placeholder {
    color: var(--text-muted);
  }

  .dropdown-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
    max-height: 280px;
  }

  .dropdown-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 8px 10px;
    border-radius: 6px;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }
  .dropdown-item:hover {
    background: var(--bg-surface);
  }
  .dropdown-item.active {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }
  .dropdown-item-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
  }
  .dropdown-item-path {
    font-size: 10px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dropdown-empty {
    padding: 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 11px;
  }

  .dropdown-footer {
    border-top: 1px solid var(--border);
    padding: 6px 8px;
  }
  .dropdown-open-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }
  .dropdown-open-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  /* ── Tabs + split ─────────────────────────────────────────────── */

  .file-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 6px;
    height: 100%;
    border-right: 1px solid var(--border);
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
