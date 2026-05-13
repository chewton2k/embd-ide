import { writable } from 'svelte/store';

export interface ToastEntry {
  id: number;
  level: 'info' | 'warn' | 'error' | 'success';
  message: string;
  durationMs: number;
}

export const toasts = writable<ToastEntry[]>([]);

let nextId = 1;

export function showToast(opts: Omit<ToastEntry, 'id'> | { level: ToastEntry['level']; message: string }): number {
  const id = nextId++;
  const durationMs = 'durationMs' in opts ? opts.durationMs :
    opts.level === 'error' ? 0 :
    opts.level === 'warn' ? 8000 : 5000;
  const entry: ToastEntry = { id, level: opts.level, message: opts.message, durationMs };
  toasts.update(list => [...list, entry]);
  if (durationMs > 0) {
    setTimeout(() => dismissToast(id), durationMs);
  }
  return id;
}

export function dismissToast(id: number): void {
  toasts.update(list => list.filter(t => t.id !== id));
}
