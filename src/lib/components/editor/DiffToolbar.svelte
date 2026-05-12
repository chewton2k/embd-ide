<script lang="ts">
  import { Check, X, RotateCcw } from 'lucide-svelte';
  import { pendingEdits, approveEdit, rejectEdit, approveAll, rejectAll } from '../../modules/stores/pendingEdits';

  let editCount = $derived(Object.values($pendingEdits).flat().filter(e => e.status === 'pending').length);
  let fileCount = $derived(Object.keys($pendingEdits).filter(k => $pendingEdits[k].some(e => e.status === 'pending')).length);
</script>

{#if editCount > 0}
  <div class="diff-toolbar">
    <span class="diff-info">
      ✦ {editCount} edit{editCount > 1 ? 's' : ''} pending in {fileCount} file{fileCount > 1 ? 's' : ''}
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
    color: var(--accent, #4a9eff);
    font-weight: 500;
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
