import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { toasts, showToast, dismissToast } from '$lib/modules/ui/toast';

beforeEach(() => {
  toasts.set([]);
  vi.useFakeTimers();
});

describe('showToast', () => {
  it('pushes a toast onto the store', () => {
    showToast({ level: 'info', message: 'Hello' });
    expect(get(toasts)).toHaveLength(1);
    expect(get(toasts)[0].message).toBe('Hello');
    expect(get(toasts)[0].level).toBe('info');
  });

  it('auto-dismisses after durationMs', () => {
    showToast({ level: 'info', message: 'Temp', durationMs: 3000 });
    expect(get(toasts)).toHaveLength(1);
    vi.advanceTimersByTime(3000);
    expect(get(toasts)).toHaveLength(0);
  });

  it('does not auto-dismiss error toasts (durationMs=0)', () => {
    showToast({ level: 'error', message: 'Sticky' });
    vi.advanceTimersByTime(60000);
    expect(get(toasts)).toHaveLength(1);
  });
});

describe('dismissToast', () => {
  it('removes a toast by id', () => {
    const id = showToast({ level: 'success', message: 'Done', durationMs: 0 });
    expect(get(toasts)).toHaveLength(1);
    dismissToast(id);
    expect(get(toasts)).toHaveLength(0);
  });
});
