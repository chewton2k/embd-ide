import { describe, expect, it } from 'vitest';
import { normalizeLineEndings } from '$lib/modules/utils/lineEndings';

describe('normalizeLineEndings', () => {
  it('converts \\r\\n to \\n', () => {
    expect(normalizeLineEndings('a\r\nb\r\nc')).toBe('a\nb\nc');
  });

  it('converts lone \\r to \\n', () => {
    expect(normalizeLineEndings('a\rb\rc')).toBe('a\nb\nc');
  });

  it('handles mixed CRLF, LF, and lone CR', () => {
    expect(normalizeLineEndings('a\r\nb\nc\rd')).toBe('a\nb\nc\nd');
  });

  it('is a no-op for already-LF strings', () => {
    expect(normalizeLineEndings('a\nb\nc')).toBe('a\nb\nc');
  });

  it('handles empty string', () => {
    expect(normalizeLineEndings('')).toBe('');
  });

  it('handles strings with no line endings', () => {
    expect(normalizeLineEndings('single line')).toBe('single line');
  });

  it('preserves trailing line endings (one transformation)', () => {
    expect(normalizeLineEndings('a\r\n')).toBe('a\n');
    expect(normalizeLineEndings('a\r')).toBe('a\n');
  });
});
