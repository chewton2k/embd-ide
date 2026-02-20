<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Editor from './Editor.svelte';

  let { filePath }: { filePath: string } = $props();

  let jsonData = $state<unknown>(null);
  let parseError = $state<string | null>(null);
  let loading = $state(true);
  let treeMode = $state(false);
  let expandedPaths = $state<Set<string>>(new Set());
  let allPaths = $state<string[]>([]);

  async function loadTree(path: string) {
    loading = true;
    parseError = null;
    jsonData = null;
    expandedPaths = new Set();
    allPaths = [];

    try {
      const content = await invoke<string>('read_file_content', { path });
      jsonData = JSON.parse(content);
      const paths: string[] = [];
      collectPaths(jsonData, '$', 0, paths);
      allPaths = paths;
      expandedPaths = new Set(paths.filter(p => {
        const depth = p.split('.').length - 1;
        return depth < 2;
      }));
    } catch (e) {
      parseError = `${e}`;
    }

    loading = false;
  }

  function collectPaths(value: unknown, path: string, depth: number, out: string[]) {
    if (value !== null && typeof value === 'object') {
      out.push(path);
      if (Array.isArray(value)) {
        value.forEach((item, i) => collectPaths(item, `${path}.${i}`, depth + 1, out));
      } else {
        Object.keys(value as Record<string, unknown>).forEach(key =>
          collectPaths((value as Record<string, unknown>)[key], `${path}.${key}`, depth + 1, out)
        );
      }
    }
  }

  function toggle(path: string) {
    const next = new Set(expandedPaths);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    expandedPaths = next;
  }

  function expandAll() {
    expandedPaths = new Set(allPaths);
  }

  function collapseAll() {
    expandedPaths = new Set();
  }

  function showTree() {
    treeMode = true;
    loadTree(filePath);
  }

  function getType(value: unknown): string {
    if (value === null) return 'null';
    if (Array.isArray(value)) return 'array';
    return typeof value;
  }
</script>

