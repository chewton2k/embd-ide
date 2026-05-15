/**
 * Self-verify: auto-run linter/typecheck after agent edits.
 *
 * Detects the project type from config files and runs the appropriate
 * check command. Errors are returned as a string for the agent to fix.
 */
import { invoke } from '@tauri-apps/api/core';

// ── Types ──

export interface VerifyResult {
  success: boolean;
  errors: string;
  command: string;
}

export interface ProjectType {
  name: string;
  checkCommand: string;
}

// ── Project type detection ──

/**
 * Detect the project type and return the appropriate check command.
 * Checks for config files in the project root.
 */
export async function detectProjectType(projectRoot: string): Promise<ProjectType | null> {
  const checks: { file: string; type: ProjectType }[] = [
    { file: 'tsconfig.json', type: { name: 'typescript', checkCommand: 'npx tsc --noEmit' } },
    { file: 'Cargo.toml', type: { name: 'rust', checkCommand: 'cargo check 2>&1' } },
    { file: 'pyproject.toml', type: { name: 'python', checkCommand: 'python -m py_compile' } },
    { file: 'package.json', type: { name: 'node', checkCommand: 'npm run check --if-present 2>&1 || npx tsc --noEmit 2>&1' } },
    { file: 'go.mod', type: { name: 'go', checkCommand: 'go build ./... 2>&1' } },
  ];

  for (const check of checks) {
    try {
      await invoke('read_file_content', { path: `${projectRoot}/${check.file}` });
      return check.type;
    } catch {
      // File doesn't exist, try next
    }
  }

  return null;
}

/**
 * Determine the check command from a project type string.
 * Used for testing without filesystem access.
 */
export function getCheckCommand(projectType: string): string | null {
  const commands: Record<string, string> = {
    typescript: 'npx tsc --noEmit',
    rust: 'cargo check 2>&1',
    node: 'npm run check --if-present 2>&1 || npx tsc --noEmit 2>&1',
    python: 'python -m py_compile',
    go: 'go build ./... 2>&1',
  };
  return commands[projectType] ?? null;
}

// ── Verify execution ──

/**
 * Run the project's type-checker/linter and return the result.
 * Returns null if no check command is available.
 */
export async function runVerify(projectRoot: string): Promise<VerifyResult | null> {
  const projectType = await detectProjectType(projectRoot);
  if (!projectType) return null;

  try {
    const result = await invoke<{ stdout: string; stderr: string; exit_code: number }>(
      'run_command_capture',
      { command: projectType.checkCommand, cwd: projectRoot, timeoutMs: 60000 }
    );

    const output = (result.stdout + '\n' + result.stderr).trim();

    if (result.exit_code === 0) {
      return { success: true, errors: '', command: projectType.checkCommand };
    }

    // Truncate long error output
    const MAX = 3000;
    const errors = output.length > MAX
      ? output.slice(0, MAX) + '\n... (truncated)'
      : output;

    return { success: false, errors, command: projectType.checkCommand };
  } catch (e) {
    return { success: false, errors: `Verify command failed: ${e}`, command: projectType.checkCommand };
  }
}

/**
 * Format verify errors for injection into the agent's context.
 */
export function formatVerifyErrors(result: VerifyResult): string {
  if (result.success) return '✓ No errors found.';
  return `⚠ Verification failed (${result.command}):\n\`\`\`\n${result.errors}\n\`\`\`\nPlease fix these errors.`;
}
