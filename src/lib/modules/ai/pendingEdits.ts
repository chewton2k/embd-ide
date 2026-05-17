import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';
import type { EditProposal } from './editParser';
import { projectRoot } from '../git/git';
import { activeFilePath } from '../explorer/files';
import { getFileContent, reloadFileContent, registerFileRenameCallback } from '../explorer/files';
import { recordAiChange } from './aiHistory';
import { log } from '../logging';
import { showToast } from '../ui/toast';
import { normalizeLineEndings } from '../utils/lineEndings';

/** Map of filePath → EditProposal[] for all pending edits across files. */
export const pendingEdits = writable<Record<string, EditProposal[]>>({});

/**
 * `trusted: true` bypasses the project-root containment check in
 * `resolveEditPath`. Reserved for paths whose chain of custody we
 * already trust (e.g. the editor's open `filePath` prop). MUST NOT be
 * used with user-controllable strings.
 */
export interface AddEditsOptions {
  trusted?: boolean;
}

function applyEditToContent(content: string, edit: EditProposal): string {
  const lines = content.split('\n');
  const before = lines.slice(0, edit.startLine - 1);
  const after = lines.slice(edit.endLine);
  const newLines = edit.newCode.split('\n');
  return [...before, ...newLines, ...after].join('\n');
}

function removeApprovedEdits(filePath: string, approvedIds: Set<string>) {
  pendingEdits.update(current => {
    const updated = { ...current };
    const remaining = (updated[filePath] || []).filter(e => !approvedIds.has(e.id));
    if (remaining.length > 0) updated[filePath] = remaining;
    else delete updated[filePath];
    return updated;
  });
}

async function loadEditableContent(filePath: string): Promise<string> {
  const cached = getFileContent(filePath);
  if (cached !== null) return cached;
  return invoke<string>('read_file_content', { path: filePath });
}

/**
 * Compare an edit's `originalCode` to the live content slice at its
 * line range. Caller passes already-split lines so a batch processing
 * the same file pays one split, not N.
 *
 * Line endings are normalized to `\n` on the originalCode side so
 * Windows-typed AI providers or copy-pasted CRLF source don't false-
 * positive against the cache (CodeMirror always stores `\n`).
 *
 * Returns `false` (not-stale) on out-of-range to defer to the
 * keystroke / wholesale re-anchor.
 */
function detectStaleAgainstSplitLines(lines: readonly string[], edit: EditProposal): boolean {
  if (edit.startLine < 1 || edit.endLine > lines.length || edit.endLine < edit.startLine) {
    return false;
  }
  const liveSlice = lines.slice(edit.startLine - 1, edit.endLine).join('\n');
  return liveSlice !== normalizeLineEndings(edit.originalCode);
}

/**
 * Resolve a relative or absolute editPath to an absolute path within
 * (untrusted) or anywhere on (trusted) the filesystem.
 *
 * Path traversal: a do/while loop collapses `dir/../` and `./` until
 * stable, so `/repo/foo/../bar.ts` → `/repo/bar.ts`. The trailing
 * guard rejects any residual `..` (paths whose `..` cannot consume
 * because no parent exists, e.g. leading `/..`). Untrusted callers
 * additionally fail any path that doesn't end up under projectRoot.
 *
 * ⚠ Trusted callers MUST pass non-user-controllable strings. The only
 * production trusted caller is `Editor.svelte`'s Cmd+K, which feeds
 * the editor's `filePath` prop; that prop's chain of custody runs
 * through `addFile` / `renameOpenFile` / file-watcher events, all
 * OS-canonical. New trusted call sites must verify the same.
 */
