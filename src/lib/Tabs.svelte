<script lang="ts">
  import { openFiles, activeFilePath, closeFile, togglePin, pinnedFiles, unpinnedFiles, sharedGitStatus } from './stores.ts';

  let tabsBar: HTMLDivElement | undefined = $state();

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
</script>

<div class="tabs-bar" bind:this={tabsBar} onwheel={handleWheel}>
  {#each [...$pinnedFiles, ...$unpinnedFiles] as file}
    <div
      class="tab"
      class:active={$activeFilePath === file.path}
      class:pinned={file.pinned}
      class:conflict={$sharedGitStatus[file.path] === 'C'}
      role="tab"
      tabindex="0"
      title={file.path}
      onclick={() => switchTab(file.path)}
      onkeydown={(e) => e.key === 'Enter' && switchTab(file.path)}
    >
      <button class="tab-pin" class:pinned={file.pinned} title={file.pinned ? 'Unpin tab' : 'Pin tab'} onclick={(e) => handlePin(e, file.path)}>
        <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor"><path d="M9.828.722a.5.5 0 0 1 .354.146l4.95 4.95a.5.5 0 0 1-.707.707l-.71-.71-3.18 3.18a3.5 3.5 0 0 1-1.272.91l.012.012a.5.5 0 0 1 0 .707l-1.06 1.06a.5.5 0 0 1-.708 0L5.57 8.838a.5.5 0 0 1 0-.707l1.06-1.06a.5.5 0 0 1 .708 0l.012.012a3.5 3.5 0 0 1 .91-1.273l3.18-3.18-.71-.71a.5.5 0 0 1 .354-.854z"/><path d="M1.5 14.5a.5.5 0 0 1 0-.707l3.793-3.793a.5.5 0 0 1 .707.707L2.207 14.5a.5.5 0 0 1-.707 0z"/></svg>
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
</div>

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

  .tabs-bar::-webkit-scrollbar {
    height: 0;
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    font-size: 12px;
    color: var(--text-muted);
    border-right: 1px solid var(--border);
    white-space: nowrap;
    background: var(--tab-inactive);
    flex-shrink: 0;
    min-width: 0;
    max-width: 160px;
    cursor: pointer;
    height: 100%;
  }

  .tab:hover {
    color: var(--text-primary);
  }

  .tab.active {
    background: var(--tab-active);
    color: var(--text-primary);
    border-bottom: 2px solid var(--accent);
  }

  .tab.conflict {
    border-bottom: 2px solid var(--error);
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
    font-size: 16px;
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
</style>
