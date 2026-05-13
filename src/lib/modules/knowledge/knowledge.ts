/**
 * Typed wrappers around the `knowledge_*` Tauri commands.
 *
 * All UI code should use this module instead of calling `invoke()` directly.
 * It's the single place where IPC shapes, error handling, and defaults live,
 * which keeps components small and makes the Rust↔TS contract easy to audit.
 */

import { invoke } from '@tauri-apps/api/core';

// ── Shared types ─────────────────────────────────────────────────

/** Matches `ProjectInfo` on the Rust side. */
export interface ProjectInfo {
  project_root: string;
  db_hash: string;
  file_count: number;
  conversation_count: number;
  db_size_bytes: number;
  /** Unix seconds; 0 when the DB has no conversations or indexed files. */
  last_updated: number;
}

/** Matches `ConversationSummary` on the Rust side. */
export interface ConversationSummary {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
}

/** Chat message as persisted in `conversations.messages` (JSON-encoded). */
export interface KnowledgeMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

// ── Commands ─────────────────────────────────────────────────────

/** List every known project (all knowledge DBs on disk). */
export async function listProjects(): Promise<ProjectInfo[]> {
  return await invoke<ProjectInfo[]>('knowledge_list_projects');
}

/** List conversation summaries for a specific project. */
export async function listConversations(
  projectRoot: string,
): Promise<ConversationSummary[]> {
  return await invoke<ConversationSummary[]>('knowledge_list_conversations', { projectRoot });
}

/** Load a single conversation's full message history. Throws when the
 *  conversation can't be found or its JSON is corrupt. */
export async function loadConversation(
  projectRoot: string,
  id: string,
): Promise<KnowledgeMessage[]> {
  const json = await invoke<string>('knowledge_load_conversation', { projectRoot, id });
  try {
    const parsed = JSON.parse(json);
    if (!Array.isArray(parsed)) throw new Error('Conversation body is not an array');
    return parsed as KnowledgeMessage[];
  } catch (e) {
    throw new Error(`Failed to parse conversation ${id}: ${String(e)}`);
  }
}

/** Delete a single conversation from a project's knowledge DB. */
export async function deleteConversation(
  projectRoot: string,
  id: string,
): Promise<void> {
  await invoke<void>('knowledge_delete_conversation', { projectRoot, id });
}

/** Delete every conversation in a project (keeps the file index). */
export async function deleteProjectConversations(projectRoot: string): Promise<void> {
  await invoke<void>('knowledge_delete_conversations', { projectRoot });
}

/** Delete an entire project's knowledge DB — files + conversations. */
export async function deleteProject(projectRoot: string): Promise<void> {
  await invoke<void>('knowledge_delete_project', { projectRoot });
}

/** Wipe every project's knowledge DB. Irreversible. */
export async function deleteAllKnowledge(): Promise<void> {
  await invoke<void>('knowledge_delete_all_projects');
}

// ── Helpers ──────────────────────────────────────────────────────

/** Human-readable path suffix — the last few path segments joined with /.
 *  Avoids showing the full `/Users/foo/Projects/bar` every time. */
export function shortProjectName(root: string, segments = 2): string {
  if (!root || root === '(unknown)') return '(unknown project)';
  const parts = root.replace(/\\/g, '/').split('/').filter(Boolean);
  if (parts.length === 0) return root;
  return parts.slice(-segments).join('/');
}

/** "12 MB" / "340 KB" / "0 B". */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const idx = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / Math.pow(1024, idx);
  const fmt = value >= 100 || idx === 0 ? value.toFixed(0) : value.toFixed(1);
  return `${fmt} ${units[idx]}`;
}

/** Relative/absolute time label: "just now", "5m ago", "Mon 5", or "Nov 3 2023". */
export function formatRelativeTime(epochSecs: number): string {
  if (!epochSecs) return 'never';
  const now = Date.now() / 1000;
  const delta = now - epochSecs;
  if (delta < 60) return 'just now';
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86_400) return `${Math.floor(delta / 3600)}h ago`;
  const date = new Date(epochSecs * 1000);
  const thisYear = new Date().getFullYear();
  const opts: Intl.DateTimeFormatOptions = date.getFullYear() === thisYear
    ? { month: 'short', day: 'numeric' }
    : { month: 'short', day: 'numeric', year: 'numeric' };
  return date.toLocaleDateString([], opts);
}
