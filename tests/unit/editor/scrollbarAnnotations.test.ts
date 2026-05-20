import { describe, expect, it } from 'vitest';
import { EditorState, StateField } from '@codemirror/state';
import {
  scrollbarAnnotations,
  setScrollbarRanges,
  type ScrollbarRange,
} from '$lib/modules/editor/scrollbarAnnotations';

/** Extract the StateField from the extension bundle. */
function getField() {
  const exts = scrollbarAnnotations();
  return exts[0] as StateField<ScrollbarRange[]>;
}

describe('scrollbarAnnotations', () => {
  it('returns an array (extension bundle)', () => {
    expect(Array.isArray(scrollbarAnnotations())).toBe(true);
    expect(scrollbarAnnotations().length).toBeGreaterThan(0);
  });

  it('setScrollbarRanges effect can be created with ranges', () => {
    const ranges: ScrollbarRange[] = [{ kind: 'add', start: 1, end: 5 }];
    const effect = setScrollbarRanges.of(ranges);
    expect(effect.value).toEqual(ranges);
  });

  it('StateField starts empty', () => {
    const field = getField();
    const state = EditorState.create({ extensions: [field] });
    expect(state.field(field)).toEqual([]);
  });

  it('StateField updates when setScrollbarRanges effect is dispatched', () => {
    const field = getField();
    const state = EditorState.create({ extensions: [field] });
    const ranges: ScrollbarRange[] = [
      { kind: 'mod', start: 3, end: 7 },
      { kind: 'del', start: 10, end: 10 },
    ];
    const next = state.update({ effects: setScrollbarRanges.of(ranges) }).state;
    expect(next.field(field)).toEqual(ranges);
  });

  it('StateField retains value when unrelated transaction occurs', () => {
    const field = getField();
    const state = EditorState.create({ doc: 'hello', extensions: [field] });
    const ranges: ScrollbarRange[] = [{ kind: 'add', start: 1, end: 2 }];
    const withRanges = state.update({ effects: setScrollbarRanges.of(ranges) }).state;
    // Dispatch an unrelated change (insert text)
    const afterEdit = withRanges.update({ changes: { from: 0, insert: 'x' } }).state;
    expect(afterEdit.field(field)).toEqual(ranges);
  });
});
