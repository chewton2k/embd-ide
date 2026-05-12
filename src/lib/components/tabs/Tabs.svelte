<script lang="ts">
  import Icon from '@iconify/svelte';
  import { TerminalSquare, Plus, FolderOpen, Eye, RefreshCw } from 'lucide-svelte';
  import { openFiles, activeFilePath, closeFile, togglePin, pinnedFiles, unpinnedFiles, sharedGitStatus, terminalSessions, killTerminalSignal, isTerminalPath, isPreviewPath, PREVIEW_PATH, showPreview, showTerminal, terminalPath, createTerminalSignal, openFileSearchSignal } from '../../modules/stores';
  import { triggerFileTreeRefresh } from '../../modules/stores';
  import { getFileIconName } from '../../modules/fileIcons';

  let tabsBar: HTMLDivElement | undefined = $state();
  let addMenuOpen = $state(false);
  let addMenuPos = $state<{ top: number; right: number } | null>(null);
  let addBtnEl: HTMLButtonElement | undefined = $state();
  let ctxMenu = $state<{ x: number; y: number; path: string; pinned: boolean } | null>(null);

  function switchTab(path: string) { activeFilePath.set(path); }

  function handleClose(e: MouseEvent, path: string) {
    e.stopPropagation();
    closeFile(path);
  }

  function handleTabContext(e: MouseEvent, path: string, pinned: boolean) {
    e.preventDefault();
    e.stopPropagation();
    ctxMenu = { x: e.clientX, y: e.clientY, path, pinned };
  }

  function closeCtx() { ctxMenu = null; }

  function ctxPin() {
    if (ctxMenu) togglePin(ctxMenu.path);
    ctxMenu = null;
  }

  function ctxClose() {
    if (ctxMenu) closeFile(ctxMenu.path);
    ctxMenu = null;
  }

  $effect(() => {
    const activePath = $activeFilePath;
    if (!tabsBar || !activePath) return;
    requestAnimationFrame(() => {
      const activeEl = tabsBar?.querySelector('.tab.active');
      if (activeEl) activeEl.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
    });
  });

  function handleWheel(e: WheelEvent) {
    if (tabsBar) { e.preventDefault(); tabsBar.scrollLeft += e.deltaY; }
  }

  function openAddMenu() {
    const rect = addBtnEl?.getBoundingClientRect();
    if (rect) addMenuPos = { top: rect.bottom + 2, right: Math.max(8, window.innerWidth - rect.right) };
    addMenuOpen = !addMenuOpen;
  }

  function handleDocumentClick(e: MouseEvent) {
    if (addMenuOpen && addBtnEl && !addBtnEl.contains(e.target as Node)) {
      const menu = document.querySelector('.tab-add-menu');
      if (!menu?.contains(e.target as Node)) addMenuOpen = false;
    }
    if (ctxMenu) closeCtx();
  }

  function openExistingFileTab() { openFileSearchSignal.update(n => n + 1); addMenuOpen = false; }
  function openPreviewTab() { activeFilePath.set(PREVIEW_PATH); addMenuOpen = false; }
  function openTerminalTab() {
    $showTerminal = true;
    if ($terminalSessions.length === 0) createTerminalSignal.update(n => n + 1);
    else activeFilePath.set(terminalPath());
    addMenuOpen = false;
  }
</script>

<svelte:document onclick={handleDocumentClick} />

