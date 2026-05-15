/**
 * Token and cost accounting per conversation.
 *
 * Tracks estimated token usage and cost for each AI interaction.
 * Uses character-based estimation (1 token ≈ 4 chars) since we don't
 * have access to the actual tokenizer. Costs are based on published
 * per-token pricing for common models.
 */
import { writable, get } from 'svelte/store';

// ── Types ──

export interface TokenUsage {
  inputTokens: number;
  outputTokens: number;
  estimatedCost: number; // USD
}

export interface ConversationCost {
  conversationId: string;
  totalInput: number;
  totalOutput: number;
  totalCost: number;
  interactions: number;
}

// ── Store ──

export const currentSessionCost = writable<ConversationCost>({
  conversationId: '',
  totalInput: 0,
  totalOutput: 0,
  totalCost: 0,
  interactions: 0,
});

// ── Token estimation ──

const CHARS_PER_TOKEN = 4;

/**
 * Estimate token count from character count.
 */
export function estimateTokens(text: string): number {
  return Math.ceil(text.length / CHARS_PER_TOKEN);
}

// ── Pricing (USD per 1M tokens) ──

interface ModelPricing {
  input: number;  // per 1M input tokens
  output: number; // per 1M output tokens
}

const MODEL_PRICING: Record<string, ModelPricing> = {
  'gpt-4o': { input: 2.5, output: 10 },
  'gpt-4o-mini': { input: 0.15, output: 0.6 },
  'gpt-5': { input: 5, output: 15 },
  'gpt-5-mini': { input: 0.3, output: 1.2 },
  'o3': { input: 10, output: 40 },
  'o4-mini': { input: 1, output: 4 },
  'claude-sonnet-4-6': { input: 3, output: 15 },
  'claude-opus-4-7': { input: 15, output: 75 },
  'claude-haiku-4-5': { input: 0.8, output: 4 },
  'openrouter/auto': { input: 2, output: 8 },
  // Local models are free
  'llama3': { input: 0, output: 0 },
  'codellama': { input: 0, output: 0 },
  'mistral': { input: 0, output: 0 },
};

const DEFAULT_PRICING: ModelPricing = { input: 2, output: 8 };

/**
 * Get pricing for a model. Falls back to default if unknown.
 */
export function getModelPricing(model: string): ModelPricing {
  // Try exact match first
  if (MODEL_PRICING[model]) return MODEL_PRICING[model];
  // Try matching the model name portion (e.g. "anthropic/claude-sonnet-4-6" → "claude-sonnet-4-6")
  const shortName = model.split('/').pop() || model;
  if (MODEL_PRICING[shortName]) return MODEL_PRICING[shortName];
  // Local models are free
  if (model.startsWith('llama') || model.startsWith('codellama') || model.startsWith('mistral') || model.startsWith('deepseek-coder') || model.startsWith('qwen')) {
    return { input: 0, output: 0 };
  }
  return DEFAULT_PRICING;
}

/**
 * Calculate cost for a given token usage and model.
 */
export function calculateCost(inputTokens: number, outputTokens: number, model: string): number {
  const pricing = getModelPricing(model);
  return (inputTokens * pricing.input + outputTokens * pricing.output) / 1_000_000;
}

/**
 * Record a new interaction's token usage.
 */
export function recordUsage(inputText: string, outputText: string, model: string): TokenUsage {
  const inputTokens = estimateTokens(inputText);
  const outputTokens = estimateTokens(outputText);
  const cost = calculateCost(inputTokens, outputTokens, model);

  currentSessionCost.update(s => ({
    ...s,
    totalInput: s.totalInput + inputTokens,
    totalOutput: s.totalOutput + outputTokens,
    totalCost: s.totalCost + cost,
    interactions: s.interactions + 1,
  }));

  return { inputTokens, outputTokens, estimatedCost: cost };
}

/**
 * Reset the session cost tracker (e.g. when starting a new conversation).
 */
export function resetSessionCost(conversationId: string) {
  currentSessionCost.set({
    conversationId,
    totalInput: 0,
    totalOutput: 0,
    totalCost: 0,
    interactions: 0,
  });
}

/**
 * Format cost for display.
 */
export function formatCost(cost: number): string {
  if (cost === 0) return 'Free';
  if (cost < 0.001) return '<$0.001';
  if (cost < 0.01) return `$${cost.toFixed(4)}`;
  return `$${cost.toFixed(3)}`;
}

/**
 * Format token count for display.
 */
export function formatTokens(tokens: number): string {
  if (tokens < 1000) return `${tokens}`;
  if (tokens < 1_000_000) return `${(tokens / 1000).toFixed(1)}k`;
  return `${(tokens / 1_000_000).toFixed(2)}M`;
}
