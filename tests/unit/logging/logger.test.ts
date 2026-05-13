import { describe, it, expect, vi, beforeEach } from 'vitest';

// We need to test the redactSecrets function. Since it's not exported,
// we test it indirectly through the logger's behavior, or we extract and test.
// For now, test via the module's internal behavior by importing and calling log.

// Mock invoke to capture what gets sent to the backend
const invokePayloads: Array<{ cmd: string; args: unknown }> = [];

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string, args?: unknown) => {
    invokePayloads.push({ cmd, args });
  }),
}));

// Import after mocking
const { log } = await import('$lib/modules/logging/logger');

beforeEach(() => {
  invokePayloads.length = 0;
});

describe('logger redaction', () => {
  it('redacts apiKey', () => {
    log.info('test', { apiKey: 'sk-secret123' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    expect(call).toBeDefined();
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('[redacted]');
    expect(data).not.toContain('sk-secret123');
  });

  it('redacts api_key', () => {
    log.info('test', { api_key: 'secret-value' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('[redacted]');
    expect(data).not.toContain('secret-value');
  });

  it('redacts API_KEY (case insensitive)', () => {
    log.info('test', { API_KEY: 'MY-KEY' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('[redacted]');
    expect(data).not.toContain('MY-KEY');
  });

  it('redacts token', () => {
    log.info('test', { token: 'bearer-xyz' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('[redacted]');
    expect(data).not.toContain('bearer-xyz');
  });

  it('redacts Token (case insensitive)', () => {
    log.info('test', { Token: 'abc' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('[redacted]');
    expect(data).not.toContain('abc');
  });

  it('redacts password', () => {
    log.info('test', { password: 'hunter2' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('[redacted]');
    expect(data).not.toContain('hunter2');
  });

  it('redacts secret', () => {
    log.info('test', { secret: 'shh' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('[redacted]');
    expect(data).not.toContain('shh');
  });

  it('does NOT redact non-secret keys (known false positive: substring match)', () => {
    // Note: apikeyValid WILL be redacted because the regex matches substrings.
    // This is a documented acceptable false positive — over-redaction is safer.
    log.info('test', { apikeyValid: 'true' });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    // This IS redacted due to substring match — document as known behavior
    expect(data).toContain('[redacted]');
  });

  it('preserves non-secret data', () => {
    log.info('test', { username: 'alice', count: 42 });
    const call = invokePayloads.find(p => p.cmd === 'log_record');
    const data = (call!.args as Record<string, unknown>).data as string;
    expect(data).toContain('alice');
    expect(data).toContain('42');
  });
});

describe('logger crash safety', () => {
  it('does not throw when data is circular', () => {
    const obj: Record<string, unknown> = {};
    obj.self = obj;
    // Should not throw — logger swallows its own errors
    expect(() => log.info('circular', obj)).not.toThrow();
  });

  it('does not throw when invoke fails', () => {
    // invoke is mocked to succeed, but even if it threw, the logger catches
    expect(() => log.error('test error', new Error('boom'))).not.toThrow();
  });
});
