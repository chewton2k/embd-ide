/**
 * Session viewer — records agent run timelines for replay and inspection.
 *
 * Each agent run produces a session with a sequence of events:
 * prompt → plan → tool calls (with args + results) → final state.
 * Sessions can be browsed, replayed, or shared.
 */
import { writable, get } from 'svelte/store';

// ── Types ──

export type SessionEventType = 'user_message' | 'plan' | 'tool_call' | 'tool_result' | 'assistant_message' | 'error' | 'checkpoint';

export interface SessionEvent {
  id: string;
  type: SessionEventType;
  timestamp: number;
  data: Record<string, unknown>;
}

export interface AgentSession {
  id: string;
  title: string;
  startedAt: number;
  endedAt: number | null;
  events: SessionEvent[];
  model: string;
  provider: string;
  status: 'running' | 'completed' | 'cancelled' | 'failed';
}

// ── Store ──

export const sessions = writable<AgentSession[]>(loadSessions());
export const activeSessionId = writable<string | null>(null);

// ── Persistence ──

const STORAGE_KEY = 'leo-agent-sessions';
const MAX_SESSIONS = 50;
const MAX_EVENTS_PER_SESSION = 200;

function loadSessions(): AgentSession[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch { return []; }
}

function saveSessions(list: AgentSession[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(list.slice(0, MAX_SESSIONS)));
}

let saveTimeout: ReturnType<typeof setTimeout> | null = null;
sessions.subscribe((list) => {
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(() => saveSessions(list), 500);
});

// ── Session lifecycle ──

export function startSession(title: string, model: string, provider: string): string {
  const session: AgentSession = {
    id: `session-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    title,
    startedAt: Date.now(),
    endedAt: null,
    events: [],
    model,
    provider,
    status: 'running',
  };
  sessions.update(s => [session, ...s].slice(0, MAX_SESSIONS));
  activeSessionId.set(session.id);
  return session.id;
}

export function endSession(sessionId: string, status: 'completed' | 'cancelled' | 'failed' = 'completed') {
  sessions.update(s => s.map(sess =>
    sess.id === sessionId ? { ...sess, endedAt: Date.now(), status } : sess
  ));
  if (get(activeSessionId) === sessionId) activeSessionId.set(null);
}

export function deleteSession(sessionId: string) {
  sessions.update(s => s.filter(sess => sess.id !== sessionId));
}

export function clearAllSessions() {
  sessions.set([]);
  activeSessionId.set(null);
}

// ── Event recording ──

export function recordEvent(sessionId: string, type: SessionEventType, data: Record<string, unknown>) {
  const event: SessionEvent = {
    id: `evt-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    type,
    timestamp: Date.now(),
    data,
  };
  sessions.update(s => s.map(sess =>
    sess.id === sessionId
      ? { ...sess, events: [...sess.events, event].slice(-MAX_EVENTS_PER_SESSION) }
      : sess
  ));
}

// ── Queries ──

export function getSession(sessionId: string): AgentSession | undefined {
  return get(sessions).find(s => s.id === sessionId);
}

export function getSessionDuration(session: AgentSession): number {
  const end = session.endedAt || Date.now();
  return end - session.startedAt;
}

export function getSessionToolCalls(session: AgentSession): SessionEvent[] {
  return session.events.filter(e => e.type === 'tool_call');
}

/**
 * Format session duration for display.
 */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const remainSecs = secs % 60;
  return `${mins}m ${remainSecs}s`;
}
