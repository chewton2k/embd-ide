import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

export interface AiChange {
  id: string;
  timestamp: number;
  description: string;
  filePath: string;
  beforeContent: string;
  afterContent: string;
}

export const aiChangeHistory = writable<AiChange[]>([]);

const MAX_HISTORY = 50;

/** Record an AI change for undo purposes. */
export function recordAiChange(filePath: string, description: string, beforeContent: string, afterContent: string) {
  const change: AiChange = {
    id: `change-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    timestamp: Date.now(),
    description,
    filePath,
    beforeContent,
    afterContent,
  };

  aiChangeHistory.update(history => {
    const updated = [change, ...history];
    return updated.slice(0, MAX_HISTORY);
  });
}

/** Revert a specific AI change by restoring the file to its before state. */
export async function revertAiChange(changeId: string): Promise<boolean> {
  const history = get(aiChangeHistory);
  const change = history.find(c => c.id === changeId);
  if (!change) return false;

  try {
    await invoke('write_file_content', { path: change.filePath, content: change.beforeContent });
    aiChangeHistory.update(h => h.filter(c => c.id !== changeId));
    return true;
  } catch {
    return false;
  }
}
