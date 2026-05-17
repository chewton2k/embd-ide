import { describe, expect, it } from 'vitest';
import { Text, ChangeSet } from '@codemirror/state';
import { reanchorEditsForChanges } from '$lib/modules/editor/aiDiffAnchoring';
import type { EditProposal } from '$lib/modules/ai/editParser';

/**
 * Pure-function unit tests for `reanchorEditsForChanges`. We construct
 * synthetic CodeMirror `Text` and `ChangeSet` values and verify the
 * helper maps line numbers correctly across the change.
 *
 * Helper notes:
 *  - `Text.of(['line 1', 'line 2', ...])` builds a multi-line doc.
 *  - `ChangeSet.of({ from, to, insert }, oldLength)` builds a single-
 *    change ChangeSet that we can apply to oldDoc and feed to the
 *    helper alongside the resulting newDoc.
 */

function makeEdit(overrides: Partial<EditProposal> = {}): EditProposal {
  return {
    id: overrides.id ?? 'e',
    filePath: overrides.filePath ?? '/repo/src/x.ts',
    startLine: overrides.startLine ?? 1,
    endLine: overrides.endLine ?? 1,
    originalCode: overrides.originalCode ?? 'orig',
    newCode: overrides.newCode ?? 'new',
    status: overrides.status ?? 'pending',
    ...(overrides.stale !== undefined ? { stale: overrides.stale } : {}),
  };
}

function applyChange(oldDoc: Text, change: { from: number; to: number; insert: string }) {
  const changes = ChangeSet.of([change], oldDoc.length);
  const newDoc = changes.apply(oldDoc);
  return { changes, newDoc };
}