<div class="json-viewer">
  <div class="json-toolbar">
    <span class="json-filename">{filePath.split('/').pop()}</span>
    <div class="json-controls">
      {#if treeMode}
        <button class="tool-btn" onclick={expandAll}>Expand All</button>
        <button class="tool-btn" onclick={collapseAll}>Collapse All</button>
        <span class="toolbar-sep"></span>
      {/if}
      <button class="tool-btn" class:active={!treeMode} onclick={() => treeMode = false}>Editor</button>
      <button class="tool-btn" class:active={treeMode} onclick={showTree}>Tree View</button>
    </div>
  </div>

  {#if treeMode}
    {#if loading}
      <div class="json-status">Loading...</div>
    {:else if parseError}
      <div class="json-status error">{parseError}</div>
    {:else}
      <div class="json-tree">
        {@render jsonNode(jsonData, '$', null, true)}
      </div>
    {/if}
  {:else}
    <div class="json-editor-wrap">
      <Editor filePath={filePath} />
    </div>
  {/if}
</div>

{#snippet jsonNode(value: unknown, path: string, key: string | number | null, isLast: boolean)}
  {@const type = getType(value)}
  {#if type === 'object' || type === 'array'}
    {@const entries = type === 'array'
      ? (value as unknown[]).map((v, i) => [i, v] as [number, unknown])
      : Object.entries(value as Record<string, unknown>)}
    {@const isExpanded = expandedPaths.has(path)}
    {@const bracket = type === 'array' ? ['[', ']'] : ['{', '}']}
    <div class="json-line">
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="json-toggle" onclick={() => toggle(path)}>
        <svg class="chevron" class:expanded={isExpanded} viewBox="0 0 16 16" fill="currentColor" width="10" height="10">
          <path d="M6 3l5 5-5 5V3z" />
        </svg>
      </span>
      {#if key !== null}
        <span class="json-key">{typeof key === 'string' ? `"${key}"` : key}</span>
        <span class="json-colon">: </span>
      {/if}
      {#if isExpanded}
        <span class="json-bracket">{bracket[0]}</span>
        <span class="json-count">{entries.length} {entries.length === 1 ? 'item' : 'items'}</span>
      {:else}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span class="json-collapsed" onclick={() => toggle(path)}>
          {bracket[0]} ... {bracket[1]}
          <span class="json-count">{entries.length}</span>
        </span>
        {#if !isLast}<span class="json-comma">,</span>{/if}
      {/if}
    </div>
    {#if isExpanded}
      <div class="json-children">
        {#each entries as [k, v], i}
          {@render jsonNode(v, `${path}.${k}`, k, i === entries.length - 1)}
        {/each}
      </div>
      <div class="json-line">
        <span class="json-indent-bracket"></span>
        <span class="json-bracket">{bracket[1]}</span>
        {#if !isLast}<span class="json-comma">,</span>{/if}
      </div>
    {/if}
  {:else}
    <div class="json-line">
      <span class="json-leaf-indent"></span>
      {#if key !== null}
        <span class="json-key">{typeof key === 'string' ? `"${key}"` : key}</span>
        <span class="json-colon">: </span>
      {/if}
      {#if type === 'string'}
        <span class="json-value json-string">"{String(value)}"</span>
      {:else if type === 'number'}
        <span class="json-value json-number">{String(value)}</span>
      {:else if type === 'boolean'}
        <span class="json-value json-boolean">{String(value)}</span>
      {:else}
        <span class="json-value json-null">null</span>
      {/if}
      {#if !isLast}<span class="json-comma">,</span>{/if}
    </div>
  {/if}
{/snippet}

<style>
  .json-viewer {
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-primary);
    overflow: hidden;
  }

  .json-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 14px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    font-size: 11px;
  }

  .json-filename {
    color: var(--text-primary);
    font-weight: 500;
  }

  .json-controls {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .tool-btn {
    padding: 2px 8px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-surface);
  }

  .tool-btn:hover {
    color: var(--text-primary);
    background: var(--border);
  }

  .tool-btn.active {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .toolbar-sep {
    width: 1px;
    height: 14px;
    background: var(--border);
    margin: 0 4px;
  }

  .json-tree {
    flex: 1;
    overflow: auto;
    padding: 8px 14px;
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', 'Consolas', monospace;
    font-size: 12px;
    line-height: 1.6;
  }

  .json-line {
    display: flex;
    align-items: baseline;
    white-space: nowrap;
  }

  .json-children {
    padding-left: 18px;
    border-left: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    margin-left: 5px;
  }

  .json-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    cursor: pointer;
    flex-shrink: 0;
    color: var(--text-muted);
    border-radius: 2px;
  }

  .json-toggle:hover {
    color: var(--text-primary);
    background: var(--bg-surface);
  }

  .chevron {
    transition: transform 0.15s ease;
  }

  .chevron.expanded {
    transform: rotate(90deg);
  }

  .json-leaf-indent {
    width: 14px;
    flex-shrink: 0;
  }

  .json-indent-bracket {
    width: 14px;
    flex-shrink: 0;
  }

  .json-key {
    color: #89b4fa;
  }

  .json-colon {
    color: var(--text-muted);
    margin-right: 4px;
  }

  .json-bracket {
    color: var(--text-muted);
  }

  .json-comma {
    color: var(--text-muted);
  }

  .json-count {
    color: var(--text-muted);
    font-size: 10px;
    margin-left: 6px;
    opacity: 0.6;
  }

  .json-collapsed {
    cursor: pointer;
    color: var(--text-muted);
    border-radius: 3px;
    padding: 0 2px;
  }

  .json-collapsed:hover {
    background: var(--bg-surface);
    color: var(--text-secondary);
  }

  .json-string {
    color: #a6e3a1;
  }

  .json-number {
    color: #fab387;
  }

  .json-boolean {
    color: #89b4fa;
  }

  .json-null {
    color: #6c7086;
    font-style: italic;
  }

  .json-status {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    font-size: 12px;
  }

  .json-status.error {
    color: var(--error);
    padding: 20px;
    text-align: center;
  }

  .json-editor-wrap {
    flex: 1;
    overflow: hidden;
  }
</style>
