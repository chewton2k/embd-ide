import { describe, it, expect, vi } from 'vitest';
import { createProseRenderer } from '$lib/modules/ai/proseRenderer';

// Make sure DOMPurify is invoked: spy on it before the renderer is built.
import DOMPurify from 'dompurify';

describe('createProseRenderer', () => {
  it('renders markdown to sanitized HTML', () => {
    const r = createProseRenderer();
    const out = r.render('# Hello\n\nworld', false);
    expect(out).toContain('<h1');
    expect(out).toContain('Hello');
  });

  it('returns identical HTML for the same content (cache hit)', () => {
    const r = createProseRenderer();
    const a = r.render('# Same', false);
    const b = r.render('# Same', false);
    expect(b).toBe(a);
    expect(r.stats().hits).toBe(1);
    expect(r.stats().misses).toBe(1);
  });

  it('does NOT cache while streaming, but DOMPurify still runs', () => {
    const sanitizeSpy = vi.spyOn(DOMPurify, 'sanitize');
    const r = createProseRenderer();
    sanitizeSpy.mockClear();

    r.render('partial chunk one', true);
    r.render('partial chunk one', true); // same content, but streaming → no cache
    expect(sanitizeSpy).toHaveBeenCalledTimes(2);
    expect(r.stats().size).toBe(0);

    sanitizeSpy.mockRestore();
  });

  it('caches non-streaming renders so the next call hits without DOMPurify', () => {
    const sanitizeSpy = vi.spyOn(DOMPurify, 'sanitize');
    const r = createProseRenderer();
    sanitizeSpy.mockClear();

    r.render('finished message', false);
    r.render('finished message', false);
    expect(sanitizeSpy).toHaveBeenCalledTimes(1);
    expect(r.stats().hits).toBe(1);

    sanitizeSpy.mockRestore();
  });

  it('returns empty string for empty content without invoking the parser', () => {
    const sanitizeSpy = vi.spyOn(DOMPurify, 'sanitize');
    const r = createProseRenderer();
    sanitizeSpy.mockClear();

    expect(r.render('', false)).toBe('');
    expect(sanitizeSpy).not.toHaveBeenCalled();

    sanitizeSpy.mockRestore();
  });

  it('evicts oldest entry once the cache exceeds its capacity', () => {
    const r = createProseRenderer(3);
    r.render('a', false);
    r.render('b', false);
    r.render('c', false);
    expect(r.stats().size).toBe(3);

    // Inserting the 4th distinct key should drop 'a' (FIFO).
    r.render('d', false);
    expect(r.stats().size).toBe(3);

    // 'a' is evicted → re-rendering it is a miss.
    const startMisses = r.stats().misses;
    r.render('a', false);
    expect(r.stats().misses).toBe(startMisses + 1);
  });

  it('clear() empties the cache and zeroes counters', () => {
    const r = createProseRenderer();
    r.render('one', false);
    r.render('two', false);
    expect(r.stats().size).toBe(2);

    r.clear();
    expect(r.stats()).toEqual({ size: 0, hits: 0, misses: 0 });
  });
});
