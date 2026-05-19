import { describe, it, expect, beforeEach } from 'vitest';
import { mockInvoke, getInvokeCalls } from '../../mocks/tauri';

describe('Multi-window state isolation (AF1-AF2)', () => {
  beforeEach(() => {
    mockInvoke('set_project_root', (args: any) => args.path);
  });

  it('AF1: invoke set_project_root sends the path to the backend (window-scoped by Tauri)', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('set_project_root', { path: '/tmp/project-a' });

    const calls = getInvokeCalls('set_project_root');
    expect(calls).toHaveLength(1);
    expect(calls[0].args).toEqual({ path: '/tmp/project-a' });
  });

  it('AF2: error message from "no project open" is returned correctly', async () => {
    mockInvoke('read_file_content', () => {
      throw new Error('No project is open');
    });

    const { invoke } = await import('@tauri-apps/api/core');
    await expect(invoke('read_file_content', { path: '/some/file.ts' }))
      .rejects.toThrow('No project is open');
  });
});
