import { get, writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { chatMessages, aiProvider, aiModel, isStreaming, type ChatMessage } from './ai';
import { projectRoot } from '../git/git';
import { parseAiEdits, hasEdits } from './editParser';
import { parseCommands, hasCommands } from './commandParser';
import { addEdits } from './pendingEdits';
import { buildProjectContext } from './contextBuilder';
import { EDIT_SYSTEM_PROMPT } from './systemPrompts';
import { terminalSessions } from '../terminal/shell';

// ── Agent state ──

export const agentRunning = writable(false);
export const agentStep = writable(0);
export const agentMaxSteps = writable(10);
export const agentAutoApprove = writable(false);

let cancelAgent = false;

// ── Tool definitions for the AI ──

const TOOLS_PROMPT = `
You have access to these tools. Use them by responding with the appropriate code block:

1. Read a file:
\`\`\`tool:read_file
path/to/file.ts
\`\`\`

2. Edit a file (use the edit format):
\`\`\`edit:path/to/file.ts:startLine-endLine
original code
---
new code
\`\`\`

3. Run a terminal command:
\`\`\`run
command here
\`\`\`

4. Search files by content:
\`\`\`tool:search
search query
\`\`\`

When you're done and have no more actions to take, respond normally without any tool blocks.
`;

// ── Agent loop ──

export async function runAgent(userRequest: string): Promise<void> {
  cancelAgent = false;
  agentRunning.set(true);
  agentStep.set(0);

  const maxSteps = get(agentMaxSteps);
  const root = get(projectRoot) || '';

  // Add user message
  chatMessages.update(msgs => [...msgs, { role: 'user', content: userRequest }]);

  for (let step = 0; step < maxSteps; step++) {
    if (cancelAgent) break;
    agentStep.set(step + 1);

    // Build context
    const projectContext = await buildProjectContext(userRequest).catch(() => '');
    const systemContent = `You are an AI coding agent. Complete the user's request step by step.\n\n${EDIT_SYSTEM_PROMPT}\n${TOOLS_PROMPT}\n${projectContext}`;

    const allMessages = get(chatMessages);
    const messages = [
      { role: 'system', content: systemContent },
      ...allMessages.map(m => ({ role: m.role, content: m.content })),
    ];

    // Call AI (blocking for agent loop)
    let response: string;
    try {
      response = await invoke<string>('ai_chat', {
        request: {
          prompt: allMessages[allMessages.length - 1]?.content || userRequest,
          context: null,
          model: get(aiModel),
          provider: get(aiProvider),
        },
      });
    } catch (e) {
      chatMessages.update(msgs => [...msgs, { role: 'assistant', content: `Agent error: ${e}` }]);
      break;
    }

    // Add assistant response
    chatMessages.update(msgs => [...msgs, { role: 'assistant', content: response }]);

    // Process tool calls
    let hasAction = false;

    // Handle read_file
    const readMatch = response.match(/```tool:read_file\n([\s\S]*?)```/);
    if (readMatch) {
      hasAction = true;
      const filePath = readMatch[1].trim();
      const fullPath = filePath.startsWith('/') ? filePath : `${root}/${filePath}`;
      try {
        const content = await invoke<string>('read_file_content', { path: fullPath });
        const truncated = content.length > 5000 ? content.slice(0, 5000) + '\n... (truncated)' : content;
        chatMessages.update(msgs => [...msgs, { role: 'user', content: `[File content of ${filePath}]:\n\`\`\`\n${truncated}\n\`\`\`` }]);
      } catch (e) {
        chatMessages.update(msgs => [...msgs, { role: 'user', content: `[Error reading ${filePath}: ${e}]` }]);
      }
    }

    // Handle search
    const searchMatch = response.match(/```tool:search\n([\s\S]*?)```/);
    if (searchMatch) {
      hasAction = true;
      const query = searchMatch[1].trim();
      try {
        const files = await invoke<string[]>('list_all_files', { path: root });
        const matches = files.filter(f => f.toLowerCase().includes(query.toLowerCase())).slice(0, 10);
        chatMessages.update(msgs => [...msgs, { role: 'user', content: `[Search results for "${query}"]:\n${matches.join('\n') || 'No matches found.'}` }]);
      } catch {
        chatMessages.update(msgs => [...msgs, { role: 'user', content: `[Search failed]` }]);
      }
    }

    // Handle edits
    if (hasEdits(response)) {
      hasAction = true;
      const { edits } = parseAiEdits(response);
      if (edits.length > 0) {
        if (get(agentAutoApprove)) {
          // Auto-apply edits
          for (const edit of edits) {
            const fullPath = edit.filePath.startsWith('/') ? edit.filePath : `${root}/${edit.filePath}`;
            try {
              const content = await invoke<string>('read_file_content', { path: fullPath });
              const lines = content.split('\n');
              const before = lines.slice(0, edit.startLine - 1);
              const after = lines.slice(edit.endLine);
              const newContent = [...before, ...edit.newCode.split('\n'), ...after].join('\n');
              await invoke('write_file_content', { path: fullPath, content: newContent });
              chatMessages.update(msgs => [...msgs, { role: 'user', content: `[Applied edit to ${edit.filePath}]` }]);
            } catch (e) {
              chatMessages.update(msgs => [...msgs, { role: 'user', content: `[Failed to apply edit to ${edit.filePath}: ${e}]` }]);
            }
          }
        } else {
          addEdits(edits);
          chatMessages.update(msgs => [...msgs, { role: 'user', content: `[${edits.length} edit(s) proposed — waiting for approval]` }]);
          break; // Pause agent until user approves
        }
      }
    }

    // Handle commands
    if (hasCommands(response)) {
      hasAction = true;
      const { commands } = parseCommands(response);
      for (const cmd of commands) {
        if (cmd.dangerous && !get(agentAutoApprove)) {
          chatMessages.update(msgs => [...msgs, { role: 'user', content: `[Dangerous command skipped: ${cmd.command}]` }]);
          continue;
        }
        // Execute in terminal
        const sessions = get(terminalSessions);
        if (sessions.length > 0) {
          const termId = sessions[0].id;
          await invoke('write_terminal', { id: termId, data: cmd.command + '\n' });
          // Wait briefly for output
          await new Promise(r => setTimeout(r, 2000));
          chatMessages.update(msgs => [...msgs, { role: 'user', content: `[Executed: ${cmd.command}]` }]);
        } else {
          chatMessages.update(msgs => [...msgs, { role: 'user', content: `[No terminal available to run: ${cmd.command}]` }]);
        }
      }
    }

    // If no tool calls were made, the agent is done
    if (!hasAction) break;
  }

  agentRunning.set(false);
  agentStep.set(0);
}

export function stopAgent() {
  cancelAgent = true;
  agentRunning.set(false);
}
