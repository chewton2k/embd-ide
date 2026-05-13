import { describe, it, expect } from 'vitest';
import { parseConflicts, hasConflictMarkers } from '$lib/modules/git/mergeUtils';

describe('parseConflicts', () => {
  it('parses a known three-way diff into expected hunks', () => {
    const content = `line 1
<<<<<<< HEAD
our change
=======
their change
>>>>>>> feature
line 2`;

    const hunks = parseConflicts(content);
    expect(hunks).toHaveLength(1);
    expect(hunks[0].startLine).toBe(1);
    expect(hunks[0].sepLine).toBe(3);
    expect(hunks[0].endLine).toBe(5);
    expect(hunks[0].currentLines).toEqual(['our change']);
    expect(hunks[0].incomingLines).toEqual(['their change']);
    expect(hunks[0].currentLabel).toBe('HEAD');
    expect(hunks[0].incomingLabel).toBe('feature');
  });

  it('handles multiple conflict hunks', () => {
    const content = `<<<<<<< HEAD
a
=======
b
>>>>>>> branch
middle
<<<<<<< HEAD
c
=======
d
>>>>>>> branch`;

    const hunks = parseConflicts(content);
    expect(hunks).toHaveLength(2);
    expect(hunks[0].currentLines).toEqual(['a']);
    expect(hunks[1].currentLines).toEqual(['c']);
  });

  it('returns empty array for no conflicts', () => {
    expect(parseConflicts('clean file')).toEqual([]);
  });
});

describe('hasConflictMarkers', () => {
  it('detects standard conflict markers', () => {
    const content = `<<<<<<< HEAD
ours
=======
theirs
>>>>>>> branch`;
    expect(hasConflictMarkers(content)).toBe(true);
  });

  it('returns false for clean content', () => {
    expect(hasConflictMarkers('no conflicts here')).toBe(false);
  });

  it('returns false for partial markers', () => {
    expect(hasConflictMarkers('<<<<<<< HEAD\nonly start')).toBe(false);
  });
});
