<script lang="ts">
  import { TerminalSquare, Plus, FolderOpen, Eye, Pin, PinOff } from 'lucide-svelte';
  import { openFiles, activeFilePath, closeFile, togglePin, pinnedFiles, unpinnedFiles, sharedGitStatus, terminalSessions, killTerminalSignal, isTerminalPath, showTerminal, terminalPath, createTerminalSignal, openFileSearchSignal, openPreviewSignal } from '../../modules/stores';

  let tabsBar: HTMLDivElement | undefined = $state();
  let addMenuOpen = $state(false);
  let addMenuPos = $state<{ top: number; right: number } | null>(null);
  let addBtnEl: HTMLButtonElement | undefined = $state();

  function switchTab(path: string) {
    activeFilePath.set(path);
  }

  function handleClose(e: MouseEvent, path: string) {
    e.stopPropagation();
    closeFile(path);
  }

  function handlePin(e: MouseEvent, path: string) {
    e.stopPropagation();
    togglePin(path);
  }

  // Scroll active tab into view when it changes
  $effect(() => {
    const activePath = $activeFilePath;
    if (!tabsBar || !activePath) return;
    requestAnimationFrame(() => {
      const activeEl = tabsBar?.querySelector('.tab.active');
      if (activeEl) {
        activeEl.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
      }
    });
  });

  function handleWheel(e: WheelEvent) {
    if (tabsBar) {
      e.preventDefault();
      tabsBar.scrollLeft += e.deltaY;
    }
  }

  function openAddMenu() {
    const rect = addBtnEl?.getBoundingClientRect();
    if (rect) {
      addMenuPos = {
        top: rect.bottom + 2,
        right: Math.max(8, window.innerWidth - rect.right)
      };
    }
    addMenuOpen = !addMenuOpen;
  }

  function handleDocumentClick(e: MouseEvent) {
    if (addMenuOpen && addBtnEl && !addBtnEl.contains(e.target as Node)) {
      const menu = document.querySelector('.tab-add-menu');
      if (!menu?.contains(e.target as Node)) {
        addMenuOpen = false;
      }
    }
  }

  const markdownExts = /\.(md|mdx|markdown)$/i;
  let previewEnabled = $derived.by(() => {
    const path = $activeFilePath;
    return !!path && !isTerminalPath(path) && markdownExts.test(path);
  });

  function openExistingFileTab() {
    openFileSearchSignal.update((n) => n + 1);
    addMenuOpen = false;
  }

  function openPreviewTab() {
    if (!previewEnabled) return;
    openPreviewSignal.update((n) => n + 1);
    addMenuOpen = false;
  }

  function openTerminalTab() {
    $showTerminal = true;
    if ($terminalSessions.length === 0) {
      createTerminalSignal.update((n) => n + 1);
    } else {
      activeFilePath.set(terminalPath());
    }
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
    >
      <button class="tab-pin" class:pinned={file.pinned} title={file.pinned ? 'Unpin tab' : 'Pin tab'} onclick={(e) => handlePin(e, file.path)}>
        {#if file.pinned}
          <Pin size={11} strokeWidth={2.2} fill="currentColor" />
        {:else}
          <Pin size={11} strokeWidth={2} />
        {/if}
      </button>
      <span class="tab-name">
        {#if file.modified}<span class="modified-dot"></span>{/if}
        {file.name}
      </span>
      {#if !file.pinned}
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
      title={`Terminal (${ $terminalSessions.length } pane${$terminalSessions.length === 1 ? '' : 's'})`}
      onclick={() => activeFilePath.set(terminalPath())}
      onkeydown={(e) => e.key === 'Enter' && activeFilePath.set(terminalPath())}
    >
      <TerminalSquare size={13} />
      <span class="tab-name">Terminal</span>
      <button class="tab-close" onclick={(e) => { e.stopPropagation(); killTerminalSignal.set('all'); }}>×</button>
    </div>
  {/if}

  <div class="tab-add-anchor">
    <button
      type="button"
      class="tab tab-add-btn"
      bind:this={addBtnEl}
      onclick={openAddMenu}
      title="Open new tab menu"
      aria-label="Open new tab menu"
      aria-expanded={addMenuOpen}
    >
      <Plus size={13} />
    </button>
  </div>
</div>

{#if addMenuOpen && addMenuPos}
  <div class="tab-add-menu" role="menu" style="top: {addMenuPos.top}px; right: {addMenuPos.right}px;">
    <button class="tab-add-menu-item" role="menuitem" onclick={openExistingFileTab}>
      <FolderOpen size={12} />
      <span>Open File</span>
    </button>
    <button class="tab-add-menu-item" class:disabled={!previewEnabled} role="menuitem" onclick={openPreviewTab} disabled={!previewEnabled}>
      <Eye size={12} />
      <span>Preview</span>
    </button>
    <button class="tab-add-menu-item" role="menuitem" onclick={openTerminalTab}>
      <TerminalSquare size={12} />
      <span>Terminal</span>
    </button>
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

  .tab-add-anchor {
    position: sticky;
    right: 0;
    margin-left: auto;
    display: flex;
    flex-shrink: 0;
    background: linear-gradient(90deg, transparent 0%, var(--bg-secondary) 18px);
    padding-left: 18px;
    z-index: 2;
  }

  .tabs-bar::-webkit-scrollbar {
    height: 0;
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 10px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    white-space: nowrap;
    background: color-mix(in srgb, var(--tab-inactive) 82%, transparent);
    flex-shrink: 0;
    min-width: 0;
    max-width: 148px;
    cursor: pointer;
    height: calc(100% - 8px);
    margin: 4px 4px 4px 0;
    border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    border-radius: 999px;
  }

  .tab:hover {
    background: color-mix(in srgb, var(--bg-surface) 88%, transparent);
    border-color: color-mix(in srgb, var(--border) 95%, transparent);
    color: var(--text-primary);
  }

  .tab-add-btn {
    max-width: none;
    justify-content: center;
    width: 34px;
    padding: 0;
  }

  .tab.active {
    background: var(--tab-active);
    color: var(--text-primary);
    font-weight: 600;
    border-color: color-mix(in srgb, var(--accent) 28%, var(--border));
    box-shadow:
      inset 0 1px 0 color-mix(in srgb, var(--accent) 22%, transparent),
      0 0 0 1px color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .tab.conflict {
    border-color: color-mix(in srgb, var(--error) 40%, var(--border));
  }

  .tab-name {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tab-pin {
    color: var(--text-muted);
    flex-shrink: 0;
    padding: 2px;
    border-radius: 3px;
  }

  .tab-pin.pinned {
    color: var(--accent);
  }

  .tab-pin:hover {
    background: var(--bg-surface);
  }

  .modified-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warning);
    display: inline-block;
    flex-shrink: 0;
  }

  .tab-close {
    font-size: 14px;
    line-height: 1;
    color: var(--text-muted);
    border-radius: 3px;
    padding: 0 2px;
    flex-shrink: 0;
  }

  .tab-close:hover {
    background: var(--bg-surface);
    color: var(--error);
  }

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
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 9px;
    border-radius: 5px;
    font-size: 11px;
    color: var(--text-secondary);
    text-align: left;
    white-space: nowrap;
  }

  .tab-add-menu-item:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .tab-add-menu-item.disabled {
    opacity: 0.45;
    cursor: default;
  }

</style>
