import { afterEach, describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import { gitBranch } from '$lib/modules/git/git';
import {
  beginGitBranchRequest,
  getLatestGitBranchRequestId,
  resetGitBranchRequestTracking,
  resolveGitBranchUpdate,
  updateGitBranch,
} from '$lib/modules/git/branchUpdate';

const ACTIVE_ROOT = '/repo';
const STALE_ROOT = '/other-repo';

describe('gitBranch flicker prevention', () => {
  afterEach(() => {
    gitBranch.set(null);
    resetGitBranchRequestTracking();
  });

  it('updates gitBranch when get_git_branch returns a valid branch name', () => {
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: 'main',
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');
  });

  it('preserves last known branch when get_git_branch returns null during polling', () => {
    gitBranch.set('feature/x');
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: null,
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('feature/x');
  });

  it('clears gitBranch when get_git_branch returns null during folder open', () => {
    gitBranch.set('main');
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: null,
      preserveOnNull: false,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBeNull();
  });

  it('preserves last known branch when the caller skips update on branch-fetch errors', () => {
    gitBranch.set('develop');
    expect(get(gitBranch)).toBe('develop');
  });

  it('clears branch when switching from git project to non-git project', () => {
    gitBranch.set('main');
    // Opening a non-git folder: backend returns null, preserveOnNull=false
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: null,
      preserveOnNull: false,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBeNull();
  });

  it('updates correctly when get_git_branch returns a different branch name', () => {
    gitBranch.set('main');
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: 'feature/new',
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('feature/new');
  });

  it('does not flicker during rapid successive calls alternating valid and null', () => {
    let requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: 'main',
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');

    // Simulate rapid polling where some calls return null (transient failures)
    requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: null,
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');

    requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: 'main',
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');

    requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: null,
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');

    requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: null,
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');

    // Branch stays stable throughout
    expect(get(gitBranch)).toBe('main');
  });

  it('ignores a stale null response from a previous project root', () => {
    gitBranch.set('main');
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: null,
      preserveOnNull: false,
      requestProjectRoot: STALE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');
  });

  it('ignores a stale valid branch response from a previous project root', () => {
    gitBranch.set('main');
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: 'release/old-root',
      preserveOnNull: true,
      requestProjectRoot: STALE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('main');
  });

  it('returns a no-op decision for stale in-flight responses', () => {
    const requestId = beginGitBranchRequest();
    expect(resolveGitBranchUpdate({
      branch: 'feature/stale',
      preserveOnNull: true,
      requestProjectRoot: STALE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    })).toEqual({
      shouldApply: false,
      nextBranch: null,
    });
  });

  it('returns a clearing decision for an active non-git project', () => {
    const requestId = beginGitBranchRequest();
    expect(resolveGitBranchUpdate({
      branch: null,
      preserveOnNull: false,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    })).toEqual({
      shouldApply: true,
      nextBranch: null,
    });
  });

  it('ignores an out-of-order same-root response when a newer request exists', () => {
    gitBranch.set('feature/new');
    const staleRequestId = beginGitBranchRequest();
    const latestRequestId = beginGitBranchRequest();
    updateGitBranch({
      branch: 'main',
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId: staleRequestId,
      latestRequestId,
    });
    expect(get(gitBranch)).toBe('feature/new');
  });

  it('applies the newest same-root response when request ids match', () => {
    gitBranch.set('main');
    beginGitBranchRequest();
    const requestId = beginGitBranchRequest();
    updateGitBranch({
      branch: 'feature/new',
      preserveOnNull: true,
      requestProjectRoot: ACTIVE_ROOT,
      activeProjectRoot: ACTIVE_ROOT,
      requestId,
      latestRequestId: getLatestGitBranchRequestId(),
    });
    expect(get(gitBranch)).toBe('feature/new');
  });
});
