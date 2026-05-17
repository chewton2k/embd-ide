<script lang="ts">
  import { Check, X, AlertTriangle } from 'lucide-svelte';
  import { pendingEdits, approveAll, rejectAll } from '../../modules/ai/pendingEdits';

  // Single pass over the store: count pending, files, and stale in
  // one traversal so we don't repeatedly flat()/filter() on every
  // store update.
  let summary = $derived.by(() => {
    let edits = 0;
    let files = 0;
    let stale = 0;
    for (const bucket of Object.values($pendingEdits)) {
      let bucketHasPending = false;
      for (const e of bucket) {
        if (e.status !== 'pending') continue;
        edits += 1;
        if (e.stale) stale += 1;
        bucketHasPending = true;
      }
      if (bucketHasPending) files += 1;
    }
    return { edits, files, stale };
  });
</script>

{#if summary.edits > 0}
  <div class="diff-toolbar">
    <span class="diff-info">
      ✦ {summary.edits} edit{summary.edits > 1 ? 's' : ''} pending in {summary.files} file{summary.files > 1 ? 's' : ''}
      {#if summary.stale > 0}
        <span
          class="stale-badge"
          role="status"
          aria-label="{summary.stale} edit{summary.stale > 1 ? 's' : ''} {summary.stale > 1 ? 'have' : 'has'} drifted from the proposal's original content; accepting will overwrite local changes"
          title="The live content under {summary.stale > 1 ? 'these' : 'this'} edit{summary.stale > 1 ? 's' : ''} has changed since the proposal was generated. Accepting will overwrite those changes."
        >
          <AlertTriangle size={11} aria-hidden="true" />
          {summary.stale} stale
        </span>
      {/if}
    </span>
    <div class="diff-actions">
      <button class="diff-btn approve" onclick={approveAll}>
        <Check size={12} /> Accept All
      </button>
      <button class="diff-btn reject" onclick={rejectAll}>
        <X size={12} /> Reject All
      </button>
    </div>
  </div>
{/if}

<style>
  .diff-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px;
    background: rgba(74, 158, 255, 0.08);
    border-bottom: 1px solid rgba(74, 158, 255, 0.2);
    font-size: 12px;
    flex-shrink: 0;
  }

  .diff-info {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--accent, #4a9eff);
    font-weight: 500;
  }

  .stale-badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 11px;
    font-weight: 600;
    color: #ffcc66;
    background: rgba(240, 160, 32, 0.15);
    border: 1px solid rgba(240, 160, 32, 0.4);
    cursor: help;
  }

  .diff-actions {
    display: flex;
    gap: 6px;
  }

  .diff-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 10px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .diff-btn.approve {
    background: rgba(78, 201, 176, 0.12);
    color: #4ec9b0;
    border: 1px solid rgba(78, 201, 176, 0.3);
  }

  .diff-btn.approve:hover { background: rgba(78, 201, 176, 0.22); }

  .diff-btn.reject {
    background: rgba(241, 76, 76, 0.08);
    color: #f14c4c;
    border: 1px solid rgba(241, 76, 76, 0.25);
  }

  .diff-btn.reject:hover { background: rgba(241, 76, 76, 0.18); }
</style>
