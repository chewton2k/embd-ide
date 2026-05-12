import { writable } from 'svelte/store';
import { persistedString } from '../persisted';

export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

export const chatMessages = writable<ChatMessage[]>([]);
export type AiProvider = 'openrouter' | 'openai' | 'anthropic';

export const apiKey = writable<string>('');
export const openaiApiKey = writable<string>('');
export const anthropicApiKey = writable<string>('');
export const aiProvider = persistedString('leo-ai-provider', 'openrouter') as import('svelte/store').Writable<AiProvider>;
export const aiModel = persistedString('leo-ai-model', 'openrouter/auto');
