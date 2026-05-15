import { gitBranch } from './git';

export interface GitBranchUpdateContext {
  branch: string | null;
  preserveOnNull: boolean;
  requestProjectRoot: string | null;
  activeProjectRoot: string | null;
  requestId: number;
  latestRequestId: number;
}

export interface GitBranchUpdateDecision {
  shouldApply: boolean;
  nextBranch: string | null;
}

export function resolveGitBranchUpdate({
  branch,
  preserveOnNull,
  requestProjectRoot,
  activeProjectRoot,
  requestId,
  latestRequestId,
}: GitBranchUpdateContext): GitBranchUpdateDecision {
  if (requestId !== latestRequestId) {
    return { shouldApply: false, nextBranch: null };
  }

  if (!requestProjectRoot || requestProjectRoot !== activeProjectRoot) {
    return { shouldApply: false, nextBranch: null };
  }

  if (branch !== null) {
    return { shouldApply: true, nextBranch: branch };
  }

  if (!preserveOnNull) {
    return { shouldApply: true, nextBranch: null };
  }

  return { shouldApply: false, nextBranch: null };
}

let latestGitBranchRequestId = 0;

export function beginGitBranchRequest(): number {
  latestGitBranchRequestId += 1;
  return latestGitBranchRequestId;
}

export function getLatestGitBranchRequestId(): number {
  return latestGitBranchRequestId;
}

export function resetGitBranchRequestTracking(): void {
  latestGitBranchRequestId = 0;
}

/**
 * Update the gitBranch store for the currently active project only.
 * Stale in-flight responses are ignored so an older request cannot
 * overwrite branch state after the project root changes.
 */
export function updateGitBranch(context: GitBranchUpdateContext): void {
  const decision = resolveGitBranchUpdate(context);
  if (decision.shouldApply) {
    gitBranch.set(decision.nextBranch);
  }
}
