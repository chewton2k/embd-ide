import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { persistedString } from '../session/persisted';
import { parseAiEdits, hasEdits } from './editParser';
import { EDIT_SYSTEM_PROMPT } from './systemPrompts';
import { buildProjectContext } from './contextBuilder';
import { addEdits } from './pendingEdits';
import { activeFilePath, getFileContent } from '../explorer/files';
import { projectRoot } from '../git/git';
import { isTerminalPath, isPreviewPath, isDiagramPath } from '../terminal/shell';

export interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export const chatMessages = writable<ChatMessage[]>([]);
export const isStreaming = writable<boolean>(false);
export const attachedFiles = writable<{ path: string; name: string }[]>([]);

export type AiProvider = 'openrouter' | 'openai' | 'anthropic';

export const apiKey = writable<string>('');
export const openaiApiKey = writable<string>('');
export const anthropicApiKey = writable<string>('');
export const aiProvider = persistedString('leo-ai-provider', 'openrouter') as import('svelte/store').Writable<AiProvider>;
export const aiModel = persistedString('leo-ai-model', 'openrouter/auto');

let currentSessionId: string | null = null;
let streamUnlisten: (() => void) | null = null;

const SYSTEM_PROMPT = `You are an AI coding assistant embedded in a lightweight IDE called leo. Help the user with their code: explain, debug, refactor, or write new code. Keep responses concise and code-focused.\n\n${EDIT_SYSTEM_PROMPT}`;

function generateSessionId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

/** Size cap for auto-attached active file content. Keeps context cost
 *  predictable; huge files aren't inlined and should be attached manually. */
const AUTO_ATTACH_MAX_BYTES = 120_000;

/**
 * Load the content of the user's current file for inclusion as chat context.
 * Returns `null` when there's no active file, when it's a virtual path
 * (terminal/preview/diagram), when it's not inside the project root, or
 * when reading fails. Content exceeding the size cap is truncated with a
 * marker so the model knows it's partial.
 */
async function readActiveFileContext(): Promise<{ path: string; content: string } | null> {
  const activeFile = get(activeFilePath);
  if (!activeFile) return null;
  if (isTerminalPath(activeFile) || isPreviewPath(activeFile) || isDiagramPath(activeFile)) {
    return null;
  }
  const root = get(projectRoot);
  // Only auto-attach files inside the opened project. Loose files opened
  // ad-hoc don't get silently inlined.
  if (!root || !activeFile.startsWith(root)) return null;

  try {
    // Prefer the in-memory editor cache so unsaved edits are sent to the
    // model. Fall back to disk for files that haven't been touched yet.
    const live = getFileContent(activeFile);
    const raw = live ?? await invoke<string>('read_file_content', { path: activeFile });
    const name = activeFile.split('/').pop() || activeFile;
    if (raw.length > AUTO_ATTACH_MAX_BYTES) {
      const truncated = raw.slice(0, AUTO_ATTACH_MAX_BYTES);
      return {
        path: name,
        content: `${truncated}\n\n… (${raw.length - AUTO_ATTACH_MAX_BYTES} more bytes truncated — attach the file manually to include the full contents)`,
      };
    }
    return { path: name, content: raw };
  } catch {
    return null;
  }
}

