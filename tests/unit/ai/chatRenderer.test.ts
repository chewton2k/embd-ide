import { describe, it, expect } from 'vitest';
import { parseUserContent, parseAssistantContent } from '$lib/modules/ai/chatRenderer';

describe('parseUserContent', () => {
  it('handles plain prose', () => {
    const blocks = parseUserContent('Hello world');
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ kind: 'prose', text: 'Hello world' });
  });

  it('handles file content results', () => {
    const blocks = parseUserContent('[File content of src/main.ts]:\n```ts\nconst x = 1;\n```');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('tool-result');
    expect(blocks[0].text).toBe('src/main.ts');
    expect(blocks[0].label).toBe('Read');
  });

  it('handles executed command results', () => {
    const blocks = parseUserContent('[Executed: npm test]');
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ kind: 'tool-result', label: 'Ran', text: 'npm test' });
  });

  it('handles activity markers', () => {
    const blocks = parseUserContent('[3 edit(s) proposed — waiting for approval]');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('activity');
  });
});

describe('parseAssistantContent', () => {
  it('handles tool-call blocks', () => {
    const content = 'Let me read that file.\n\n```tool:read_file\nsrc/main.ts\n```\n\nHere it is.';
    const blocks = parseAssistantContent(content);
    expect(blocks.length).toBeGreaterThanOrEqual(3);
    expect(blocks[0]).toMatchObject({ kind: 'prose', text: 'Let me read that file.' });
    expect(blocks[1]).toMatchObject({ kind: 'tool-read', text: 'src/main.ts' });
    expect(blocks[2]).toMatchObject({ kind: 'prose', text: 'Here it is.' });
  });

  it('handles plain prose with no tool blocks', () => {
    const blocks = parseAssistantContent('Just a normal response.');
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ kind: 'prose', text: 'Just a normal response.' });
  });

  it('handles empty content', () => {
    const blocks = parseAssistantContent('');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('prose');
  });
});
