/**
 * Cmd+K inline edit orchestration.
 *
 * Flow: user selects code → presses Cmd+K → types instruction →
 * streams AI response → shows inline diff → accept/reject.
 *
 * Reuses: ai_chat_stream backend, pendingEdits store, aiDiffExtension.
 */
import { get, writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { aiProvider, aiModel } from './ai';
import { editModel } from '../settings/settings';
import { activeFilePath } from '../explorer/files';

// ── Types ──

export interface InlineEditRequest {
  /** The selected code to edit */
  selectedCode: string;
  /** User's instruction for the edit */
  instruction: string;
  /** Full file path */
  filePath: string;
  /** 1-indexed start line of selection */
  startLine: number;
  /** 1-indexed end line of selection */
  endLine: number;
}

export interface InlineEditResult {
  /** The new code to replace the selection */
  newCode: string;
  /** Whether the stream completed successfully */
  success: boolean;
}

// ── State ──

export const inlineEditStreaming = writable(false);
export const inlineEditResponse = writable('');

let currentSessionId: string | null = null;
let streamUnlisten: (() => void) | null = null;

// ── Prompt construction ──

export function buildInlineEditPrompt(selectedCode: string, instruction: string, filePath: string): string {
  const name = filePath.split('/').pop() || '';
  const ext = name.includes('.') ? name.split('.').pop() || '' : '';
  return `You are editing code inline. The user selected the following code from \`${filePath}\`:

\`\`\`${ext}
${selectedCode}
\`\`\`

Instruction: ${instruction}

Respond with ONLY the replacement code. No explanations, no markdown fences, no extra text. Just the code that should replace the selection.`;
}

// ── Streaming ──

export async function startInlineEdit(request: InlineEditRequest): Promise<InlineEditResult> {
  if (get(inlineEditStreaming)) {
    cancelInlineEdit();
  }

  const sessionId = `inline-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  currentSessionId = sessionId;
  inlineEditStreaming.set(true);
  inlineEditResponse.set('');

  const prompt = buildInlineEditPrompt(request.selectedCode, request.instruction, request.filePath);

  // Set up listener BEFORE invoking to avoid race
  let resolvePromise: (result: InlineEditResult) => void;
  const resultPromise = new Promise<InlineEditResult>((resolve) => { resolvePromise = resolve; });

  if (streamUnlisten) { streamUnlisten(); streamUnlisten = null; }

  streamUnlisten = await listen<{ session_id: string; delta: string; done: boolean }>(
    'ai-stream-chunk',
    (event) => {
      if (event.payload.session_id !== sessionId) return;

      if (event.payload.done) {
        inlineEditStreaming.set(false);
        currentSessionId = null;
        if (streamUnlisten) { streamUnlisten(); streamUnlisten = null; }
        const finalCode = cleanResponse(get(inlineEditResponse));
        resolvePromise({ newCode: finalCode, success: true });
        return;
      }

      inlineEditResponse.update(r => r + event.payload.delta);
    }
  ) as unknown as () => void;

  // Start streaming — if this throws, clean up and resolve with failure
  try {
    await invoke('ai_chat_stream', {
      request: {
        messages: [
          { role: 'system', content: 'You are a code editor. Output only code, no explanations.' },
          { role: 'user', content: prompt },
        ],
        model: get(editModel) || get(aiModel),
        provider: get(aiProvider),
        session_id: sessionId,
      },
    });
  } catch (e) {
    inlineEditStreaming.set(false);
    currentSessionId = null;
    if (streamUnlisten) { streamUnlisten(); streamUnlisten = null; }
    return { newCode: '', success: false };
  }

  return resultPromise;
}

export function cancelInlineEdit() {
  if (currentSessionId) {
    invoke('ai_chat_cancel', { sessionId: currentSessionId }).catch(() => {});
  }
  inlineEditStreaming.set(false);
  inlineEditResponse.set('');
  currentSessionId = null;
  if (streamUnlisten) { streamUnlisten(); streamUnlisten = null; }
}

// ── Response cleaning ──

/** Strip markdown fences if the model wraps its response in them. */
export function cleanResponse(raw: string): string {
  let code = raw.trim();
  // Remove leading ```lang and trailing ```
  const fenceStart = /^```[\w]*\n?/;
  const fenceEnd = /\n?```\s*$/;
  if (fenceStart.test(code) && fenceEnd.test(code)) {
    code = code.replace(fenceStart, '').replace(fenceEnd, '');
  }
  return code;
}
