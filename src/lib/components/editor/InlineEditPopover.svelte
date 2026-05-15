<!--
  InlineEditPopover.svelte

  Floating input that appears above the editor selection when the user
  presses Cmd+K. Styled like VSCode's inline chat — compact, dark,
  with a text input and submit/cancel controls.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { inlineEditStreaming } from '../../modules/ai/inlineEdit';

  interface Props {
    top: number;
    left: number;
    width: number;
    onSubmit: (instruction: string) => void;
    onCancel: () => void;
  }

  let { top, left, width, onSubmit, onCancel }: Props = $props();

  let input = $state('');
  let inputEl: HTMLInputElement;

  onMount(() => {
    inputEl?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && input.trim()) {
      e.preventDefault();
      onSubmit(input.trim());
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<div
  class="inline-edit-popover"
  style="top: {top}px; left: {left}px; width: {Math.max(320, width)}px;"
>
  <div class="popover-inner">
    <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="14" height="14">
      <path d="M12 20h9" /><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
    </svg>
    <input
      bind:this={inputEl}
      bind:value={input}
      onkeydown={handleKeydown}
      placeholder="Describe the edit..."
      disabled={$inlineEditStreaming}
      spellcheck="false"
      autocomplete="off"
    />
    {#if $inlineEditStreaming}
      <div class="spinner"></div>
    {:else}
      <button
        class="submit-btn"
        disabled={!input.trim()}
        onclick={() => input.trim() && onSubmit(input.trim())}
        aria-label="Submit edit"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="14" height="14">
          <path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
        </svg>
      </button>
    {/if}
    <button class="cancel-btn" onclick={onCancel} aria-label="Cancel">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12">
        <path d="M18 6 6 18" /><path d="m6 6 12 12" />
      </svg>
    </button>
  </div>
</div>

<style>
  .inline-edit-popover {
    position: absolute;
    z-index: 100;
    transform: translateY(-100%);
    margin-top: -4px;
  }

  .popover-inner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: var(--bg-secondary, #1e1e2e);
    border: 1px solid var(--accent, #4a9eff);
    border-radius: 8px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
  }

  .icon {
    flex-shrink: 0;
    color: var(--accent, #4a9eff);
  }

  input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text-primary, #e0e0e0);
    font-size: 12.5px;
    font-family: inherit;
    padding: 4px 0;
    min-width: 0;
  }

  input::placeholder {
    color: var(--text-muted, #6e6e6e);
  }

  input:disabled {
    opacity: 0.6;
  }

  .submit-btn, .cancel-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: none;
    cursor: pointer;
    transition: background 0.12s;
  }

  .submit-btn {
    background: var(--accent, #4a9eff);
    color: #fff;
  }
  .submit-btn:hover:not(:disabled) { opacity: 0.85; }
  .submit-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .cancel-btn {
    background: transparent;
    color: var(--text-muted, #6e6e6e);
  }
  .cancel-btn:hover { background: var(--bg-surface, #2a2a3a); color: var(--text-primary, #e0e0e0); }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--text-muted, #6e6e6e);
    border-top-color: var(--accent, #4a9eff);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
