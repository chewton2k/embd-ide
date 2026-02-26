<script lang="ts">
  import type { ConflictHunk, Resolution } from './mergeUtils';

  interface Props {
    rawContent: string;
    hunks: ConflictHunk[];
    resolutions: Map<number, Resolution>;
    onResolve: (hunkIndex: number, resolution: Resolution) => void;
    onUnresolve: (hunkIndex: number) => void;
  }

  let { rawContent, hunks, resolutions, onResolve, onUnresolve }: Props = $props();

  interface Row {
    type: 'context' | 'conflict';
    leftLine?: string;
    rightLine?: string;
    hunk?: ConflictHunk;
    hunkIndex?: number;
    isHunkStart?: boolean;
    side?: 'current' | 'incoming';
    rowIndex?: number;
  }

  let rows = $derived.by(() => {
    const lines = rawContent.split('\n');
    const result: Row[] = [];
    let cursor = 0;

    for (const hunk of hunks) {
      // Context lines before this hunk
      while (cursor < hunk.startLine) {
        const line = lines[cursor];
        result.push({ type: 'context', leftLine: line, rightLine: line });
        cursor++;
      }

      const resolved = resolutions.get(hunk.index);

      if (resolved) {
        // Show resolved state
        const maxLen = Math.max(hunk.currentLines.length, hunk.incomingLines.length);
        for (let r = 0; r < maxLen; r++) {
          result.push({
            type: 'context',
            leftLine: hunk.currentLines[r] ?? '',
            rightLine: hunk.incomingLines[r] ?? '',
            hunkIndex: hunk.index,
            isHunkStart: r === 0,
          });
        }
      } else {
        // Show conflict: pair up lines from both sides
        const maxLen = Math.max(hunk.currentLines.length, hunk.incomingLines.length);
        for (let r = 0; r < maxLen; r++) {
          result.push({
            type: 'conflict',
            leftLine: r < hunk.currentLines.length ? hunk.currentLines[r] : undefined,
            rightLine: r < hunk.incomingLines.length ? hunk.incomingLines[r] : undefined,
            hunk,
            hunkIndex: hunk.index,
            isHunkStart: r === 0,
            rowIndex: r,
          });
        }
      }

      cursor = hunk.endLine + 1;
    }

    // Remaining context
    while (cursor < lines.length) {
      const line = lines[cursor];
      result.push({ type: 'context', leftLine: line, rightLine: line });
      cursor++;
    }

    return result;
  });

  let leftPane: HTMLDivElement | undefined = $state();
  let rightPane: HTMLDivElement | undefined = $state();
  let syncing = false;

  function syncScroll(source: 'left' | 'right') {
    if (syncing) return;
    syncing = true;
    const from = source === 'left' ? leftPane : rightPane;
    const to = source === 'left' ? rightPane : leftPane;
    if (from && to) {
      to.scrollTop = from.scrollTop;
      to.scrollLeft = from.scrollLeft;
    }
    requestAnimationFrame(() => { syncing = false; });
  }
</script>

<div class="sbs-view">
  <div class="sbs-header">
    <div class="sbs-col-header">Current (Ours)</div>
    <div class="sbs-col-header">Incoming (Theirs)</div>
  </div>
  <div class="sbs-body">
    <div class="sbs-pane" bind:this={leftPane} onscroll={() => syncScroll('left')}>
      {#each rows as row}
        {#if row.type === 'context'}
          <div class="sbs-line context">{row.leftLine ?? '\u200b'}</div>
        {:else}
          <div class="sbs-line current-highlight">{row.leftLine ?? '\u200b'}</div>
        {/if}
      {/each}
    </div>
    <div class="sbs-actions-column">
      {#each rows as row}
        {#if row.hunkIndex != null && row.isHunkStart}
          {@const resolved = resolutions.get(row.hunkIndex)}
          <div class="sbs-hunk-actions">
            {#if resolved}
              <span class="resolved-badge-small">{resolved}</span>
              <button class="sbs-action-btn" onclick={() => onUnresolve(row.hunkIndex!)} title="Undo">↩</button>
            {:else}
              <button class="sbs-action-btn current-btn" onclick={() => onResolve(row.hunkIndex!, 'current')} title="Accept Current">←</button>
              <button class="sbs-action-btn both-btn" onclick={() => onResolve(row.hunkIndex!, 'both')} title="Accept Both">⇄</button>
              <button class="sbs-action-btn incoming-btn" onclick={() => onResolve(row.hunkIndex!, 'incoming')} title="Accept Incoming">→</button>
            {/if}
          </div>
        {:else}
          <div class="sbs-line context">{'\u200b'}</div>
        {/if}
      {/each}
    </div>
    <div class="sbs-pane" bind:this={rightPane} onscroll={() => syncScroll('right')}>
      {#each rows as row}
        {#if row.type === 'context'}
          <div class="sbs-line context">{row.rightLine ?? '\u200b'}</div>
        {:else}
          <div class="sbs-line incoming-highlight">{row.rightLine ?? '\u200b'}</div>
        {/if}
      {/each}
    </div>
  </div>
</div>

<style>
  .sbs-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    font-size: 13px;
  }

  .sbs-header {
    display: flex;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .sbs-col-header {
    flex: 1;
    padding: 4px 12px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .sbs-col-header:first-child {
    border-right: 1px solid var(--border);
  }

  .sbs-body {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .sbs-pane {
    flex: 1;
    overflow: auto;
    min-width: 0;
  }

  .sbs-pane:first-child {
    border-right: 1px solid var(--border);
  }

  .sbs-line {
    padding: 0 12px;
    white-space: pre;
    line-height: 1.5;
    min-height: 1.5em;
  }

  .sbs-line.context {
    color: var(--text-primary);
  }

  .sbs-line.current-highlight {
    background: color-mix(in srgb, var(--success) 10%, transparent);
    color: var(--text-primary);
  }

  .sbs-line.incoming-highlight {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--text-primary);
  }

  .sbs-actions-column {
    display: flex;
    flex-direction: column;
    justify-content: space-around;
    flex-shrink: 0;
    width: 80px;
    background: var(--bg-secondary);
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
    padding: 4px 0;
    gap: 8px;
    align-items: center;
    overflow-y: auto;
  }

  .sbs-hunk-actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: center;
    padding: 4px;
  }

  .sbs-action-btn {
    width: 28px;
    height: 22px;
    border-radius: 3px;
    font-size: 12px;
    cursor: pointer;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-primary);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .sbs-action-btn:hover {
    background: var(--bg-tertiary);
  }

  .current-btn:hover {
    border-color: var(--success);
    color: var(--success);
  }

  .incoming-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .both-btn:hover {
    border-color: var(--warning);
    color: var(--warning);
  }

  .resolved-badge-small {
    font-size: 9px;
    font-weight: 600;
    color: var(--success);
    text-transform: uppercase;
  }
</style>
