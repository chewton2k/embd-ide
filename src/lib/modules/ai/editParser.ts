export interface EditProposal {
  id: string;
  filePath: string;
  startLine: number;
  endLine: number;
  originalCode: string;
  newCode: string;
  status: 'pending' | 'approved' | 'rejected';
  /**
   * Optional staleness marker. `true` means the live document content
   * at `[startLine, endLine]` no longer matches `originalCode` —
   * approving would silently overwrite content the user (or another
   * source) changed since the proposal was generated.
   *
   * Computed by `reanchorEditsForChanges` (keystroke path) when the
   * change touched the edit's range. Cleared by
   * `reanchorEditsForContent` (wholesale-replacement path) on a
   * successful unique-match re-anchor (which by construction proves
   * the originalCode is present at the new line range).
   *
   * Default `false` / `undefined`. Producers (`tools.ts edit_file`,
   * Cmd+K inline edit, `parseAiEdits`) should not set this directly;
   * it's a derived UI signal. The diff widget renders stale edits
   * with a warning indicator; `approveEdit` logs a warn before
   * applying a stale edit (still allows the apply — the user
   * remains the authority on their data).
   */
  stale?: boolean;
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
      id: crypto.randomUUID(),
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
