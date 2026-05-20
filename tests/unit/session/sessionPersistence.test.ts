import { describe, it, expect, beforeEach } from 'vitest';
import { openFiles, activeFilePath, expandedDirsStore } from '$lib/modules/explorer/files';
import { terminalTabs, showTerminal } from '$lib/modules/terminal/shell';
import { buildSessionData } from '$lib/modules/session/session';

beforeEach(() => {
  openFiles.set([]);
  activeFilePath.set(null);
  terminalTabs.set([]);
  showTerminal.set(false);
  expandedDirsStore.set(new Set());
});

describe('buildSessionData', () => {
  it('returns terminal_count matching the number of terminal tabs', () => {
    terminalTabs.set([{ id: 1, name: 'bash' }, { id: 2, name: 'zsh' }, { id: 3, name: 'node' }]);
    expect(buildSessionData().terminal_count).toBe(3);
  });

  it('returns terminal_visible matching showTerminal store value', () => {
    showTerminal.set(true);
    expect(buildSessionData().terminal_visible).toBe(true);

    showTerminal.set(false);
    expect(buildSessionData().terminal_visible).toBe(false);
  });

  it('returns expanded_dirs as an array from the expandedDirsStore Set', () => {
    expandedDirsStore.set(new Set(['/src', '/src/lib', '/tests']));
    const result = buildSessionData();
    expect(result.expanded_dirs).toEqual(expect.arrayContaining(['/src', '/src/lib', '/tests']));
    expect(result.expanded_dirs).toHaveLength(3);
  });

  it('returns empty expanded_dirs when store is empty', () => {
    expandedDirsStore.set(new Set());
    expect(buildSessionData().expanded_dirs).toEqual([]);
  });

  it('returns 0 terminal_count when no tabs are open', () => {
    terminalTabs.set([]);
    expect(buildSessionData().terminal_count).toBe(0);
  });

  it('includes all open files with pinned status', () => {
    openFiles.set([
      { path: '/a.ts', name: 'a.ts', content: '', modified: false, pinned: true, version: 1 },
      { path: '/b.ts', name: 'b.ts', content: '', modified: false, pinned: false, version: 1 },
    ]);
    const result = buildSessionData();
    expect(result.open_files).toEqual([
      { path: '/a.ts', pinned: true },
      { path: '/b.ts', pinned: false },
    ]);
  });
});