describe('reanchorEditsForChanges', () => {
  it('insertion before the edit shifts both startLine and endLine down', () => {
    // Doc: 5 lines. Edit covers line 3.
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({ id: 'shift', startLine: 3, endLine: 3 });

    // Insert a new line at the very start of the document. The
    // ChangeSet inserts "x\n" at offset 0, which adds one line above.
    const { changes, newDoc } = applyChange(oldDoc, { from: 0, to: 0, insert: 'x\n' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ id: 'shift', startLine: 4, endLine: 4 });
  });

  it('insertion strictly above a multi-line edit shifts both endpoints by the line count inserted', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({ id: 'multi', startLine: 3, endLine: 4 });

    // Insert two new lines above line 3.
    const { changes, newDoc } = applyChange(oldDoc, { from: 0, to: 0, insert: 'x\ny\n' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out[0]).toMatchObject({ startLine: 5, endLine: 6 });
  });

  it('insertion after the edit (below endLine) leaves the range unchanged and preserves identity', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({ id: 'after', startLine: 2, endLine: 2 });

    // Insert a new line at the very end (after line 5).
    const { changes, newDoc } = applyChange(oldDoc, {
      from: oldDoc.length,
      to: oldDoc.length,
      insert: '\nz',
    });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 2 });
    // Identity preserved when nothing changed (cheap reference check
    // for the call-site fast path).
    expect(out[0]).toBe(edit);
  });

  it('deletion before the edit shifts both endpoints up', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({ id: 'del-before', startLine: 4, endLine: 5 });

    // Delete line 1 entirely (chars [0, 2): "a\n").
    const { changes, newDoc } = applyChange(oldDoc, { from: 0, to: 2, insert: '' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out[0]).toMatchObject({ startLine: 3, endLine: 4 });
  });

  it('deletion that fully consumes the edit drops it', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({ id: 'consumed', startLine: 2, endLine: 4 });

    // Delete lines 2-4 entirely. line(2).from = 2, line(4).to = 7
    // (the `d` is at offset 6, line(4).to is 7). We also consume the
    // newline after line 4 so the surrounding text joins cleanly.
    // Line offsets in "a\nb\nc\nd\ne": a=0, b=2, c=4, d=6, e=8.
    // Remove from offset 2 (start of b) through offset 8 (start of e).
    const { changes, newDoc } = applyChange(oldDoc, { from: 2, to: 8, insert: '' });

    // Sanity: newDoc should now be "a\ne".
    expect(newDoc.toString()).toBe('a\ne');

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toEqual([]);
  });

  it('deletion overlapping only the start of the edit narrows the range', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({ id: 'partial', startLine: 2, endLine: 4 });

    // Delete line 2 only: chars [2, 4) = "b\n".
    const { changes, newDoc } = applyChange(oldDoc, { from: 2, to: 4, insert: '' });

    expect(newDoc.toString()).toBe('a\nc\nd\ne');

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    // Original lines 2-4 contained b/c/d. After removing b, the range
    // collapses to lines 2-3 (c/d) in the new doc.
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 3 });
  });

  it('preserves order across multiple edits', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const e1 = makeEdit({ id: 'e1', startLine: 2, endLine: 2 });
    const e2 = makeEdit({ id: 'e2', startLine: 4, endLine: 4 });
    const e3 = makeEdit({ id: 'e3', startLine: 5, endLine: 5 });

    // Insert "x\n" at the start.
    const { changes, newDoc } = applyChange(oldDoc, { from: 0, to: 0, insert: 'x\n' });

    const out = reanchorEditsForChanges([e1, e2, e3], oldDoc, newDoc, changes);
    expect(out.map(e => e.id)).toEqual(['e1', 'e2', 'e3']);
    expect(out.map(e => e.startLine)).toEqual([3, 5, 6]);
    expect(out.map(e => e.endLine)).toEqual([3, 5, 6]);
  });

  it('out-of-range edit (startLine > doc.lines) is passed through unchanged', () => {
    const oldDoc = Text.of(['a', 'b']);
    // Edit references line 99 of a 2-line doc (e.g. carried in across
    // a wholesale doc replacement). Helper should not crash; the
    // syncDiffField projection filters it later.
    const edit = makeEdit({ id: 'oor', startLine: 99, endLine: 99 });

    const { changes, newDoc } = applyChange(oldDoc, { from: 0, to: 0, insert: 'x\n' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toEqual([edit]);
    // Identity preserved (we never constructed a new object).
    expect(out[0]).toBe(edit);
  });

  it('inverted range (endLine < startLine) is passed through unchanged', () => {
    const oldDoc = Text.of(['a', 'b', 'c']);
    const edit = makeEdit({ id: 'inv', startLine: 3, endLine: 1 });

    const { changes, newDoc } = applyChange(oldDoc, { from: 0, to: 0, insert: 'x\n' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toEqual([edit]);
    expect(out[0]).toBe(edit);
  });

  it('empty input returns empty output', () => {
    const oldDoc = Text.of(['a']);
    const { changes, newDoc } = applyChange(oldDoc, { from: 1, to: 1, insert: '!' });

    expect(reanchorEditsForChanges([], oldDoc, newDoc, changes)).toEqual([]);
  });

  it('a no-op change returns the same array contents with identity preserved', () => {
    const oldDoc = Text.of(['a', 'b', 'c']);
    const edit = makeEdit({ id: 'noop', startLine: 2, endLine: 2 });

    // ChangeSet.empty produces an empty change. Apply yields the same doc.
    const changes = ChangeSet.empty(oldDoc.length);
    const newDoc = changes.apply(oldDoc);

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out[0]).toBe(edit);
  });
});

import { reanchorEditsForContent } from '$lib/modules/editor/aiDiffAnchoring';

describe('reanchorEditsForContent (Group 4 — wholesale-replacement re-anchor)', () => {
  // mapPos through a `from: 0, to: doc.length` change is meaningless
  // (everything maps to the boundary). For external-edit reloads we
  // use line-aligned content search instead. These tests cover the
  // pure helper.

  it('re-anchors a single edit when its originalCode appears exactly once', () => {
    const edit = makeEdit({
      id: 'unique',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
    });
    // Content shifted: a new line was added at the start.
    const newContent = 'header\na\nb\nc';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ id: 'unique', startLine: 3, endLine: 3 });
  });

  it('re-anchors a multi-line edit by matching the full block contiguously', () => {
    const edit = makeEdit({
      id: 'multi',
      startLine: 5,
      endLine: 7,
      originalCode: 'foo\nbar\nbaz',
      newCode: 'X',
    });
    // The 3-line block moved from lines 5-7 to lines 2-4.
    const newContent = 'header\nfoo\nbar\nbaz\ntrailer';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 4 });
  });

  it('preserves identity when the edit lines are unchanged', () => {
    const edit = makeEdit({
      id: 'same',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'A',
    });
    const newContent = 'a\nb\nc';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toBe(edit); // same reference
  });

  it('drops an edit when its originalCode no longer appears in the new content', () => {
    const edit = makeEdit({
      id: 'gone',
      startLine: 2,
      endLine: 2,
      originalCode: 'deleted-by-user',
      newCode: 'irrelevant',
    });
    const newContent = 'a\nb\nc';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toEqual([]);
  });

  it('drops an edit when its originalCode appears more than once (ambiguous)', () => {
    // The user duplicated a block; we can't tell which one the edit
    // was anchored to. Better to drop than mis-apply.
    const edit = makeEdit({
      id: 'ambiguous',
      startLine: 3,
      endLine: 3,
      originalCode: 'duplicate',
      newCode: 'fixed',
    });
    const newContent = 'a\nduplicate\nb\nduplicate\nc';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toEqual([]);
  });

  it('passes through edits with empty originalCode unchanged', () => {
    const edit = makeEdit({
      id: 'empty-orig',
      startLine: 1,
      endLine: 1,
      originalCode: '',
      newCode: 'inserted',
    });
    const newContent = 'a\nb';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toEqual([edit]);
    expect(out[0]).toBe(edit);
  });

  it('exact match — whitespace and case sensitive', () => {
    // The match must be exact. Indentation differences are NOT a
    // match; the edit gets dropped. This is intentional — a fuzzy
    // match could re-anchor to a false-positive that happens to be
    // similar but semantically different.
    const edit = makeEdit({
      id: 'whitespace',
      startLine: 1,
      endLine: 1,
      originalCode: '  indented',
      newCode: 'fixed',
    });
    const newContent = 'indented\n  indented';
    // First line is "indented" (no leading spaces) — different.
    // Second line is "  indented" (matches).
    // Exactly one match.
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 2 });
  });

  it('processes multiple edits independently, preserving order', () => {
    const e1 = makeEdit({ id: 'e1', startLine: 5, endLine: 5, originalCode: 'a', newCode: 'A' });
    const e2 = makeEdit({ id: 'e2', startLine: 6, endLine: 6, originalCode: 'gone-from-doc', newCode: 'X' });
    const e3 = makeEdit({ id: 'e3', startLine: 7, endLine: 7, originalCode: 'b', newCode: 'B' });

    const newContent = 'a\nb';
    const out = reanchorEditsForContent([e1, e2, e3], newContent);
    // e2 dropped (no occurrence). e1 and e3 re-anchored to lines 1 / 2.
    expect(out).toHaveLength(2);
    expect(out.map(e => e.id)).toEqual(['e1', 'e3']);
    expect(out[0]).toMatchObject({ startLine: 1, endLine: 1 });
    expect(out[1]).toMatchObject({ startLine: 2, endLine: 2 });
  });

  it('empty input returns empty output', () => {
    expect(reanchorEditsForContent([], 'anything')).toEqual([]);
  });
});

