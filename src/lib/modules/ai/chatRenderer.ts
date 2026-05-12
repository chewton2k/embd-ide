/**
 * Render-time parsing for chat messages.
 *
 * The chat store preserves raw message strings (for backward compatibility
 * with saved SQLite conversations). The UI uses this module to parse those
 * strings into a typed `ChatBlock[]` timeline that renders as prose paragraphs
 * interleaved with tool-call cards (read_file, search, run, edit, etc.).
 *
 * Two sources of tool-call content:
 *
 * 1. **Assistant messages** can contain fenced tool blocks produced by the
 *    model, e.g. ```tool:read_file, ```tool:search, ```run, ```edit:..., or
 *    our post-processed ```proposed-edit / ```command / ```dangerous-command
 *    markers. Everything outside these blocks is rendered as markdown prose.
 *
 * 2. **User messages** produced by the agent loop as tool *results*, e.g.
 *    "[File content of X]:\n\`\`\`...\`\`\`", "[Executed: X]",
 *    "[Search results for X]:\n...", "[Applied edit to X]", "[Error …]",
 *    "[N edit(s) proposed — waiting for approval]". These are turned into
 *    activity chips so the user doesn't see the raw brackets.
 */

export type ChatBlockKind =
  | 'prose'             // assistant markdown paragraph(s)
  | 'tool-read'         // read_file tool call
  | 'tool-search'       // search tool call
  | 'tool-run'          // shell command request from the assistant
  | 'tool-run-dangerous'// dangerous shell command
  | 'tool-edit'         // proposed edit (file + line range)
  | 'tool-result'       // agent loop produced a result for a prior tool call
  | 'activity'          // agent-loop progress marker
  | 'error';            // error message

export interface ChatBlock {
  kind: ChatBlockKind;
  /** Main text: prose body, file path, search query, command, edit summary. */
  text: string;
  /** Optional secondary content: tool output / file contents / search results. */
  detail?: string;
  /** Optional human-readable label (e.g. "Applied", "Skipped"). */
  label?: string;
}

// ── Fenced-block patterns for assistant messages ──────────────────

interface FenceSpec {
  lang: string;
  toBlock(body: string): ChatBlock;
}

const FENCE_SPECS: FenceSpec[] = [
  // Model-requested tools (agent loop)
  { lang: 'tool:read_file', toBlock: (body) => ({ kind: 'tool-read', text: body.trim() || '(file)' }) },
  { lang: 'tool:search',    toBlock: (body) => ({ kind: 'tool-search', text: body.trim() || '(query)' }) },
  { lang: 'run',            toBlock: (body) => ({ kind: 'tool-run', text: body.trim() }) },
  // Post-processed markers (written by commandParser / editParser)
  { lang: 'command',           toBlock: (body) => ({ kind: 'tool-run', text: body.trim() }) },
  { lang: 'dangerous-command', toBlock: (body) => ({ kind: 'tool-run-dangerous', text: body.trim() }) },
  { lang: 'proposed-edit',     toBlock: (body) => ({ kind: 'tool-edit', text: body.trim() }) },
];

// Matches ```<lang>\n<body>\n``` — non-greedy body.
const FENCE_RE = /```([a-zA-Z][\w:-]*)\n([\s\S]*?)```/g;

/**
 * Parse an assistant message into a list of blocks (prose + tool-call fences).
 * Fences for unrecognized languages are left inside the prose so Marked can
 * still render them as code blocks.
 */
export function parseAssistantContent(content: string): ChatBlock[] {
  const specByLang = new Map(FENCE_SPECS.map(s => [s.lang, s]));
  const blocks: ChatBlock[] = [];
  let cursor = 0;
  let match: RegExpExecArray | null;
  FENCE_RE.lastIndex = 0;

  while ((match = FENCE_RE.exec(content)) !== null) {
    const lang = match[1];
    const spec = specByLang.get(lang);
    if (!spec) continue; // leave unknown fences embedded in prose
    const [fullMatch, , body] = match;
    const before = content.slice(cursor, match.index);
    const beforeTrimmed = before.trim();
    if (beforeTrimmed) blocks.push({ kind: 'prose', text: beforeTrimmed });
    blocks.push(spec.toBlock(body));
    cursor = match.index + fullMatch.length;
  }

  const tail = content.slice(cursor).trim();
  if (tail) blocks.push({ kind: 'prose', text: tail });
  // If content was entirely whitespace and produced no blocks, preserve an
  // empty prose block so streaming placeholders render their container.
  if (blocks.length === 0) blocks.push({ kind: 'prose', text: content });
  return blocks;
}