function resolveEditPath(editPath: string, opts: AddEditsOptions = {}): string | null {
  const root = get(projectRoot);
  let resolved: string;
  if (editPath.startsWith('/')) {
    resolved = editPath;
  } else if (root) {
    resolved = `${root}/${editPath}`;
  } else {
    return null;
  }
  let normalized = resolved;
  let prev: string;
  do {
    prev = normalized;
    normalized = normalized.replace(/\/\.\//g, '/').replace(/\/[^/]+\/\.\.\//g, '/');
  } while (normalized !== prev);
  if (!opts.trusted && root && !normalized.startsWith(root)) return null;
  if (normalized.includes('/../') || normalized.endsWith('/..')) return null;
  return normalized;
}

/**
 * Add edits to the canonical store. Each edit's `stale` flag is set
 * by comparing its `originalCode` against the live `fileContentCache`
 * slice when available; this catches AI hallucinations on freeform
 * paths and AI/file races on the agent path before the user types.
 *
 * `opts.trusted = true` bypasses `resolveEditPath`'s project-root check.
 */
export function addEdits(edits: EditProposal[], opts: AddEditsOptions = {}) {
  const byFile: Record<string, EditProposal[]> = {};
  // Cache split lines per file so a batch of N edits in one file
  // doesn't pay N · split() costs for stale detection.
  const splitCache = new Map<string, string[] | null>();
  const linesFor = (path: string): string[] | null => {
    if (splitCache.has(path)) return splitCache.get(path)!;
    const cached = getFileContent(path);
    const split = cached === null ? null : cached.split('\n');
    splitCache.set(path, split);
    return split;
  };
  const markStale = (path: string, edit: EditProposal): EditProposal => {
    const lines = linesFor(path);
    if (lines && detectStaleAgainstSplitLines(lines, edit)) {
      return { ...edit, stale: true };
    }
    return edit;
  };

  for (const edit of edits) {
    const fullPath = resolveEditPath(edit.filePath, opts);
    if (!fullPath) continue;
    const resolved: EditProposal = markStale(fullPath, { ...edit, filePath: fullPath });
    if (!byFile[fullPath]) byFile[fullPath] = [];
    byFile[fullPath].push(resolved);
  }

  // Filename-fallback: when the AI returns a relative path that resolves
  // to something different from the currently-open file but has the
  // same basename, route the edit to the open file's exact path so the
  // diff renders in the editor the user is looking at. Re-run stale
  // detection against the active path's cache (different content than
  // the originally-resolved path) AND rewrite each edit's `filePath`
  // so downstream consumers don't see a stale path string.
  const active = get(activeFilePath);
  if (active && edits.length > 0) {
    for (const edit of edits) {
      const editName = edit.filePath.split('/').pop();
      const activeName = active.split('/').pop();
      if (editName === activeName && !byFile[active]) {
        const resolved = resolveEditPath(edit.filePath, opts);
        if (resolved && resolved !== active && byFile[resolved]) {
          byFile[active] = byFile[resolved].map(e => {
            // Re-detect stale against active's cache; the previous
            // computation used the (different) resolved path's cache.
            const repathed: EditProposal = { ...e, filePath: active };
            // Reset stale before re-detecting so we don't carry over
            // a flag computed against the wrong file.
            delete repathed.stale;
            return markStale(active, repathed);
          });
          delete byFile[resolved];
        }
      }
    }
  }

  pendingEdits.update(current => {
    const updated = { ...current };
    for (const [path, fileEdits] of Object.entries(byFile)) {
      updated[path] = [...(updated[path] || []), ...fileEdits];
    }
    return updated;
  });
}

/**
 * Re-key pending edits when the file tree renames a file. Without
 * this, edits stay stuck under `oldPath` while the editor follows
 * `newPath`, and Approve All would write to `oldPath` (which either
 * fails or, worse, overwrites whatever was renamed into that slot).
 *
 * Exported for direct test access; production callers go through the
 * `registerFileRenameCallback` registration below.
 */
export function rekeyPendingEditsOnRename(oldPath: string, newPath: string): void {
  pendingEdits.update(current => {
    const moving = current[oldPath];
    if (!moving || moving.length === 0) return current;
    const updated = { ...current };
    delete updated[oldPath];
    const moved = moving.map(e => ({ ...e, filePath: newPath }));
    updated[newPath] = updated[newPath] ? [...updated[newPath], ...moved] : moved;
    return updated;
  });
}

const _unregisterRekeyOnRename = registerFileRenameCallback(rekeyPendingEditsOnRename);
if (import.meta.hot) {
  // Prevent duplicate listeners across HMR reloads.
  import.meta.hot.dispose(() => _unregisterRekeyOnRename());
}

/** Approve a single edit — apply the change to the file. */
export async function approveEdit(editId: string) {
  const all = get(pendingEdits);
  for (const [filePath, edits] of Object.entries(all)) {
    const edit = edits.find(e => e.id === editId);
    if (!edit) continue;

    try {
      const content = await loadEditableContent(filePath);
      const newContent = applyEditToContent(content, edit);

      if (edit.stale) {
        log.warn(`approveEdit: applying stale edit for ${filePath} lines ${edit.startLine}-${edit.endLine}; live content has drifted from the proposal's originalCode`);
        // Use present-participle "Applying" (not "Applied") because
        // the write hasn't happened yet — if it fails, we don't want
        // a contradictory pair of toasts ("Applied... / Failed...").
        showToast({
          level: 'warn',
          message: `Applying stale edit at ${displayPath(filePath)} lines ${edit.startLine}-${edit.endLine}; live content has drifted from the proposal's originalCode`,
        });
      }

      // Write first, then record history. If the write throws, we
      // skip recording — phantom entries with a "before" that matches
      // disk are misleading and would make `revertAiChange` a no-op.
      await invoke('write_file_content', { path: filePath, content: newContent });
      recordAiChange(filePath, `Edit lines ${edit.startLine}-${edit.endLine}`, content, newContent);
      reloadFileContent(filePath, newContent);

      const oldLineCount = edit.endLine - edit.startLine + 1;
      const newLineCount = edit.newCode.split('\n').length;
      const delta = newLineCount - oldLineCount;
      removeApprovedEdits(filePath, new Set([editId]));
      const droppedOverlaps = countAndDropOverlappingEdits(filePath, edit.startLine, edit.endLine);
      if (droppedOverlaps > 0) {
        log.warn(`approveEdit: dropped ${droppedOverlaps} overlapping edit(s) for ${filePath} to prevent corruption`);
        showToast({
          level: 'warn',
          message: `Dropped ${droppedOverlaps} overlapping edit(s) for ${displayPath(filePath)} to prevent corruption`,
        });
      }
      if (delta !== 0) {
        shiftEditsAfterApply(filePath, edit.endLine, delta);
      }
      return;
    } catch (e) {
      // Edit stays in `pendingEdits`; the diff widget remains visible
      // (the click handler doesn't mutate CM optimistically) so the
      // user can retry.
      log.error('Failed to apply edit', e);
      showToast({
        level: 'error',
        message: `Failed to apply AI edit to ${displayPath(filePath)}: ${e instanceof Error ? e.message : String(e)}`,
      });
    }
    return;
  }
}

/** Shorten an absolute path for user-facing messages. */
function displayPath(absPath: string): string {
  const root = get(projectRoot);
  if (root && absPath.startsWith(root + '/')) return absPath.slice(root.length + 1);
  if (root && absPath === root) return absPath;
  // Fall back to basename for paths outside the project root.
  const slash = absPath.lastIndexOf('/');
  return slash >= 0 ? absPath.slice(slash + 1) : absPath;
}

/**
 * Shift sibling edits' line numbers after an approveEdit. Skips edits
 * whose range intersects the applied range — those are dropped first
 * by `dropOverlappingEdits` in the approveEdit pipeline. Exported
 * for testability.
 */
export function shiftEditsAfterApply(filePath: string, afterLine: number, delta: number): void {
  if (delta === 0) return;
  pendingEdits.update(current => {
    const fileEdits = current[filePath];
    if (!fileEdits || fileEdits.length === 0) return current;
    let changed = false;
    const shifted = fileEdits.map(e => {
      if (e.startLine > afterLine) {
        changed = true;
        return { ...e, startLine: e.startLine + delta, endLine: e.endLine + delta };
      }
      return e;
    });
    if (!changed) return current;
    return { ...current, [filePath]: shifted };
  });
}

/**
 * Drop pending edits in `filePath` whose range intersects
 * `[appliedStart, appliedEnd]`. Their `originalCode` is now stale
 * post-approve; carrying them forward would let the user approve a
 * corruption. Exported for testability.
 */
export function dropOverlappingEdits(filePath: string, appliedStart: number, appliedEnd: number): void {
  countAndDropOverlappingEdits(filePath, appliedStart, appliedEnd);
}

function countAndDropOverlappingEdits(filePath: string, appliedStart: number, appliedEnd: number): number {
  let droppedCount = 0;
  pendingEdits.update(current => {
    const fileEdits = current[filePath];
    if (!fileEdits || fileEdits.length === 0) return current;
    const kept = fileEdits.filter(e => e.endLine < appliedStart || e.startLine > appliedEnd);
    if (kept.length === fileEdits.length) return current;
    droppedCount = fileEdits.length - kept.length;
    if (kept.length === 0) {
      const { [filePath]: _, ...rest } = current;
      return rest;
    }
    return { ...current, [filePath]: kept };
  });
  return droppedCount;
}

/** Reject a single edit — just remove it from pending. */
export function rejectEdit(editId: string) {
  pendingEdits.update(current => {
    const updated: Record<string, EditProposal[]> = {};
    for (const [path, edits] of Object.entries(current)) {
      const filtered = edits.filter(e => e.id !== editId);
      if (filtered.length > 0) updated[path] = filtered;
    }
    return updated;
  });
}

/**
 * Approve all pending edits. Per file: sort descending by startLine
 * (id-tiebreaker for determinism), drop overlapping edits, apply
 * bottom-up. If any of the surviving (to-apply) edits are stale,
 * prompts the user with a single confirmation dialog before applying
 * any file's edits. Cancel leaves all edits in place.
 */
export async function approveAll() {
  const all = get(pendingEdits);

  // Pre-pass: compute per-file plan + global stale total so we can
  // show a single confirmation dialog (instead of per-file prompts
  // that would interrupt the user repeatedly for a multi-file batch).
  type Plan = { filePath: string; toApply: EditProposal[]; allIds: Set<string>; droppedCount: number };
  const plans: Plan[] = [];
  let totalStale = 0;
  for (const [filePath, edits] of Object.entries(all)) {
    const sorted = [...edits].sort((a, b) => {
      if (b.startLine !== a.startLine) return b.startLine - a.startLine;
      return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
    });
    const acceptedRanges: Array<{ start: number; end: number }> = [];
    const toApply: EditProposal[] = [];
    for (const e of sorted) {
      const overlaps = acceptedRanges.some(r => !(e.endLine < r.start || e.startLine > r.end));
      if (overlaps) continue;
      acceptedRanges.push({ start: e.startLine, end: e.endLine });
      toApply.push(e);
    }
    const droppedCount = sorted.length - toApply.length;
    plans.push({ filePath, toApply, allIds: new Set(edits.map(e => e.id)), droppedCount });
    totalStale += toApply.reduce((n, e) => n + (e.stale ? 1 : 0), 0);
  }

  if (totalStale > 0) {
    const staleFileCount = plans.reduce((n, p) => n + (p.toApply.some(e => e.stale) ? 1 : 0), 0);
    const fileNoun = staleFileCount > 1 ? 'files' : 'file';
    const editNoun = totalStale > 1 ? 'edits' : 'edit';
    const verb = totalStale > 1 ? 'have' : 'has';
    const message = `${totalStale} pending ${editNoun} ${verb} drifted from the original content (typically because you typed within the range). Accepting will overwrite those local changes across ${staleFileCount} ${fileNoun}. Continue?`;
    let confirmed = false;
    try {
      confirmed = await ask(message, { title: 'Apply stale AI edits?', kind: 'warning' });
    } catch (e) {
      // Dialog plugin unavailable (e.g. tests) — proceed conservatively
      // by treating no-response as "no". Tests that want the apply
      // path can mock `ask` to resolve true.
      log.warn('approveAll: confirmation dialog unavailable, aborting stale apply', e);
      confirmed = false;
    }
    if (!confirmed) {
      showToast({ level: 'info', message: 'Cancelled — no edits were applied.' });
      return;
    }
  }

  // Apply phase.
  for (const { filePath, toApply, allIds, droppedCount } of plans) {
    if (droppedCount > 0) {
      log.warn(`approveAll: dropped ${droppedCount} overlapping edit(s) for ${filePath} to prevent corruption`);
      showToast({
        level: 'warn',
        message: `Dropped ${droppedCount} overlapping edit(s) for ${displayPath(filePath)} to prevent corruption`,
      });
    }
    const staleHere = toApply.reduce((n, e) => n + (e.stale ? 1 : 0), 0);
    if (staleHere > 0) {
      log.warn(`approveAll: applying ${staleHere} stale edit(s) for ${filePath}; live content has drifted from the proposals' originalCode`);
      // No toast here — the global confirmation dialog already showed
      // the user the count; per-file toasts would add noise.
    }
    try {
      let content = await loadEditableContent(filePath);
      const historyChain: Array<{ description: string; before: string; after: string }> = [];
      for (const edit of toApply) {
        const newContent = applyEditToContent(content, edit);
        historyChain.push({
          description: `Edit lines ${edit.startLine}-${edit.endLine}`,
          before: content,
          after: newContent,
        });
        content = newContent;
      }
      await invoke('write_file_content', { path: filePath, content });
      // Commit history only after the write succeeds; otherwise we'd
      // leave phantom entries that revertAiChange would write back as
      // a no-op (or, worse, mask the actual disk state from any future
      // diff against the in-memory history).
      for (const h of historyChain) {
        recordAiChange(filePath, h.description, h.before, h.after);
      }
      reloadFileContent(filePath, content);
      removeApprovedEdits(filePath, allIds);
    } catch (e) {
      log.error('Failed to apply edits to ' + filePath, e);
      showToast({ level: 'error', message: `Failed to apply edits to ${displayPath(filePath)}: ${e instanceof Error ? e.message : String(e)}` });
    }
  }
}

/** Reject all pending edits. */
export function rejectAll() {
  pendingEdits.set({});
}
