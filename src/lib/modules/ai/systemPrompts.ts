/** System prompt that instructs the AI to use structured edit format. */
export const EDIT_SYSTEM_PROMPT = `When the user asks you to modify code, respond with structured edit blocks using this format:

\`\`\`edit:<filepath>:<startLine>-<endLine>
<original code that will be replaced>
---
<new code to insert>
\`\`\`

Rules:
- filepath is relative to the project root
- startLine and endLine are 1-indexed line numbers of the original code
- The original code section must match the file exactly (used for verification)
- The --- separator divides original from new code
- You can include multiple edit blocks in one response for multi-file changes
- If you're explaining something without editing, just respond normally without edit blocks
- For new files, use startLine 1 and endLine 0 (indicates insertion)

Example:
\`\`\`edit:src/utils.ts:5-8
function add(a, b) {
  return a + b;
}
---
function add(a: number, b: number): number {
  return a + b;
}
\`\`\`
`;
