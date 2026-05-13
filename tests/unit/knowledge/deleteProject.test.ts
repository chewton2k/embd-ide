import { describe, it, expect } from 'vitest';
import { mockInvoke, getInvokeCalls } from '../../mocks/tauri';

describe('deleteProjectByHash', () => {
  it('invokes knowledge_delete_by_hash with the correct hash', async () => {
    mockInvoke('knowledge_delete_by_hash', () => undefined);
    const { deleteProjectByHash } = await import('$lib/modules/knowledge/knowledge');
    await deleteProjectByHash('abcdef0123456789');
    const calls = getInvokeCalls('knowledge_delete_by_hash');
    expect(calls).toHaveLength(1);
    expect(calls[0].args).toEqual({ dbHash: 'abcdef0123456789' });
  });
});

describe('deleteProject (valid path)', () => {
  it('invokes knowledge_delete_project with project_root', async () => {
    mockInvoke('knowledge_delete_project', () => undefined);
    const { deleteProject } = await import('$lib/modules/knowledge/knowledge');
    await deleteProject('/Users/test/project');
    const calls = getInvokeCalls('knowledge_delete_project');
    expect(calls).toHaveLength(1);
    expect(calls[0].args).toEqual({ projectRoot: '/Users/test/project' });
  });
});
