/**
 * Normalize CRLF (`\r\n`) and lone CR (`\r`) line endings to LF (`\n`).
 *
 * Used to compare AI-producer-supplied strings (which may carry CRLF
 * from Windows providers or copy-pasted source) against CodeMirror
 * document content (always `\n`).
 */
export function normalizeLineEndings(s: string): string {
  return s.replace(/\r\n?/g, '\n');
}
