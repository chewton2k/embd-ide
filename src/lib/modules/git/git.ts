import { writable } from 'svelte/store';

export const projectRoot = writable<string | null>(null);
export const gitBranch = writable<string | null>(null);
export const sharedGitStatus = writable<Record<string, string>>({});
export const sharedGitRemoteStatus = writable<Record<string, string>>({});

export const fileTreeRefreshTrigger = writable<number>(0);
export function triggerFileTreeRefresh() {
  fileTreeRefreshTrigger.update(n => n + 1);
}
