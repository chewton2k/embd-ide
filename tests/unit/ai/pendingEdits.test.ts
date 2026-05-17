import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { ask } from '@tauri-apps/plugin-dialog';
import { mockInvoke, getInvokeCalls } from '../../mocks/tauri';
import { addFile, activeFilePath, closeAllUnpinned, getFileContent, openFiles, renameOpenFile, updateFileContent } from '$lib/modules/explorer/files';
import { projectRoot } from '$lib/modules/git/git';
import { addEdits, approveAll, approveEdit, pendingEdits, rejectEdit, rekeyPendingEditsOnRename, shiftEditsAfterApply } from '$lib/modules/ai/pendingEdits';
import { log } from '$lib/modules/logging';
import { toasts } from '$lib/modules/ui/toast';
import { aiChangeHistory } from '$lib/modules/ai/aiHistory';

beforeEach(() => {
  closeAllUnpinned();
  openFiles.set([]);
  activeFilePath.set(null);
  projectRoot.set('/repo');
  pendingEdits.set({});
  toasts.set([]);
  aiChangeHistory.set([]);
});

describe('approveEdit', () => {
  it('writes the accepted edit and reloads the open file state', async () => {
    addFile('/repo/src/app.ts', 'app.ts');
    activeFilePath.set('/repo/src/app.ts');

    addEdits([{
      id: 'edit-1',
      filePath: 'src/app.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'const value = 1;',
      newCode: 'const value = 2;',
      status: 'pending',
    }]);

    mockInvoke('read_file_content', () => 'line 1\nconst value = 1;\nline 3');
    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('edit-1');

    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/repo/src/app.ts',
      content: 'line 1\nconst value = 2;\nline 3',
    });
    expect(get(pendingEdits)).toEqual({});
    expect(getFileContent('/repo/src/app.ts')).toBe('line 1\nconst value = 2;\nline 3');
    expect(get(openFiles)[0]).toMatchObject({
      path: '/repo/src/app.ts',
      content: 'line 1\nconst value = 2;\nline 3',
      modified: false,
      version: 1,
    });
  });

  it('keeps the edit pending when applying it fails', async () => {
    addEdits([{
      id: 'edit-2',
      filePath: 'src/app.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'old',
      newCode: 'new',
      status: 'pending',
    }]);

    mockInvoke('read_file_content', () => {
      throw new Error('read failed');
    });

    await approveEdit('edit-2');

    expect(get(pendingEdits)['/repo/src/app.ts']).toHaveLength(1);
  });

  it('applies against live cached content before falling back to disk', async () => {
    addFile('/repo/src/app.ts', 'app.ts');
    updateFileContent('/repo/src/app.ts', 'draft line\nconst value = 1;\nline 3');

    addEdits([{
      id: 'edit-3',
      filePath: 'src/app.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'const value = 1;',
      newCode: 'const value = 2;',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('edit-3');

    expect(getInvokeCalls('read_file_content')).toEqual([]);
    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/repo/src/app.ts',
      content: 'draft line\nconst value = 2;\nline 3',
    });
  });
});

describe('approveAll', () => {
  it('applies all edits to an open file and clears only the successful file bucket', async () => {
    addFile('/repo/src/all.ts', 'all.ts');
    activeFilePath.set('/repo/src/all.ts');

    addEdits([
      {
        id: 'edit-a',
        filePath: 'src/all.ts',
        startLine: 1,
        endLine: 1,
        originalCode: 'const a = 1;',
        newCode: 'const a = 2;',
        status: 'pending',
      },
      {
        id: 'edit-b',
        filePath: 'src/all.ts',
        startLine: 3,
        endLine: 3,
        originalCode: 'const b = 1;',
        newCode: 'const b = 2;',
        status: 'pending',
      },
    ]);

    mockInvoke('read_file_content', () => 'const a = 1;\nkeep\nconst b = 1;');
    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/repo/src/all.ts',
      content: 'const a = 2;\nkeep\nconst b = 2;',
    });
    expect(get(pendingEdits)).toEqual({});
    expect(getFileContent('/repo/src/all.ts')).toBe('const a = 2;\nkeep\nconst b = 2;');
  });

  it('preserves failed file buckets instead of clearing everything', async () => {
    addEdits([
      {
        id: 'edit-ok',
        filePath: 'src/ok.ts',
        startLine: 1,
        endLine: 1,
        originalCode: 'old',
        newCode: 'new',
        status: 'pending',
      },
      {
        id: 'edit-fail',
        filePath: 'src/fail.ts',
        startLine: 1,
        endLine: 1,
        originalCode: 'bad',
        newCode: 'worse',
        status: 'pending',
      },
    ]);

    mockInvoke('read_file_content', ({ path }) => {
      if (path === '/repo/src/fail.ts') throw new Error('boom');
      return 'old';
    });
    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    expect(get(pendingEdits)).toEqual({
      '/repo/src/fail.ts': [
        expect.objectContaining({ id: 'edit-fail' }),
      ],
    });
  });
});

