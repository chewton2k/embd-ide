<script lang="ts">
  import type { ConflictHunk, Resolution } from './mergeUtils';
  import { resolveHunkLines } from './mergeUtils';

  interface Props {
    rawContent: string;
    hunks: ConflictHunk[];
    resolutions: Map<number, Resolution>;
    onResolve: (hunkIndex: number, resolution: Resolution) => void;
    onUnresolve: (hunkIndex: number) => void;
  }

  let { rawContent, hunks, resolutions, onResolve, onUnresolve }: Props = $props();

  interface Segment {
    type: 'text' | 'conflict';
    lines: string[];
    hunk?: ConflictHunk;
  }

  let segments = $derived.by(() => {
    const lines = rawContent.split('\n');
    const result: Segment[] = [];
    let cursor = 0;

    for (const hunk of hunks) {
      if (cursor < hunk.startLine) {
        result.push({ type: 'text', lines: lines.slice(cursor, hunk.startLine) });
      }
      result.push({ type: 'conflict', lines: [], hunk });
      cursor = hunk.endLine + 1;
    }

    if (cursor < lines.length) {
      result.push({ type: 'text', lines: lines.slice(cursor) });
    }

    return result;
  });
</script>

<div class="inline-view">
  {#each segments as seg, i}
    {#if seg.type === 'text'}
      <div class="text-block">
        {#each seg.lines as line}
          <div class="code-line">{line || '\u200b'}</div>
        {/each}
      </div>
    {:else if seg.hunk}
      {@const hunk = seg.hunk}
      {@const resolved = resolutions.get(hunk.index)}
      {#if resolved}
        <div class="conflict-block resolved">
          <div class="conflict-header resolved-header">
            <span class="resolved-badge">Resolved ({resolved})</span>
            <button class="undo-btn" onclick={() => onUnresolve(hunk.index)}>Undo</button>
          </div>
          <div class="resolved-content">
            {#each resolveHunkLines(hunk, resolved) as line}
              <div class="code-line">{line || '\u200b'}</div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="conflict-block">
          <div class="conflict-header">
            <div class="conflict-actions">
              <button class="action-btn accept-current" onclick={() => onResolve(hunk.index, 'current')}>Accept Current</button>
              <button class="action-btn accept-incoming" onclick={() => onResolve(hunk.index, 'incoming')}>Accept Incoming</button>
              <button class="action-btn accept-both" onclick={() => onResolve(hunk.index, 'both')}>Accept Both</button>
            </div>
          </div>
          <div class="current-region">
            <div class="region-label">Current Change {hunk.currentLabel ? `(${hunk.currentLabel})` : ''}</div>
            {#each hunk.currentLines as line}
              <div class="code-line">{line || '\u200b'}</div>
            {/each}
          </div>
          <div class="separator">
            <span class="separator-line"></span>
          </div>
          <div class="incoming-region">
            <div class="region-label">Incoming Change {hunk.incomingLabel ? `(${hunk.incomingLabel})` : ''}</div>
            {#each hunk.incomingLines as line}
              <div class="code-line">{line || '\u200b'}</div>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
  {/each}
</div>

<style>
  .inline-view {
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    font-size: 13px;
    line-height: 1.5;
  }

  .code-line {
    padding: 0 12px;
    white-space: pre;
    min-height: 1.5em;
  }

  .text-block .code-line {
    color: var(--text-primary);
  }

  .conflict-block {
    border-left: 3px solid var(--warning);
    margin: 2px 0;
  }

  .conflict-block.resolved {
    border-left-color: var(--success);
  }

  .conflict-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px;
    background: color-mix(in srgb, var(--warning) 10%, var(--bg-tertiary));
    font-size: 11px;
  }

  .resolved-header {
    background: color-mix(in srgb, var(--success) 10%, var(--bg-tertiary));
  }

  .conflict-actions {
    display: flex;
    gap: 6px;
  }

  .action-btn {
    padding: 2px 8px;
    border-radius: 3px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .action-btn:hover {
    background: var(--bg-tertiary);
  }

  .accept-current:hover {
    border-color: var(--success);
    color: var(--success);
  }

  .accept-incoming:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .accept-both:hover {
    border-color: var(--warning);
    color: var(--warning);
  }

  .resolved-badge {
    color: var(--success);
    font-weight: 600;
    font-size: 11px;
  }

  .undo-btn {
    padding: 2px 8px;
    border-radius: 3px;
    font-size: 11px;
    cursor: pointer;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-secondary);
  }

  .undo-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .current-region {
    background: color-mix(in srgb, var(--success) 8%, transparent);
  }

  .incoming-region {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .region-label {
    padding: 2px 12px;
    font-size: 10px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .current-region .region-label {
    color: var(--success);
  }

  .incoming-region .region-label {
    color: var(--accent);
  }

  .separator {
    display: flex;
    align-items: center;
    padding: 0 12px;
    height: 1px;
  }

  .separator-line {
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .resolved-content {
    background: color-mix(in srgb, var(--success) 5%, transparent);
  }

  .resolved-content .code-line {
    color: var(--text-secondary);
  }
</style>
