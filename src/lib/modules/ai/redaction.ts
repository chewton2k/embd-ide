/**
 * Pattern-based secret redaction.
 *
 * Scrubs sensitive values (API keys, tokens, passwords, connection strings)
 * from text before it's sent to AI providers. Applied to file contents
 * attached as context and to tool call results.
 */

// ── Types ──

export interface RedactionRule {
  name: string;
  pattern: RegExp;
  replacement: string;
}

// ── Default rules ──

export const DEFAULT_REDACTION_RULES: RedactionRule[] = [
  { name: 'AWS Access Key', pattern: /AKIA[0-9A-Z]{16}/g, replacement: '[REDACTED:AWS_KEY]' },
  { name: 'AWS Secret Key', pattern: /(?<=aws_secret_access_key\s*=\s*)[A-Za-z0-9/+=]{40}/g, replacement: '[REDACTED:AWS_SECRET]' },
  { name: 'Generic API Key', pattern: /(?<=(?:api[_-]?key|apikey|secret[_-]?key|access[_-]?token)\s*[:=]\s*['"]?)[A-Za-z0-9_\-]{20,}/gi, replacement: '[REDACTED:API_KEY]' },
  { name: 'Bearer Token', pattern: /(?<=Bearer\s+)[A-Za-z0-9_\-.]{20,}/g, replacement: '[REDACTED:TOKEN]' },
  { name: 'GitHub Token', pattern: /gh[ps]_[A-Za-z0-9_]{36,}/g, replacement: '[REDACTED:GITHUB_TOKEN]' },
  { name: 'Anthropic Key', pattern: /sk-ant-[A-Za-z0-9_\-]{20,}/g, replacement: '[REDACTED:ANTHROPIC_KEY]' },
  { name: 'OpenAI Key', pattern: /sk-[A-Za-z0-9_\-]{20,}/g, replacement: '[REDACTED:OPENAI_KEY]' },
  { name: 'Slack Token', pattern: /xox[baprs]-[A-Za-z0-9\-]{10,}/g, replacement: '[REDACTED:SLACK_TOKEN]' },
  { name: 'Private Key Block', pattern: /-----BEGIN (?:RSA |EC |DSA )?PRIVATE KEY-----[\s\S]*?-----END (?:RSA |EC |DSA )?PRIVATE KEY-----/g, replacement: '[REDACTED:PRIVATE_KEY]' },
  { name: 'Connection String Password', pattern: /(?<=:\/\/[^:]+:)[^@\s]{8,}(?=@)/g, replacement: '[REDACTED:PASSWORD]' },
  { name: 'Env File Values', pattern: /(?<=^[A-Z_]+=)['"]?(?!sk-|ghp_|ghs_|xox[baprs]-|AKIA|\[REDACTED)[^\s'"]{16,}['"]?/gm, replacement: '[REDACTED:ENV_VALUE]' },
];

// ── Redaction ──

/**
 * Apply redaction rules to text. Returns the scrubbed text.
 * Non-destructive: the original is never modified.
 */
export function redact(text: string, rules: RedactionRule[] = DEFAULT_REDACTION_RULES): string {
  let result = text;
  for (const rule of rules) {
    // Reset lastIndex for global regexes
    rule.pattern.lastIndex = 0;
    result = result.replace(rule.pattern, rule.replacement);
  }
  return result;
}

/**
 * Check if text contains any patterns that would be redacted.
 * Useful for showing a warning before sending.
 */
export function containsSecrets(text: string, rules: RedactionRule[] = DEFAULT_REDACTION_RULES): boolean {
  for (const rule of rules) {
    rule.pattern.lastIndex = 0;
    if (rule.pattern.test(text)) return true;
  }
  return false;
}