describe('reanchorEditsForContent trailing-newline defensive guard (Group 4 reviewer Issue 3)', () => {
  it('strips a trailing empty element when originalCode ends in \\n', () => {
    // If originalCode = 'foo\n', split('\n') === ['foo', '']. Without
    // the guard, the search would require a blank line right after
    // 'foo' in the doc, which would almost always fail. Current
    // producers don't emit trailing newlines, but this test locks
    // in the defensive behavior so a future producer doesn't trigger
    // a silent regression.
    const edit = makeEdit({
      id: 'trailing-nl',
      startLine: 2,
      endLine: 2,
      originalCode: 'foo\n',
      newCode: 'bar',
    });
    const newContent = 'header\nfoo\ntrailer';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 2 });
  });
});

describe('reanchorEditsForChanges stale-flag computation (Group 5)', () => {
  // The keystroke re-anchor (Group 3) maps line numbers but doesn't
  // validate that the proposal's `originalCode` still matches what's
  // there. The stale flag (Group 5) is set when the user types within
  // the edit's range, surfacing in the UI that approving will
  // overwrite their intervening changes.

  it('marks an edit stale when the user types WITHIN its range', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({
      id: 'within',
      startLine: 3,
      endLine: 3,
      originalCode: 'c',
      newCode: 'C',
    });
    // Insert a character into line 3 ('c' becomes 'cX').
    // Line 3 starts at offset 4 (after 'a\nb\n'), 'c' is at 4..5.
    const { changes, newDoc } = applyChange(oldDoc, { from: 5, to: 5, insert: 'X' });
    expect(newDoc.toString()).toBe('a\nb\ncX\nd\ne');

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 3, endLine: 3, stale: true });
  });

  it('does NOT mark stale when typing happens entirely OUTSIDE the edit range', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd', 'e']);
    const edit = makeEdit({
      id: 'far',
      startLine: 3,
      endLine: 3,
      originalCode: 'c',
      newCode: 'C',
    });
    // Insert at the end of line 5 (offset 9: end of 'e').
    const { changes, newDoc } = applyChange(oldDoc, { from: 9, to: 9, insert: 'Y' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 3, endLine: 3 });
    // Stale should be undefined / false — the change didn't touch
    // the edit's range, so we don't pay the string-compare cost.
    expect(out[0].stale ?? false).toBe(false);
    // Identity preserved (no line shift, no stale change).
    expect(out[0]).toBe(edit);
  });

  it('preserves an existing stale flag when typing far away from a stale edit', () => {
    const oldDoc = Text.of(['a', 'b', 'c', 'd']);
    const edit = makeEdit({
      id: 'pre-stale',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      stale: true,
    });
    // Insert at the end of line 4.
    const { changes, newDoc } = applyChange(oldDoc, { from: 7, to: 7, insert: '!' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 2, stale: true });
    expect(out[0]).toBe(edit); // identity preserved
  });

  it('clears the stale flag when the change brings live content back in sync with originalCode', () => {
    // Simulates: user typed within the range (stale=true), then undid
    // the change (the ChangeSet for the undo restores originalCode at
    // the same lines). The keystroke re-anchor should re-compare and
    // set stale=false.
    const oldDoc = Text.of(['a', 'bX', 'c']); // user has 'bX' at line 2
    const edit = makeEdit({
      id: 'undone',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      stale: true,
    });
    // Delete the 'X' from line 2: 'bX' becomes 'b'.
    // Line 2 starts at offset 2 ('a\n'), 'b' at 2, 'X' at 3.
    const { changes, newDoc } = applyChange(oldDoc, { from: 3, to: 4, insert: '' });
    expect(newDoc.toString()).toBe('a\nb\nc');

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 2, stale: false });
  });

  it('preserves stale=true when a far-away change leaves the drifted content intact', () => {
    const oldDoc = Text.of(['a', 'bX', 'c', 'd']);
    const edit = makeEdit({
      id: 'drifted',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      stale: true,
    });
    // Insert at the end of line 4 — far from the drifted line 2.
    const { changes, newDoc } = applyChange(oldDoc, { from: oldDoc.length, to: oldDoc.length, insert: '!' });

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    // Stale stays true because the drift hasn't been corrected.
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 2, stale: true });
  });
});

