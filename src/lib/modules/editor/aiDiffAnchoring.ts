import type { Text, ChangeSet } from '@codemirror/state';
import type { EditProposal } from '../ai/editParser';
import { normalizeLineEndings } from '../utils/lineEndings';/**
 * Pure helper that re-anchors a list of `EditProposal`s' line numbers
 * across a CodeMirror document change.
 *
 * Background: pending edits store `startLine` / `endLine` as 1-indexed
 * absolute line numbers. When the user types into a file with pending
 * edits (or any other transaction modifies the doc), those numbers
 * silently go stale — the edit at "line 5" is now actually at line 6
 * because the user inserted a line above. The diff widget then renders
 * against the wrong lines, and `approveEdit` would write at the wrong
 * range.
 *
 * This function maps each edit's start-of-startLine and end-of-endLine
 * positions through the transaction's `ChangeSet` and converts back to
 * line numbers in the new document. Edits whose range is fully consumed
 * by deletion (i.e. the mapped `from` ends up past the mapped `to`)
 * are dropped — the original content no longer exists, so the proposal
 * is meaningless.
 *
 * Invariants:
 *  - The returned array preserves the order of input edits (with
 *    consumed edits removed).
 *  - For edits whose line range is unchanged, the *same* object
 *    reference is returned (cheap reference-equality check downstream).
 *  - Edits with line numbers already outside `oldDoc` (e.g. a stale
 *    proposal carried in across a wholesale doc replacement) are
 *    passed through unchanged. The downstream filter in
 *    `syncDiffFieldFromPendingEdits` hides them from the UI.
 *
 * Pure / synchronous / no I/O: easy to unit-test with synthetic
 * `Text` and `ChangeSet` values.
 */
export function reanchorEditsForChanges(
  edits: readonly EditProposal[],
  oldDoc: Text,
  newDoc: Text,
  changes: ChangeSet,
): EditProposal[] {
  const result: EditProposal[] = [];
  for (const edit of edits) {
    if (edit.startLine < 1 || edit.endLine > oldDoc.lines || edit.endLine < edit.startLine) {
      // Already-invalid input: don't try to map; downstream filtering
      // will hide it from the rendered diff.
      result.push(edit);
      continue;
    }
    const oldFrom = oldDoc.line(edit.startLine).from;
    const oldTo = oldDoc.line(edit.endLine).to;
    // Bias `from` toward the line below (assoc 1) and `to` toward the
    // line above (assoc -1). This makes insertions at the boundary
    // attach to the surrounding text, not the edit's range.
    const newFrom = changes.mapPos(oldFrom, 1);
    const newTo = changes.mapPos(oldTo, -1);
    // Detect full consumption by deletion. `mapPos(p, 1)` on a position
    // inside a deletion lands at the deletion's end; `mapPos(p, -1)`
    // lands at the deletion's start. When the entire `[oldFrom, oldTo]`
    // range is inside one deletion, those two land at the *same* point,
    // i.e. `newFrom === newTo`. Combined with the original range having
    // content (`oldTo > oldFrom`), that signals the user deleted the
    // edit's lines and the proposal's `originalCode` no longer exists
    // in the document. We must drop it — otherwise it'd be silently
    // re-anchored to whatever line the join point now falls on.
    if (oldTo > oldFrom && newTo <= newFrom) {
      continue;
    }
    if (newFrom > newTo) {
      // Defensive: mapPos shouldn't produce an inverted range under
      // normal use, but guard anyway.
      continue;
    }
    const newStartLine = newDoc.lineAt(newFrom).number;
    const newEndLine = newDoc.lineAt(newTo).number;
    // Compute staleness — whether the live content at the new range
    // still matches the proposal's `originalCode`. Skip the string
    // compare entirely when the change didn't touch the edit's range
    // (the common case for typing far away from a pending edit). When
    // untouched, preserve the previous `stale` value: a successful
    // content re-anchor elsewhere (or a fresh proposal) might already
    // have set it appropriately.
    const touched = changes.touchesRange(oldFrom, oldTo);
    let stale = edit.stale ?? false;
    if (touched) {
      const liveContent = newDoc.sliceString(
        newDoc.line(newStartLine).from,
        newDoc.line(newEndLine).to,
      );
      stale = liveContent !== normalizeLineEndings(edit.originalCode);
    }
    const linesChanged = newStartLine !== edit.startLine || newEndLine !== edit.endLine;
    const staleChanged = stale !== (edit.stale ?? false);
    if (!linesChanged && !staleChanged) {
      // No effective change — preserve identity for cheap reference
      // equality at the call site.
      result.push(edit);
    } else {
      result.push({ ...edit, startLine: newStartLine, endLine: newEndLine, stale });
    }
  }
  return result;
}

/**
 * Re-anchor pending edits across a wholesale document replacement
 * (file watcher reload, external edit, fresh disk read after a
 * conflict). `mapPos` is meaningless for these — every old position
 * maps to 0 or the boundary — so we use line-aligned content search
 * instead: find each edit's `originalCode` as a contiguous run of
 * lines in the new content.
 *
 * Resolution rules per edit:
 *   - Exactly one line-aligned occurrence: re-anchor to that
 *     position (update `startLine` / `endLine`).
 *   - Zero occurrences: drop the edit. The original content no
 *     longer exists in the file; applying would replace random lines
 *     with the proposed `newCode`.
 *   - Multiple occurrences: drop. Picking the "right" one would
 *     require heuristics (closest to the original line number?) that
 *     can silently mis-apply the edit. Better to drop and let the
 *     user re-request from the AI than corrupt their file.
 *
 * Edits with empty `originalCode` are passed through unchanged —
 * there's nothing to anchor against. Downstream filters handle them.
 *
 * The match is exact. Whitespace, indentation, and trailing newlines
 * matter. This is intentional: a fuzzy match would risk re-anchoring
 * to a false-positive that happens to be similar.
 */
