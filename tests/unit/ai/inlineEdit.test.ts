import { describe, it, expect } from 'vitest';
import { buildInlineEditPrompt, cleanResponse } from '$lib/modules/ai/inlineEdit';

describe('inlineEdit', () => {
  describe('buildInlineEditPrompt', () => {
    it('includes the selected code in a fenced block', () => {
      const prompt = buildInlineEditPrompt('const x = 1;', 'make it a let', 'src/main.ts');
      expect(prompt).toContain('```ts\nconst x = 1;\n```');
    });

    it('includes the user instruction', () => {
      const prompt = buildInlineEditPrompt('foo()', 'add error handling', 'app.js');
      expect(prompt).toContain('Instruction: add error handling');
    });

    it('includes the file path', () => {
      const prompt = buildInlineEditPrompt('x', 'fix', 'src/lib/utils.ts');
      expect(prompt).toContain('`src/lib/utils.ts`');
    });

    it('extracts file extension for the code fence language', () => {
      const py = buildInlineEditPrompt('def foo():', 'add type hints', 'main.py');
      expect(py).toContain('```py');

      const rs = buildInlineEditPrompt('fn main() {}', 'add docs', 'src/main.rs');
      expect(rs).toContain('```rs');
    });

    it('handles files with no extension', () => {
      const prompt = buildInlineEditPrompt('#!/bin/bash', 'fix', 'Makefile');
      // Should not crash, uses empty string for language
      expect(prompt).toContain('```\n#!/bin/bash');
    });

    it('instructs the model to respond with only code', () => {
      const prompt = buildInlineEditPrompt('x', 'fix', 'a.ts');
      expect(prompt).toContain('ONLY the replacement code');
      expect(prompt).toContain('No explanations');
    });
  });

  describe('cleanResponse', () => {
    it('returns plain code unchanged', () => {
      expect(cleanResponse('const x = 1;')).toBe('const x = 1;');
    });

    it('strips markdown fences with language tag', () => {
      const input = '```typescript\nconst x = 1;\n```';
      expect(cleanResponse(input)).toBe('const x = 1;');
    });

    it('strips markdown fences without language tag', () => {
      const input = '```\nconst x = 1;\n```';
      expect(cleanResponse(input)).toBe('const x = 1;');
    });

    it('preserves code that contains backticks but is not fenced', () => {
      const input = 'const s = `hello ${name}`;';
      expect(cleanResponse(input)).toBe('const s = `hello ${name}`;');
    });

    it('handles multi-line code in fences', () => {
      const input = '```js\nfunction add(a, b) {\n  return a + b;\n}\n```';
      expect(cleanResponse(input)).toBe('function add(a, b) {\n  return a + b;\n}');
    });

    it('trims leading/trailing whitespace', () => {
      expect(cleanResponse('  const x = 1;  ')).toBe('const x = 1;');
    });

    it('handles empty response', () => {
      expect(cleanResponse('')).toBe('');
    });

    it('handles response that is only fences', () => {
      expect(cleanResponse('```\n```')).toBe('');
    });

    it('does not strip fences if only opening fence exists', () => {
      const input = '```ts\nconst x = 1;';
      // No closing fence — should not strip
      expect(cleanResponse(input)).toBe('```ts\nconst x = 1;');
    });
  });
});