describe('reanchorEditsForContent clears stale on successful unique re-anchor (Group 5)', () => {
  it('a unique line-aligned match clears a previously stale flag', () => {
    // Scenario: user typed within an edit's range (stale=true). Then
    // an external reload (e.g. file watcher) restored the original
    // content. The content-search re-anchor proves originalCode is
    // present at the new range, so the proposal is fresh again.
    const edit = makeEdit({
      id: 'restored',
      startLine: 5,
      endLine: 5,
      originalCode: 'b',
      newCode: 'B',
      stale: true,
    });
    const newContent = 'a\nb\nc'; // 'b' is at line 2 of new doc
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 2, stale: false });
  });

  it('unchanged-line case: clears stale flag while preserving line numbers', () => {
    const edit = makeEdit({
      id: 'same-line-clear-stale',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'A',
      stale: true,
    });
    const newContent = 'a\nb';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 1, endLine: 1, stale: false });
    // Identity NOT preserved because stale flag changed.
    expect(out[0]).not.toBe(edit);
  });

  it('preserves identity when nothing changed (lines same, stale already false)', () => {
    const edit = makeEdit({
      id: 'true-noop',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'A',
      // stale not set (== undefined, treated as false)
    });
    const newContent = 'a\nb';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toBe(edit); // identity preserved
  });
});