export async function sendStreamingMessage(userContent: string, fileContexts?: { path: string; content: string }[]) {
  if (get(isStreaming)) return;

  // Auto-attach the active file's content if the user hasn't manually
  // attached anything. This matches the "my chat should know what I'm
  // looking at" expectation without double-sending when they explicitly
  // added files via the paperclip button.
  let effectiveContexts = fileContexts;
  if (!effectiveContexts || effectiveContexts.length === 0) {
    const auto = await readActiveFileContext();
    if (auto) effectiveContexts = [auto];
  }

  // Build context from attached files
  let contextPrefix = '';
  if (effectiveContexts && effectiveContexts.length > 0) {
    contextPrefix = effectiveContexts
      .map(f => `File: ${f.path}\n\`\`\`\n${f.content}\n\`\`\``)
      .join('\n\n') + '\n\n';
  }

  const userMsg: ChatMessage = { role: 'user', content: userContent };
  chatMessages.update(msgs => [...msgs, userMsg]);

  // Build messages array for the API
  const allMessages = get(chatMessages);
  const projectContext = await buildProjectContext(userContent).catch(() => '');
  const activeFile = get(activeFilePath) || '';
  const root = get(projectRoot) || '';
  const activeFileHint = activeFile ? `\nThe user currently has this file open: ${activeFile}\nWhen editing this file, use the FULL path: ${activeFile}\n` : '';
  const systemContent = SYSTEM_PROMPT + activeFileHint + projectContext;
  const messages = [
    { role: 'system', content: systemContent },
    ...allMessages.map(m => ({
      role: m.role,
      content: m === userMsg && contextPrefix ? contextPrefix + m.content : m.content,
    })),
  ];

  // Token budget: rough estimate, truncate oldest if too long
  const totalChars = messages.reduce((sum, m) => sum + m.content.length, 0);
  if (totalChars > 400000) { // ~100k tokens
    // Keep system + last 10 messages
    const kept = [messages[0], ...messages.slice(-10)];
    messages.length = 0;
    messages.push(...kept);
  }

  const sessionId = generateSessionId();
  currentSessionId = sessionId;
  isStreaming.set(true);

  // Add empty assistant message that we'll stream into
  chatMessages.update(msgs => [...msgs, { role: 'assistant', content: '' }]);

  // Listen for stream chunks
  if (streamUnlisten) { streamUnlisten(); streamUnlisten = null; }
  streamUnlisten = await listen<{ session_id: string; delta: string; done: boolean }>('ai-stream-chunk', (event) => {
    if (event.payload.session_id !== sessionId) return;

    if (event.payload.done) {
      isStreaming.set(false);
      currentSessionId = null;
      if (streamUnlisten) { streamUnlisten(); streamUnlisten = null; }

      // Check if the completed message contains edit proposals
      const msgs = get(chatMessages);
      const lastMsg = msgs[msgs.length - 1];
      if (lastMsg && lastMsg.role === 'assistant' && hasEdits(lastMsg.content)) {
        const { edits, displayText } = parseAiEdits(lastMsg.content);
        if (edits.length > 0) {
          // Replace message content with display text (edit blocks replaced with summary)
          chatMessages.update(m => [...m.slice(0, -1), { ...lastMsg, content: displayText }]);
          addEdits(edits);
        }
      }

      // Auto-save conversation
      scheduleSaveConversation();
      return;
    }

    // Append delta to the last assistant message
    chatMessages.update(msgs => {
      const last = msgs[msgs.length - 1];
      if (last && last.role === 'assistant') {
        return [...msgs.slice(0, -1), { ...last, content: last.content + event.payload.delta }];
      }
      return msgs;
    });
  }) as unknown as () => void;

  // Start streaming
  try {
    await invoke('ai_chat_stream', {
      request: {
        messages,
        model: get(aiModel),
        provider: get(aiProvider),
        session_id: sessionId,
      },
    });
  } catch (e) {
    chatMessages.update(msgs => {
      const last = msgs[msgs.length - 1];
      if (last && last.role === 'assistant' && last.content === '') {
        return [...msgs.slice(0, -1), { ...last, content: `Error: ${e}` }];
      }
      return msgs;
    });
    isStreaming.set(false);
    currentSessionId = null;
  }
}

export async function cancelStream() {
  if (currentSessionId) {
    try {
      await invoke('ai_chat_cancel', { sessionId: currentSessionId });
    } catch { /* ignore */ }
  }
}

export function clearChat() {
  chatMessages.set([]);
  attachedFiles.set([]);
  currentConversationId = generateSessionId();
}

// ── Conversation persistence ──

let currentConversationId = generateSessionId();
let saveTimeout: ReturnType<typeof setTimeout> | null = null;

export const conversationId = writable<string>(currentConversationId);

/** Auto-save conversation after messages change (debounced). */
export function scheduleSaveConversation() {
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(() => saveConversationNow(), 2000);
}

/** Immediately save the current conversation to SQLite. */
export async function saveConversationNow(): Promise<void> {
  if (saveTimeout) { clearTimeout(saveTimeout); saveTimeout = null; }
  const msgs = get(chatMessages);
  if (msgs.length < 2) return;
  const root = get(projectRoot);
  if (!root) return;
  const title = msgs.find(m => m.role === 'user')?.content.slice(0, 60) || 'Untitled';
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      await invoke('knowledge_save_conversation', {
        projectRoot: root,
        id: currentConversationId,
        title,
        messages: JSON.stringify(msgs),
      });
      return;
    } catch (e) {
      if (attempt < 2) {
        await new Promise(r => setTimeout(r, 500 * (attempt + 1)));
      } else {
        console.warn('[leo] Failed to save conversation after 3 attempts:', e);
      }
    }
  }
}

export async function loadConversation(id: string): Promise<void> {
  const root = get(projectRoot);
  if (!root) return;
  try {
    const json = await invoke<string>('knowledge_load_conversation', { projectRoot: root, id });
    const msgs = JSON.parse(json) as ChatMessage[];
    chatMessages.set(msgs);
    currentConversationId = id;
    conversationId.set(id);
  } catch { /* ignore */ }
}

export async function listConversations(): Promise<{ id: string; title: string; created_at: number; updated_at: number }[]> {
  const root = get(projectRoot);
  if (!root) return [];
  try {
    return await invoke('knowledge_list_conversations', { projectRoot: root });
  } catch { return []; }
}

export async function deleteAllConversations(): Promise<void> {
  const root = get(projectRoot);
  if (!root) return;
  try {
    await invoke('knowledge_delete_conversations', { projectRoot: root });
  } catch { /* ignore */ }
}
