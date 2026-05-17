import { describe, expect, it, vi } from 'vitest';
import { bindAiDiffResolve } from '$lib/modules/editor/aiDiffEvents';

describe('bindAiDiffResolve', () => {
  it('forwards ai-diff-resolve custom events to the callback', () => {
    const container = document.createElement('div');
    const onResolve = vi.fn();
    const unbind = bindAiDiffResolve(container, onResolve);

    container.dispatchEvent(new CustomEvent('ai-diff-resolve', {
      detail: { id: 'edit-1', action: 'approve' },
      bubbles: true,
    }));

    expect(onResolve).toHaveBeenCalledWith({ id: 'edit-1', action: 'approve' });

    unbind();
    onResolve.mockClear();

    container.dispatchEvent(new CustomEvent('ai-diff-resolve', {
      detail: { id: 'edit-2', action: 'reject' },
      bubbles: true,
    }));

    expect(onResolve).not.toHaveBeenCalled();
  });
});
