import { describe, it, expect } from 'vitest';
import { formatCheckpointTime } from '$lib/modules/ai/checkpoints';

describe('checkpoints', () => {
  describe('formatCheckpointTime', () => {
    it('returns empty string for 0 timestamp', () => {
      expect(formatCheckpointTime(0)).toBe('');
    });

    it('returns "just now" for timestamps less than 1 minute ago', () => {
      const now = Math.floor(Date.now() / 1000);
      expect(formatCheckpointTime(now)).toBe('just now');
      expect(formatCheckpointTime(now - 30)).toBe('just now');
    });

    it('returns minutes ago for recent timestamps', () => {
      const now = Math.floor(Date.now() / 1000);
      expect(formatCheckpointTime(now - 120)).toBe('2m ago');
      expect(formatCheckpointTime(now - 300)).toBe('5m ago');
      expect(formatCheckpointTime(now - 3540)).toBe('59m ago');
    });

    it('returns hours ago for timestamps within 24h', () => {
      const now = Math.floor(Date.now() / 1000);
      expect(formatCheckpointTime(now - 3600)).toBe('1h ago');
      expect(formatCheckpointTime(now - 7200)).toBe('2h ago');
      expect(formatCheckpointTime(now - 82800)).toBe('23h ago');
    });

    it('returns date string for timestamps older than 24h', () => {
      const old = Math.floor(Date.now() / 1000) - 172800; // 2 days ago
      const result = formatCheckpointTime(old);
      // Should be a date string like "5/13/2026" — just verify it's not a relative format
      expect(result).not.toContain('ago');
      expect(result).not.toBe('just now');
      expect(result.length).toBeGreaterThan(0);
    });
  });
});
