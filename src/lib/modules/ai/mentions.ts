/**
 * @-mention parsing for chat input.
 *
 * Parses mentions like @file:src/main.ts, @folder:src/lib, @terminal,
 * @diff from user messages and resolves them into structured context
 * that gets injected into the AI request.
 */
import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { projectRoot } from '../git/git';

// ── Types ──

export type MentionType = 'file' | 'folder' | 'terminal' | 'diff' | 'error';

export interface Mention {
  type: MentionType;
  value: string; // path for file/folder, empty for terminal/diff/error
  raw: string;   // original matched text (e.g. "@file:src/main.ts")
}

export interface ResolvedMention {
  type: MentionType;
  label: string;
  content: string;
}

// ── Parsing ──

const MENTION_REGEX = /@(file|folder|terminal|diff|error)(?::([^\s]+))?/g;

/**
 * Parse @-mentions from a user message.
 * Returns the mentions found and the message with mentions stripped.
 */
export function parseMentions(message: string): { mentions: Mention[]; cleanMessage: string } {
  const mentions: Mention[] = [];
  let cleanMessage = message;

  const matches = [...message.matchAll(MENTION_REGEX)];
  for (const match of matches) {
    const [raw, type, value] = match;
    mentions.push({
      type: type as MentionType,
      value: value || '',
      raw,
    });
    cleanMessage = cleanMessage.replace(raw, '');
  }

  cleanMessage = cleanMessage.replace(/\s{2,}/g, ' ').trim();

  return { mentions, cleanMessage };
}

/**
 * Resolve mentions into actual content by reading files, etc.
 */
export async function resolveMentions(mentions: Mention[]): Promise<ResolvedMention[]> {
  const root = get(projectRoot) || '';
  const resolved: ResolvedMention[] = [];

  for (const mention of mentions) {
    try {
      const result = await resolveSingle(mention, root);
      if (result) resolved.push(result);
    } catch {
      // Skip unresolvable mentions
    }
  }

  return resolved;
}

async function resolveSingle(mention: Mention, root: string): Promise<ResolvedMention | null> {
  switch (mention.type) {
    case 'file': {
      if (!mention.value) return null;
      const path = mention.value.startsWith('/') ? mention.value : `${root}/${mention.value}`;
      const content = await invoke<string>('read_file_content', { path });
      const truncated = content.length > 8000
        ? content.slice(0, 8000) + '\n... (truncated)'
        : content;
      return { type: 'file', label: mention.value, content: truncated };
    }

    case 'folder': {
      if (!mention.value) return null;
      const path = mention.value.startsWith('/') ? mention.value : `${root}/${mention.value}`;
      const entries = await invoke<{ name: string; is_dir: boolean }[]>('read_dir_tree', { path, depth: 2 });
      const listing = entries.map(e => `${e.is_dir ? '📁' : '📄'} ${e.name}`).join('\n');
      return { type: 'folder', label: mention.value, content: listing };
    }

    case 'terminal':
      // Terminal content would come from the terminal store — placeholder
      return { type: 'terminal', label: 'Terminal', content: '(terminal output not yet captured)' };

    case 'diff': {
      try {
        const diff = await invoke<string>('git_diff', { repoPath: root, filePath: null });
        const truncated = diff.length > 4000 ? diff.slice(0, 4000) + '\n... (truncated)' : diff;
        return { type: 'diff', label: 'Working changes', content: truncated || '(no changes)' };
      } catch {
        return { type: 'diff', label: 'Diff', content: '(no git diff available)' };
      }
    }

    case 'error':
      return { type: 'error', label: 'Errors', content: '(diagnostics not yet captured)' };

    default:
      return null;
  }
}

/**
 * Format resolved mentions into a context string for the AI.
 */
export function formatMentionsContext(resolved: ResolvedMention[]): string {
  if (resolved.length === 0) return '';

  const sections = resolved.map(m => {
    const header = m.type === 'file' ? `File: ${m.label}` :
                   m.type === 'folder' ? `Folder: ${m.label}` :
                   m.type === 'diff' ? 'Git Diff' :
                   m.type === 'terminal' ? 'Terminal Output' :
                   'Diagnostics';
    return `### ${header}\n\`\`\`\n${m.content}\n\`\`\``;
  });

  return '\n## Referenced Context\n' + sections.join('\n\n') + '\n';
}
