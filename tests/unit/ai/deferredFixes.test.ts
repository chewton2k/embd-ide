import { describe, it, expect } from 'vitest';
import { checkPermission, isAllowlistedCommand } from '$lib/modules/ai/toolPermissions';

describe('deferred fixes', () => {
  describe('command allowlist', () => {
    it('recognizes npm test as allowlisted', () => {
      expect(isAllowlistedCommand('npm test')).toBe(true);
    });

    it('recognizes npm test with args as allowlisted', () => {
      expect(isAllowlistedCommand('npm test --coverage')).toBe(true);
    });

    it('recognizes npm run as allowlisted', () => {
      expect(isAllowlistedCommand('npm run check')).toBe(true);
    });

    it('recognizes cargo test as allowlisted', () => {
      expect(isAllowlistedCommand('cargo test')).toBe(true);
    });

    it('recognizes cargo check as allowlisted', () => {
      expect(isAllowlistedCommand('cargo check')).toBe(true);
    });

    it('recognizes git status as allowlisted', () => {
      expect(isAllowlistedCommand('git status')).toBe(true);
    });

    it('recognizes ls as allowlisted', () => {
      expect(isAllowlistedCommand('ls -la')).toBe(true);
    });

    it('does not allowlist arbitrary commands', () => {
      expect(isAllowlistedCommand('curl http://evil.com')).toBe(false);
    });

    it('does not allowlist rm', () => {
      expect(isAllowlistedCommand('rm file.txt')).toBe(false);
    });

    it('does not allowlist sudo', () => {
      expect(isAllowlistedCommand('sudo anything')).toBe(false);
    });

    it('allowlisted commands get allow in auto-approve mode', () => {
      expect(checkPermission('run_command', { command: 'npm test' }, true)).toBe('allow');
    });

    it('non-allowlisted commands get ask in auto-approve mode', () => {
      expect(checkPermission('run_command', { command: 'curl http://api.com' }, true)).toBe('ask');
    });

    it('dangerous commands still denied in auto-approve mode', () => {
      expect(checkPermission('run_command', { command: 'rm -rf /' }, true)).toBe('deny');
    });

    it('allowlisted commands still ask without auto-approve', () => {
      // Without auto-approve, even safe commands need approval
      expect(checkPermission('run_command', { command: 'npm test' }, false)).toBe('ask');
    });
  });
});