import { projectEditsForDiffField } from '$lib/modules/editor/aiDiffAnchoring';

describe('projectEditsForDiffField (Group 6 \u2014 perf-skip helper)', () => {
  // Pure decision helper used by Editor.svelte's
  // syncDiffFieldFromPendingEdits to avoid cascading CM decoration
  // recomputes on no-op store notifications.

  it("returns 'unchanged' when both sides are empty", () => {
    expect(projectEditsForDiffField(undefined, [], 100)).toEqual({ kind: 'unchanged' });
    expect(projectEditsForDiffField([], [], 100)).toEqual({ kind: 'unchanged' });
  });

  it("returns 'clear' when fileEdits is empty but current has edits", () => {
    const current = [makeEdit({ id: 'old' })];
    expect(projectEditsForDiffField(undefined, current, 100)).toEqual({ kind: 'clear' });
    expect(projectEditsForDiffField([], current, 100)).toEqual({ kind: 'clear' });
  });

  it("returns 'unchanged' when every projected edit is reference-identical to current", () => {
    // The keystroke re-anchor (Group 3+5) preserves identity for edits
    // it didn't change. A no-op store update fires the subscribe with
    // the same array contents \u2014 the helper must recognize this and
    // skip the dispatch.
    const e1 = makeEdit({ id: 'a', startLine: 1, endLine: 1 });
    const e2 = makeEdit({ id: 'b', startLine: 5, endLine: 5 });
    const fileEdits = [e1, e2];
    const current = [e1, e2]; // same object references
    expect(projectEditsForDiffField(fileEdits, current, 100)).toEqual({ kind: 'unchanged' });
  });

  it("returns 'set' when reference identity differs even if content matches", () => {
    // Conservative: only reference equality counts as no-op. If a
    // re-anchor swapped the object (e.g. flipped stale), the field
    // must rebuild so the widget DOM reflects the new flag.
    const e1 = makeEdit({ id: 'a' });
    const e1Clone = { ...e1 };
    const projection = projectEditsForDiffField([e1Clone], [e1], 100);
    expect(projection.kind).toBe('set');
    if (projection.kind === 'set') {
      expect(projection.edits).toEqual([e1Clone]);
    }
  });

  it('filters out edits whose line range exceeds docLines', () => {
    const inRange = makeEdit({ id: 'in', startLine: 1, endLine: 5 });
    const outOfRange = makeEdit({ id: 'out', startLine: 10, endLine: 12 });
    const projection = projectEditsForDiffField([inRange, outOfRange], [], 8);
    expect(projection.kind).toBe('set');
    if (projection.kind === 'set') {
      expect(projection.edits).toEqual([inRange]);
    }
  });

  it("returns 'clear' when all edits are out of range and current is non-empty", () => {
    const oor = makeEdit({ id: 'oor', startLine: 100, endLine: 100 });
    const current = [makeEdit({ id: 'old' })];
    expect(projectEditsForDiffField([oor], current, 5)).toEqual({ kind: 'clear' });
  });

  it("returns 'unchanged' when all edits are out of range and current is also empty", () => {
    const oor = makeEdit({ id: 'oor', startLine: 100, endLine: 100 });
    expect(projectEditsForDiffField([oor], [], 5)).toEqual({ kind: 'unchanged' });
  });

  it("returns 'set' when the edit count matches but the order differs", () => {
    // Ordering matters \u2014 the field renders decorations in the order
    // we hand them in, and pairwise reference equality enforces that.
    const a = makeEdit({ id: 'a', startLine: 1, endLine: 1 });
    const b = makeEdit({ id: 'b', startLine: 5, endLine: 5 });
    const projection = projectEditsForDiffField([a, b], [b, a], 100);
    expect(projection.kind).toBe('set');
  });
});