export function reanchorEditsForContent(
  edits: readonly EditProposal[],
  newContent: string,
): EditProposal[] {
  const newLines = newContent.split('\n');
  const result: EditProposal[] = [];
  for (const edit of edits) {
    if (!edit.originalCode) {
      result.push(edit);
      continue;
    }
    const searchLines = normalizeLineEndings(edit.originalCode).split('\n');
    // Defensive: if `originalCode` ends with `\n`, split produces a
    // trailing empty element that would force the search to require
    // a blank line at that position in the doc.
    if (searchLines.length > 1 && searchLines[searchLines.length - 1] === '') {
      searchLines.pop();
    }
    if (searchLines.length === 0) {
      result.push(edit);
      continue;
    }
    const occurrences = findLineAlignedOccurrences(newLines, searchLines);
    if (occurrences.length !== 1) {
      // 0 → can't anchor. >1 → ambiguous. Drop in either case.
      continue;
    }
    const newStartLine = occurrences[0]; // already 1-indexed
    const newEndLine = newStartLine + searchLines.length - 1;
    // A unique line-aligned match guarantees the live content at the
    // new range equals `originalCode` — the proposal is fresh again,
    // even if it had been marked stale by an earlier keystroke
    // re-anchor. Clearing the flag here lets the diff widget drop the
    // warning indicator after an external reload restores the
    // original lines.
    const newStale = false;
    const linesChanged = newStartLine !== edit.startLine || newEndLine !== edit.endLine;
    const staleChanged = newStale !== (edit.stale ?? false);
    if (!linesChanged && !staleChanged) {
      // Lines unchanged AND stale flag unchanged — preserve identity
      // for cheap reference equality at the call site.
      result.push(edit);
    } else {
      result.push({ ...edit, startLine: newStartLine, endLine: newEndLine, stale: newStale });
    }
  }
  return result;
}

/**
 * Decide what to dispatch to the CM `aiDiffField` for a given file's
 * pending edits. Returns one of:
 *   - `'unchanged'` — projection is reference-identical to `current`;
 *     no dispatch needed (avoids cascading CM decoration recompute on
 *     no-op store updates).
 *   - `'clear'` — projection is empty AND current is non-empty;
 *     dispatch `clearDiffEffect`.
 *   - `EditProposal[]` — dispatch `addDiffEffect.of(value)`.
 *
 * The `'clear' vs 'unchanged'` distinction matters because dispatching
 * `clearDiffEffect` against an already-empty field is wasted work.
 */
export type DiffFieldProjection =
  | { kind: 'unchanged' }
  | { kind: 'clear' }
  | { kind: 'set'; edits: EditProposal[] };

export function projectEditsForDiffField(
  fileEdits: readonly EditProposal[] | undefined,
  current: readonly EditProposal[],
  docLines: number,
): DiffFieldProjection {
  if (!fileEdits || fileEdits.length === 0) {
    return current.length === 0 ? { kind: 'unchanged' } : { kind: 'clear' };
  }
  // Common path: every edit is in-range. Avoid filter+spread allocation
  // by checking in-range up front; fall through only if some edit
  // exceeds the doc.
  let allInRange = true;
  for (const e of fileEdits) {
    if (e.startLine < 1 || e.endLine > docLines) { allInRange = false; break; }
  }
  if (allInRange) {
    if (current.length === fileEdits.length && current.every((e, i) => e === fileEdits[i])) {
      return { kind: 'unchanged' };
    }
    return { kind: 'set', edits: fileEdits as EditProposal[] };
  }
  const validEdits = fileEdits.filter(e => e.startLine >= 1 && e.endLine <= docLines);
  if (validEdits.length === 0) {
    return current.length === 0 ? { kind: 'unchanged' } : { kind: 'clear' };
  }
  if (current.length === validEdits.length && current.every((e, i) => e === validEdits[i])) {
    return { kind: 'unchanged' };
  }
  return { kind: 'set', edits: validEdits };
}

/**
 * 1-indexed positions where `searchLines` appears as a contiguous,
 * exact, line-aligned subsequence within `docLines`.
 *
 * KMP-based: builds a failure table on `searchLines` (cost O(S)) and
 * scans `docLines` once (cost O(D)), giving O(D + S) total — versus
 * the O(D × S) naive nested-loop version. Matters for file-watcher
 * reloads on large files with many pending edits.
 *
 * The "alphabet" is whole lines compared with `===`, so the failure
 * table is indexed by line position rather than character position.
 */
function findLineAlignedOccurrences(docLines: readonly string[], searchLines: readonly string[]): number[] {
  const result: number[] = [];
  const S = searchLines.length;
  const D = docLines.length;
  if (S === 0 || D < S) return result;

  // KMP failure table: lps[i] = length of the longest proper prefix
  // of searchLines[0..i] that is also a suffix.
  const lps = new Int32Array(S);
  let len = 0;
  for (let i = 1; i < S; ) {
    if (searchLines[i] === searchLines[len]) {
      lps[i++] = ++len;
    } else if (len !== 0) {
      len = lps[len - 1];
    } else {
      lps[i++] = 0;
    }
  }

  // Scan.
  let i = 0; // index in docLines
  let j = 0; // index in searchLines
  while (i < D) {
    if (docLines[i] === searchLines[j]) {
      i++;
      j++;
      if (j === S) {
        result.push(i - S + 1); // 1-indexed start
        j = lps[j - 1];
      }
    } else if (j !== 0) {
      j = lps[j - 1];
    } else {
      i++;
    }
  }
  return result;
}
