<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { projectRoot } from '../../modules';

  interface DiffLine {
    kind: string;
    old_num: number | null;
    new_num: number | null;
    text: string;
  }

  let { filePath }: { filePath: string } = $props();

  let diffLines = $state<DiffLine[]>([]);
  let error = $state('');
  let loading = $state(true);

  let relPath = $derived((() => {
    const root = $projectRoot;
    if (root && filePath.startsWith(root)) return filePath.slice(root.length + 1);
    return filePath.split('/').pop() || filePath;
  })());

  $effect(() => {
    loadDiff();
  });

  async function loadDiff() {
    const root = $projectRoot;
    if (!root) return;
    loading = true;
    try {
      diffLines = await invoke<DiffLine[]>('git_diff', {
        repoPath: root,
        filePath: relPath,
        staged: false,
      });
      error = '';
    } catch (e) {
      error = String(e);
      diffLines = [];
    } finally {
      loading = false;
    }
  }
</script>

<div class="diff-viewer">
  <div class="diff-header">
    <span class="diff-filename">{relPath}</span>
    <span class="diff-tag">Working Tree</span>
  </div>
  {#if loading}
    <div class="diff-loading">Loading diff...</div>
  {:else if error}
    <div class="diff-error">{error}</div>
  {:else if diffLines.length === 0}
    <div class="diff-empty">No changes</div>
  {:else}
    <div class="diff-content">
      {#each diffLines as line}
        <div class="diff-line {line.kind}">
          <span class="diff-gutter old">{line.old_num ?? ''}</span>
          <span class="diff-gutter new">{line.new_num ?? ''}</span>
          <span class="diff-sign">{line.kind === 'add' ? '+' : line.kind === 'del' ? '-' : ' '}</span>
          <span class="diff-text">{line.text}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .diff-viewer {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-primary);
  }

  .diff-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }

  .diff-filename {
    color: var(--text-primary);
    font-weight: 600;
    font-family: var(--font-mono);
  }

  .diff-tag {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 3px;
    background: var(--bg-surface);
    color: var(--text-muted);
    font-weight: 500;
  }

  .diff-content {
    flex: 1;
    overflow-y: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
  }

  .diff-line {
    display: flex;
    padding: 0 16px;
    min-height: 20px;
  }

  .diff-line.add {
    background: color-mix(in srgb, var(--diff-add) 12%, transparent);
  }

  .diff-line.del {
    background: color-mix(in srgb, var(--diff-del) 12%, transparent);
  }

  .diff-gutter {
    width: 40px;
    flex-shrink: 0;
    text-align: right;
    padding-right: 8px;
    color: var(--gutter, var(--text-muted));
    user-select: none;
  }

  .diff-sign {
    width: 16px;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  .diff-line.add .diff-sign { color: var(--diff-add); }
  .diff-line.del .diff-sign { color: var(--diff-del); }

  .diff-text {
    white-space: pre;
    flex: 1;
    min-width: 0;
  }

  .diff-loading, .diff-error, .diff-empty {
    padding: 24px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }

  .diff-error { color: var(--error); }
</style>
