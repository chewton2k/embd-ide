import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { projectRoot } from '../git/git';
import { activeFile } from '../explorer/files';
import { loadRepoMemory, formatRepoMemory } from './repoMemory';

interface FileInfo {
  path: string;
  language: string;
  summary: string;
  exports: string;
}

/**
 * Query the knowledge store for relevant project context based on the user's message.
 * Returns a context string to inject into the system prompt.
 */
export async function buildProjectContext(userMessage: string): Promise<string> {
  const root = get(projectRoot);
  if (!root) return '';

  const currentFile = get(activeFile);
  const relFile = currentFile?.replace(root + '/', '') || undefined;

  let context = '';

  // Load repo memory (project-specific instructions)
  const memory = await loadRepoMemory();
  if (memory) context += formatRepoMemory(memory);

  try {
    const files = await invoke<FileInfo[]>('knowledge_get_context', {
      projectRoot: root,
      query: userMessage,
      currentFile: relFile ?? null,
    });

    if (files.length > 0) {
      const lines = files.map(f => {
        let line = `- ${f.path}`;
        if (f.exports) line += `: exports ${f.exports}`;
        else if (f.summary) line += `: ${f.summary}`;
        return line;
      });
      context += `\n## Project Context\n### Relevant Files:\n${lines.join('\n')}\n`;
    }
  } catch {
    // Knowledge store unavailable — continue with memory only
  }

  return context;
}

/** Initialize the knowledge store and trigger indexing for the current project. */
export async function initKnowledge(): Promise<void> {
  const root = get(projectRoot);
  if (!root) return;

  try {
    await invoke('knowledge_init', { projectRoot: root });
    // Index in background — don't await
    invoke('knowledge_index', { projectRoot: root }).catch(() => {});
  } catch {
    // Knowledge store is optional — don't break the app
  }
}
