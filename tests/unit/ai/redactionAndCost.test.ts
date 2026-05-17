import { describe, it, expect } from 'vitest';
import { redact, containsSecrets } from '$lib/modules/ai/redaction';
import { estimateTokens, calculateCost, formatCost, formatTokens, getModelPricing } from '$lib/modules/ai/costTracker';

describe('redaction', () => {
  describe('redact', () => {
    it('redacts AWS access keys', () => {
      const text = 'key = AKIAIOSFODNN7EXAMPLE';
      expect(redact(text)).toContain('[REDACTED:AWS_KEY]');
      expect(redact(text)).not.toContain('AKIAIOSFODNN7EXAMPLE');
    });

    it('redacts GitHub tokens', () => {
      expect(redact('token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij')).toContain('[REDACTED:GITHUB_TOKEN]');
    });

    it('redacts OpenAI keys', () => {
      expect(redact('OPENAI_KEY=sk-proj-abcdefghijklmnopqrstuvwx')).toContain('[REDACTED:OPENAI_KEY]');
    });

    it('redacts Anthropic keys', () => {
      expect(redact('key: sk-ant-api03-abcdefghijklmnopqrstuvwxyz')).toContain('[REDACTED:ANTHROPIC_KEY]');
    });

    it('redacts Slack tokens', () => {
      expect(redact('SLACK_TOKEN=xoxb-1234567890-abcdefghij')).toContain('[REDACTED:SLACK_TOKEN]');
    });

    it('redacts private key blocks', () => {
      const pem = '-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA...\n-----END RSA PRIVATE KEY-----';
      expect(redact(pem)).toContain('[REDACTED:PRIVATE_KEY]');
      expect(redact(pem)).not.toContain('MIIEpAIBAAKCAQEA');
    });

    it('preserves non-secret text', () => {
      const safe = 'function hello() { return "world"; }';
      expect(redact(safe)).toBe(safe);
    });

    it('handles multiple secrets in one text', () => {
      const text = 'AWS=AKIAIOSFODNN7EXAMPLE\nGH=ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij';
      const result = redact(text);
      expect(result).toContain('[REDACTED:AWS_KEY]');
      expect(result).toContain('[REDACTED:GITHUB_TOKEN]');
    });

    it('handles empty string', () => {
      expect(redact('')).toBe('');
    });
  });

  describe('containsSecrets', () => {
    it('returns true for text with secrets', () => {
      expect(containsSecrets('key = AKIAIOSFODNN7EXAMPLE')).toBe(true);
    });

    it('returns false for safe text', () => {
      expect(containsSecrets('const x = 1;')).toBe(false);
    });

    it('returns false for empty string', () => {
      expect(containsSecrets('')).toBe(false);
    });
  });
});

describe('costTracker', () => {
  describe('estimateTokens', () => {
    it('estimates ~1 token per 4 chars', () => {
      expect(estimateTokens('hello world')).toBe(3); // 11 chars / 4 = 2.75 → 3
    });

    it('returns 0 for empty string', () => {
      expect(estimateTokens('')).toBe(0);
    });

    it('handles long text', () => {
      const text = 'a'.repeat(4000);
      expect(estimateTokens(text)).toBe(1000);
    });
  });

  describe('getModelPricing', () => {
    it('returns pricing for known models', () => {
      const pricing = getModelPricing('gpt-4o');
      expect(pricing.input).toBe(2.5);
      expect(pricing.output).toBe(10);
    });

    it('handles provider-prefixed model names', () => {
      const pricing = getModelPricing('anthropic/claude-sonnet-4-6');
      expect(pricing.input).toBe(3);
    });

    it('returns zero for local models', () => {
      expect(getModelPricing('llama3').input).toBe(0);
      expect(getModelPricing('llama3').output).toBe(0);
    });

    it('returns default pricing for unknown models', () => {
      const pricing = getModelPricing('unknown-model-xyz');
      expect(pricing.input).toBeGreaterThan(0);
    });
  });

  describe('calculateCost', () => {
    it('calculates cost correctly', () => {
      // 1000 input tokens at $2.5/1M + 500 output tokens at $10/1M
      const cost = calculateCost(1000, 500, 'gpt-4o');
      expect(cost).toBeCloseTo(0.0025 + 0.005, 6);
    });

    it('returns 0 for local models', () => {
      expect(calculateCost(10000, 5000, 'llama3')).toBe(0);
    });
  });

  describe('formatCost', () => {
    it('formats zero as Free', () => {
      expect(formatCost(0)).toBe('Free');
    });

    it('formats tiny costs', () => {
      expect(formatCost(0.0001)).toBe('<$0.001');
    });

    it('formats small costs with 4 decimals', () => {
      expect(formatCost(0.0052)).toBe('$0.0052');
    });

    it('formats normal costs with 3 decimals', () => {
      expect(formatCost(0.125)).toBe('$0.125');
    });
  });

  describe('formatTokens', () => {
    it('formats small numbers as-is', () => {
      expect(formatTokens(500)).toBe('500');
    });

    it('formats thousands with k suffix', () => {
      expect(formatTokens(2500)).toBe('2.5k');
    });

    it('formats millions with M suffix', () => {
      expect(formatTokens(1500000)).toBe('1.50M');
    });
  });
});
