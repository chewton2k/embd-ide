import { describe, it, expect } from 'vitest';
import { parseAiEdits, hasEdits } from '$lib/modules/ai/editParser';

describe('parseAiEdits', () => {
  it('round-trips a known assistant message and extracts edits', () => {
    const response = `Here's the fix:

\`\`\`edit:src/main.ts:5-10
const old = 'code';
---
const fixed = 'code';
\`\`\`

Done!`;

    const { edits, displayText } = parseAiEdits(response);
    expect(edits).toHaveLength(1);
    expect(edits[0].filePath).toBe('src/main.ts');
    expect(edits[0].startLine).toBe(5);
    expect(edits[0].endLine).toBe(10);
    expect(edits[0].originalCode).toBe("const old = 'code';");
    expect(edits[0].newCode).toBe("const fixed = 'code';");
    expect(edits[0].status).toBe('pending');
    expect(displayText).toContain('proposed-edit');
    expect(displayText).not.toContain('edit:src/main.ts');
  });

  it('returns empty edits and original text for no edit blocks', () => {
    const { edits, displayText } = parseAiEdits('Just some text');
    expect(edits).toHaveLength(0);
    expect(displayText).toBe('Just some text');
  });

  it('returns empty edits for empty input', () => {
    const { edits, displayText } = parseAiEdits('');
    expect(edits).toHaveLength(0);
    expect(displayText).toBe('');
  });
});

describe('hasEdits', () => {
  it('detects edit blocks', () => {
    expect(hasEdits('```edit:foo.ts:1-2\n')).toBe(true);
  });

  it('returns false for plain text', () => {
    expect(hasEdits('no edits here')).toBe(false);
  });
});
