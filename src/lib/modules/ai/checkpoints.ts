/**
 * Frontend checkpoint management for agent undo.
 *
 * Wraps the Rust git_create_checkpoint / git_restore_checkpoint /
 * git_list_checkpoints commands with a Svelte store.
 */
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { projectRoot } from '../git/git';
import { log } from '../logging';

// ── Types ──

export interface Checkpoint {
  id: string;
  message: string;
  timestamp: number;
}

// ── Store ──

export const checkpoints = writable<Checkpoint[]>([]);

// ── API ──

/**
 * Create a checkpoint before agent execution.
 * Returns the checkpoint ID, or null if creation failed (non-fatal).
 */
export async function createCheckpoint(message: string): Promise<string | null> {
  const root = get(projectRoot);
  if (!root) return null;

  try {
    const id = await invoke<string>('git_create_checkpoint', {
      repoPath: root,
      message,
    });
    await refreshCheckpoints();
    return id;
  } catch (e) {
    log.warn('Failed to create checkpoint', e);
    return null;
  }
}

/**
 * Restore a checkpoint — reverts working directory to that point.
 */
export async function restoreCheckpoint(checkpointId: string): Promise<boolean> {
  const root = get(projectRoot);
  if (!root) return false;

  try {
    await invoke('git_restore_checkpoint', {
      repoPath: root,
      checkpointId,
    });
    await refreshCheckpoints();
    return true;
  } catch (e) {
    log.error('Failed to restore checkpoint', e);
    return false;
  }
}

/**
 * Refresh the checkpoint list from git.
 */
export async function refreshCheckpoints(): Promise<void> {
  const root = get(projectRoot);
  if (!root) return;

  try {
    const list = await invoke<Checkpoint[]>('git_list_checkpoints', {
      repoPath: root,
    });
    checkpoints.set(list);
  } catch {
    checkpoints.set([]);
  }
}

/**
 * Format a checkpoint timestamp for display.
 */
export function formatCheckpointTime(timestamp: number): string {
  if (!timestamp) return '';
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);

  if (diffMin < 1) return 'just now';
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  return date.toLocaleDateString();
}