// ── User-side (tool-result) patterns ──────────────────────────────

interface ResultPattern {
  test: RegExp;
  toBlock(content: string, match: RegExpMatchArray): ChatBlock;
}

const RESULT_PATTERNS: ResultPattern[] = [
  {
    // "[File content of path/to/file.ts]:\n```\n...\n```"
    test: /^\[File content of ([^\]]+)\]:?\s*([\s\S]*)$/,
    toBlock: (_c, m) => {
      const path = m[1].trim();
      const body = stripOuterFences(m[2].trim());
      return { kind: 'tool-result', label: 'Read', text: path, detail: body || undefined };
    },
  },
  {
    // "[Search results for "foo"]:\n..."
    test: /^\[Search results for (.+)\]:?\s*([\s\S]*)$/,
    toBlock: (_c, m) => ({
      kind: 'tool-result',
      label: 'Searched',
      text: m[1].trim().replace(/^["']|["']$/g, ''),
      detail: m[2].trim() || undefined,
    }),
  },
  {
    // "[Executed: npm test]"
    test: /^\[Executed:\s*([^\]]+)\]\s*([\s\S]*)$/,
    toBlock: (_c, m) => ({
      kind: 'tool-result',
      label: 'Ran',
      text: m[1].trim(),
      detail: m[2].trim() || undefined,
    }),
  },
  {
    // "[Applied edit to path/to/file.ts]"
    test: /^\[Applied edit to ([^\]]+)\]\s*$/,
    toBlock: (_c, m) => ({ kind: 'tool-result', label: 'Applied', text: m[1].trim() }),
  },
  {
    // "[Failed to apply edit to path: reason]"
    test: /^\[Failed to apply edit to ([^:\]]+):\s*([^\]]+)\]\s*$/,
    toBlock: (_c, m) => ({ kind: 'error', label: 'Edit failed', text: m[1].trim(), detail: m[2].trim() }),
  },
  {
    // "[Error reading path: reason]" and generic "[Error …]"
    test: /^\[Error (?:reading )?([^:\]]+)(?::\s*([^\]]+))?\]\s*$/,
    toBlock: (_c, m) => ({ kind: 'error', label: 'Error', text: m[1].trim(), detail: m[2]?.trim() }),
  },
  {
    // "[3 edit(s) proposed — waiting for approval]"
    test: /^\[(\d+) edit\(s\) proposed[^\]]*\]\s*$/,
    toBlock: (_c, m) => ({ kind: 'activity', label: 'Waiting for approval', text: `${m[1]} edit${m[1] === '1' ? '' : 's'} proposed` }),
  },
  {
    // "[Dangerous command skipped: rm -rf /]"
    test: /^\[Dangerous command skipped:\s*([^\]]+)\]\s*$/,
    toBlock: (_c, m) => ({ kind: 'activity', label: 'Skipped', text: m[1].trim() }),
  },
  {
    // "[No terminal available to run: npm test]"
    test: /^\[No terminal available to run:\s*([^\]]+)\]\s*$/,
    toBlock: (_c, m) => ({ kind: 'error', label: 'No terminal', text: m[1].trim() }),
  },
  {
    // Catch-all: any other "[...]" single-line activity marker.
    test: /^\[([^\]]+)\]\s*$/,
    toBlock: (_c, m) => ({ kind: 'activity', text: m[1].trim() }),
  },
];

/**
 * Parse a user-role message into blocks. Most user messages are plain prose
 * the user typed; when the agent loop posts a tool result it uses a "[...]"
 * prefix that we turn into a tool-result or activity block.
 */
export function parseUserContent(content: string): ChatBlock[] {
  const trimmed = content.trimStart();
  if (!trimmed.startsWith('[')) return [{ kind: 'prose', text: content }];
  for (const pat of RESULT_PATTERNS) {
    const m = trimmed.match(pat.test);
    if (m) return [pat.toBlock(content, m)];
  }
  return [{ kind: 'prose', text: content }];
}

/** Strip a single outer ```...``` fence from a body, preserving inner fences. */
function stripOuterFences(text: string): string {
  const m = text.match(/^```[a-zA-Z0-9_-]*\n([\s\S]*?)\n```\s*$/);
  return m ? m[1] : text;
}

/** Truncate `text` to at most `max` characters, adding an ellipsis marker. */
export function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, max - 1).trimEnd() + '…';
}

/** Basename helper for file paths (cross-platform). */
export function basename(path: string): string {
  const norm = path.replace(/\\/g, '/');
  const i = norm.lastIndexOf('/');
  return i >= 0 ? norm.slice(i + 1) : norm;
}
