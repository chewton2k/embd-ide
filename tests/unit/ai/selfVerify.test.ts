import { describe, it, expect } from 'vitest';
import { getCheckCommand, formatVerifyErrors, type VerifyResult } from '$lib/modules/ai/selfVerify';

describe('selfVerify', () => {
  describe('getCheckCommand', () => {
    it('returns tsc for typescript projects', () => {
      expect(getCheckCommand('typescript')).toBe('npx tsc --noEmit');
    });

    it('returns cargo check for rust projects', () => {
      expect(getCheckCommand('rust')).toBe('cargo check 2>&1');
    });

    it('returns npm run check for node projects', () => {
      const cmd = getCheckCommand('node');
      expect(cmd).toContain('npm run check');
    });

    it('returns go build for go projects', () => {
      expect(getCheckCommand('go')).toBe('go build ./... 2>&1');
    });

    it('returns python compile for python projects', () => {
      expect(getCheckCommand('python')).toBe('python -m py_compile');
    });

    it('returns null for unknown project types', () => {
      expect(getCheckCommand('unknown')).toBeNull();
      expect(getCheckCommand('')).toBeNull();
    });
  });

  describe('formatVerifyErrors', () => {
    it('returns success message when no errors', () => {
      const result: VerifyResult = { success: true, errors: '', command: 'tsc --noEmit' };
      expect(formatVerifyErrors(result)).toBe('✓ No errors found.');
    });

    it('formats errors with command name and code block', () => {
      const result: VerifyResult = {
        success: false,
        errors: 'src/main.ts(5,3): error TS2322: Type string is not assignable',
        command: 'npx tsc --noEmit',
      };
      const formatted = formatVerifyErrors(result);
      expect(formatted).toContain('⚠ Verification failed');
      expect(formatted).toContain('npx tsc --noEmit');
      expect(formatted).toContain('```');
      expect(formatted).toContain('TS2322');
      expect(formatted).toContain('Please fix these errors');
    });

    it('includes the full error output in a code block', () => {
      const result: VerifyResult = {
        success: false,
        errors: 'error[E0308]: mismatched types\n  --> src/main.rs:5:5',
        command: 'cargo check',
      };
      const formatted = formatVerifyErrors(result);
      expect(formatted).toContain('error[E0308]');
      expect(formatted).toContain('src/main.rs:5:5');
    });
  });
});
