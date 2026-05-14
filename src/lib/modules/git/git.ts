import { writable, type Writable, type Subscriber, type Unsubscriber } from 'svelte/store';

export const projectRoot = writable<string | null>(null);
export const gitBranch = writable<string | null>(null);

/**
 * Consolidated git decoration state for the file tree and tabs.
 *
 * Previously this was two separate writables (`sharedGitStatus` and
 * `sharedGitRemoteStatus`) that the file tree had to .set() in two
 * separate ticks after each `fetchGitStatus()` round. With the unified
 * `gitState` store, callers that want to update both slices at once
 * can do so in a single subscriber notification:
 *
 *   gitState.update(s => ({ status, remoteStatus }));
 *
 * The previous individual stores still exist as thin proxy slices over
 * `gitState`, so existing readers (`$sharedGitStatus[path]`) and
 * writers (`sharedGitStatus.set(record)`) keep working without
 * modification — call sites can be migrated incrementally.
 */
export interface GitState {
  status: Record<string, string>;
  remoteStatus: Record<string, string>;
}

export const gitState = writable<GitState>({ status: {}, remoteStatus: {} });

type SliceKey = 'status' | 'remoteStatus';

/**
 * Construct a `Writable<Record<string, string>>` that reads + writes the
 * named slice of `gitState`. Subscriber semantics match the legacy
 * standalone stores: subscribers are notified only when their slice
 * changes (Svelte's writable does its own dedup on `===` reference
 * equality), and `.set(v)` short-circuits when `v` is identical to the
 * current slice value.
 */
function makeGitSlice(key: SliceKey): Writable<Record<string, string>> {
  return {
    subscribe(run: Subscriber<Record<string, string>>): Unsubscriber {
      let last: Record<string, string> | undefined;
      const unsub = gitState.subscribe((s) => {
        const next = s[key];
        if (next === last) return;
        last = next;
        run(next);
      });
      return unsub;
    },
    set(value: Record<string, string>) {
      gitState.update((s) => (s[key] === value ? s : { ...s, [key]: value }));
    },
    update(fn: (current: Record<string, string>) => Record<string, string>) {
      gitState.update((s) => {
        const next = fn(s[key]);
        return next === s[key] ? s : { ...s, [key]: next };
      });
    },
  };
}

/** Working-tree git status keyed by absolute path. Backed by `gitState.status`. */
export const sharedGitStatus = makeGitSlice('status');

/** Remote-vs-local diff status keyed by absolute path. Backed by `gitState.remoteStatus`. */
export const sharedGitRemoteStatus = makeGitSlice('remoteStatus');

export const fileTreeRefreshTrigger = writable<number>(0);
export function triggerFileTreeRefresh() {
  fileTreeRefreshTrigger.update(n => n + 1);
}