describe('CRLF normalization in re-anchor helpers (Group 7)', () => {
  it('reanchorEditsForChanges: typing within range does NOT mark stale when originalCode uses internal CRLF that matches live LF content', () => {
    // Doc: 4 lines. Edit covers lines 2-3 with originalCode 'b\r\nc'
    // (internal CRLF — common when pasting from Windows source).
    // Cache content stores LF; after normalization the originalCode
    // becomes 'b\nc' which matches the live slice exactly.
    const oldDoc = Text.of(['a', 'b', 'c', 'd']);
    const edit = makeEdit({
      id: 'crlf-keystroke',
      startLine: 2,
      endLine: 3,
      originalCode: 'b\r\nc',
      newCode: 'X',
    });
    // Touch the edit's range with a no-op replacement so touchesRange
    // is true and the stale check actually runs.
    // Replace 'c' (offset 4-5) with 'c' — same content, but counts as
    // a change for ChangeSet purposes.
    const { changes, newDoc } = applyChange(oldDoc, { from: 4, to: 5, insert: 'c' });
    expect(newDoc.toString()).toBe('a\nb\nc\nd');

    const out = reanchorEditsForChanges([edit], oldDoc, newDoc, changes);
    expect(out).toHaveLength(1);
    expect(out[0].stale).toBeFalsy();
  });

  it('reanchorEditsForContent: matches an originalCode block that uses CRLF against an LF doc', () => {
    const edit = makeEdit({
      id: 'crlf-content',
      startLine: 5,
      endLine: 6,
      originalCode: 'foo\r\nbar', // CRLF
      newCode: 'X',
    });
    const newContent = 'header\nfoo\nbar\ntrailer';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 3, stale: false });
  });

  it('reanchorEditsForContent: matches lone CR line endings too', () => {
    const edit = makeEdit({
      id: 'cr-content',
      startLine: 5,
      endLine: 6,
      originalCode: 'foo\rbar',
      newCode: 'X',
    });
    const newContent = 'header\nfoo\nbar\ntrailer';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0].startLine).toBe(2);
  });
});

describe('projectEditsForDiffField allocation skip (Group 7 perf)', () => {
  it('returns the input array reference (not a clone) when all edits are in-range and projection differs from current', () => {
    const e1 = makeEdit({ id: 'a', startLine: 1, endLine: 1 });
    const e2 = makeEdit({ id: 'b', startLine: 5, endLine: 5 });
    const fileEdits = [e1, e2];
    // Current is empty so the projection differs; all-in-range path
    // should return fileEdits as-is rather than building a filtered
    // clone.
    const projection = projectEditsForDiffField(fileEdits, [], 100);
    expect(projection.kind).toBe('set');
    if (projection.kind === 'set') {
      expect(projection.edits).toBe(fileEdits);
    }
  });

  it('still falls through to filter when at least one edit is out-of-range', () => {
    const inRange = makeEdit({ id: 'in', startLine: 1, endLine: 1 });
    const outOfRange = makeEdit({ id: 'out', startLine: 100, endLine: 100 });
    const fileEdits = [inRange, outOfRange];
    const projection = projectEditsForDiffField(fileEdits, [], 5);
    expect(projection.kind).toBe('set');
    if (projection.kind === 'set') {
      // The returned array is a filtered clone, not the input.
      expect(projection.edits).not.toBe(fileEdits);
      expect(projection.edits).toEqual([inRange]);
    }
  });

  it('all-in-range fast path still returns unchanged when current matches by reference', () => {
    const e1 = makeEdit({ id: 'a', startLine: 1, endLine: 1 });
    const e2 = makeEdit({ id: 'b', startLine: 5, endLine: 5 });
    expect(projectEditsForDiffField([e1, e2], [e1, e2], 100)).toEqual({ kind: 'unchanged' });
  });
});

