import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { openFiles, activeFilePath, addFile, closeFile, updateFileContent, getFileContent, reloadFileContent, closeAllUnpinned, renameOpenFile, togglePin, markFileSaved } from '$lib/modules/explorer/files';

beforeEach(() => {
  openFiles.set([]);
  activeFilePath.set(null);
});

describe('addFile', () => {
  it('adds a new tab with empty content and pinned=false', () => {
    addFile('/test/file.ts', 'file.ts');
    const files = get(openFiles);
    expect(files).toHaveLength(1);
    expect(files[0]).toMatchObject({ path: '/test/file.ts', name: 'file.ts', content: '', modified: false, pinned: false });
  });

  it('activates an already-open file rather than duplicating', () => {
    addFile('/test/file.ts', 'file.ts');
    addFile('/test/other.ts', 'other.ts');
    addFile('/test/file.ts', 'file.ts');
    expect(get(openFiles)).toHaveLength(2);
    expect(get(activeFilePath)).toBe('/test/file.ts');
  });

  it('evicts oldest unpinned/unmodified when over maxTabs', () => {
    // maxTabs defaults to 9; add 10 files
    for (let i = 0; i < 10; i++) {
      addFile(`/test/file${i}.ts`, `file${i}.ts`);
    }
    const files = get(openFiles);
    expect(files.length).toBeLessThanOrEqual(9);
    // First file should have been evicted
    expect(files.find(f => f.path === '/test/file0.ts')).toBeUndefined();
  });
});

describe('closeFile', () => {
  it('keeps pinned files', () => {
    addFile('/test/file.ts', 'file.ts');
    togglePin('/test/file.ts');
    closeFile('/test/file.ts');
    expect(get(openFiles)).toHaveLength(1);
  });

  it('removes the content-cache entry', () => {
    addFile('/test/file.ts', 'file.ts');
    updateFileContent('/test/file.ts', 'hello');
    expect(getFileContent('/test/file.ts')).toBe('hello');
    closeFile('/test/file.ts');
    expect(getFileContent('/test/file.ts')).toBeNull();
  });
});

describe('updateFileContent', () => {
  it('flips modified only on the first edit', () => {
    addFile('/test/file.ts', 'file.ts');
    expect(get(openFiles)[0].modified).toBe(false);
    updateFileContent('/test/file.ts', 'a');
    expect(get(openFiles)[0].modified).toBe(true);
    // Second edit should not trigger another store update (modified already true)
    updateFileContent('/test/file.ts', 'ab');
    expect(get(openFiles)[0].modified).toBe(true);
  });
});

describe('getFileContent', () => {
  it('returns the cached value after updateFileContent', () => {
    addFile('/test/file.ts', 'file.ts');
    updateFileContent('/test/file.ts', 'cached content');
    expect(getFileContent('/test/file.ts')).toBe('cached content');
  });
});

describe('reloadFileContent', () => {
  it('updates both store content and cache and bumps version', () => {
    addFile('/test/file.ts', 'file.ts');
    updateFileContent('/test/file.ts', 'old');
    reloadFileContent('/test/file.ts', 'new from disk');
    expect(getFileContent('/test/file.ts')).toBe('new from disk');
    const file = get(openFiles)[0];
    expect(file.content).toBe('new from disk');
    expect(file.modified).toBe(false);
    expect(file.version).toBe(1);
  });
});

describe('closeAllUnpinned', () => {
  it('purges cache for closed files only', () => {
    addFile('/test/a.ts', 'a.ts');
    addFile('/test/b.ts', 'b.ts');
    togglePin('/test/a.ts');
    updateFileContent('/test/a.ts', 'pinned content');
    updateFileContent('/test/b.ts', 'unpinned content');
    closeAllUnpinned();
    expect(getFileContent('/test/a.ts')).toBe('pinned content');
    expect(getFileContent('/test/b.ts')).toBeNull();
  });
});

describe('renameOpenFile', () => {
  it('migrates the cache key', () => {
    addFile('/test/old.ts', 'old.ts');
    updateFileContent('/test/old.ts', 'content');
    renameOpenFile('/test/old.ts', '/test/new.ts', 'new.ts');
    expect(getFileContent('/test/old.ts')).toBeNull();
    expect(getFileContent('/test/new.ts')).toBe('content');
    expect(get(openFiles)[0].path).toBe('/test/new.ts');
  });
});
