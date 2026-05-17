import { describe, expect, it } from 'vitest';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import {
  aiDiffExtension,
  aiDiffField,
  addDiffEffect,
} from '$lib/modules/editor/aiDiffExtension';
import type { EditProposal } from '$lib/modules/ai/editParser';

/**
 * These tests pin down the source-of-truth invariant for the AI diff
 * UI: the CodeMirror `aiDiffField` is a *projection*, not a writer.
 *
 * Specifically: when the user clicks the inline ✓ Accept / ✗ Reject
 * button, the click handler must NOT mutate the aiDiffField directly
 * — it must only emit the `ai-diff-resolve` custom event. The Svelte
 * layer then mutates `pendingEdits`, and the editor's
 * `pendingEdits.subscribe` is the only path that pushes changes back
 * into `aiDiffField`.
 *
 * The original bug surfaced as "Accept does nothing": the widget
 * removed itself from the field optimistically, but if `pendingEdits`
 * didn't know about the edit (or the write failed), the file was
 * never updated. This test prevents the regression.
 */

interface DiffResolveDetail { id: string; action: 'approve' | 'reject' }

function makeView(edits: EditProposal[]): { view: EditorView; container: HTMLDivElement } {
  const container = document.createElement('div');
  document.body.appendChild(container);

  const state = EditorState.create({
    doc: 'line 1\nline 2\nline 3\n',
    extensions: [aiDiffExtension()],
  });
  const view = new EditorView({ state, parent: container });

  if (edits.length > 0) {
    view.dispatch({ effects: addDiffEffect.of(edits) });
  }

  return { view, container };
}

function makeEdit(id: string, overrides: Partial<EditProposal> = {}): EditProposal {
  return {
    id,
    filePath: '/repo/src/x.ts',
    startLine: 1,
    endLine: 1,
    originalCode: 'line 1',
    newCode: 'replaced',
    status: 'pending',
    ...overrides,
  };
}

describe('aiDiffExtension widget click behaviour', () => {
  it('clicking ✓ Accept dispatches an `ai-diff-resolve` event with action="approve" and bubbles', () => {
    const edit = makeEdit('edit-accept');
    const { view, container } = makeView([edit]);

    const events: DiffResolveDetail[] = [];
    container.addEventListener('ai-diff-resolve', (e) => {
      events.push((e as CustomEvent<DiffResolveDetail>).detail);
    });

    const approveBtn = view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-approve');
    expect(approveBtn).not.toBeNull();
    approveBtn!.click();

    expect(events).toEqual([{ id: 'edit-accept', action: 'approve' }]);

    view.destroy();
    container.remove();
  });

  it('clicking ✗ Reject dispatches an `ai-diff-resolve` event with action="reject" and bubbles', () => {
    const edit = makeEdit('edit-reject');
    const { view, container } = makeView([edit]);

    const events: DiffResolveDetail[] = [];
    container.addEventListener('ai-diff-resolve', (e) => {
      events.push((e as CustomEvent<DiffResolveDetail>).detail);
    });

    const rejectBtn = view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-reject');
    expect(rejectBtn).not.toBeNull();
    rejectBtn!.click();

    expect(events).toEqual([{ id: 'edit-reject', action: 'reject' }]);

    view.destroy();
    container.remove();
  });

  it('clicking ✓ Accept does NOT mutate aiDiffField (the source-of-truth invariant)', () => {
    // This is the specific regression we are guarding against. If the
    // click handler ever re-introduces an optimistic
    // `view.dispatch({ effects: resolveDiffEffect.of(...) })`, the
    // field will be mutated here and this test will fail.
    const edit = makeEdit('edit-no-mutate');
    const { view, container } = makeView([edit]);

    const beforeIds = view.state.field(aiDiffField).map(e => e.id);
    expect(beforeIds).toEqual(['edit-no-mutate']);

    const approveBtn = view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-approve');
    approveBtn!.click();

    const afterIds = view.state.field(aiDiffField).map(e => e.id);
    expect(afterIds).toEqual(['edit-no-mutate']);

    view.destroy();
    container.remove();
  });

  it('clicking ✗ Reject does NOT mutate aiDiffField either', () => {
    const edit = makeEdit('edit-no-mutate-2');
    const { view, container } = makeView([edit]);

    const approveBtn = view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-reject');
    approveBtn!.click();

    expect(view.state.field(aiDiffField).map(e => e.id)).toEqual(['edit-no-mutate-2']);

    view.destroy();
    container.remove();
  });

  it('two rapid clicks both fire events (no synchronous suppression)', () => {
    // The widget intentionally does not disable buttons on first click:
    // approveEdit is idempotent on the same id (the second invocation
    // is a no-op once the edit is removed from the store), and
    // disabling would interfere with CM's `eq`-based widget DOM reuse
    // — a stuck-disabled widget would block retries on write failure.
    const edit = makeEdit('edit-rapid');
    const { view, container } = makeView([edit]);

    const events: DiffResolveDetail[] = [];
    container.addEventListener('ai-diff-resolve', (e) => {
      events.push((e as CustomEvent<DiffResolveDetail>).detail);
    });

    const approveBtn = view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-approve');
    approveBtn!.click();
    approveBtn!.click();

    expect(events).toEqual([
      { id: 'edit-rapid', action: 'approve' },
      { id: 'edit-rapid', action: 'approve' },
    ]);

    view.destroy();
    container.remove();
  });
});

