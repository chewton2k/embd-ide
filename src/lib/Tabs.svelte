<script lang="ts">
  import { openFiles, activeFilePath, closeFile } from './stores.ts';

  let tabsBar: HTMLDivElement | undefined = $state();

  function switchTab(path: string) {
    activeFilePath.set(path);
  }

  function handleClose(e: MouseEvent, path: string) {
    e.stopPropagation();
    closeFile(path);
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
  {#each $openFiles as file}
    <div
      class="tab"
      class:active={$activeFilePath === file.path}
      role="tab"
      tabindex="0"
      title={file.path}
      onclick={() => switchTab(file.path)}
      onkeydown={(e) => e.key === 'Enter' && switchTab(file.path)}
    >
      <span class="tab-name">
        {#if file.modified}<span class="modified-dot"></span>{/if}
        {file.name}
      </span>
      <button class="tab-close" onclick={(e) => handleClose(e, file.path)}>×</button>
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

  .tab-name {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
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
