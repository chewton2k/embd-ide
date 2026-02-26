<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { projectRoot, sharedGitStatus } from './stores';
  import { parseConflicts, buildResolvedContent, type ConflictHunk, type Resolution } from './mergeUtils';
  import MergeConflictInline from './MergeConflictInline.svelte';
  import MergeConflictSideBySide from './MergeConflictSideBySide.svelte';

  interface Props {
    filePath: string;
  }

  let { filePath }: Props = $props();

  let rawContent = $state('');
  let hunks = $state<ConflictHunk[]>([]);
  let resolutions = $state(new Map<number, Resolution>());
  let viewMode = $state<'inline' | 'side-by-side'>('inline');
  let saving = $state(false);
  let error = $state('');

  let resolvedCount = $derived(resolutions.size);
  let totalConflicts = $derived(hunks.length);
  let allResolved = $derived(resolvedCount === totalConflicts && totalConflicts > 0);

  let loadRequestId = 0;

  async function loadFile() {
    const requestId = ++loadRequestId;
    try {
      const content = await invoke<string>('read_file_content', { path: filePath });
      if (requestId !== loadRequestId) return;
      rawContent = content;
      hunks = parseConflicts(content);
      resolutions = new Map();
      error = '';
    } catch (e) {
      if (requestId !== loadRequestId) return;
      rawContent = '';
      hunks = [];
      resolutions = new Map();
      error = `Failed to load file: ${e}`;
    }
  }

  $effect(() => {
    filePath;
    loadFile();
  });

  function handleResolve(hunkIndex: number, resolution: Resolution) {
    const next = new Map(resolutions);
    next.set(hunkIndex, resolution);
    resolutions = next;
  }

  function handleUnresolve(hunkIndex: number) {
    const next = new Map(resolutions);
    next.delete(hunkIndex);
    resolutions = next;
  }

  function acceptAllCurrent() {
    const next = new Map(resolutions);
    for (const hunk of hunks) next.set(hunk.index, 'current');
    resolutions = next;
  }

  function acceptAllIncoming() {
    const next = new Map(resolutions);
    for (const hunk of hunks) next.set(hunk.index, 'incoming');
    resolutions = next;
  }

  async function saveAndFinish() {
    if (!allResolved) return;
    const root = $projectRoot;
    if (!root) return;

    saving = true;
    error = '';

    try {
      const resolved = buildResolvedContent(rawContent, hunks, resolutions);
      const relPath = filePath.startsWith(root) ? filePath.slice(root.length + 1) : filePath;

      await invoke('git_resolve_conflict', {
        repoPath: root,
        filePath: relPath,
        content: resolved,
        stage: true,
      });

      // Refresh git status
      const status = await invoke<Record<string, string>>('get_git_status', { path: root });
      sharedGitStatus.set(status);

      // Reload the file content to show clean version
      rawContent = resolved;
      hunks = [];
      resolutions = new Map();
    } catch (e) {
      error = `Failed to save: ${e}`;
    }

    saving = false;
  }
</script>

<div class="merge-editor">
  <div class="merge-toolbar">
    <div class="toolbar-left">
      <span class="conflict-count">
        {resolvedCount}/{totalConflicts} conflicts resolved
      </span>
      <div class="toolbar-progress">
        <div class="progress-bar" style="width: {totalConflicts > 0 ? (resolvedCount / totalConflicts) * 100 : 0}%"></div>
      </div>
    </div>
    <div class="toolbar-actions">
      <button class="toolbar-btn" onclick={acceptAllCurrent}>Accept All Current</button>
      <button class="toolbar-btn" onclick={acceptAllIncoming}>Accept All Incoming</button>
      <div class="view-toggle">
        <button
          class="toggle-btn"
          class:active={viewMode === 'inline'}
          onclick={() => viewMode = 'inline'}
        >Inline</button>
        <button
          class="toggle-btn"
          class:active={viewMode === 'side-by-side'}
          onclick={() => viewMode = 'side-by-side'}
        >Side by Side</button>
      </div>
      <button
        class="toolbar-btn save-btn"
        disabled={!allResolved || saving}
        onclick={saveAndFinish}
      >
        {saving ? 'Saving...' : 'Save & Finish'}
      </button>
    </div>
  </div>

  {#if error}
    <div class="merge-error">{error}</div>
  {/if}

  <div class="merge-content">
    {#if hunks.length === 0 && rawContent}
      <div class="no-conflicts">
        <p>No merge conflicts found in this file.</p>
      </div>
    {:else if viewMode === 'inline'}
      <MergeConflictInline
        {rawContent}
        {hunks}
        {resolutions}
        onResolve={handleResolve}
        onUnresolve={handleUnresolve}
      />
    {:else}
      <MergeConflictSideBySide
        {rawContent}
        {hunks}
        {resolutions}
        onResolve={handleResolve}
        onUnresolve={handleUnresolve}
      />
    {/if}
  </div>
</div>

<style>
  .merge-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .merge-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    gap: 12px;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .conflict-count {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .toolbar-progress {
    width: 80px;
    height: 4px;
    background: var(--bg-surface);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    background: var(--success);
    border-radius: 2px;
    transition: width 0.2s ease;
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toolbar-btn {
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .toolbar-btn:hover {
    background: var(--bg-tertiary);
  }

  .save-btn {
    background: var(--success);
    color: var(--bg-primary);
    border-color: var(--success);
  }

  .save-btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .save-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .view-toggle {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }

  .toggle-btn {
    padding: 4px 10px;
    font-size: 11px;
    cursor: pointer;
    background: var(--bg-surface);
    color: var(--text-secondary);
    border: none;
    border-right: 1px solid var(--border);
  }

  .toggle-btn:last-child {
    border-right: none;
  }

  .toggle-btn.active {
    background: var(--accent);
    color: var(--bg-primary);
  }

  .toggle-btn:not(.active):hover {
    background: var(--bg-tertiary);
  }

  .merge-error {
    padding: 6px 12px;
    background: color-mix(in srgb, var(--error) 10%, var(--bg-primary));
    color: var(--error);
    font-size: 12px;
    border-bottom: 1px solid var(--error);
  }

  .merge-content {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .no-conflicts {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    font-size: 14px;
  }
</style>
