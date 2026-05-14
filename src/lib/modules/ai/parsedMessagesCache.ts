/**
 * Incremental parse cache for chat messages.
 *
 * During streaming, only the actively-streaming message (last in array)
 * keeps growing. Earlier messages are immutable once a model finishes
 * emitting them. By memoizing the parsed `ChatBlock[]` result keyed on a
 * cheap fingerprint (conversation id, index, role, content length), we can
 * skip re-running `parseUserContent` / `parseAssistantContent` for the
 * stable prefix on every keystroke / chunk.
 *
 * Fingerprint rationale:
 * - `content.length` changes on every streaming chunk for the active
 *   message, triggering re-parse.
 * - Completed messages have stable length → cache hit.
 * - Including the conversation id makes cache invalidation automatic on
 *   `clearChat()` / `loadConversation()` — entries from the prior chat
 *   become unreachable and are pruned by the size cap.
 */

import {
  parseUserContent,
  parseAssistantContent,
  type ChatBlock,
} from './chatRenderer';
import type { ChatMessage } from './ai';

export interface ParsedMessage {
  role: ChatMessage['role'];
  blocks: ChatBlock[];
  index: number;
}

interface CacheEntry {
  role: ChatMessage['role'];
  blocks: ChatBlock[];
}

interface CacheStats {
  size: number;
  hits: number;
  misses: number;
}

export interface ParsedMessagesCache {
  parse(msgs: readonly ChatMessage[], conversationId: string): ParsedMessage[];
  clear(): void;
  stats(): CacheStats;
}

function parseMessage(msg: ChatMessage): ChatBlock[] {
  if (msg.role === 'user') return parseUserContent(msg.content);
  if (msg.role === 'assistant') return parseAssistantContent(msg.content);
  return [{ kind: 'prose', text: msg.content }];
}

/**
 * Create a per-component parse cache. Not shared across components: each
 * `FloatingChat` instance owns its own.
 */
export function createParsedMessagesCache(): ParsedMessagesCache {
  let cache = new Map<string, CacheEntry>();
  let hits = 0;
  let misses = 0;

  function fingerprint(
    conversationId: string,
    i: number,
    msg: ChatMessage,
  ): string {
    return `${conversationId}\u0000${i}\u0000${msg.role}\u0000${msg.content.length}`;
  }

  function parse(
    msgs: readonly ChatMessage[],
    conversationId: string,
  ): ParsedMessage[] {
    const result: ParsedMessage[] = [];
    for (let i = 0; i < msgs.length; i++) {
      const msg = msgs[i];
      const fp = fingerprint(conversationId, i, msg);
      const cached = cache.get(fp);
      if (cached) {
        hits++;
        result.push({ role: cached.role, blocks: cached.blocks, index: i });
        continue;
      }
      misses++;
      const blocks = parseMessage(msg);
      const entry: CacheEntry = { role: msg.role, blocks };
      cache.set(fp, entry);
      result.push({ role: msg.role, blocks, index: i });
    }
    // Prune stale entries when the cache grows much larger than the live
    // working set. Keep the most-recently-inserted N entries (Map iteration
    // order is insertion order in JS).
    const cap = Math.max(8, msgs.length * 2);
    if (cache.size > cap) {
      const keep = Math.max(msgs.length, 1);
      const entries = [...cache];
      cache = new Map(entries.slice(-keep));
    }
    return result;
  }

  function clear(): void {
    cache.clear();
    hits = 0;
    misses = 0;
  }

  function stats(): CacheStats {
    return { size: cache.size, hits, misses };
  }

  return { parse, clear, stats };
}
