<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { projectRoot, addFile } from './stores';

  let { onClose }: { onClose: () => void } = $props();

  let query = $state('');
  let debouncedQuery = $state('');
  let allFiles = $state<string[]>([]);
  let selectedIndex = $state(0);
  let searchInput: HTMLInputElement | undefined = $state();
  let resultsList: HTMLDivElement | undefined = $state();
  let searchDebounce: ReturnType<typeof setTimeout> | null = null;

  // Debounce the query to avoid scoring all files on every keystroke
  $effect(() => {
    const q = query;
    if (searchDebounce) clearTimeout(searchDebounce);
    searchDebounce = setTimeout(() => { debouncedQuery = q; }, 150);
  });

  const filtered = $derived.by(() => {
    if (!debouncedQuery.trim()) return allFiles.slice(0, 50);
    const q = debouncedQuery.toLowerCase();
    const parts = q.split(/\s+/);
    // Score and filter
    const scored = allFiles
      .map(f => {
        const lower = f.toLowerCase();
        const name = f.split('/').pop()?.toLowerCase() || '';
        // All parts must match somewhere in the path
        if (!parts.every(p => lower.includes(p))) return null;
        // Score: prefer filename matches over path matches, exact starts over contains
        let score = 0;
        if (name.startsWith(parts[0])) score += 10;
        if (name.includes(parts[0])) score += 5;
        // Shorter paths score higher
        score -= f.split('/').length;
        return { file: f, score };
      })
      .filter((x): x is { file: string; score: number } => x !== null)
      .sort((a, b) => b.score - a.score);
    return scored.slice(0, 50).map(s => s.file);
  });

  $effect(() => {
    // Reset selection when results change
    filtered;
    selectedIndex = 0;
  });

  $effect(() => {
    // Load files when root changes
    const root = $projectRoot;
    if (root) {
      invoke<string[]>('list_all_files', { path: root }).then(files => {
        allFiles = files;
      });
    }
  });

  $effect(() => {
    // Focus input on mount
    requestAnimationFrame(() => searchInput?.focus());
  });

  function selectFile(relPath: string) {
    const root = $projectRoot;
    if (!root) return;
    const fullPath = `${root}/${relPath}`;
    const name = relPath.split('/').pop() || relPath;
    addFile(fullPath, name);
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
      scrollToSelected();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      scrollToSelected();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (filtered[selectedIndex]) {
        selectFile(filtered[selectedIndex]);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function scrollToSelected() {
    requestAnimationFrame(() => {
      const item = resultsList?.children[selectedIndex] as HTMLElement | undefined;
      item?.scrollIntoView({ block: 'nearest' });
    });
  }

  function highlightMatch(text: string): string {
    if (!query.trim()) return escapeHtml(text);
    const parts = query.toLowerCase().split(/\s+/);
    let result = escapeHtml(text);
    for (const part of parts) {
      const regex = new RegExp(`(${escapeRegex(part)})`, 'gi');
      result = result.replace(regex, '<mark>$1</mark>');
    }
    return result;
  }

  function escapeHtml(s: string): string {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function escapeRegex(s: string): string {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="search-overlay" onclick={onClose} onkeydown={handleKeydown}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="search-dialog" onclick={(e) => e.stopPropagation()}>
    <div class="search-input-row">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
        <circle cx="7" cy="7" r="4.5" />
        <path d="M10.5 10.5L14 14" />
      </svg>
      <input
        bind:this={searchInput}
        bind:value={query}
        class="search-input"
        placeholder="Search files by name..."
        autocapitalize="off"
        autocomplete="off"
        spellcheck="false"
        onkeydown={handleKeydown}
      />
    </div>
    <div class="search-results" bind:this={resultsList}>
      {#each filtered as file, i}
        <button
          class="search-result"
          class:selected={i === selectedIndex}
          onclick={() => selectFile(file)}
          onmouseenter={() => selectedIndex = i}
        >
          <span class="result-name">{@html highlightMatch(file.split('/').pop() || file)}</span>
          <span class="result-path">{@html highlightMatch(file)}</span>
        </button>
      {/each}
      {#if filtered.length === 0}
        <div class="no-results">No files found</div>
      {/if}
    </div>
  </div>
</div>

<style>
  .search-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 1000;
    display: flex;
    justify-content: center;
    padding-top: 15vh;
  }

  .search-dialog {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 500px;
    max-height: 400px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    overflow: hidden;
    align-self: flex-start;
  }

  .search-input-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
  }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 14px;
    outline: none;
    font-family: inherit;
  }

  .search-input::placeholder {
    color: var(--text-muted);
  }

  .search-results {
    overflow-y: auto;
    max-height: 340px;
    padding: 4px 0;
  }

  .search-result {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 8px 14px;
    text-align: left;
    cursor: pointer;
    border: none;
    background: none;
    color: var(--text-primary);
    font-family: inherit;
  }

  .search-result.selected {
    background: var(--bg-surface);
  }

  .result-name {
    font-size: 13px;
    font-weight: 500;
  }

  .result-path {
    font-size: 11px;
    color: var(--text-muted);
  }

  .search-result :global(mark) {
    background: color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--accent);
    border-radius: 2px;
    padding: 0 1px;
  }

  .no-results {
    padding: 20px 14px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
