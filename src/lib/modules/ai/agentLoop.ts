/**
 * Agent loop — streaming, native tool-calling.
 *
 * Replaces the old regex-based agent with proper JSON tool_calls.
 * The model streams its thinking into chat in real-time, and tool
 * calls are dispatched as they arrive.
 */
import { get, writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { chatMessages, aiProvider, aiModel, isStreaming, type ChatMessage, scheduleSaveConversation } from './ai';
import { projectRoot } from '../git/git';
import { buildProjectContext } from './contextBuilder';
import { TOOL_SCHEMAS, dispatchTool, parseToolArgs, type ToolCall, type ToolResult } from './tools';
import { checkPermission, getBlockReason, type PermissionLevel } from './toolPermissions';
import { addEdits } from './pendingEdits';
import { recordAiChange } from './aiHistory';
import { log } from '../logging';

// ── Agent state ──

export const agentRunning = writable(false);
export const agentStep = writable(0);
export const agentMaxSteps = writable(10);
export const agentAutoApprove = writable(false);

let cancelRequested = false;
let currentSessionId: string | null = null;

// ── Public API ──

export async function runAgent(userRequest: string): Promise<void> {
  cancelRequested = false;
  agentRunning.set(true);
  agentStep.set(0);

  const maxSteps = get(agentMaxSteps);
  const root = get(projectRoot) || '';

  // Add user message
  chatMessages.update(msgs => [...msgs, { role: 'user', content: userRequest }]);

  // Build initial context
  const projectContext = await buildProjectContext(userRequest).catch(() => '');
  const systemContent = buildSystemPrompt(projectContext);

  for (let step = 0; step < maxSteps; step++) {
    if (cancelRequested) break;
    agentStep.set(step + 1);

    const allMessages = get(chatMessages);
    const messages = [
      { role: 'system', content: systemContent },
      ...allMessages.map(m => ({ role: m.role, content: m.content })),
    ];

    // Stream the agent's response
    const result = await streamAgentTurn(messages, root);

    if (cancelRequested) break;

    // If no tool calls, agent is done
    if (!result.toolCalls || result.toolCalls.length === 0) break;

    // Process tool calls
    let blocked = false;
    for (const toolCall of result.toolCalls) {
      if (cancelRequested) break;

      const args = parseToolArgs(toolCall.function.arguments);
      const permission = checkPermission(
        toolCall.function.name as any,
        args,
        get(agentAutoApprove),
      );

      if (permission === 'deny') {
        const reason = getBlockReason(toolCall.function.name as any, args);
        chatMessages.update(msgs => [...msgs, {
          role: 'user' as const,
          content: `[🚫 ${reason}]`,
        }]);
        blocked = true;
        continue;
      }

      if (permission === 'ask') {
        // For edit_file, use the pending-edit flow
        if (toolCall.function.name === 'edit_file') {
          const toolResult = await dispatchTool(toolCall, {
            projectRoot: root,
            onEdit: (path, startLine, endLine, newContent) => {
              addEdits([{
                id: `agent-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
                filePath: path,
                startLine,
                endLine,
                originalCode: '',
                newCode: newContent,
                status: 'pending',
              }]);
            },
          });
          chatMessages.update(msgs => [...msgs, {
            role: 'user' as const,
            content: `[Tool: ${toolCall.function.name}] ${toolResult.content}`,
          }]);
          blocked = true; // Pause for approval
          continue;
        }

        // For run_command without auto-approve, block
        chatMessages.update(msgs => [...msgs, {
          role: 'user' as const,
          content: `[⏸ Requires approval: ${toolCall.function.name}(${JSON.stringify(args).slice(0, 100)})]`,
        }]);
        blocked = true;
        continue;
      }

      // Permission is 'allow' — execute
      const toolResult = await dispatchTool(toolCall, {
        projectRoot: root,
        onEdit: (path, startLine, endLine, newContent) => {
          // Auto-approve mode: apply directly
          recordAiChange(path, `Edit lines ${startLine}-${endLine}`, '', newContent);
        },
      });

      chatMessages.update(msgs => [...msgs, {
        role: 'user' as const,
        content: `[Tool: ${toolCall.function.name}] ${toolResult.content}`,
      }]);
    }

    if (blocked) break; // Pause agent for user action
  }

  agentRunning.set(false);
  agentStep.set(0);
  currentSessionId = null;
  scheduleSaveConversation();
}

export function stopAgent() {
  cancelRequested = true;
  if (currentSessionId) {
    invoke('ai_chat_cancel', { sessionId: currentSessionId }).catch(() => {});
  }
  agentRunning.set(false);
}

// ── Streaming turn ──

interface AgentTurnResult {
  content: string;
  toolCalls: ToolCall[] | null;
}

async function streamAgentTurn(
  messages: { role: string; content: string }[],
  projectRoot: string,
): Promise<AgentTurnResult> {
  const sessionId = `agent-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  currentSessionId = sessionId;

  return new Promise<AgentTurnResult>(async (resolve) => {
    let content = '';
    let toolCalls: ToolCall[] = [];
    let unlisten: (() => void) | null = null;

    // Add empty assistant message to stream into
    chatMessages.update(msgs => [...msgs, { role: 'assistant', content: '' }]);

    unlisten = await listen<{
      session_id: string;
      delta: string;
      done: boolean;
      tool_calls?: ToolCall[];
    }>('ai-stream-chunk', (event) => {
      if (event.payload.session_id !== sessionId) return;

      if (event.payload.done) {
        if (unlisten) { unlisten(); unlisten = null; }
        resolve({ content, toolCalls: toolCalls.length > 0 ? toolCalls : null });
        return;
      }

      // Accumulate tool calls if present
      if (event.payload.tool_calls) {
        for (const tc of event.payload.tool_calls) {
          const existing = toolCalls.find(t => t.id === tc.id);
          if (existing) {
            // Append to arguments (streamed incrementally)
            existing.function.arguments += tc.function.arguments;
          } else {
            toolCalls.push({ ...tc });
          }
        }
      }

      // Accumulate text content
      if (event.payload.delta) {
        content += event.payload.delta;
        chatMessages.update(msgs => {
          const last = msgs[msgs.length - 1];
          if (last && last.role === 'assistant') {
            return [...msgs.slice(0, -1), { ...last, content }];
          }
          return msgs;
        });
      }
    }) as unknown as () => void;

    // Start streaming with tools
    try {
      await invoke('ai_chat_stream', {
        request: {
          messages,
          model: get(aiModel),
          provider: get(aiProvider),
          session_id: sessionId,
          tools: TOOL_SCHEMAS,
        },
      });
    } catch (e) {
      if (unlisten) { unlisten(); unlisten = null; }
      chatMessages.update(msgs => {
        const last = msgs[msgs.length - 1];
        if (last && last.role === 'assistant' && last.content === '') {
          return [...msgs.slice(0, -1), { ...last, content: `Agent error: ${e}` }];
        }
        return msgs;
      });
      resolve({ content: '', toolCalls: null });
    }
  });
}

// ── System prompt ──

function buildSystemPrompt(projectContext: string): string {
  return `You are an AI coding agent embedded in leo-IDE. You help the user by reading files, making edits, running commands, and searching the codebase.

Use the provided tools to accomplish the user's request. Work step by step:
1. Read relevant files to understand the codebase
2. Make targeted edits to implement the requested changes
3. Verify your changes if possible (run tests, type-check)

Guidelines:
- Make minimal, focused changes
- Preserve existing code style
- Explain your reasoning briefly before acting
- If unsure, read more context before editing
${projectContext}`;
}
