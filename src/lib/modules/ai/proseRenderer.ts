/**
 * Cached prose renderer for chat messages.
 *
 * `marked.parse + DOMPurify.sanitize` is the hot path for rendering
 * assistant prose. With H4 in place, completed messages return a stable
 * `ChatBlock[]` reference, but the template's `{@html render(text)}`
 * expression still re-runs on any nearby reactive update. By memoizing
 * the rendered HTML keyed on the raw prose string, completed messages
 * never re-render.
 *
 * During streaming the *content* string grows on every chunk, so each
 * intermediate would be a distinct cache key. We deliberately skip the
 * cache write when `isStreaming` is true to avoid filling the cache
 * with throw-away intermediates; only the final, stable content gets
 * cached.
 *
 * **Security:** DOMPurify always runs before HTML enters the cache.
 * No security regression vs the unmemoized version.
 */

import { marked } from 'marked';
import DOMPurify from 'dompurify';

interface RendererStats {
  size: number;
  hits: number;
  misses: number;
}

export interface ProseRenderer {
  /** Render `content` to sanitized HTML, using the cache when stable. */
  render(content: string, isStreaming: boolean): string;
  /** Drop all cached entries. */
  clear(): void;
  /** Diagnostics. */
  stats(): RendererStats;
}

/** Default cap. Each entry is a small HTML string; 200 ≈ a few hundred KB. */
const DEFAULT_CAP = 200;

export function createProseRenderer(cap: number = DEFAULT_CAP): ProseRenderer {
  const cache = new Map<string, string>();
  let hits = 0;
  let misses = 0;

  function render(content: string, isStreaming: boolean): string {
    if (!content) return '';
    const cached = cache.get(content);
    if (cached !== undefined) {
      hits++;
      return cached;
    }
    misses++;
    // DOMPurify is the security boundary. Always runs, regardless of cache
    // state.
    const html = DOMPurify.sanitize(
      marked.parse(content, { async: false }) as string,
    );
    if (!isStreaming) {
      cache.set(content, html);
      // FIFO eviction: insertion order is iteration order in JS Map.
      if (cache.size > cap) {
        const first = cache.keys().next().value;
        if (first !== undefined) cache.delete(first);
      }
    }
    return html;
  }

  function clear(): void {
    cache.clear();
    hits = 0;
    misses = 0;
  }

  function stats(): RendererStats {
    return { size: cache.size, hits, misses };
  }

  return { render, clear, stats };
}
