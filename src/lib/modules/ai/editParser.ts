export interface EditProposal {
  id: string;
  filePath: string;
  startLine: number;
  endLine: number;
  originalCode: string;
  newCode: string;
  status: 'pending' | 'approved' | 'rejected';
}

const EDIT_BLOCK_RE = /```edit:([^:\n]+):(\d+)-(\d+)\n([\s\S]*?)```/g;

/**
 * Parse AI response for structured edit blocks.
 * Returns extracted edits and the response text with edit blocks removed (for display).
 */
export function parseAiEdits(response: string): { edits: EditProposal[]; displayText: string } {
  const edits: EditProposal[] = [];
  let displayText = response;

  let match: RegExpExecArray | null;
  const re = new RegExp(EDIT_BLOCK_RE.source, 'g');

  while ((match = re.exec(response)) !== null) {
    const [fullMatch, filePath, startStr, endStr, body] = match;
    const separatorIdx = body.indexOf('\n---\n');

    if (separatorIdx === -1) continue;

    const originalCode = body.slice(0, separatorIdx);
    const newCode = body.slice(separatorIdx + 5); // skip \n---\n

    edits.push({
      id: `edit-${Date.now()}-${edits.length}`,
      filePath: filePath.trim(),
      startLine: parseInt(startStr, 10),
      endLine: parseInt(endStr, 10),
      originalCode: originalCode.trimEnd(),
      newCode: newCode.trimEnd(),
      status: 'pending',
    });

    displayText = displayText.replace(fullMatch, `\`\`\`proposed-edit\n${filePath.trim()} · lines ${startStr}-${endStr}\n\`\`\``);
  }

  return { edits, displayText };
}

/** Check if a response contains any edit blocks. */
export function hasEdits(response: string): boolean {
  return /```edit:[^:\n]+:\d+-\d+\n/.test(response);
}
