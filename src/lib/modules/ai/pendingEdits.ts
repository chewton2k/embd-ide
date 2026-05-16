import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { EditProposal } from './editParser';
import { projectRoot } from '../git/git';
import { activeFilePath } from '../explorer/files';
import { getFileContent, reloadFileContent } from '../explorer/files';
import { recordAiChange } from './aiHistory';
import { log } from '../logging';

/** Map of filePath → EditProposal[] for all pending edits across files. */
export const pendingEdits = writable<Record<string, EditProposal[]>>({});

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

/** Resolve a relative file path from an edit to an absolute path.
 *  Validates the result stays within the project root. */
function resolveEditPath(editPath: string): string | null {
  const root = get(projectRoot);
  let resolved: string;
  if (editPath.startsWith('/')) {
    resolved = editPath;
  } else if (root) {
    resolved = `${root}/${editPath}`;
  } else {
    return null;
  }
  // Normalize and reject path traversal
  const normalized = resolved.replace(/\/\.\//g, '/').replace(/\/[^/]+\/\.\.\//g, '/');
  if (root && !normalized.startsWith(root)) return null;
  if (normalized.includes('/../') || normalized.endsWith('/..')) return null;
  return normalized;
}

/** Add edits from an AI response. Does NOT open new tabs — edits show inline in already-open files. */
export function addEdits(edits: EditProposal[]) {
  const byFile: Record<string, EditProposal[]> = {};

  for (const edit of edits) {
    const fullPath = resolveEditPath(edit.filePath);
    if (!fullPath) continue;
    if (!byFile[fullPath]) byFile[fullPath] = [];
    byFile[fullPath].push({ ...edit, filePath: fullPath });
  }

  // If the edit targets the currently active file, use that path directly
  const active = get(activeFilePath);
  if (active && edits.length > 0) {
    // Check if any edit's filename matches the active file's filename
    for (const edit of edits) {
      const editName = edit.filePath.split('/').pop();
      const activeName = active.split('/').pop();
      if (editName === activeName && !byFile[active]) {
        const resolved = resolveEditPath(edit.filePath);
        if (resolved && resolved !== active && byFile[resolved]) {
          byFile[active] = byFile[resolved];
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

/** Approve a single edit — apply the change to the file. */
export async function approveEdit(editId: string) {
  const all = get(pendingEdits);
  for (const [filePath, edits] of Object.entries(all)) {
    const edit = edits.find(e => e.id === editId);
    if (!edit) continue;

    try {
      const content = await loadEditableContent(filePath);
      const newContent = applyEditToContent(content, edit);

      recordAiChange(filePath, `Edit lines ${edit.startLine}-${edit.endLine}`, content, newContent);
      await invoke('write_file_content', { path: filePath, content: newContent });
      reloadFileContent(filePath, newContent);
      removeApprovedEdits(filePath, new Set([editId]));
      return;
    } catch (e) {
      log.error('Failed to apply edit', e);
    }
    return;
  }
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

/** Approve all pending edits across all files.
 *  Applies edits bottom-up (highest startLine first) within each file
 *  so that earlier line numbers remain valid after each edit is applied. */
export async function approveAll() {
  const all = get(pendingEdits);
  for (const [filePath, edits] of Object.entries(all)) {
    // Sort descending by startLine so we apply from bottom to top
    const sorted = [...edits].sort((a, b) => b.startLine - a.startLine);
    try {
      let content = await loadEditableContent(filePath);
      for (const edit of sorted) {
        const newContent = applyEditToContent(content, edit);
        recordAiChange(filePath, `Edit lines ${edit.startLine}-${edit.endLine}`, content, newContent);
        content = newContent;
      }
      await invoke('write_file_content', { path: filePath, content });
      reloadFileContent(filePath, content);
      removeApprovedEdits(filePath, new Set(edits.map(edit => edit.id)));
    } catch (e) {
      log.error('Failed to apply edits to ' + filePath, e);
    }
  }
}

/** Reject all pending edits. */
export function rejectAll() {
  pendingEdits.set({});
}
