import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  gitState,
  sharedGitStatus,
  sharedGitRemoteStatus,
} from '$lib/modules/git/git';

describe('gitState consolidation', () => {
  beforeEach(() => {
    // Reset to a clean baseline so tests are order-independent.
    gitState.set({ status: {}, remoteStatus: {} });
  });

  afterEach(() => {
    gitState.set({ status: {}, remoteStatus: {} });
  });

  it('round-trips the unified state', () => {
    const next = {
      status: { '/a': 'M', '/b': 'A' },
      remoteStatus: { '/a': 'M' },
    };
    gitState.set(next);
    expect(get(gitState)).toBe(next);
  });

  it('sharedGitStatus.set writes through to gitState.status', () => {
    sharedGitStatus.set({ '/foo': 'M' });
    expect(get(gitState).status).toEqual({ '/foo': 'M' });
    // The other slice is untouched.
    expect(get(gitState).remoteStatus).toEqual({});
  });

  it('sharedGitRemoteStatus.set writes through to gitState.remoteStatus', () => {
    sharedGitRemoteStatus.set({ '/x': 'D' });
    expect(get(gitState).remoteStatus).toEqual({ '/x': 'D' });
    expect(get(gitState).status).toEqual({});
  });

  it('sharedGitStatus subscribers see status updates', () => {
    const seen: Record<string, string>[] = [];
    const unsub = sharedGitStatus.subscribe((v) => seen.push(v));
    expect(seen).toHaveLength(1); // initial empty
    sharedGitStatus.set({ '/x': 'M' });
    expect(seen).toHaveLength(2);
    expect(seen[1]).toEqual({ '/x': 'M' });
    unsub();
  });

  it('sharedGitStatus subscribers do NOT fire when remoteStatus changes', () => {
    const cb = vi.fn();
    const unsub = sharedGitStatus.subscribe(cb);
    cb.mockClear(); // discard initial fire
    sharedGitRemoteStatus.set({ '/x': 'M' });
    expect(cb).not.toHaveBeenCalled();
    unsub();
  });

  it('sharedGitRemoteStatus subscribers do NOT fire when status changes', () => {
    const cb = vi.fn();
    const unsub = sharedGitRemoteStatus.subscribe(cb);
    cb.mockClear();
    sharedGitStatus.set({ '/x': 'M' });
    expect(cb).not.toHaveBeenCalled();
    unsub();
  });

  it('gitState.update with both slices fires each shim subscriber exactly once', () => {
    const sCb = vi.fn();
    const rCb = vi.fn();
    const sUnsub = sharedGitStatus.subscribe(sCb);
    const rUnsub = sharedGitRemoteStatus.subscribe(rCb);
    sCb.mockClear();
    rCb.mockClear();

    // Atomic update: both slices change in a single notification.
    gitState.update(() => ({
      status: { '/a': 'M' },
      remoteStatus: { '/a': 'M' },
    }));

    expect(sCb).toHaveBeenCalledTimes(1);
    expect(rCb).toHaveBeenCalledTimes(1);
    sUnsub();
    rUnsub();
  });

  it('sharedGitStatus.update receives current slice and writes the result back', () => {
    sharedGitStatus.set({ '/a': 'M' });
    sharedGitStatus.update((curr) => ({ ...curr, '/b': 'A' }));
    expect(get(gitState).status).toEqual({ '/a': 'M', '/b': 'A' });
  });

  it('sharedGitStatus.set with the same reference is a no-op (no extra notification)', () => {
    const value = { '/x': 'M' };
    sharedGitStatus.set(value);
    const cb = vi.fn();
    const unsub = sharedGitStatus.subscribe(cb);
    cb.mockClear();
    sharedGitStatus.set(value); // same reference
    expect(cb).not.toHaveBeenCalled();
    unsub();
  });
});
