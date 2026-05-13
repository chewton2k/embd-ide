import { describe, it, expect } from 'vitest';
import { shortProjectName, formatBytes, formatRelativeTime } from '$lib/modules/knowledge/knowledge';

describe('shortProjectName', () => {
  it('returns last 2 segments for a normal path', () => {
    expect(shortProjectName('/Users/x/Projects/leo')).toBe('Projects/leo');
  });

  it('returns "(unknown project)" for "(unknown)"', () => {
    expect(shortProjectName('(unknown)')).toBe('(unknown project)');
  });

  it('returns "(unknown project)" for empty string', () => {
    expect(shortProjectName('')).toBe('(unknown project)');
  });
});

describe('formatBytes', () => {
  it('returns "0 B" for 0', () => {
    expect(formatBytes(0)).toBe('0 B');
  });

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1.0 KB');
  });

  it('formats megabytes', () => {
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
  });
});

describe('formatRelativeTime', () => {
  it('returns "never" for 0', () => {
    expect(formatRelativeTime(0)).toBe('never');
  });

  it('returns "just now" for recent timestamps', () => {
    const now = Math.floor(Date.now() / 1000);
    expect(formatRelativeTime(now - 30)).toBe('just now');
  });

  it('returns minutes ago for timestamps within the hour', () => {
    const now = Math.floor(Date.now() / 1000);
    expect(formatRelativeTime(now - 300)).toBe('5m ago');
  });
});
