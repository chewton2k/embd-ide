import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mockInvoke, getInvokeCalls, resetInvokeMocks } from '../../mocks/tauri';

// Control the window label returned by getCurrentWebviewWindow()
const mockLabel = { value: 'main' };
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({ label: mockLabel.value }),
}));

import {
  listConversations,
  loadConversation,
  deleteConversation,
  deleteProjectConversations,
} from '$lib/modules/knowledge/knowledge';

beforeEach(() => {
  resetInvokeMocks();
  mockLabel.value = 'main';
});

describe('admin command routing', () => {
  const root = '/projects/test';
  const id = 'conv-1';

  describe('listConversations', () => {
    it('uses knowledge_admin_list_conversations from settings window', async () => {
      mockLabel.value = 'settings';
      mockInvoke('knowledge_admin_list_conversations', () => []);
      await listConversations(root);
      expect(getInvokeCalls('knowledge_admin_list_conversations')).toHaveLength(1);
    });

    it('uses knowledge_list_conversations from non-settings window', async () => {
      mockLabel.value = 'main';
      mockInvoke('knowledge_list_conversations', () => []);
      await listConversations(root);
      expect(getInvokeCalls('knowledge_list_conversations')).toHaveLength(1);
    });
  });

  describe('loadConversation', () => {
    it('uses knowledge_admin_load_conversation from settings window', async () => {
      mockLabel.value = 'settings';
      mockInvoke('knowledge_admin_load_conversation', () => '[]');
      await loadConversation(root, id);
      expect(getInvokeCalls('knowledge_admin_load_conversation')).toHaveLength(1);
    });

    it('uses knowledge_load_conversation from non-settings window', async () => {
      mockLabel.value = 'main';
      mockInvoke('knowledge_load_conversation', () => '[]');
      await loadConversation(root, id);
      expect(getInvokeCalls('knowledge_load_conversation')).toHaveLength(1);
    });

    it('throws when JSON is not an array', async () => {
      mockInvoke('knowledge_load_conversation', () => '{"not":"array"}');
      await expect(loadConversation(root, id)).rejects.toThrow('not an array');
    });

    it('throws when JSON is corrupt', async () => {
      mockInvoke('knowledge_load_conversation', () => '{broken');
      await expect(loadConversation(root, id)).rejects.toThrow('Failed to parse conversation');
    });
  });

  describe('deleteConversation', () => {
    it('uses knowledge_admin_delete_conversation from settings window', async () => {
      mockLabel.value = 'settings';
      mockInvoke('knowledge_admin_delete_conversation', () => undefined);
      await deleteConversation(root, id);
      expect(getInvokeCalls('knowledge_admin_delete_conversation')).toHaveLength(1);
    });

    it('uses knowledge_delete_conversation from non-settings window', async () => {
      mockLabel.value = 'main';
      mockInvoke('knowledge_delete_conversation', () => undefined);
      await deleteConversation(root, id);
      expect(getInvokeCalls('knowledge_delete_conversation')).toHaveLength(1);
    });
  });

  describe('deleteProjectConversations', () => {
    it('uses knowledge_admin_delete_conversations from settings window', async () => {
      mockLabel.value = 'settings';
      mockInvoke('knowledge_admin_delete_conversations', () => undefined);
      await deleteProjectConversations(root);
      expect(getInvokeCalls('knowledge_admin_delete_conversations')).toHaveLength(1);
    });

    it('uses knowledge_delete_conversations from non-settings window', async () => {
      mockLabel.value = 'main';
      mockInvoke('knowledge_delete_conversations', () => undefined);
      await deleteProjectConversations(root);
      expect(getInvokeCalls('knowledge_delete_conversations')).toHaveLength(1);
    });
  });
});
