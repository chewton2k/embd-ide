import { describe, it, expect } from 'vitest';
import { parseMentions, formatMentionsContext, type ResolvedMention } from '$lib/modules/ai/mentions';
import { formatRepoMemory } from '$lib/modules/ai/repoMemory';

describe('mentions', () => {
  describe('parseMentions', () => {
    it('parses @file mention with path', () => {
      const { mentions } = parseMentions('look at @file:src/main.ts please');
      expect(mentions).toHaveLength(1);
      expect(mentions[0].type).toBe('file');
      expect(mentions[0].value).toBe('src/main.ts');
    });

    it('parses @folder mention with path', () => {
      const { mentions } = parseMentions('check @folder:src/lib');
      expect(mentions).toHaveLength(1);
      expect(mentions[0].type).toBe('folder');
      expect(mentions[0].value).toBe('src/lib');
    });

    it('parses @terminal mention without value', () => {
      const { mentions } = parseMentions('see @terminal for errors');
      expect(mentions).toHaveLength(1);
      expect(mentions[0].type).toBe('terminal');
      expect(mentions[0].value).toBe('');
    });

    it('parses @diff mention', () => {
      const { mentions } = parseMentions('review @diff');
      expect(mentions).toHaveLength(1);
      expect(mentions[0].type).toBe('diff');
    });

    it('parses @error mention', () => {
      const { mentions } = parseMentions('fix @error');
      expect(mentions).toHaveLength(1);
      expect(mentions[0].type).toBe('error');
    });

    it('parses multiple mentions', () => {
      const { mentions } = parseMentions('@file:a.ts and @file:b.ts with @diff');
      expect(mentions).toHaveLength(3);
      expect(mentions[0].value).toBe('a.ts');
      expect(mentions[1].value).toBe('b.ts');
      expect(mentions[2].type).toBe('diff');
    });

    it('strips mentions from clean message', () => {
      const { cleanMessage } = parseMentions('fix @file:src/main.ts please');
      expect(cleanMessage).toBe('fix please');
    });

    it('returns empty mentions for no @-mentions', () => {
      const { mentions, cleanMessage } = parseMentions('just a normal message');
      expect(mentions).toHaveLength(0);
      expect(cleanMessage).toBe('just a normal message');
    });

    it('handles @file with nested path', () => {
      const { mentions } = parseMentions('@file:src/lib/modules/ai/tools.ts');
      expect(mentions[0].value).toBe('src/lib/modules/ai/tools.ts');
    });

    it('does not match invalid mention types', () => {
      const { mentions } = parseMentions('@invalid:something');
      expect(mentions).toHaveLength(0);
    });
  });

  describe('formatMentionsContext', () => {
    it('returns empty string for no mentions', () => {
      expect(formatMentionsContext([])).toBe('');
    });

    it('formats file mention with header and code block', () => {
      const resolved: ResolvedMention[] = [
        { type: 'file', label: 'src/main.ts', content: 'const x = 1;' },
      ];
      const result = formatMentionsContext(resolved);
      expect(result).toContain('## Referenced Context');
      expect(result).toContain('### File: src/main.ts');
      expect(result).toContain('```');
      expect(result).toContain('const x = 1;');
    });

    it('formats diff mention', () => {
      const resolved: ResolvedMention[] = [
        { type: 'diff', label: 'Working changes', content: '+added line' },
      ];
      const result = formatMentionsContext(resolved);
      expect(result).toContain('### Git Diff');
    });

    it('formats multiple mentions', () => {
      const resolved: ResolvedMention[] = [
        { type: 'file', label: 'a.ts', content: 'code' },
        { type: 'folder', label: 'src', content: '📁 lib' },
      ];
      const result = formatMentionsContext(resolved);
      expect(result).toContain('### File: a.ts');
      expect(result).toContain('### Folder: src');
    });
  });
});

describe('repoMemory', () => {
  describe('formatRepoMemory', () => {
    it('returns empty string for empty content', () => {
      expect(formatRepoMemory('')).toBe('');
      expect(formatRepoMemory('   ')).toBe('');
    });

    it('wraps content in Project Memory section', () => {
      const result = formatRepoMemory('Always use TypeScript strict mode.');
      expect(result).toContain('## Project Memory');
      expect(result).toContain('Always use TypeScript strict mode.');
    });

    it('preserves multi-line content', () => {
      const content = '- Rule 1\n- Rule 2\n- Rule 3';
      const result = formatRepoMemory(content);
      expect(result).toContain('- Rule 1');
      expect(result).toContain('- Rule 3');
    });
  });
});
