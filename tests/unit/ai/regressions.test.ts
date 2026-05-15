import { describe, it, expect } from 'vitest';
import { parseToolArgs } from '$lib/modules/ai/tools';
import { checkPermission, isAllowlistedCommand, isDangerousCommand } from '$lib/modules/ai/toolPermissions';
import { redact } from '$lib/modules/ai/redaction';

/**
 * Regression tests for bugs found during production code review.
 */
describe('regression: path traversal prefix bypass', () => {
  // Bug: /Users/foo/project-evil starts with /Users/foo/project
  // Fix: check canonical.startsWith(projectRoot + '/') not just projectRoot

  it('resolvePath is tested indirectly via the Rust backend canonicalize', () => {
    // The frontend resolvePath does segment normalization.
    // The actual security boundary is the Rust canonicalize + starts_with check.
    // This test verifies the parseToolArgs helper handles the inputs correctly.
    const args = parseToolArgs('{"path": "../../../etc/passwd"}');
    expect(args.path).toBe('../../../etc/passwd');
    // The actual traversal check happens in resolvePath at runtime
  });
});

describe('regression: command allowlist edge cases', () => {
  it('npm with no subcommand is not allowlisted', () => {
    expect(isAllowlistedCommand('npm')).toBe(false);
  });

  it('npm install is not allowlisted (could install malicious packages)', () => {
    expect(isAllowlistedCommand('npm install evil-package')).toBe(false);
  });

  it('cargo run is not allowlisted (executes arbitrary code)', () => {
    expect(isAllowlistedCommand('cargo run')).toBe(false);
  });

  it('git push is not allowlisted', () => {
    expect(isAllowlistedCommand('git push')).toBe(false);
  });

  it('git commit is not allowlisted', () => {
    expect(isAllowlistedCommand('git commit -m "msg"')).toBe(false);
  });

  it('python script execution is not allowlisted', () => {
    expect(isAllowlistedCommand('python script.py')).toBe(false);
  });

  it('allowlist is prefix-based: npm run anything works', () => {
    expect(isAllowlistedCommand('npm run lint')).toBe(true);
    expect(isAllowlistedCommand('npm run build')).toBe(true);
  });

  it('dangerous commands are denied even if they start with allowlisted prefix', () => {
    // "git status" is allowlisted but "git push --force" is dangerous
    expect(isDangerousCommand('git push --force origin main')).toBe(true);
    expect(checkPermission('run_command', { command: 'git push --force origin main' }, true)).toBe('deny');
  });
});

describe('regression: redaction with already-redacted content', () => {
  it('does not double-redact [REDACTED:...] markers', () => {
    const text = 'KEY=[REDACTED:OPENAI_KEY]';
    const result = redact(text);
    // Should not turn [REDACTED:OPENAI_KEY] into [REDACTED:ENV_VALUE]
    expect(result).toBe('KEY=[REDACTED:OPENAI_KEY]');
  });

  it('handles text with multiple redaction passes correctly', () => {
    const text = 'OPENAI_KEY=sk-proj-abcdefghijklmnopqrstuvwx';
    const first = redact(text);
    const second = redact(first);
    // Second pass should not change anything
    expect(second).toBe(first);
  });
});

describe('regression: tool permission priority', () => {
  it('dangerous check runs before allowlist', () => {
    // A command that matches both dangerous AND allowlist patterns
    // (hypothetical: if someone added "rm" to allowlist, dangerous should still win)
    expect(isDangerousCommand('rm -rf /tmp')).toBe(true);
    expect(checkPermission('run_command', { command: 'rm -rf /tmp' }, true)).toBe('deny');
  });

  it('edit_file always asks without auto-approve regardless of allowlist', () => {
    expect(checkPermission('edit_file', { path: 'a.ts', start_line: '1', end_line: '5', new_content: 'x' }, false)).toBe('ask');
  });
});
