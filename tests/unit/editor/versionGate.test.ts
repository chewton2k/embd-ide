import { describe, expect, it } from 'vitest';
import { shouldDispatchVersionUpdate } from '$lib/modules/editor/versionGate';

describe('shouldDispatchVersionUpdate', () => {
  it('returns false when the file is undefined', () => {
    expect(shouldDispatchVersionUpdate(undefined, 0)).toBe(false);
  });

  it('returns false when version is 0 (no reload has happened yet)', () => {
    expect(shouldDispatchVersionUpdate({ version: 0, content: 'x' }, 0)).toBe(false);
  });

  it('returns true when version exceeds the last-handled value', () => {
    expect(shouldDispatchVersionUpdate({ version: 1, content: 'x' }, 0)).toBe(true);
    expect(shouldDispatchVersionUpdate({ version: 5, content: 'x' }, 3)).toBe(true);
  });

  it('returns false when version equals the last-handled value (no new reload)', () => {
    // The regression case: after acting on version 1, every subsequent
    // openFiles change still has version 1. Without this guard the
    // $effect re-fires and reverts user typing.
    expect(shouldDispatchVersionUpdate({ version: 1, content: 'x' }, 1)).toBe(false);
    expect(shouldDispatchVersionUpdate({ version: 7, content: 'x' }, 7)).toBe(false);
  });

  it('returns false when version is older than the last-handled value (defensive)', () => {
    // A newly-opened tab with a stale persisted version vs. a previous
    // session's lastHandled counter. Treated as "already handled" so
    // we don't re-dispatch stale content.
    expect(shouldDispatchVersionUpdate({ version: 2, content: 'x' }, 5)).toBe(false);
  });
});

describe('shouldDispatchVersionUpdate — regression scenario', () => {
  // Simulates the user-reported flow:
  //   1. accept AI edit → reloadFileContent bumps version 0→1
  //   2. effect runs once with file.version=1, lastHandled=0 → dispatches
  //   3. user types → updateFileContent flips modified → openFiles changes
  //   4. effect re-runs with file.version=1, lastHandled=1 → MUST NOT dispatch
  it('first dispatch on accept, then bails on subsequent typing-induced store changes', () => {
    let lastHandled = 0;
    const file = { version: 1, content: 'AI-applied content' };

    // Step 2: first run after the version bump.
    expect(shouldDispatchVersionUpdate(file, lastHandled)).toBe(true);
    lastHandled = file.version; // simulate the effect committing the version

    // Step 4: user typed; openFiles fired; version unchanged.
    expect(shouldDispatchVersionUpdate(file, lastHandled)).toBe(false);

    // A *second* AI accept later bumps version again.
    const fileAfterSecondAccept = { version: 2, content: 'second AI content' };
    expect(shouldDispatchVersionUpdate(fileAfterSecondAccept, lastHandled)).toBe(true);
  });
});

/**
 * The remaining behavior — `lastHandledVersion` Map maintenance,
 * close/rename callbacks — lives in Editor.svelte and is exercised
 * implicitly by the helper's contract. The most direct simulation is
 * the regression sequence above. The helper test is what locks down
 * the gate; the call-site logic is small and verified by inspection.
 *
 * Sanity test for the Map-based pattern Editor.svelte uses:
 */

describe('lastHandledVersion Map pattern (mirrors Editor.svelte usage)', () => {
  it('correctly gates per-path so unrelated paths do not affect each other', () => {
    const lastHandled = new Map<string, number>();

    const fileA = { version: 1, content: 'A' };
    const fileB = { version: 2, content: 'B' };

    // Path A: first dispatch.
    expect(shouldDispatchVersionUpdate(fileA, lastHandled.get('/A') ?? 0)).toBe(true);
    lastHandled.set('/A', fileA.version);

    // Path B is independent — its lastHandled is undefined → 0, so v=2 dispatches.
    expect(shouldDispatchVersionUpdate(fileB, lastHandled.get('/B') ?? 0)).toBe(true);
    lastHandled.set('/B', fileB.version);

    // Re-running with the same versions on either path is a no-op.
    expect(shouldDispatchVersionUpdate(fileA, lastHandled.get('/A') ?? 0)).toBe(false);
    expect(shouldDispatchVersionUpdate(fileB, lastHandled.get('/B') ?? 0)).toBe(false);
  });

  it('after a close/rename the cleanup of lastHandled should not affect other paths', () => {
    const lastHandled = new Map<string, number>();
    lastHandled.set('/A', 5);
    lastHandled.set('/B', 3);

    // Close A: simulate the cleanup effect's purge.
    lastHandled.delete('/A');

    // B is unaffected.
    expect(shouldDispatchVersionUpdate({ version: 3, content: 'B' }, lastHandled.get('/B') ?? 0)).toBe(false);
    // A is fresh again — a new reload starting from version 1 should
    // dispatch (since lastHandled for A is now undefined → 0).
    expect(shouldDispatchVersionUpdate({ version: 1, content: 'A' }, lastHandled.get('/A') ?? 0)).toBe(true);
  });

  it('rename migration preserves the gate', () => {
    const lastHandled = new Map<string, number>();
    lastHandled.set('/old.ts', 4);

    // Simulate the rename callback's migration.
    const versionHandled = lastHandled.get('/old.ts');
    if (versionHandled !== undefined) {
      lastHandled.delete('/old.ts');
      lastHandled.set('/new.ts', versionHandled);
    }

    // A version-4 file at the new path is treated as already-handled.
    expect(shouldDispatchVersionUpdate({ version: 4, content: 'x' }, lastHandled.get('/new.ts') ?? 0)).toBe(false);
    // A version-5 reload at the new path dispatches.
    expect(shouldDispatchVersionUpdate({ version: 5, content: 'x' }, lastHandled.get('/new.ts') ?? 0)).toBe(true);
    // Old path is no longer tracked.
    expect(lastHandled.has('/old.ts')).toBe(false);
  });
});