describe('reanchorEditsForContent KMP-correctness on overlapping prefixes (Group 8)', () => {
  // The KMP failure-table optimization changes the algorithm but must
  // produce identical results to the naive scan. These tests target
  // patterns where naive vs KMP could diverge if the failure table
  // were computed incorrectly.

  it('matches a search pattern with self-overlap (e.g. ababa)', () => {
    // searchLines = [a, b, a]. In a doc with [a, b, a, b, a], naive
    // would find positions 1 and 3. KMP must too.
    const edit = makeEdit({
      id: 'ababa',
      startLine: 1,
      endLine: 3,
      originalCode: 'a\nb\na',
      newCode: 'X',
    });
    const newContent = 'a\nb\na\nb\na';
    const out = reanchorEditsForContent([edit], newContent);
    // Two occurrences (1 and 3) → ambiguous → drop.
    expect(out).toEqual([]);
  });

  it('matches when search pattern starts with repeated lines', () => {
    // searchLines = [x, x, y]. Failure table needs lps[1] = 1 to skip
    // correctly.
    const edit = makeEdit({
      id: 'xxy',
      startLine: 1,
      endLine: 3,
      originalCode: 'x\nx\ny',
      newCode: 'Z',
    });
    const newContent = 'x\nx\nx\ny';
    // The last 3 lines (x, x, y) match — exactly one occurrence at line 2.
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 2, endLine: 4 });
  });

  it('returns no occurrences when search is longer than doc', () => {
    const edit = makeEdit({
      id: 'too-long',
      startLine: 1,
      endLine: 5,
      originalCode: 'a\nb\nc\nd\ne',
      newCode: 'X',
    });
    const newContent = 'only one line';
    expect(reanchorEditsForContent([edit], newContent)).toEqual([]);
  });

  it('handles a 1-line search by returning every matching line position', () => {
    // searchLines = [foo]. Doc has foo at lines 2 and 5.
    const edit = makeEdit({
      id: 'one-line',
      startLine: 1,
      endLine: 1,
      originalCode: 'foo',
      newCode: 'X',
    });
    const newContent = 'a\nfoo\nb\nc\nfoo\nd';
    // Two occurrences → ambiguous → drop.
    expect(reanchorEditsForContent([edit], newContent)).toEqual([]);
  });

  it('correctness on a large doc (smoke test for the O(D+S) scan)', () => {
    const docLines = [];
    for (let i = 0; i < 5000; i++) docLines.push(`line${i}`);
    docLines[3217] = 'unique-marker';
    docLines[3218] = 'second-marker';
    const newContent = docLines.join('\n');
    const edit = makeEdit({
      id: 'large',
      startLine: 1,
      endLine: 2,
      originalCode: 'unique-marker\nsecond-marker',
      newCode: 'X',
    });
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 3218, endLine: 3219 });
  });
});

describe('Group 8 reviewer test-gap fills', () => {
  it('KMP single-line search with exactly one occurrence re-anchors correctly', () => {
    const edit = makeEdit({
      id: 'one-line-one-match',
      startLine: 1,
      endLine: 1,
      originalCode: 'unique-marker',
      newCode: 'X',
    });
    const newContent = 'a\nb\nunique-marker\nc\nd';
    const out = reanchorEditsForContent([edit], newContent);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ startLine: 3, endLine: 3 });
  });
});
