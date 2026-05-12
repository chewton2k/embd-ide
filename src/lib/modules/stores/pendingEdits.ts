import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { EditProposal } from '../ai/editParser';
import { projectRoot } from './git';
import { activeFilePath } from './files';
import { recordAiChange } from './aiHistory';

/** Map of filePath → EditProposal[] for all pending edits across files. */
export const pendingEdits = writable<Record<string, EditProposal[]>>({});

/** Resolve a relative file path from an edit to an absolute path. */
function resolveEditPath(editPath: string): string {
  const root = get(projectRoot);
  if (editPath.startsWith('/')) return editPath;
  if (root) return `${root}/${editPath}`;
  return editPath;
}

/** Add edits from an AI response. Does NOT open new tabs — edits show inline in already-open files. */
export function addEdits(edits: EditProposal[]) {
  const byFile: Record<string, EditProposal[]> = {};

  for (const edit of edits) {
    const fullPath = resolveEditPath(edit.filePath);
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
        // Remap to the active file path
        const resolved = resolveEditPath(edit.filePath);
        if (resolved !== active && byFile[resolved]) {
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
      const content = await invoke<string>('read_file_content', { path: filePath });
      const lines = content.split('\n');

      const before = lines.slice(0, edit.startLine - 1);
      const after = lines.slice(edit.endLine);
      const newLines = edit.newCode.split('\n');
      const newContent = [...before, ...newLines, ...after].join('\n');

      recordAiChange(filePath, `Edit lines ${edit.startLine}-${edit.endLine}`, content, newContent);
      await invoke('write_file_content', { path: filePath, content: newContent });
    } catch (e) {
      console.error('Failed to apply edit:', e);
    }

    // Remove from pending
    pendingEdits.update(current => {
      const updated = { ...current };
      updated[filePath] = edits.filter(e => e.id !== editId);
      if (updated[filePath].length === 0) delete updated[filePath];
      return updated;
    });
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

/** Approve all pending edits across all files. */
export async function approveAll() {
  const all = get(pendingEdits);
  for (const edits of Object.values(all)) {
    for (const edit of edits) {
      await approveEdit(edit.id);
    }
  }
}

/** Reject all pending edits. */
export function rejectAll() {
  pendingEdits.set({});
}