describe('inline-edit (Cmd+K) approve flow — regression', () => {
  // Repro of the original bug: the Cmd+K inline edit path used to bypass
  // `pendingEdits` and write straight to the CodeMirror diff field, so
  // when the user clicked Accept, `approveEdit` couldn't find the id and
  // silently no-op'd. After the fix, `handleInlineEditSubmit` calls
  // `addEdits` (this canonical store), so this id-shaped scenario must
  // round-trip cleanly through approve.
  it('addEdits -> approveEdit writes the file for an inline-shape edit (absolute path)', async () => {
    addFile('/repo/src/inline.ts', 'inline.ts');
    activeFilePath.set('/repo/src/inline.ts');
    updateFileContent('/repo/src/inline.ts', 'a\nb\nc');

    addEdits([{
      id: 'inline-1700000000000-abcd',
      filePath: '/repo/src/inline.ts', // already absolute, like Editor.svelte's prop
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/src/inline.ts']).toHaveLength(1);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('inline-1700000000000-abcd');

    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/repo/src/inline.ts',
      content: 'a\nB\nc',
    });
    expect(get(pendingEdits)).toEqual({});
    expect(getFileContent('/repo/src/inline.ts')).toBe('a\nB\nc');
  });

  it('rejectEdit removes an inline-shape edit from the store', () => {
    addEdits([{
      id: 'inline-rej',
      filePath: '/repo/src/inline.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/src/inline.ts']).toHaveLength(1);

    rejectEdit('inline-rej');

    expect(get(pendingEdits)).toEqual({});
  });

  it('approveEdit on an unknown id is a silent no-op (does not write)', async () => {
    addEdits([{
      id: 'real-id',
      filePath: '/repo/src/inline.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);

    // Idempotency / robustness: simulates the case where the click
    // handler fires after the edit was already removed.
    await approveEdit('non-existent-id');

    expect(getInvokeCalls('write_file_content')).toEqual([]);
    // Pending edit untouched.
    expect(get(pendingEdits)['/repo/src/inline.ts']).toHaveLength(1);
  });

  it('two concurrent approveEdit calls on the same id are safe (idempotent)', async () => {
    addFile('/repo/src/inline.ts', 'inline.ts');
    updateFileContent('/repo/src/inline.ts', 'a\nb\nc');

    addEdits([{
      id: 'inline-race',
      filePath: '/repo/src/inline.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);

    // Simulates the user clicking Accept twice rapidly — both clicks
    // race; the second must not double-apply or corrupt the file.
    await Promise.all([
      approveEdit('inline-race'),
      approveEdit('inline-race'),
    ]);

    const writes = getInvokeCalls('write_file_content');
    // We don't strictly require exactly-one write — both invocations
    // may issue a write (with identical content). What we DO require:
    // every write produces the same idempotent result, and the store
    // converges to empty.
    expect(writes.length).toBeGreaterThanOrEqual(1);
    for (const w of writes) {
      expect((w.args as { content: string }).content).toBe('a\nB\nc');
    }
    expect(get(pendingEdits)).toEqual({});
    expect(getFileContent('/repo/src/inline.ts')).toBe('a\nB\nc');
  });
});

describe('source-of-truth invariant', () => {
  it('addEdits stores under the resolved absolute path even when given a relative path', () => {
    addEdits([{
      id: 'rel',
      filePath: 'src/rel.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/src/rel.ts']).toHaveLength(1);
    // The stored edit's filePath is rewritten to the absolute path so
    // that approveEdit's `invoke('write_file_content', { path })` is
    // unambiguous.
    expect(get(pendingEdits)['/repo/src/rel.ts'][0].filePath).toBe('/repo/src/rel.ts');
  });

  it('addEdits drops edits that escape the project root (security guard)', () => {
    addEdits([{
      id: 'escape',
      filePath: '/etc/passwd',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);

    expect(get(pendingEdits)).toEqual({});
  });

  it('addEdits appends to existing buckets rather than replacing them', () => {
    addEdits([{
      id: 'first',
      filePath: '/repo/a.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);
    addEdits([{
      id: 'second',
      filePath: '/repo/a.ts',
      startLine: 5,
      endLine: 5,
      originalCode: 'p',
      newCode: 'q',
      status: 'pending',
    }]);

    const bucket = get(pendingEdits)['/repo/a.ts'];
    expect(bucket).toHaveLength(2);
    expect(bucket.map(e => e.id)).toEqual(['first', 'second']);
  });
});

describe('addEdits trusted-vs-untrusted (Group 2 fix)', () => {
  // Repro of the deferred Group 1 finding: Cmd+K on a file outside the
  // project root used to silently drop the edit because resolveEditPath
  // enforced project-root containment for *all* callers. The fix adds
  // an `{ trusted: true }` opt-in for user-initiated paths.
  it('untrusted (default) drops out-of-root paths — security guard intact', () => {
    addEdits([{
      id: 'untrusted-escape',
      filePath: '/etc/passwd',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);

    expect(get(pendingEdits)).toEqual({});
  });

  it('trusted: true accepts out-of-root paths (Cmd+K on recent files outside the project)', () => {
    addEdits([{
      id: 'trusted-out-of-root',
      filePath: '/Users/me/scratch/notes.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }], { trusted: true });

    expect(get(pendingEdits)['/Users/me/scratch/notes.ts']).toHaveLength(1);
    expect(get(pendingEdits)['/Users/me/scratch/notes.ts'][0].id).toBe('trusted-out-of-root');
  });

  it('trusted: true accepts paths whose .. segments fully resolve (loop normalizer)', () => {
    // Trusted = the user explicitly opened this file. The normalizer
    // loops until stable so multi-level `..` collapse to canonical
    // form. `/Users/me/scratch/../../../etc/passwd` resolves to
    // `/etc/passwd` (Users/me/scratch all consumed by the three `..`)
    // and is accepted because trusted bypasses the project-root check.
    // In practice trusted paths come from the editor's `filePath`
    // prop, which is OS-canonical and never contains `..`; this test
    // documents the semantics, not a real input shape.
    addEdits([{
      id: 'trusted-fully-resolves',
      filePath: '/Users/me/scratch/../../../etc/passwd',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }], { trusted: true });

    expect(get(pendingEdits)['/etc/passwd']).toHaveLength(1);
  });

  it('trusted: true rejects paths with .. that cannot fully resolve (no parent to consume)', () => {
    // Leading `..` has no parent directory available, so the normalizer
    // leaves them in place. The trailing guard catches them and rejects.
    // This is the actual defense-in-depth invariant under loop
    // normalization: paths that can't fully resolve are rejected
    // regardless of trust.
    addEdits([{
      id: 'trusted-unresolvable',
      filePath: '/../etc/passwd',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }], { trusted: true });

    expect(get(pendingEdits)).toEqual({});
  });

  it('untrusted accepts multi-level .. that fully resolves into project root (false-rejection fix)', () => {
    // Pre-loop normalizer rejected this because of residual `..` after
    // a single pass. Semantically the path is `/repo/safe.ts` (in root)
    // so accepting it is correct.
    addEdits([{
      id: 'untrusted-resolves-into-root',
      filePath: 'foo/bar/../../safe.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/safe.ts']).toHaveLength(1);
  });

  it('untrusted still rejects paths that resolve OUT of project root (root check is primary defense)', () => {
    // Even though `..` fully resolves, the resulting path is outside
    // the project root and the untrusted root check rejects.
    addEdits([{
      id: 'untrusted-resolves-out',
      filePath: 'foo/../../etc/passwd',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }]);

    expect(get(pendingEdits)).toEqual({});
  });

  it('trusted: true approveEdit writes to an out-of-root path', async () => {
    addEdits([{
      id: 'trusted-approve',
      filePath: '/Users/me/scratch/notes.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'x',
      newCode: 'y',
      status: 'pending',
    }], { trusted: true });

    mockInvoke('read_file_content', () => 'x');
    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('trusted-approve');

    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/Users/me/scratch/notes.ts',
      content: 'y',
    });
    expect(get(pendingEdits)).toEqual({});
  });
});

describe('rekeyPendingEditsOnRename (Group 2 fix)', () => {
  // Repro of the deferred Group 1 finding: renaming a file with
  // pending edits left them stranded under the old key — invisible
  // in the diff UI and dangerous for Approve All (would write to the
  // old path).
  it('moves a single bucket from oldPath to newPath and rewrites stored filePath', () => {
    addEdits([{
      id: 'rk-1',
      filePath: '/repo/src/old.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'b',
      status: 'pending',
    }]);

    rekeyPendingEditsOnRename('/repo/src/old.ts', '/repo/src/new.ts');

    expect(get(pendingEdits)['/repo/src/old.ts']).toBeUndefined();
    expect(get(pendingEdits)['/repo/src/new.ts']).toHaveLength(1);
    expect(get(pendingEdits)['/repo/src/new.ts'][0]).toMatchObject({
      id: 'rk-1',
      filePath: '/repo/src/new.ts',
    });
  });

  it('is a no-op for a path with no pending edits', () => {
    addEdits([{
      id: 'rk-noop',
      filePath: '/repo/src/keep.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'b',
      status: 'pending',
    }]);

    const before = get(pendingEdits);
    rekeyPendingEditsOnRename('/repo/src/never-existed.ts', '/repo/src/elsewhere.ts');
    const after = get(pendingEdits);

    // No structural change; same shape.
    expect(after).toEqual(before);
    // Specifically, the rename target shouldn't have been created
    // empty.
    expect(after['/repo/src/elsewhere.ts']).toBeUndefined();
  });

  it('defensively merges with an existing newPath bucket rather than overwriting', () => {
    addEdits([{
      id: 'merge-old',
      filePath: '/repo/old.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'b',
      status: 'pending',
    }]);
    addEdits([{
      id: 'merge-new',
      filePath: '/repo/new.ts',
      startLine: 5,
      endLine: 5,
      originalCode: 'p',
      newCode: 'q',
      status: 'pending',
    }]);

    rekeyPendingEditsOnRename('/repo/old.ts', '/repo/new.ts');

    expect(get(pendingEdits)['/repo/old.ts']).toBeUndefined();
    const merged = get(pendingEdits)['/repo/new.ts'];
    expect(merged).toHaveLength(2);
    // Existing newPath edits come first; renamed edits appended.
    expect(merged.map(e => e.id)).toEqual(['merge-new', 'merge-old']);
    // The renamed edit's filePath was rewritten.
    expect(merged.find(e => e.id === 'merge-old')?.filePath).toBe('/repo/new.ts');
  });

  it('end-to-end: renaming an open file via renameOpenFile triggers the re-key', () => {
    // The pendingEdits module registers itself as a fileRenameCallback
    // at module load. Calling renameOpenFile (which the file-tree UI
    // uses) must therefore re-key pendingEdits transparently.
    addFile('/repo/src/before.ts', 'before.ts');
    addEdits([{
      id: 'e2e',
      filePath: '/repo/src/before.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'b',
      status: 'pending',
    }]);

    renameOpenFile('/repo/src/before.ts', '/repo/src/after.ts', 'after.ts');

    expect(get(pendingEdits)['/repo/src/before.ts']).toBeUndefined();
    expect(get(pendingEdits)['/repo/src/after.ts']).toHaveLength(1);
    expect(get(pendingEdits)['/repo/src/after.ts'][0].filePath).toBe('/repo/src/after.ts');
  });

  it('end-to-end: post-rename Approve writes to the new path, not the old', async () => {
    // This is the dangerous-corruption case from the Group 1 finding:
    // before this fix, Approve would invoke write_file_content with
    // oldPath, which either fails or — if some other file was renamed
    // into that slot — corrupts an unrelated file. After the fix the
    // write must go to newPath.
    addFile('/repo/src/before.ts', 'before.ts');
    updateFileContent('/repo/src/before.ts', 'a\nb');
    addEdits([{
      id: 'e2e-approve',
      filePath: '/repo/src/before.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    renameOpenFile('/repo/src/before.ts', '/repo/src/after.ts', 'after.ts');

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('e2e-approve');

    const writes = getInvokeCalls('write_file_content');
    expect(writes).toHaveLength(1);
    expect(writes[0].args).toEqual({
      path: '/repo/src/after.ts',
      content: 'a\nB',
    });
    expect(get(pendingEdits)).toEqual({});
  });
});

describe('shiftEditsAfterApply (Group 4 fix — sibling edit line numbers)', () => {
  // Repro for the cross-edit shift bug: `approveEdit` on a single
  // edit didn't update the line numbers of other pending edits in
  // the same file. After approving an edit at lines 5-5 with a
  // 3-line `newCode`, the file gains 2 lines but a sibling edit at
  // line 10 stayed at line 10 in the store — visually wrong in the
  // diff widget and dangerous if approved next (would write to the
  // wrong range).
  it('shifts edits whose startLine is strictly below the applied range', () => {
    addEdits([
      { id: 'above', filePath: '/repo/a.ts', startLine: 2, endLine: 2, originalCode: 'a', newCode: 'a', status: 'pending' },
      { id: 'applied', filePath: '/repo/a.ts', startLine: 5, endLine: 5, originalCode: 'b', newCode: 'b', status: 'pending' },
      { id: 'below', filePath: '/repo/a.ts', startLine: 10, endLine: 12, originalCode: 'c', newCode: 'c', status: 'pending' },
      { id: 'far-below', filePath: '/repo/a.ts', startLine: 20, endLine: 20, originalCode: 'd', newCode: 'd', status: 'pending' },
    ]);

    // Pretend the applied edit replaced 1 line (5-5) with 3 lines:
    // delta = +2 for everything strictly below endLine = 5.
    shiftEditsAfterApply('/repo/a.ts', 5, 2);

    const bucket = get(pendingEdits)['/repo/a.ts'];
    const byId = Object.fromEntries(bucket.map(e => [e.id, e]));
    expect(byId.above).toMatchObject({ startLine: 2, endLine: 2 }); // unchanged
    expect(byId.applied).toMatchObject({ startLine: 5, endLine: 5 }); // not strictly above 5
    expect(byId.below).toMatchObject({ startLine: 12, endLine: 14 });
    expect(byId['far-below']).toMatchObject({ startLine: 22, endLine: 22 });
  });

  it('handles negative deltas (newCode shorter than original range)', () => {
    addEdits([
      { id: 'a', filePath: '/repo/a.ts', startLine: 5, endLine: 5, originalCode: 'a', newCode: 'a', status: 'pending' },
      { id: 'b', filePath: '/repo/a.ts', startLine: 10, endLine: 10, originalCode: 'b', newCode: 'b', status: 'pending' },
    ]);

    // Applied edit replaced lines 1-3 with 1 line: delta = -2.
    shiftEditsAfterApply('/repo/a.ts', 3, -2);

    const byId = Object.fromEntries(get(pendingEdits)['/repo/a.ts'].map(e => [e.id, e]));
    expect(byId.a).toMatchObject({ startLine: 3, endLine: 3 });
    expect(byId.b).toMatchObject({ startLine: 8, endLine: 8 });
  });

  it('delta of 0 is a no-op', () => {
    addEdits([
      { id: 'x', filePath: '/repo/a.ts', startLine: 10, endLine: 10, originalCode: 'x', newCode: 'x', status: 'pending' },
    ]);
    const before = get(pendingEdits);
    shiftEditsAfterApply('/repo/a.ts', 1, 0);
    expect(get(pendingEdits)).toBe(before); // identity preserved
  });

  it('does not touch edits in other files', () => {
    addEdits([
      { id: 'a-low', filePath: '/repo/a.ts', startLine: 10, endLine: 10, originalCode: 'a', newCode: 'a', status: 'pending' },
      { id: 'b-low', filePath: '/repo/b.ts', startLine: 10, endLine: 10, originalCode: 'b', newCode: 'b', status: 'pending' },
    ]);

    shiftEditsAfterApply('/repo/a.ts', 5, 3);

    expect(get(pendingEdits)['/repo/a.ts'][0]).toMatchObject({ startLine: 13, endLine: 13 });
    expect(get(pendingEdits)['/repo/b.ts'][0]).toMatchObject({ startLine: 10, endLine: 10 });
  });

  it('end-to-end: approveEdit shifts sibling edits by the applied delta', async () => {
    addFile('/repo/multi.ts', 'multi.ts');
    activeFilePath.set('/repo/multi.ts');
    updateFileContent('/repo/multi.ts', 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10');

    addEdits([
      { id: 'first', filePath: '/repo/multi.ts', startLine: 3, endLine: 3, originalCode: 'l3', newCode: 'A\nB\nC', status: 'pending' },
      { id: 'second', filePath: '/repo/multi.ts', startLine: 7, endLine: 7, originalCode: 'l7', newCode: 'l7-new', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('first');

    // After approving 'first': replaced 1 line with 3 lines, delta = +2.
    // 'second' must now point at line 9 (was 7).
    const remaining = get(pendingEdits)['/repo/multi.ts'];
    expect(remaining).toHaveLength(1);
    expect(remaining[0]).toMatchObject({ id: 'second', startLine: 9, endLine: 9 });
    // And the applied edit's content matches expectations.
    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/repo/multi.ts',
      content: 'l1\nl2\nA\nB\nC\nl4\nl5\nl6\nl7\nl8\nl9\nl10',
    });
  });

  it('end-to-end: now-stale second approve writes the correct range', async () => {
    // Without the shift fix, this would write at the wrong range and
    // corrupt unrelated content. With the fix, the second approve
    // targets the *post-shift* lines and writes correctly.
    addFile('/repo/multi.ts', 'multi.ts');
    activeFilePath.set('/repo/multi.ts');
    updateFileContent('/repo/multi.ts', 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10');

    addEdits([
      { id: 'first', filePath: '/repo/multi.ts', startLine: 3, endLine: 3, originalCode: 'l3', newCode: 'A\nB\nC', status: 'pending' },
      { id: 'second', filePath: '/repo/multi.ts', startLine: 7, endLine: 7, originalCode: 'l7', newCode: 'l7-new', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('first');
    await approveEdit('second');

    const writes = getInvokeCalls('write_file_content');
    expect(writes).toHaveLength(2);
    expect(writes[1].args).toEqual({
      path: '/repo/multi.ts',
      content: 'l1\nl2\nA\nB\nC\nl4\nl5\nl6\nl7-new\nl8\nl9\nl10',
    });
    expect(get(pendingEdits)).toEqual({});
  });
});

describe('approveEdit drops overlapping siblings (Group 4 \u2014 reviewer-flagged corruption fix)', () => {
  // Repro for Issue 1 from the Group 4 reviewer: when edit A at
  // lines 5-10 is approved, sibling B at lines 7-12 was previously
  // left alone (its startLine 7 was not > A.endLine 10). But B's
  // originalCode was captured against pre-A content that no longer
  // exists. Applying B next would overwrite content that A's
  // replacement put there \u2014 silent corruption. The fix drops any
  // edit whose range overlaps A's pre-application range before
  // shifting the rest.
  it('drops a sibling whose range starts inside the applied range', async () => {
    addFile('/repo/over.ts', 'over.ts');
    activeFilePath.set('/repo/over.ts');
    updateFileContent('/repo/over.ts', 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13');

    addEdits([
      // A spans lines 5\u201310.
      { id: 'A', filePath: '/repo/over.ts', startLine: 5, endLine: 10, originalCode: 'l5\nl6\nl7\nl8\nl9\nl10', newCode: 'X', status: 'pending' },
      // B starts inside A (line 7) and ends below it (line 12) \u2014 overlap.
      { id: 'B', filePath: '/repo/over.ts', startLine: 7, endLine: 12, originalCode: 'l7\nl8\nl9\nl10\nl11\nl12', newCode: 'Y', status: 'pending' },
      // C is strictly above (line 2) \u2014 should survive untouched.
      { id: 'C', filePath: '/repo/over.ts', startLine: 2, endLine: 2, originalCode: 'l2', newCode: 'Z', status: 'pending' },
      // D is strictly below (line 13) \u2014 should survive and be shifted.
      { id: 'D', filePath: '/repo/over.ts', startLine: 13, endLine: 13, originalCode: 'l13', newCode: 'W', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('A');

    // B was overlapping; dropped.
    // C is above; unchanged.
    // D is below; shifted by delta = 1 - 6 = -5 (replaced 6 lines with 1).
    const remaining = get(pendingEdits)['/repo/over.ts'];
    const byId = Object.fromEntries(remaining.map(e => [e.id, e]));
    expect(byId.B).toBeUndefined();
    expect(byId.C).toMatchObject({ startLine: 2, endLine: 2 });
    expect(byId.D).toMatchObject({ startLine: 8, endLine: 8 });
    expect(remaining).toHaveLength(2);
  });

  it('drops a sibling whose range fully contains the applied range', async () => {
    addFile('/repo/contain.ts', 'contain.ts');
    activeFilePath.set('/repo/contain.ts');
    updateFileContent('/repo/contain.ts', 'l1\nl2\nl3\nl4\nl5');

    addEdits([
      // A is the small inner edit.
      { id: 'A', filePath: '/repo/contain.ts', startLine: 3, endLine: 3, originalCode: 'l3', newCode: 'X', status: 'pending' },
      // B fully contains A.
      { id: 'B', filePath: '/repo/contain.ts', startLine: 2, endLine: 4, originalCode: 'l2\nl3\nl4', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('A');

    // B fully contained A \u2014 dropped.
    expect(get(pendingEdits)).toEqual({});
  });

  it('drops a sibling identical to the applied range', async () => {
    addFile('/repo/dup.ts', 'dup.ts');
    activeFilePath.set('/repo/dup.ts');
    updateFileContent('/repo/dup.ts', 'l1\nl2\nl3');

    addEdits([
      { id: 'A', filePath: '/repo/dup.ts', startLine: 2, endLine: 2, originalCode: 'l2', newCode: 'X', status: 'pending' },
      { id: 'B', filePath: '/repo/dup.ts', startLine: 2, endLine: 2, originalCode: 'l2', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('A');

    // B targeted the same range; dropped.
    expect(get(pendingEdits)).toEqual({});
  });

  it('shiftEditsAfterApply boundary: edit at startLine === afterLine is NOT shifted', () => {
    // Documents the strict-`>` boundary in shiftEditsAfterApply.
    // An edit whose startLine equals the applied edit's endLine is
    // treated as overlapping (and would be dropped by
    // dropOverlappingEdits in the approveEdit pipeline). The shift
    // function alone leaves it.
    addEdits([
      { id: 'edge', filePath: '/repo/edge.ts', startLine: 5, endLine: 5, originalCode: 'x', newCode: 'x', status: 'pending' },
    ]);
    shiftEditsAfterApply('/repo/edge.ts', 5, 10);
    expect(get(pendingEdits)['/repo/edge.ts'][0]).toMatchObject({ startLine: 5, endLine: 5 });
  });
});

describe('approveAll overlap handling (Group 4 \u2014 round-2 reviewer New Finding)', () => {
  // Repro for the second-pass reviewer's New Finding #2: approveAll
  // applies edits bottom-up (highest startLine first) and assumed
  // non-overlap. With overlapping edits, the second-applied edit's
  // originalCode is captured against pre-application content that no
  // longer exists \u2014 same corruption class as the approveEdit single
  // overlap case fixed earlier in Group 4. The fix pre-filters
  // overlapping edits, keeping the first in sort order (highest
  // startLine) and dropping the rest. Both applied and dropped IDs
  // are cleared from the bucket after a successful write.
  it('drops a lower-startLine edit that overlaps a higher-startLine edit', async () => {
    addFile('/repo/aa-over.ts', 'aa-over.ts');
    activeFilePath.set('/repo/aa-over.ts');
    updateFileContent('/repo/aa-over.ts', 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13');

    addEdits([
      // A: lines 5-10. Will be dropped (B has higher startLine).
      { id: 'A', filePath: '/repo/aa-over.ts', startLine: 5, endLine: 10, originalCode: 'l5\nl6\nl7\nl8\nl9\nl10', newCode: 'X', status: 'pending' },
      // B: lines 7-12. Higher startLine \u2014 wins.
      { id: 'B', filePath: '/repo/aa-over.ts', startLine: 7, endLine: 12, originalCode: 'l7\nl8\nl9\nl10\nl11\nl12', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('read_file_content', () => 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13');
    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    // Only one write \u2014 with B's newCode at lines 7-12.
    const writes = getInvokeCalls('write_file_content');
    expect(writes).toHaveLength(1);
    expect(writes[0].args).toEqual({
      path: '/repo/aa-over.ts',
      content: 'l1\nl2\nl3\nl4\nl5\nl6\nY\nl13',
    });
    // Bucket cleared \u2014 both A and B are gone.
    expect(get(pendingEdits)).toEqual({});
  });

  it('applies non-overlapping edits as before (no regression)', async () => {
    addFile('/repo/aa-clean.ts', 'aa-clean.ts');
    activeFilePath.set('/repo/aa-clean.ts');

    addEdits([
      { id: 'a', filePath: '/repo/aa-clean.ts', startLine: 1, endLine: 1, originalCode: 'const a = 1;', newCode: 'const a = 2;', status: 'pending' },
      { id: 'b', filePath: '/repo/aa-clean.ts', startLine: 3, endLine: 3, originalCode: 'const b = 1;', newCode: 'const b = 2;', status: 'pending' },
    ]);

    mockInvoke('read_file_content', () => 'const a = 1;\nkeep\nconst b = 1;');
    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/repo/aa-clean.ts',
      content: 'const a = 2;\nkeep\nconst b = 2;',
    });
    expect(get(pendingEdits)).toEqual({});
  });

  it('drops two duplicates targeting the same range, deterministic tie-breaker by id', async () => {
    addFile('/repo/aa-dup.ts', 'aa-dup.ts');
    activeFilePath.set('/repo/aa-dup.ts');

    addEdits([
      { id: 'first', filePath: '/repo/aa-dup.ts', startLine: 1, endLine: 1, originalCode: 'a', newCode: 'X', status: 'pending' },
      { id: 'second', filePath: '/repo/aa-dup.ts', startLine: 1, endLine: 1, originalCode: 'a', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('read_file_content', () => 'a');
    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    // Exactly one write applied. The sort uses startLine descending
    // with a secondary lexicographic key on `id`, so on a tie the
    // alphabetically-first id wins. 'first' < 'second', so X wins.
    // Pinning the winner makes the test self-documenting and survives
    // any engine sort-stability quirks.
    const writes = getInvokeCalls('write_file_content');
    expect(writes).toHaveLength(1);
    expect((writes[0].args as { content: string }).content).toBe('X');
    expect(get(pendingEdits)).toEqual({});
  });

  it('end-of-edit overlap (shape d): edit ends inside applied range, starts above', async () => {
    // Round-2 reviewer's missing-test-shape-d gap: B at lines 3-7
    // when A spans 5-10. B.startLine (3) < A.startLine (5),
    // B.endLine (7) > A.startLine. They overlap.
    addFile('/repo/aa-shape-d.ts', 'aa-shape-d.ts');
    activeFilePath.set('/repo/aa-shape-d.ts');
    updateFileContent('/repo/aa-shape-d.ts', 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10');

    addEdits([
      // A: lines 5-10. Higher startLine \u2014 wins in sort order.
      { id: 'A', filePath: '/repo/aa-shape-d.ts', startLine: 5, endLine: 10, originalCode: 'l5\nl6\nl7\nl8\nl9\nl10', newCode: 'X', status: 'pending' },
      // B: lines 3-7. Lower startLine \u2014 dropped.
      { id: 'B', filePath: '/repo/aa-shape-d.ts', startLine: 3, endLine: 7, originalCode: 'l3\nl4\nl5\nl6\nl7', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    expect(getInvokeCalls('write_file_content')[0]?.args).toEqual({
      path: '/repo/aa-shape-d.ts',
      content: 'l1\nl2\nl3\nl4\nX',
    });
    expect(get(pendingEdits)).toEqual({});
  });
});

describe('approveEdit overlap shape coverage (Group 4 \u2014 round-2 reviewer test gap)', () => {
  it('shape (d): drops sibling whose endLine is inside applied range with startLine above', async () => {
    addFile('/repo/shape-d.ts', 'shape-d.ts');
    activeFilePath.set('/repo/shape-d.ts');
    updateFileContent('/repo/shape-d.ts', 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10');

    addEdits([
      // A: applied edit, lines 5-10.
      { id: 'A', filePath: '/repo/shape-d.ts', startLine: 5, endLine: 10, originalCode: 'l5\nl6\nl7\nl8\nl9\nl10', newCode: 'X', status: 'pending' },
      // B: starts above A but ends inside it (startLine 3, endLine 7).
      { id: 'B', filePath: '/repo/shape-d.ts', startLine: 3, endLine: 7, originalCode: 'l3\nl4\nl5\nl6\nl7', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('A');

    // B is dropped (its endLine 7 is within [A.startLine, A.endLine]).
    expect(get(pendingEdits)).toEqual({});
  });
});

describe('approveEdit logs warn when applying a stale edit (Group 5)', () => {
  it('emits log.warn before applying a stale edit, includes file path and line range', async () => {
    addFile('/repo/stale.ts', 'stale.ts');
    activeFilePath.set('/repo/stale.ts');
    updateFileContent('/repo/stale.ts', 'l1\nuser-typed-here\nl3');

    addEdits([{
      id: 'stale-applied',
      filePath: '/repo/stale.ts',
      startLine: 2,
      endLine: 2,
      // originalCode reflects what the AI thought was on line 2 at
      // proposal time. The cached file content has 'user-typed-here'
      // on line 2, so the edit is stale.
      originalCode: 'l2-original',
      newCode: 'l2-replacement',
      status: 'pending',
      stale: true,
    }]);

    mockInvoke('write_file_content', ({ content }) => content);
    const warnSpy = vi.spyOn(log, 'warn').mockImplementation(() => {});

    await approveEdit('stale-applied');

    // The file is still written (user retains authority on their data),
    // BUT a warn was emitted with the actionable details.
    expect(getInvokeCalls('write_file_content')).toHaveLength(1);
    expect(warnSpy).toHaveBeenCalled();
    const warnMessage = warnSpy.mock.calls[0][0];
    expect(warnMessage).toContain('/repo/stale.ts');
    expect(warnMessage).toContain('lines 2-2');
    expect(warnMessage).toContain('drifted');
    expect(get(pendingEdits)).toEqual({});

    warnSpy.mockRestore();
  });

  it('does NOT emit a stale-warn when applying a non-stale edit', async () => {
    addFile('/repo/clean.ts', 'clean.ts');
    activeFilePath.set('/repo/clean.ts');
    updateFileContent('/repo/clean.ts', 'a\nb\nc');

    addEdits([{
      id: 'clean-applied',
      filePath: '/repo/clean.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
      // stale not set
    }]);

    mockInvoke('write_file_content', ({ content }) => content);
    const warnSpy = vi.spyOn(log, 'warn').mockImplementation(() => {});

    await approveEdit('clean-applied');

    // No stale-warn fired. (Other warns might exist for unrelated
    // reasons, so we filter by the stale-specific message text.)
    const staleWarns = warnSpy.mock.calls.filter(args =>
      typeof args[0] === 'string' && args[0].includes('drifted'),
    );
    expect(staleWarns).toHaveLength(0);

    warnSpy.mockRestore();
  });
});

describe('addEdits stale-at-add-time detection (Group 6)', () => {
  // The keystroke re-anchor (Group 3+5) catches drift after the user
  // types. addEdits-time detection (Group 6) catches drift that
  // already exists when the proposal arrives \u2014 typically AI
  // hallucination on the freeform path or an AI/file race on the
  // agent path. No new I/O: looks at fileContentCache only.

  it('marks stale when originalCode does not match cached content at the range', () => {
    addFile('/repo/drift.ts', 'drift.ts');
    updateFileContent('/repo/drift.ts', 'header\nuser-typed\nfooter');

    addEdits([{
      id: 'hallucinated',
      filePath: '/repo/drift.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'l2-as-the-AI-thought',
      newCode: 'replacement',
      status: 'pending',
    }]);

    const e = get(pendingEdits)['/repo/drift.ts'][0];
    expect(e.stale).toBe(true);
  });

  it('does NOT mark stale when originalCode matches cached content', () => {
    addFile('/repo/clean.ts', 'clean.ts');
    updateFileContent('/repo/clean.ts', 'a\nb\nc');

    addEdits([{
      id: 'matches',
      filePath: '/repo/clean.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    const e = get(pendingEdits)['/repo/clean.ts'][0];
    // stale should be undefined / falsy.
    expect(e.stale).toBeFalsy();
  });

  it('multi-line originalCode: matches a contiguous slice exactly', () => {
    addFile('/repo/multi.ts', 'multi.ts');
    updateFileContent('/repo/multi.ts', 'a\nb\nc\nd\ne');

    addEdits([{
      id: 'multi-match',
      filePath: '/repo/multi.ts',
      startLine: 2,
      endLine: 4,
      originalCode: 'b\nc\nd',
      newCode: 'X',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/multi.ts'][0].stale).toBeFalsy();
  });

  it('does NOT mark stale when the file is not cached (no I/O fallback)', () => {
    // No `addFile` / `updateFileContent` for this path \u2014 the cache
    // is empty. addEdits-time detection conservatively defaults to
    // not-stale; the keystroke re-anchor catches drift later if the
    // file is ever opened.
    addEdits([{
      id: 'no-cache',
      filePath: '/repo/uncached.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'whatever',
      newCode: 'X',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/uncached.ts'][0].stale).toBeFalsy();
  });

  it('does NOT mark stale when the line range is out of bounds for the cached content', () => {
    addFile('/repo/short.ts', 'short.ts');
    updateFileContent('/repo/short.ts', 'only-one-line');

    addEdits([{
      id: 'oob',
      filePath: '/repo/short.ts',
      startLine: 5,
      endLine: 5,
      originalCode: 'whatever',
      newCode: 'X',
      status: 'pending',
    }]);

    // Line 5 doesn't exist; we can't compare. Default to not-stale.
    expect(get(pendingEdits)['/repo/short.ts'][0].stale).toBeFalsy();
  });

  it('detects stale per-edit independently within a batch', () => {
    addFile('/repo/mixed.ts', 'mixed.ts');
    updateFileContent('/repo/mixed.ts', 'a\nb\nc\nd');

    addEdits([
      { id: 'good', filePath: '/repo/mixed.ts', startLine: 1, endLine: 1, originalCode: 'a', newCode: 'A', status: 'pending' },
      { id: 'drifted', filePath: '/repo/mixed.ts', startLine: 2, endLine: 2, originalCode: 'b-stale', newCode: 'B', status: 'pending' },
    ]);

    const bucket = get(pendingEdits)['/repo/mixed.ts'];
    const byId = Object.fromEntries(bucket.map(e => [e.id, e]));
    expect(byId.good.stale).toBeFalsy();
    expect(byId.drifted.stale).toBe(true);
  });
});

describe('approveAll stale-edit log (Group 6)', () => {
  it('emits a single combined warn when applying stale edits, not one per edit', async () => {
    addFile('/repo/aa-stale.ts', 'aa-stale.ts');
    updateFileContent('/repo/aa-stale.ts', 'l1\nl2\nl3');
    activeFilePath.set('/repo/aa-stale.ts');

    addEdits([
      { id: 'stale-a', filePath: '/repo/aa-stale.ts', startLine: 1, endLine: 1, originalCode: 'l1-stale', newCode: 'A', status: 'pending' },
      { id: 'stale-b', filePath: '/repo/aa-stale.ts', startLine: 3, endLine: 3, originalCode: 'l3-stale', newCode: 'C', status: 'pending' },
    ]);

    // Both should have been marked stale by addEdits-time detection.
    expect(get(pendingEdits)['/repo/aa-stale.ts'].every(e => e.stale)).toBe(true);

    mockInvoke('write_file_content', ({ content }) => content);
    const warnSpy = vi.spyOn(log, 'warn').mockImplementation(() => {});

    await approveAll();

    const staleWarns = warnSpy.mock.calls.filter(args =>
      typeof args[0] === 'string' && args[0].includes('stale edit'),
    );
    expect(staleWarns).toHaveLength(1);
    expect(staleWarns[0][0]).toContain('2 stale edit(s)');
    expect(staleWarns[0][0]).toContain('/repo/aa-stale.ts');

    warnSpy.mockRestore();
  });
});

describe('addEdits filename-fallback + stale interaction (Group 6 reviewer)', () => {
  // Reviewer Issue #2: when the filename-fallback rekeys edits from
  // `resolved` to `active`, both (a) the stale flag was computed
  // against the wrong file's cache and (b) the edit's filePath
  // property still pointed at `resolved`. After the fix, both are
  // re-computed against the active path.

  it('rewrites filePath on moved edits to the active path', () => {
    projectRoot.set('/repo');
    addFile('/repo/src/foo.ts', 'foo.ts');
    activeFilePath.set('/repo/src/foo.ts');
    updateFileContent('/repo/src/foo.ts', 'a\nb\nc');

    // AI proposes a relative path that resolves to a different absolute
    // path than the active one but shares the same basename.
    // resolveEditPath('foo.ts') with root='/repo' resolves to
    // '/repo/foo.ts' (NOT '/repo/src/foo.ts'). Active is
    // '/repo/src/foo.ts'. Filename matches → fallback fires.
    addEdits([{
      id: 'fallback',
      filePath: 'foo.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    // Edit landed under active's path.
    expect(get(pendingEdits)['/repo/src/foo.ts']).toHaveLength(1);
    expect(get(pendingEdits)['/repo/foo.ts']).toBeUndefined();
    // And the edit's own filePath was rewritten to active.
    expect(get(pendingEdits)['/repo/src/foo.ts'][0].filePath).toBe('/repo/src/foo.ts');
  });

  it('re-detects stale against the active path after rekey, not the resolved path', () => {
    // Set up: two files with the same basename. The AI's originalCode
    // matches the resolved path's cached content but NOT the active
    // path's. After fallback rekey, the edit must be marked stale.
    projectRoot.set('/repo');
    addFile('/repo/foo.ts', 'foo.ts');
    addFile('/repo/src/foo.ts', 'foo.ts');
    activeFilePath.set('/repo/src/foo.ts');
    // resolved cache: matches originalCode 'b'
    updateFileContent('/repo/foo.ts', 'a\nb\nc');
    // active cache: 'b' is replaced with 'b-edited' — stale-trigger
    updateFileContent('/repo/src/foo.ts', 'a\nb-edited\nc');

    addEdits([{
      id: 'fallback-stale',
      filePath: 'foo.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    const moved = get(pendingEdits)['/repo/src/foo.ts'];
    expect(moved).toHaveLength(1);
    expect(moved[0].stale).toBe(true);
  });

  it('clears a previously-set stale flag if active path content matches', () => {
    // Inverse: resolved path has drifted content, active path's cache
    // matches originalCode. Fallback must reset stale to false.
    projectRoot.set('/repo');
    addFile('/repo/foo.ts', 'foo.ts');
    addFile('/repo/src/foo.ts', 'foo.ts');
    activeFilePath.set('/repo/src/foo.ts');
    // resolved: drifted (would mark stale)
    updateFileContent('/repo/foo.ts', 'a\nDRIFT\nc');
    // active: matches originalCode
    updateFileContent('/repo/src/foo.ts', 'a\nb\nc');

    addEdits([{
      id: 'fallback-clear-stale',
      filePath: 'foo.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    const moved = get(pendingEdits)['/repo/src/foo.ts'];
    expect(moved[0].stale).toBeFalsy();
  });
});

describe('addEdits stale detection edge cases (Group 6 reviewer test gaps)', () => {
  it('marks stale when originalCode is empty string but the live line is non-empty', () => {
    // Reviewer test gap: empty originalCode against a non-empty line
    // should be considered drift.
    addFile('/repo/empty.ts', 'empty.ts');
    updateFileContent('/repo/empty.ts', 'a\nsomething\nc');

    addEdits([{
      id: 'empty-orig',
      filePath: '/repo/empty.ts',
      startLine: 2,
      endLine: 2,
      originalCode: '',
      newCode: 'X',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/empty.ts'][0].stale).toBe(true);
  });

  it('does NOT mark stale when originalCode is empty AND the live line is empty', () => {
    addFile('/repo/blank.ts', 'blank.ts');
    updateFileContent('/repo/blank.ts', 'a\n\nc'); // line 2 is literally empty

    addEdits([{
      id: 'empty-match',
      filePath: '/repo/blank.ts',
      startLine: 2,
      endLine: 2,
      originalCode: '',
      newCode: 'X',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/blank.ts'][0].stale).toBeFalsy();
  });

  it('memoizes the file split: a batch with N edits in the same file does not re-split N times', () => {
    // Reviewer perf concern #3: split-cache hoisted in addEdits. We
    // can't easily measure splits without surgery, but we can verify
    // correctness on a large batch.
    addFile('/repo/big.ts', 'big.ts');
    const lines = Array.from({ length: 100 }, (_, i) => `line${i + 1}`);
    updateFileContent('/repo/big.ts', lines.join('\n'));

    // 50 edits, half drifted, half clean.
    const edits = Array.from({ length: 50 }, (_, i) => ({
      id: `e${i}`,
      filePath: '/repo/big.ts',
      startLine: i + 1,
      endLine: i + 1,
      originalCode: i % 2 === 0 ? `line${i + 1}` : `DRIFT${i + 1}`,
      newCode: 'X',
      status: 'pending' as const,
    }));
    addEdits(edits);

    const bucket = get(pendingEdits)['/repo/big.ts'];
    expect(bucket).toHaveLength(50);
    expect(bucket.filter(e => e.stale).map(e => e.id).sort()).toEqual(
      Array.from({ length: 25 }, (_, i) => `e${i * 2 + 1}`).sort(),
    );
  });
});

describe('approveAll mixed stale/non-stale combined warn (Group 6 reviewer test gap)', () => {
  it('counts only stale edits in the combined warn, not non-stale ones', async () => {
    addFile('/repo/mixed-warn.ts', 'mixed-warn.ts');
    activeFilePath.set('/repo/mixed-warn.ts');
    updateFileContent('/repo/mixed-warn.ts', 'l1\nl2\nl3\nl4');

    addEdits([
      // Will detect stale (originalCode mismatch).
      { id: 'a-stale', filePath: '/repo/mixed-warn.ts', startLine: 1, endLine: 1, originalCode: 'l1-DRIFT', newCode: 'A', status: 'pending' },
      // Will NOT be stale (matches).
      { id: 'b-clean', filePath: '/repo/mixed-warn.ts', startLine: 3, endLine: 3, originalCode: 'l3', newCode: 'C', status: 'pending' },
    ]);

    const bucket = get(pendingEdits)['/repo/mixed-warn.ts'];
    expect(bucket.find(e => e.id === 'a-stale')!.stale).toBe(true);
    expect(bucket.find(e => e.id === 'b-clean')!.stale).toBeFalsy();

    mockInvoke('write_file_content', ({ content }) => content);
    const warnSpy = vi.spyOn(log, 'warn').mockImplementation(() => {});

    await approveAll();

    const staleWarns = warnSpy.mock.calls.filter(args =>
      typeof args[0] === 'string' && args[0].includes('stale edit'),
    );
    expect(staleWarns).toHaveLength(1);
    expect(staleWarns[0][0]).toContain('1 stale edit(s)');

    warnSpy.mockRestore();
  });
});

describe('CRLF normalization in stale detection (Group 7)', () => {
  // Producers from Windows-typed AI providers or copy-pasted CRLF
  // source can hand `originalCode` with `\r\n` line endings even
  // though the cache (CodeMirror) always stores `\n`. The comparison
  // must normalize on the originalCode side; otherwise every
  // CRLF-bearing edit false-positives as stale.

  it('does NOT mark stale when originalCode uses \\r\\n but live cache uses \\n', () => {
    addFile('/repo/crlf.ts', 'crlf.ts');
    updateFileContent('/repo/crlf.ts', 'a\nb\nc\nd');

    addEdits([{
      id: 'crlf',
      filePath: '/repo/crlf.ts',
      startLine: 2,
      endLine: 3,
      originalCode: 'b\r\nc',
      newCode: 'X',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/crlf.ts'][0].stale).toBeFalsy();
  });

  it('does NOT mark stale when originalCode uses lone \\r as line ending', () => {
    addFile('/repo/cr.ts', 'cr.ts');
    updateFileContent('/repo/cr.ts', 'a\nb\nc');

    addEdits([{
      id: 'cr',
      filePath: '/repo/cr.ts',
      startLine: 2,
      endLine: 3,
      originalCode: 'b\rc',
      newCode: 'X',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/cr.ts'][0].stale).toBeFalsy();
  });

  it('still marks stale when content genuinely differs (not just line endings)', () => {
    addFile('/repo/real-drift.ts', 'real-drift.ts');
    updateFileContent('/repo/real-drift.ts', 'a\nb\nc');

    addEdits([{
      id: 'real',
      filePath: '/repo/real-drift.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'DRIFTED\r\n',
      newCode: 'X',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/real-drift.ts'][0].stale).toBe(true);
  });
});

describe('toast surface for warns (Group 7)', () => {
  it('shows a warn toast when applying a stale edit (alongside log.warn)', async () => {
    addFile('/repo/stale-toast.ts', 'stale-toast.ts');
    activeFilePath.set('/repo/stale-toast.ts');
    updateFileContent('/repo/stale-toast.ts', 'a\nb\nc');

    addEdits([{
      id: 'stale-toast',
      filePath: '/repo/stale-toast.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'BAD-DRIFT',
      newCode: 'B',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/stale-toast.ts'][0].stale).toBe(true);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('stale-toast');

    const list = get(toasts);
    const staleToasts = list.filter(t =>
      t.level === 'warn' && t.message.includes('stale-toast.ts') && t.message.includes('drifted'),
    );
    expect(staleToasts).toHaveLength(1);
    // displayPath should produce the project-relative path, not the
    // absolute /repo/... in the user-facing toast.
    expect(staleToasts[0].message).not.toContain('/repo/');
  });

  it('shows an error toast when the file write fails', async () => {
    addFile('/repo/fail-toast.ts', 'fail-toast.ts');
    activeFilePath.set('/repo/fail-toast.ts');
    updateFileContent('/repo/fail-toast.ts', 'a');

    addEdits([{
      id: 'fail',
      filePath: '/repo/fail-toast.ts',
      startLine: 1,
      endLine: 1,
      originalCode: 'a',
      newCode: 'A',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', () => {
      throw new Error('disk full');
    });

    await approveEdit('fail');

    const errorToasts = get(toasts).filter(t => t.level === 'error');
    expect(errorToasts).toHaveLength(1);
    expect(errorToasts[0].message).toContain('disk full');
    expect(errorToasts[0].message).toContain('fail-toast.ts');
  });

  it('shows a warn toast when overlapping edits are dropped', async () => {
    addFile('/repo/overlap-toast.ts', 'overlap-toast.ts');
    activeFilePath.set('/repo/overlap-toast.ts');
    updateFileContent('/repo/overlap-toast.ts', 'l1\nl2\nl3\nl4\nl5\nl6');

    addEdits([
      { id: 'A', filePath: '/repo/overlap-toast.ts', startLine: 2, endLine: 4, originalCode: 'l2\nl3\nl4', newCode: 'X', status: 'pending' },
      { id: 'B', filePath: '/repo/overlap-toast.ts', startLine: 3, endLine: 3, originalCode: 'l3', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('A');

    const overlapToasts = get(toasts).filter(t =>
      t.level === 'warn' && t.message.includes('overlapping'),
    );
    expect(overlapToasts).toHaveLength(1);
    expect(overlapToasts[0].message).toContain('1 overlapping edit(s)');
  });
});

describe('approveAll stale-confirmation dialog (Group 7)', () => {
  it('asks the user to confirm before applying any stale edits', async () => {
    addFile('/repo/aa-confirm.ts', 'aa-confirm.ts');
    activeFilePath.set('/repo/aa-confirm.ts');
    updateFileContent('/repo/aa-confirm.ts', 'a\nb\nc');

    addEdits([{
      id: 'stale-aa',
      filePath: '/repo/aa-confirm.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'DRIFT',
      newCode: 'B',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);
    vi.mocked(ask).mockResolvedValueOnce(true);

    await approveAll();

    expect(ask).toHaveBeenCalledTimes(1);
    const [message, opts] = vi.mocked(ask).mock.calls[0];
    expect(message).toContain('1 pending edit');
    expect(message).toContain('drifted');
    expect((opts as { kind: string }).kind).toBe('warning');
    // Confirmed → write happens.
    expect(getInvokeCalls('write_file_content')).toHaveLength(1);
  });

  it('cancels the apply when the user dismisses the dialog', async () => {
    addFile('/repo/aa-cancel.ts', 'aa-cancel.ts');
    activeFilePath.set('/repo/aa-cancel.ts');
    updateFileContent('/repo/aa-cancel.ts', 'a\nb\nc');

    addEdits([{
      id: 'stale-cancel',
      filePath: '/repo/aa-cancel.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'DRIFT',
      newCode: 'B',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);
    vi.mocked(ask).mockResolvedValueOnce(false);

    await approveAll();

    // No write fired.
    expect(getInvokeCalls('write_file_content')).toHaveLength(0);
    // Edits stay in the store so the user can review or accept later.
    expect(get(pendingEdits)['/repo/aa-cancel.ts']).toHaveLength(1);
    // Info toast tells the user the operation was cancelled.
    expect(get(toasts).some(t => t.level === 'info' && t.message.toLowerCase().includes('cancel'))).toBe(true);
  });

  it('does NOT prompt when no edits are stale', async () => {
    addFile('/repo/aa-clean.ts', 'aa-clean.ts');
    activeFilePath.set('/repo/aa-clean.ts');
    updateFileContent('/repo/aa-clean.ts', 'a\nb\nc');

    addEdits([{
      id: 'clean-aa',
      filePath: '/repo/aa-clean.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    expect(ask).not.toHaveBeenCalled();
    expect(getInvokeCalls('write_file_content')).toHaveLength(1);
  });

  it('summarizes counts across multiple files in the confirmation message', async () => {
    addFile('/repo/aa-multi-a.ts', 'aa-multi-a.ts');
    addFile('/repo/aa-multi-b.ts', 'aa-multi-b.ts');
    activeFilePath.set('/repo/aa-multi-a.ts');
    updateFileContent('/repo/aa-multi-a.ts', 'a\nb');
    updateFileContent('/repo/aa-multi-b.ts', 'x\ny');

    addEdits([
      { id: 's-a', filePath: '/repo/aa-multi-a.ts', startLine: 2, endLine: 2, originalCode: 'DRIFT-A', newCode: 'B', status: 'pending' },
      { id: 's-b', filePath: '/repo/aa-multi-b.ts', startLine: 2, endLine: 2, originalCode: 'DRIFT-B', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);
    vi.mocked(ask).mockResolvedValueOnce(true);

    await approveAll();

    const [message] = vi.mocked(ask).mock.calls[0];
    expect(message).toContain('2 pending edits');
    expect(message).toContain('2 files');
  });
});

describe('approveAll dialog edge cases (Group 7 reviewer test gaps)', () => {
  it('treats dialog rejection (throw) as cancel and does not apply', async () => {
    addFile('/repo/aa-throw.ts', 'aa-throw.ts');
    activeFilePath.set('/repo/aa-throw.ts');
    updateFileContent('/repo/aa-throw.ts', 'a\nb\nc');

    addEdits([{
      id: 'stale-throw',
      filePath: '/repo/aa-throw.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'DRIFT',
      newCode: 'B',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);
    vi.mocked(ask).mockRejectedValueOnce(new Error('plugin not loaded'));

    await approveAll();

    expect(getInvokeCalls('write_file_content')).toHaveLength(0);
    expect(get(pendingEdits)['/repo/aa-throw.ts']).toHaveLength(1);
    expect(get(toasts).some(t => t.level === 'info' && t.message.toLowerCase().includes('cancel'))).toBe(true);
  });

  it('reports stale-file count, not total-file count, when only some files have stale edits', async () => {
    addFile('/repo/stale-only.ts', 'stale-only.ts');
    addFile('/repo/clean-only.ts', 'clean-only.ts');
    activeFilePath.set('/repo/stale-only.ts');
    updateFileContent('/repo/stale-only.ts', 'a\nb');
    updateFileContent('/repo/clean-only.ts', 'x\ny');

    addEdits([
      { id: 'stale-here', filePath: '/repo/stale-only.ts', startLine: 2, endLine: 2, originalCode: 'DRIFT', newCode: 'B', status: 'pending' },
      { id: 'clean-here', filePath: '/repo/clean-only.ts', startLine: 2, endLine: 2, originalCode: 'y', newCode: 'Y', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);
    vi.mocked(ask).mockResolvedValueOnce(true);

    await approveAll();

    const [message] = vi.mocked(ask).mock.calls[0];
    // 1 stale edit in 1 file (NOT "across 2 files" — that would
    // include the clean file).
    expect(message).toContain('1 pending edit');
    expect(message).toContain('1 file');
    expect(message).not.toContain('2 files');
  });
});

describe('history-on-success commits (Group 8)', () => {
  // Audit finding: recordAiChange used to fire BEFORE the disk write.
  // If the write threw, history would have a phantom entry whose
  // "before" matches what's still on disk — making revertAiChange a
  // no-op and confusing any future history-replay diagnostic. The fix:
  // write first, record only on success.

  it('approveEdit: history is empty when the write fails', async () => {
    addFile('/repo/hist-fail.ts', 'hist-fail.ts');
    activeFilePath.set('/repo/hist-fail.ts');
    updateFileContent('/repo/hist-fail.ts', 'a\nb\nc');

    addEdits([{
      id: 'fail-hist',
      filePath: '/repo/hist-fail.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', () => {
      throw new Error('disk full');
    });

    await approveEdit('fail-hist');

    expect(get(aiChangeHistory)).toEqual([]);
    // Edit stays pending.
    expect(get(pendingEdits)['/repo/hist-fail.ts']).toHaveLength(1);
  });

  it('approveEdit: history records exactly one entry on success', async () => {
    addFile('/repo/hist-ok.ts', 'hist-ok.ts');
    activeFilePath.set('/repo/hist-ok.ts');
    updateFileContent('/repo/hist-ok.ts', 'a\nb\nc');

    addEdits([{
      id: 'ok-hist',
      filePath: '/repo/hist-ok.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'b',
      newCode: 'B',
      status: 'pending',
    }]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveEdit('ok-hist');

    const history = get(aiChangeHistory);
    expect(history).toHaveLength(1);
    expect(history[0]).toMatchObject({
      filePath: '/repo/hist-ok.ts',
      description: 'Edit lines 2-2',
      beforeContent: 'a\nb\nc',
      afterContent: 'a\nB\nc',
    });
  });

  it('approveAll: history is empty for a file whose write fails', async () => {
    addFile('/repo/aa-hist-fail.ts', 'aa-hist-fail.ts');
    activeFilePath.set('/repo/aa-hist-fail.ts');
    updateFileContent('/repo/aa-hist-fail.ts', 'a\nb\nc\nd');

    addEdits([
      { id: 'a1', filePath: '/repo/aa-hist-fail.ts', startLine: 1, endLine: 1, originalCode: 'a', newCode: 'A', status: 'pending' },
      { id: 'a2', filePath: '/repo/aa-hist-fail.ts', startLine: 3, endLine: 3, originalCode: 'c', newCode: 'C', status: 'pending' },
    ]);

    mockInvoke('write_file_content', () => {
      throw new Error('disk full');
    });

    await approveAll();

    // No history entries — neither edit was actually applied.
    expect(get(aiChangeHistory)).toEqual([]);
    // Edits stay pending so the user can retry.
    expect(get(pendingEdits)['/repo/aa-hist-fail.ts']).toHaveLength(2);
  });

  it('approveAll: history records one entry per applied edit on success, before/after chain is correct', async () => {
    addFile('/repo/aa-hist-ok.ts', 'aa-hist-ok.ts');
    activeFilePath.set('/repo/aa-hist-ok.ts');
    updateFileContent('/repo/aa-hist-ok.ts', 'a\nb\nc\nd');

    addEdits([
      { id: 'a1', filePath: '/repo/aa-hist-ok.ts', startLine: 1, endLine: 1, originalCode: 'a', newCode: 'A', status: 'pending' },
      { id: 'a2', filePath: '/repo/aa-hist-ok.ts', startLine: 3, endLine: 3, originalCode: 'c', newCode: 'C', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ content }) => content);

    await approveAll();

    // Two history entries (sorted descending by startLine, so the
    // higher-line edit applies first). aiChangeHistory pushes newest
    // to the front, so [0] is the second-applied entry.
    const history = get(aiChangeHistory);
    expect(history).toHaveLength(2);
    // The chain: before → afterFirstApply → afterSecondApply.
    // Bottom-up applies the line-3 edit first, then line-1.
    // Latest entry (history[0]) = the second apply: line-1.
    expect(history[0]).toMatchObject({
      description: 'Edit lines 1-1',
      beforeContent: 'a\nb\nC\nd',
      afterContent: 'A\nb\nC\nd',
    });
    expect(history[1]).toMatchObject({
      description: 'Edit lines 3-3',
      beforeContent: 'a\nb\nc\nd',
      afterContent: 'a\nb\nC\nd',
    });
  });
});

describe('Group 8 reviewer test-gap fills', () => {
  it("stale toast on write failure says 'Applying' (not 'Applied') so the toast pair isn't contradictory", async () => {
    addFile('/repo/stale-fail.ts', 'stale-fail.ts');
    activeFilePath.set('/repo/stale-fail.ts');
    updateFileContent('/repo/stale-fail.ts', 'a\nb\nc');

    addEdits([{
      id: 'stale-then-fail',
      filePath: '/repo/stale-fail.ts',
      startLine: 2,
      endLine: 2,
      originalCode: 'DRIFTED',
      newCode: 'B',
      status: 'pending',
    }]);

    expect(get(pendingEdits)['/repo/stale-fail.ts'][0].stale).toBe(true);

    mockInvoke('write_file_content', () => {
      throw new Error('disk full');
    });

    await approveEdit('stale-then-fail');

    const warnToasts = get(toasts).filter(t => t.level === 'warn' && t.message.includes('drifted'));
    expect(warnToasts).toHaveLength(1);
    // Critical: the stale toast must use the present participle so it
    // doesn't claim success when paired with the failure toast.
    expect(warnToasts[0].message).toContain('Applying');
    expect(warnToasts[0].message).not.toContain('Applied');

    const errorToasts = get(toasts).filter(t => t.level === 'error');
    expect(errorToasts).toHaveLength(1);
    expect(errorToasts[0].message).toContain('disk full');
  });

  it('approveAll: per-file write failure is isolated — successful files clear, failed files retain edits', async () => {
    addFile('/repo/aa-iso-ok.ts', 'aa-iso-ok.ts');
    addFile('/repo/aa-iso-fail.ts', 'aa-iso-fail.ts');
    activeFilePath.set('/repo/aa-iso-ok.ts');
    updateFileContent('/repo/aa-iso-ok.ts', 'a\nb');
    updateFileContent('/repo/aa-iso-fail.ts', 'x\ny');

    addEdits([
      { id: 'ok-edit', filePath: '/repo/aa-iso-ok.ts', startLine: 1, endLine: 1, originalCode: 'a', newCode: 'A', status: 'pending' },
      { id: 'fail-edit', filePath: '/repo/aa-iso-fail.ts', startLine: 1, endLine: 1, originalCode: 'x', newCode: 'X', status: 'pending' },
    ]);

    mockInvoke('write_file_content', ({ path, content }) => {
      if (path === '/repo/aa-iso-fail.ts') throw new Error('readonly');
      return content;
    });

    await approveAll();

    // Successful file: edit cleared.
    expect(get(pendingEdits)['/repo/aa-iso-ok.ts']).toBeUndefined();
    // Failed file: edit stays so the user can retry.
    expect(get(pendingEdits)['/repo/aa-iso-fail.ts']).toHaveLength(1);
    // History only records the successful apply.
    const history = get(aiChangeHistory);
    expect(history).toHaveLength(1);
    expect(history[0].filePath).toBe('/repo/aa-iso-ok.ts');
    // Error toast surfaces the failed file.
    expect(get(toasts).some(t => t.level === 'error' && t.message.includes('aa-iso-fail.ts'))).toBe(true);
  });
});
