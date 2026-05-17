import { describe, it, expect } from 'vitest';
import { createParsedMessagesCache } from '$lib/modules/ai/parsedMessagesCache';
import type { ChatMessage } from '$lib/modules/ai/ai';

function userMsg(content: string): ChatMessage {
  return { role: 'user', content };
}
function asstMsg(content: string): ChatMessage {
  return { role: 'assistant', content };
}

describe('createParsedMessagesCache', () => {
  it('returns identical block arrays for unchanged messages on repeat parse (cache hit)', () => {
    const cache = createParsedMessagesCache();
    const msgs: ChatMessage[] = [
      userMsg('Hello'),
      asstMsg('Hi there'),
      userMsg('How are you?'),
    ];
    const first = cache.parse(msgs, 'conv-1');
    const second = cache.parse(msgs, 'conv-1');

    // Same parsed-block array reference because the cache returned the same
    // entry. This is the key memoization guarantee.
    expect(first).toHaveLength(3);
    expect(second).toHaveLength(3);
    for (let i = 0; i < 3; i++) {
      expect(second[i].blocks).toBe(first[i].blocks);
    }
    const stats = cache.stats();
    expect(stats.misses).toBe(3);
    expect(stats.hits).toBe(3);
  });

  it('only re-parses the actively-streaming last message', () => {
    const cache = createParsedMessagesCache();
    const initial: ChatMessage[] = [
      userMsg('msg-0'),
      asstMsg('msg-1'),
      userMsg('msg-2'),
      asstMsg('msg-3'),
      asstMsg('partial'), // streaming message
    ];
    const before = cache.parse(initial, 'conv-1');
    expect(cache.stats().misses).toBe(5);

    // Streaming chunk arrives — only the LAST message grew.
    const updated: ChatMessage[] = [
      ...initial.slice(0, 4),
      asstMsg('partial response with more text'),
    ];
    const startMisses = cache.stats().misses;
    const startHits = cache.stats().hits;
    const after = cache.parse(updated, 'conv-1');

    // Stable prefix (indices 0..3) must hit cache: 4 hits, 1 miss for
    // index 4.
    expect(cache.stats().hits - startHits).toBe(4);
    expect(cache.stats().misses - startMisses).toBe(1);

    // Block identity preserved for the stable prefix.
    for (let i = 0; i < 4; i++) {
      expect(after[i].blocks).toBe(before[i].blocks);
    }
    // Last block is freshly parsed (different reference).
    expect(after[4].blocks).not.toBe(before[4].blocks);
  });

  it('clear() resets the cache so every entry is re-parsed', () => {
    const cache = createParsedMessagesCache();
    const msgs: ChatMessage[] = [userMsg('a'), asstMsg('b')];
    cache.parse(msgs, 'conv-1');
    expect(cache.stats().size).toBe(2);

    cache.clear();
    expect(cache.stats().size).toBe(0);
    expect(cache.stats().hits).toBe(0);
    expect(cache.stats().misses).toBe(0);

    const re = cache.parse(msgs, 'conv-1');
    expect(cache.stats().misses).toBe(2);
    expect(re).toHaveLength(2);
  });

  it('invalidates entries when the conversation id changes', () => {
    const cache = createParsedMessagesCache();
    const msgs: ChatMessage[] = [userMsg('hello')];
    cache.parse(msgs, 'conv-A');
    expect(cache.stats().misses).toBe(1);

    // Same content, different conversation id → must miss (different fp).
    cache.parse(msgs, 'conv-B');
    expect(cache.stats().misses).toBe(2);
    expect(cache.stats().hits).toBe(0);
  });

  it('preserves correct ChatBlock output for user and assistant messages', () => {
    const cache = createParsedMessagesCache();
    const msgs: ChatMessage[] = [
      userMsg('plain user prose'),
      asstMsg('Let me check.\n\n```tool:read_file\nsrc/main.ts\n```\n\nDone.'),
    ];
    const out = cache.parse(msgs, 'c1');
    expect(out[0].role).toBe('user');
    expect(out[0].blocks[0]).toMatchObject({ kind: 'prose', text: 'plain user prose' });
    expect(out[1].role).toBe('assistant');
    const kinds = out[1].blocks.map(b => b.kind);
    expect(kinds).toContain('tool-read');
  });

  it('prunes the cache when it grows much larger than the working set', () => {
    const cache = createParsedMessagesCache();
    // Insert 50 distinct entries (each with a unique content length).
    const big: ChatMessage[] = [];
    for (let i = 0; i < 50; i++) big.push(userMsg('x'.repeat(i + 1)));
    cache.parse(big, 'conv-1');
    expect(cache.stats().size).toBe(50);

    // Now switch to a small working set in a different conversation. The
    // prune logic should kick in and shrink the cache.
    const small: ChatMessage[] = [userMsg('one'), userMsg('two')];
    cache.parse(small, 'conv-2');
    expect(cache.stats().size).toBeLessThanOrEqual(Math.max(8, small.length * 2));
  });
});
