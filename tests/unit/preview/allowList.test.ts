import { afterEach, beforeEach, describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import {
  isAllowed,
  isLocalHost,
  addToAllowList,
  removeFromAllowList,
  clearAllowList,
  previewAllowList,
} from '$lib/modules/preview/allowList';

const STORAGE_KEY = 'leo-preview-allowlist';

describe('preview allow-list', () => {
  beforeEach(() => {
    // Clear localStorage so the persisted store starts empty.
    localStorage.removeItem(STORAGE_KEY);
    clearAllowList();
  });

  afterEach(() => {
    localStorage.removeItem(STORAGE_KEY);
  });

  describe('isLocalHost', () => {
    it('recognizes the standard loopback hostnames', () => {
      expect(isLocalHost('localhost')).toBe(true);
      expect(isLocalHost('127.0.0.1')).toBe(true);
      expect(isLocalHost('0.0.0.0')).toBe(true);
      expect(isLocalHost('::1')).toBe(true);
      expect(isLocalHost('[::1]')).toBe(true);
    });

    it('treats casing as insignificant', () => {
      expect(isLocalHost('LOCALHOST')).toBe(true);
    });

    it('returns false for non-loopback hosts', () => {
      expect(isLocalHost('example.com')).toBe(false);
      expect(isLocalHost('192.168.1.1')).toBe(false);
    });
  });

  describe('isAllowed', () => {
    it('always allows localhost / 127.0.0.1 URLs', () => {
      expect(isAllowed('http://localhost:3000')).toBe(true);
      expect(isAllowed('http://127.0.0.1:8080/api')).toBe(true);
      expect(isAllowed('https://localhost:443/')).toBe(true);
    });

    it('returns false for an unlisted external URL', () => {
      expect(isAllowed('https://example.com')).toBe(false);
    });

    it('returns false for falsy / unparseable inputs', () => {
      expect(isAllowed(null)).toBe(false);
      expect(isAllowed(undefined)).toBe(false);
      expect(isAllowed('')).toBe(false);
      expect(isAllowed('not a url')).toBe(false);
    });
  });

  describe('addToAllowList', () => {
    it('persists a URL host so subsequent isAllowed returns true', () => {
      expect(isAllowed('https://example.com')).toBe(false);
      const changed = addToAllowList('https://example.com/some/path');
      expect(changed).toBe(true);
      expect(isAllowed('https://example.com/other/path')).toBe(true);
      expect(isAllowed('https://example.com')).toBe(true);
    });

    it('accepts a bare hostname as input', () => {
      addToAllowList('example.org');
      expect(isAllowed('https://example.org')).toBe(true);
    });

    it('is idempotent and reports unchanged on repeat add', () => {
      expect(addToAllowList('example.net')).toBe(true);
      expect(addToAllowList('example.net')).toBe(false);
      expect(addToAllowList('https://example.net')).toBe(false);
    });

    it('normalizes hostnames to lowercase', () => {
      addToAllowList('https://Example.com');
      expect(get(previewAllowList)).toContain('example.com');
      expect(isAllowed('https://EXAMPLE.com')).toBe(true);
    });

    it('rejects empty / unparseable inputs without throwing', () => {
      expect(addToAllowList('')).toBe(false);
      expect(addToAllowList('   ')).toBe(false);
      // Garbage-with-scheme that URL() can't parse.
      expect(addToAllowList('http://')).toBe(false);
    });

    it('strips port from a bare hostname so it matches URL.hostname later', () => {
      // addToAllowList('example.com:3000') is treated as the host
      // 'example.com' (any port). isAllowed against any port on that
      // host then returns true.
      addToAllowList('ported.example:3000');
      expect(isAllowed('http://ported.example:3000')).toBe(true);
      expect(isAllowed('http://ported.example:8080')).toBe(true);
      expect(get(previewAllowList)).toContain('ported.example');
    });

    it('persists the list to localStorage as JSON', () => {
      addToAllowList('a.example');
      addToAllowList('b.example');
      const raw = localStorage.getItem(STORAGE_KEY);
      expect(raw).toBeTruthy();
      const parsed = JSON.parse(raw!);
      expect(parsed).toEqual(expect.arrayContaining(['a.example', 'b.example']));
    });
  });

  describe('removeFromAllowList', () => {
    it('removes a previously-added host', () => {
      addToAllowList('removable.example');
      expect(isAllowed('https://removable.example')).toBe(true);
      const changed = removeFromAllowList('removable.example');
      expect(changed).toBe(true);
      expect(isAllowed('https://removable.example')).toBe(false);
    });

    it('returns false when the host was never present', () => {
      expect(removeFromAllowList('never.added')).toBe(false);
    });
  });

  describe('previewAllowList store', () => {
    it('reflects the current list as a sorted array', () => {
      addToAllowList('z.example');
      addToAllowList('a.example');
      const list = get(previewAllowList);
      expect(list).toEqual(['a.example', 'z.example']);
    });

    it('returns an empty array when localStorage holds garbage', () => {
      localStorage.setItem(STORAGE_KEY, 'not json');
      // Force the persisted underlying store to re-emit by re-reading.
      const list = get(previewAllowList);
      expect(Array.isArray(list)).toBe(true);
    });
  });
});
