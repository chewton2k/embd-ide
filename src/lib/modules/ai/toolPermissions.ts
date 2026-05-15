/**
 * Per-tool permission rules for the agent.
 *
 * Replaces the old DANGEROUS_PATTERNS regex approach with structured
 * per-tool checks. Each tool has a permission level:
 *
 * - 'allow'  — always permitted, no user prompt
 * - 'ask'    — requires user confirmation before execution
 * - 'deny'   — blocked entirely
 *
 * The `run_command` tool additionally checks command content against
 * dangerous patterns to escalate from 'allow' to 'ask'.
 */
import type { ToolName } from './tools';

export type PermissionLevel = 'allow' | 'ask' | 'deny';

export interface ToolPermission {
  tool: ToolName;
  level: PermissionLevel;
}

// ── Default permission policy ──

const DEFAULT_PERMISSIONS: Record<ToolName, PermissionLevel> = {
  read_file: 'allow',
  edit_file: 'ask',
  run_command: 'ask',
  search_files: 'allow',
  grep: 'allow',
  list_dir: 'allow',
};

// ── Dangerous command patterns ──

const DANGEROUS_COMMAND_PATTERNS: RegExp[] = [
  /rm\s+(-rf?|--recursive)\s/i,
  /rm\s+-[a-z]*f/i,
  /rmdir/i,
  /git\s+push\s+--force/i,
  /git\s+reset\s+--hard/i,
  /git\s+clean\s+-f/i,
  /drop\s+(table|database)/i,
  /truncate\s+table/i,
  /delete\s+from\s+\w+\s*;?\s*$/i,
  /:()\s*\{\s*:\|:&\s*\}\s*;/,
  /mkfs/i,
  /dd\s+if=/i,
  />\s*\/dev\/sd/i,
  /chmod\s+-R\s+777/i,
  /curl.*\|\s*(bash|sh)/i,
  /wget.*\|\s*(bash|sh)/i,
  /sudo\s+rm/i,
];

// ── Permission checks ──

/**
 * Check if a tool call is permitted under the current policy.
 * Returns the permission level for the specific invocation.
 */
export function checkPermission(
  tool: ToolName,
  args: Record<string, string>,
  autoApprove: boolean,
): PermissionLevel {
  // Dangerous command check takes priority over everything
  if (tool === 'run_command' && args.command) {
    if (isDangerousCommand(args.command)) return 'deny';
  }

  const base = DEFAULT_PERMISSIONS[tool] ?? 'ask';

  // Auto-approve mode bypasses 'ask' (but never bypasses 'deny')
  if (autoApprove && base === 'ask') return 'allow';

  return base;
}

/**
 * Check if a shell command matches any dangerous pattern.
 */
export function isDangerousCommand(command: string): boolean {
  return DANGEROUS_COMMAND_PATTERNS.some(p => p.test(command));
}

/**
 * Get a human-readable description of why a tool call was blocked.
 */
export function getBlockReason(tool: ToolName, args: Record<string, string>): string {
  if (tool === 'run_command' && args.command && isDangerousCommand(args.command)) {
    return `Dangerous command blocked: "${args.command}"`;
  }
  return `Tool "${tool}" requires approval.`;
}
