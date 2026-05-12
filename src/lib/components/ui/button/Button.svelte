<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import { cn } from '../../../utils';
  import { buttonVariants, type ButtonVariant } from './button';

  let {
    class: className,
    variant = 'ghost',
    size = 'sm',
    children,
    ...rest
  }: HTMLButtonAttributes & ButtonVariant = $props();
</script>

<button class={cn(buttonVariants({ variant, size }), className)} {...rest}>
  {@render children?.()}
</button>

<style>
  .ui-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.45rem;
    white-space: nowrap;
    border-radius: var(--radius, 8px);
    font-size: 12px;
    font-weight: 500;
    line-height: 1;
    transition: background 0.14s ease, color 0.14s ease, border-color 0.14s ease, opacity 0.14s ease;
  }

  .ui-button:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent);
  }

  .ui-button:disabled {
    pointer-events: none;
    opacity: 0.5;
  }

  .ui-button--default {
    background: var(--accent);
    color: var(--bg-tertiary);
  }

  .ui-button--default:hover {
    background: var(--accent-hover);
  }

  .ui-button--ghost {
    background: transparent;
    color: var(--text-muted);
  }

  .ui-button--ghost:hover {
    background: color-mix(in srgb, var(--bg-surface) 78%, transparent);
    color: var(--text-primary);
  }

  .ui-button--subtle {
    background: color-mix(in srgb, var(--bg-surface) 72%, transparent);
    border: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    color: var(--text-secondary);
  }

  .ui-button--subtle:hover {
    color: var(--text-primary);
    border-color: var(--border);
  }

  .ui-button--sm {
    height: 28px;
    padding: 0 10px;
  }

  .ui-button--md {
    height: 32px;
    padding: 0 12px;
  }

  .ui-button--icon {
    width: 28px;
    height: 28px;
    padding: 0;
  }
</style>
