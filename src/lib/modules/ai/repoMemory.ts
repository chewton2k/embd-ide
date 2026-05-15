/**
 * Repo memory — auto-load .leo/memory.md into agent context.
 *
 * Similar to .cursorrules or CLAUDE.md, this file persists project-specific
 * instructions that the agent should always know about. The agent can also
 * update it ("remember this for next time").
 */
import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { projectRoot } from '../git/git';

const MEMORY_PATHS = ['.leo/memory.md', '.leo/MEMORY.md', 'CLAUDE.md', '.cursorrules'];

/**
 * Load the repo memory file content.
 * Tries multiple conventional paths in order.
 * Returns empty string if no memory file exists (non-fatal).
 */
export async function loadRepoMemory(): Promise<string> {
  const root = get(projectRoot);
  if (!root) return '';

  for (const relPath of MEMORY_PATHS) {
    try {
      const content = await invoke<string>('read_file_content', { path: `${root}/${relPath}` });
      if (content.trim()) {
        // Truncate very large memory files
        const MAX = 4000;
        if (content.length > MAX) {
          return content.slice(0, MAX) + '\n... (memory file truncated)';
        }
        return content;
      }
    } catch {
      // File doesn't exist, try next
    }
  }

  return '';
}

/**
 * Format repo memory for injection into the system prompt.
 */
export function formatRepoMemory(content: string): string {
  if (!content.trim()) return '';
  return `\n## Project Memory\n${content}\n`;
}

/**
 * Get the path where memory should be written.
 */
export function getMemoryPath(): string {
  const root = get(projectRoot);
  return root ? `${root}/.leo/memory.md` : '';
}
