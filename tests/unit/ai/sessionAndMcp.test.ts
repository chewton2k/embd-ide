import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  sessions, startSession, endSession, deleteSession, clearAllSessions,
  recordEvent, getSession, getSessionDuration, getSessionToolCalls,
  formatDuration,
} from '$lib/modules/ai/sessionViewer';
import { addServer, removeServer, toggleServer, mcpServers } from '$lib/modules/ai/mcp';

describe('sessionViewer', () => {
  beforeEach(() => {
    sessions.set([]);
  });

  describe('startSession', () => {
    it('creates a session and adds it to the store', () => {
      const id = startSession('Test task', 'gpt-4o', 'openai');
      const all = get(sessions);
      expect(all).toHaveLength(1);
      expect(all[0].id).toBe(id);
      expect(all[0].title).toBe('Test task');
      expect(all[0].status).toBe('running');
      expect(all[0].model).toBe('gpt-4o');
    });
  });

  describe('endSession', () => {
    it('marks session as completed', () => {
      const id = startSession('Task', 'model', 'provider');
      endSession(id, 'completed');
      const session = get(sessions)[0];
      expect(session.status).toBe('completed');
      expect(session.endedAt).toBeGreaterThan(0);
    });

    it('marks session as cancelled', () => {
      const id = startSession('Task', 'model', 'provider');
      endSession(id, 'cancelled');
      expect(get(sessions)[0].status).toBe('cancelled');
    });
  });

  describe('recordEvent', () => {
    it('adds events to the session', () => {
      const id = startSession('Task', 'model', 'provider');
      recordEvent(id, 'user_message', { content: 'hello' });
      recordEvent(id, 'tool_call', { name: 'read_file', args: { path: 'a.ts' } });
      const session = get(sessions)[0];
      expect(session.events).toHaveLength(2);
      expect(session.events[0].type).toBe('user_message');
      expect(session.events[1].type).toBe('tool_call');
    });
  });

  describe('deleteSession', () => {
    it('removes a session', () => {
      const id = startSession('Task', 'model', 'provider');
      deleteSession(id);
      expect(get(sessions)).toHaveLength(0);
    });
  });

  describe('clearAllSessions', () => {
    it('removes all sessions', () => {
      startSession('A', 'm', 'p');
      startSession('B', 'm', 'p');
      clearAllSessions();
      expect(get(sessions)).toHaveLength(0);
    });
  });

  describe('getSessionToolCalls', () => {
    it('filters only tool_call events', () => {
      const id = startSession('Task', 'model', 'provider');
      recordEvent(id, 'user_message', { content: 'hi' });
      recordEvent(id, 'tool_call', { name: 'read_file' });
      recordEvent(id, 'assistant_message', { content: 'done' });
      recordEvent(id, 'tool_call', { name: 'edit_file' });
      const session = get(sessions)[0];
      const toolCalls = getSessionToolCalls(session);
      expect(toolCalls).toHaveLength(2);
    });
  });

  describe('formatDuration', () => {
    it('formats milliseconds', () => {
      expect(formatDuration(500)).toBe('500ms');
    });

    it('formats seconds', () => {
      expect(formatDuration(5000)).toBe('5s');
      expect(formatDuration(45000)).toBe('45s');
    });

    it('formats minutes and seconds', () => {
      expect(formatDuration(90000)).toBe('1m 30s');
      expect(formatDuration(125000)).toBe('2m 5s');
    });
  });
});

describe('mcp', () => {
  beforeEach(() => {
    mcpServers.set([]);
  });

  describe('addServer', () => {
    it('adds a server with generated id', () => {
      const server = addServer({
        name: 'GitHub',
        transport: 'stdio',
        command: 'npx @modelcontextprotocol/server-github',
        enabled: true,
      });
      expect(server.id).toMatch(/^mcp-/);
      expect(get(mcpServers)).toHaveLength(1);
      expect(get(mcpServers)[0].name).toBe('GitHub');
    });
  });

  describe('removeServer', () => {
    it('removes a server by id', () => {
      const server = addServer({ name: 'Test', transport: 'stdio', command: 'test', enabled: true });
      removeServer(server.id);
      expect(get(mcpServers)).toHaveLength(0);
    });
  });

  describe('toggleServer', () => {
    it('toggles enabled state', () => {
      const server = addServer({ name: 'Test', transport: 'stdio', command: 'test', enabled: true });
      expect(get(mcpServers)[0].enabled).toBe(true);
      toggleServer(server.id);
      expect(get(mcpServers)[0].enabled).toBe(false);
      toggleServer(server.id);
      expect(get(mcpServers)[0].enabled).toBe(true);
    });
  });
});
