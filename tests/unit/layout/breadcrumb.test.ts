import { describe, it, expect } from 'vitest';
import { breadcrumbSegmentsFor } from '$lib/modules/layout/breadcrumb';

describe('breadcrumbSegmentsFor', () => {
  it('returns an empty array when no active file is set', () => {
    expect(breadcrumbSegmentsFor(null, '/root')).toEqual([]);
    expect(breadcrumbSegmentsFor(undefined, '/root')).toEqual([]);
    expect(breadcrumbSegmentsFor('', '/root')).toEqual([]);
  });

  it('produces root + relative segments for a file under the project root', () => {
    const segs = breadcrumbSegmentsFor(
      '/Users/x/project/src/lib/foo.ts',
      '/Users/x/project',
    );
    expect(segs).toEqual([
      { name: 'project', path: '/Users/x/project' },
      { name: 'src', path: '/Users/x/project/src' },
      { name: 'lib', path: '/Users/x/project/src/lib' },
      { name: 'foo.ts', path: '/Users/x/project/src/lib/foo.ts' },
    ]);
  });

  it('returns a single basename segment when the file is outside the root', () => {
    const segs = breadcrumbSegmentsFor('/elsewhere/notes.md', '/Users/x/project');
    expect(segs).toEqual([{ name: 'notes.md', path: '/elsewhere/notes.md' }]);
  });

  it('returns a single basename segment when no project root is set', () => {
    const segs = breadcrumbSegmentsFor('/Users/x/scratch.txt', null);
    expect(segs).toEqual([{ name: 'scratch.txt', path: '/Users/x/scratch.txt' }]);
  });

  it('treats a file at the project root as root + filename', () => {
    const segs = breadcrumbSegmentsFor(
      '/Users/x/project/README.md',
      '/Users/x/project',
    );
    expect(segs).toEqual([
      { name: 'project', path: '/Users/x/project' },
      { name: 'README.md', path: '/Users/x/project/README.md' },
    ]);
  });

  it('normalizes Windows backslashes to forward slashes', () => {
    const segs = breadcrumbSegmentsFor(
      'C:\\dev\\project\\src\\main.ts',
      'C:\\dev\\project',
    );
    expect(segs).toEqual([
      { name: 'project', path: 'C:/dev/project' },
      { name: 'src', path: 'C:/dev/project/src' },
      { name: 'main.ts', path: 'C:/dev/project/src/main.ts' },
    ]);
  });

  it('does not match a project root that is a string-prefix but not a directory boundary', () => {
    // /proj should NOT match /project/file.ts as "inside the root".
    const segs = breadcrumbSegmentsFor('/project/file.ts', '/proj');
    // Falls through to single-basename branch.
    expect(segs).toEqual([{ name: 'file.ts', path: '/project/file.ts' }]);
  });
});