<div class="tabs-bar" bind:this={tabsBar} onwheel={handleWheel}>
  {#each [...$pinnedFiles, ...$unpinnedFiles] as file}
    <div
      class="tab"
      class:active={$activeFilePath === file.path && !isTerminalPath($activeFilePath)}
      class:pinned={file.pinned}
      class:conflict={$sharedGitStatus[file.path] === 'C'}
      role="tab"
      tabindex="0"
      title={file.path}
      onclick={() => switchTab(file.path)}
      onkeydown={(e) => e.key === 'Enter' && switchTab(file.path)}
      oncontextmenu={(e) => handleTabContext(e, file.path, file.pinned)}
    >
      <Icon class="tab-icon" icon={getFileIconName(file.name)} width={14} height={14} />
      <span class="tab-name">
        {#if file.modified}<span class="modified-dot"></span>{/if}
        {file.name}
      </span>
      {#if file.pinned}
        <span class="pin-dot" title="Pinned"></span>
      {:else}
        <button class="tab-close" onclick={(e) => handleClose(e, file.path)}>×</button>
      {/if}
    </div>
  {/each}

  {#if $showTerminal && $terminalSessions.length > 0}
    <div
      class="tab terminal-tab"
      class:active={isTerminalPath($activeFilePath)}
      role="tab"
      tabindex="0"
      title="Terminal"
      onclick={() => activeFilePath.set(terminalPath())}
      onkeydown={(e) => e.key === 'Enter' && activeFilePath.set(terminalPath())}
    >
      <TerminalSquare size={13} />
      <span class="tab-name">Terminal</span>
      <button class="tab-close" onclick={(e) => { e.stopPropagation(); killTerminalSignal.set('all'); }}>×</button>
    </div>
  {/if}

  {#if $showPreview}
    <div
      class="tab"
      class:active={isPreviewPath($activeFilePath)}
      role="tab"
      tabindex="0"
      title="Web Preview"
      onclick={() => activeFilePath.set(PREVIEW_PATH)}
      onkeydown={(e) => e.key === 'Enter' && activeFilePath.set(PREVIEW_PATH)}
    >
      <Eye size={13} />
      <span class="tab-name">Preview</span>
      <button class="tab-close" onclick={(e) => { e.stopPropagation(); showPreview.set(false); activeFilePath.set($openFiles.at(-1)?.path ?? null); }}>×</button>
    </div>
  {/if}

  <div class="tab-actions">
    <button type="button" class="tab-action-btn" onclick={() => triggerFileTreeRefresh()} title="Reload file tree" aria-label="Reload file tree">
      <RefreshCw size={12} />
    </button>
    <button type="button" class="tab-action-btn" bind:this={addBtnEl} onclick={openAddMenu} title="New tab" aria-label="New tab" aria-expanded={addMenuOpen}>
      <Plus size={13} />
    </button>
  </div>
</div>

{#if addMenuOpen && addMenuPos}
  <div class="tab-add-menu" role="menu" style="top: {addMenuPos.top}px; right: {addMenuPos.right}px;">
    <button class="tab-add-menu-item" role="menuitem" onclick={openExistingFileTab}>
      <FolderOpen size={12} /> <span>Open File</span>
    </button>
    <button class="tab-add-menu-item" role="menuitem" onclick={openPreviewTab}>
      <Eye size={12} /> <span>Preview</span>
    </button>
    <button class="tab-add-menu-item" role="menuitem" onclick={openTerminalTab}>
      <TerminalSquare size={12} /> <span>Terminal</span>
    </button>
  </div>
{/if}

{#if ctxMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ctx-backdrop" role="presentation" onclick={closeCtx} onkeydown={(e) => e.key === 'Escape' && closeCtx()}></div>
  <div class="ctx-menu" role="menu" style="left:{ctxMenu.x}px;top:{ctxMenu.y}px">
    <button class="ctx-item" role="menuitem" onclick={ctxPin}>
      {ctxMenu.pinned ? 'Unpin tab' : 'Pin tab'}
    </button>
    {#if !ctxMenu.pinned}
      <button class="ctx-item" role="menuitem" onclick={ctxClose}>Close tab</button>
    {/if}
  </div>
{/if}

<style>
  .tabs-bar {
    display: flex;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    overflow-x: auto;
    overflow-y: hidden;
    flex-shrink: 0;
    height: var(--density-tabs-height, 36px);
    max-width: 100%;
    scrollbar-width: none;
  }
  .tabs-bar::-webkit-scrollbar { height: 0; display: none; }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    white-space: nowrap;
    background: color-mix(in srgb, var(--tab-inactive) 82%, transparent);
    flex-shrink: 0;
    min-width: 0;
    max-width: 160px;
    cursor: pointer;
    height: calc(100% - 8px);
    margin: 4px 3px 4px 0;
    border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    border-radius: 20px;
  }
  .tab:hover {
    background: color-mix(in srgb, var(--bg-surface) 88%, transparent);
    border-color: color-mix(in srgb, var(--border) 95%, transparent);
    color: var(--text-primary);
  }
  .tab.active {
    background: var(--tab-active);
    color: var(--text-primary);
    font-weight: 600;
    border-color: var(--border);
  }
  .tab.pinned { opacity: 0.9; }
  .tab.conflict { border-color: color-mix(in srgb, var(--error) 40%, var(--border)); }

  :global(.tab-icon) { flex-shrink: 0; }

  .tab-name {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .modified-dot {
    width: 6px; height: 6px; border-radius: 50%;
    background: var(--warning); display: inline-block; flex-shrink: 0;
  }
  .tab-close {
    font-size: 14px; line-height: 1; color: var(--text-muted);
    border-radius: 3px; padding: 0 2px; flex-shrink: 0;
  }
  .tab-close:hover { background: var(--bg-surface); color: var(--error); }

  .pin-dot {
    width: 6px; height: 6px; border-radius: 50%;
    background: var(--text-primary); flex-shrink: 0;
    opacity: 0.7;
  }

  .tab-actions {
    position: sticky;
    right: 0;
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    background: linear-gradient(90deg, transparent 0%, var(--bg-secondary) 12px);
    padding-left: 14px;
    padding-right: 6px;
    z-index: 2;
  }
  .tab-action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px; height: 26px;
    border-radius: 5px;
    color: var(--text-muted);
    cursor: pointer;
  }
  .tab-action-btn:hover { background: var(--bg-surface); color: var(--text-primary); }

  .tab-add-menu {
    position: fixed;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 4px;
    z-index: 220;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 140px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.28);
  }
  .tab-add-menu-item {
    display: flex; align-items: center; gap: 8px;
    padding: 7px 9px; border-radius: 5px;
    font-size: 11px; color: var(--text-secondary);
    text-align: left; white-space: nowrap;
  }
  .tab-add-menu-item:hover { background: var(--bg-tertiary); color: var(--text-primary); }

  .ctx-backdrop { position: fixed; inset: 0; z-index: 999; }
  .ctx-menu {
    position: fixed; z-index: 1000;
    background: var(--bg-secondary); border: 1px solid var(--border);
    border-radius: 8px; padding: 4px; min-width: 120px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.3);
  }
  .ctx-item {
    display: flex; align-items: center; gap: 8px;
    width: 100%; padding: 7px 10px; border-radius: 5px;
    font-size: 12px; color: var(--text-primary); cursor: pointer;
  }
  .ctx-item:hover { background: var(--bg-surface); }
</style>
