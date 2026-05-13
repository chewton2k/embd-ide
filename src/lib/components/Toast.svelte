<script lang="ts">
  import { fly } from 'svelte/transition';
  import { Info, AlertTriangle, XCircle, CheckCircle2, X } from 'lucide-svelte';
  import { toasts, dismissToast, type ToastEntry } from '../modules/ui/toast';

  const ICONS = {
    info: Info,
    warn: AlertTriangle,
    error: XCircle,
    success: CheckCircle2,
  } as const;
</script>

{#if $toasts.length > 0}
  <div class="toast-container" aria-live="polite">
    {#each $toasts as toast (toast.id)}
      <div
        class="toast toast-{toast.level}"
        role="alert"
        in:fly={{ x: 20, duration: 200 }}
        out:fly={{ x: 20, duration: 150 }}
      >
        <svelte:component this={ICONS[toast.level]} size={16} />
        <span class="toast-msg">{toast.message}</span>
        <button class="toast-close" onclick={() => dismissToast(toast.id)} aria-label="Dismiss">
          <X size={14} />
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed;
    bottom: 12px;
    right: 12px;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 380px;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-radius: 6px;
    font-size: 12.5px;
    line-height: 1.4;
    background: var(--bg-surface, #1e1e1e);
    border: 1px solid var(--border, #333);
    color: var(--text-primary, #e0e0e0);
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
  }
  .toast-warn { border-color: #b08800; }
  .toast-error { border-color: #c44; }
  .toast-success { border-color: #2a7; }
  .toast-msg { flex: 1; word-break: break-word; }
  .toast-close {
    background: none;
    border: none;
    color: inherit;
    opacity: 0.6;
    cursor: pointer;
    padding: 2px;
    border-radius: 3px;
  }
  .toast-close:hover { opacity: 1; background: rgba(255,255,255,0.08); }
</style>
