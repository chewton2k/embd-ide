export interface ConflictHunk {
  index: number;
  startLine: number;   // line index of <<<<<<<
  sepLine: number;     // line index of =======
  endLine: number;     // line index of >>>>>>>
  currentLines: string[];  // "ours" content
  incomingLines: string[]; // "theirs" content
  currentLabel: string;
  incomingLabel: string;
}

export type Resolution = 'current' | 'incoming' | 'both';

export function parseConflicts(content: string): ConflictHunk[] {
  const lines = content.split('\n');
  const hunks: ConflictHunk[] = [];
  let i = 0;
  let hunkIndex = 0;

  while (i < lines.length) {
    if (lines[i].startsWith('<<<<<<<')) {
      const startLine = i;
      const currentLabel = lines[i].slice(7).trim();
      const currentLines: string[] = [];
      i++;

      while (i < lines.length && !lines[i].startsWith('=======')) {
        currentLines.push(lines[i]);
        i++;
      }

      if (i >= lines.length) break;
      const sepLine = i;
      i++;

      const incomingLines: string[] = [];
      while (i < lines.length && !lines[i].startsWith('>>>>>>>')) {
        incomingLines.push(lines[i]);
        i++;
      }

      if (i >= lines.length) break;
      const endLine = i;
      const incomingLabel = lines[i].slice(7).trim();

      hunks.push({
        index: hunkIndex++,
        startLine,
        sepLine,
        endLine,
        currentLines,
        incomingLines,
        currentLabel,
        incomingLabel,
      });
    }
    i++;
  }

  return hunks;
}

export function hasConflictMarkers(content: string): boolean {
  return /^<{7}\s/m.test(content) && /^={7}$/m.test(content) && /^>{7}\s/m.test(content);
}

export function resolveHunkLines(hunk: ConflictHunk, resolution: Resolution): string[] {
  switch (resolution) {
    case 'current':
      return hunk.currentLines;
    case 'incoming':
      return hunk.incomingLines;
    case 'both':
      return [...hunk.currentLines, ...hunk.incomingLines];
  }
}

export function buildResolvedContent(
  content: string,
  hunks: ConflictHunk[],
  resolutions: Map<number, Resolution>
): string {
  const lines = content.split('\n');
  const result: string[] = [];
  let i = 0;

  for (const hunk of hunks) {
    // Add lines before this hunk
    while (i < hunk.startLine) {
      result.push(lines[i]);
      i++;
    }

    const resolution = resolutions.get(hunk.index);
    if (resolution) {
      result.push(...resolveHunkLines(hunk, resolution));
    } else {
      // Keep conflict markers if not resolved
      while (i <= hunk.endLine) {
        result.push(lines[i]);
        i++;
      }
      continue;
    }

    // Skip past the conflict block
    i = hunk.endLine + 1;
  }

  // Add remaining lines after last hunk
  while (i < lines.length) {
    result.push(lines[i]);
    i++;
  }

  return result.join('\n');
}