describe('aiDiffExtension stale-edit visual rendering (Group 5)', () => {
  it('non-stale edit does NOT render the warning badge', () => {
    const edit = makeEdit('clean'); // stale not set
    const { view, container } = makeView([edit]);

    const badge = view.dom.querySelector('.cm-ai-stale-badge');
    expect(badge).toBeNull();

    // The button label should be the unmodified "✓ Accept", not
    // "Accept anyway".
    const approveBtn = view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-approve');
    expect(approveBtn?.textContent).toBe('✓ Accept');
    expect(approveBtn?.classList.contains('cm-ai-btn-stale')).toBe(false);

    // The diff-controls wrap should not have the stale class.
    const controls = view.dom.querySelector('.cm-ai-diff-controls');
    expect(controls?.classList.contains('cm-ai-stale-controls')).toBe(false);

    view.destroy();
    container.remove();
  });

  it('stale edit renders the warning badge with accessible role + label', () => {
    const edit = makeEdit('drift', { stale: true });
    const { view, container } = makeView([edit]);

    const badge = view.dom.querySelector<HTMLSpanElement>('.cm-ai-stale-badge');
    expect(badge).not.toBeNull();
    expect(badge!.textContent).toBe('⚠ Stale');
    // Accessibility: the warning should surface to assistive tech, not
    // just hover users. Both `title` (mouse) and `aria-label` (screen
    // readers / keyboard) carry the explanation; `role="status"` lets
    // assistive tech announce it.
    expect(badge!.getAttribute('role')).toBe('status');
    expect(badge!.getAttribute('aria-label')).toContain('overwrite');
    expect(badge!.title).toContain('overwrite');

    view.destroy();
    container.remove();
  });

  it('stale edit shows "Accept anyway" button label and stale class', () => {
    const edit = makeEdit('drift-btn', { stale: true });
    const { view, container } = makeView([edit]);

    const approveBtn = view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-approve');
    expect(approveBtn?.textContent).toBe('✓ Accept anyway');
    expect(approveBtn?.classList.contains('cm-ai-btn-stale')).toBe(true);

    // The diff-controls wrap and added-block both get a stale class
    // so the CSS theme can apply the yellow-orange palette.
    const controls = view.dom.querySelector('.cm-ai-diff-controls');
    expect(controls?.classList.contains('cm-ai-stale-controls')).toBe(true);

    const block = view.dom.querySelector('.cm-ai-added-block');
    expect(block?.classList.contains('cm-ai-stale-block')).toBe(true);

    view.destroy();
    container.remove();
  });

  it('clicking ✓ Accept anyway on a stale edit still dispatches an approve event (user retains authority)', () => {
    const edit = makeEdit('stale-approve', { stale: true });
    const { view, container } = makeView([edit]);

    const events: DiffResolveDetail[] = [];
    container.addEventListener('ai-diff-resolve', (e) => {
      events.push((e as CustomEvent<DiffResolveDetail>).detail);
    });

    view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-approve')!.click();

    expect(events).toEqual([{ id: 'stale-approve', action: 'approve' }]);

    view.destroy();
    container.remove();
  });

  it('flipping the stale flag rebuilds the widget DOM (eq() honors stale)', () => {
    // Render edit as non-stale, then dispatch a state with the same
    // edit but stale=true. The widget's eq() should return false
    // because stale differs, forcing CM to rebuild the DOM with the
    // badge and "Accept anyway" label.
    const cleanEdit = makeEdit('toggle');
    const { view, container } = makeView([cleanEdit]);

    expect(view.dom.querySelector('.cm-ai-stale-badge')).toBeNull();

    const staleEdit = { ...cleanEdit, stale: true };
    view.dispatch({ effects: addDiffEffect.of([staleEdit]) });

    expect(view.dom.querySelector('.cm-ai-stale-badge')).not.toBeNull();
    expect(
      view.dom.querySelector<HTMLButtonElement>('.cm-ai-btn-approve')?.textContent,
    ).toBe('✓ Accept anyway');

    view.destroy();
    container.remove();
  });
});
