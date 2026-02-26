<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { chatMessages, apiKey, activeFile, openFiles, type ChatMessage } from './stores.ts';
  import { get } from 'svelte/store';

  let input = $state('');
  let loading = $state(false);
  let messagesContainer: HTMLDivElement;
  let showKeyInput = $state(false);
  let keyInput = $state(get(apiKey));

  async function saveKey() {
    apiKey.set(keyInput);
    // Store key on backend so it's not sent on every request
    try { await invoke('set_api_key', { key: keyInput }); } catch {}
    showKeyInput = false;
  }

  async function sendMessage() {
    const key = get(apiKey);
    if (!key) {
      showKeyInput = true;
      return;
    }

    if (!input.trim() || loading) return;

    const userMsg: ChatMessage = { role: 'user', content: input };
    chatMessages.update(msgs => [...msgs, userMsg]);
    const prompt = input;
    input = '';
    loading = true;

    let context: string | undefined;
    const currentPath = get(activeFile);
    if (currentPath) {
      const files = get(openFiles);
      const file = files.find(f => f.path === currentPath);
      if (file?.content) {
        context = file.content;
      }
    }

    try {
      const response = await invoke<string>('ai_chat', {
        request: { prompt, context: context || null }
      });
      const assistantMsg: ChatMessage = { role: 'assistant', content: response };
      chatMessages.update(msgs => [...msgs, assistantMsg]);
    } catch (e) {
      const errorMsg: ChatMessage = { role: 'assistant', content: `Error: ${e}` };
      chatMessages.update(msgs => [...msgs, errorMsg]);
    }

    loading = false;
    scrollToBottom();
  }

  function scrollToBottom() {
    setTimeout(() => {
      if (messagesContainer) {
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
      }
    }, 50);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }
</script>

<div class="chat-container">
  {#if showKeyInput}
    <div class="key-setup">
      <p>Enter your Anthropic API key:</p>
      <input
        type="password"
        bind:value={keyInput}
        placeholder="sk-ant-..."
        onkeydown={(e) => e.key === 'Enter' && saveKey()}
      />
      <button class="save-key-btn" onclick={saveKey}>Save</button>
    </div>
  {/if}

  <div class="messages" bind:this={messagesContainer}>
    {#if $chatMessages.length === 0}
      <div class="empty-chat">
        <p>Ask anything about your code.</p>
        <p class="hint">The current file is sent as context.</p>
      </div>
    {/if}
    {#each $chatMessages as msg}
      <div class="message {msg.role}">
        <div class="message-header">{msg.role === 'user' ? 'You' : 'AI'}</div>
        <div class="message-body">{msg.content}</div>
      </div>
    {/each}
    {#if loading}
      <div class="message assistant">
        <div class="message-header">AI</div>
        <div class="message-body typing">Thinking...</div>
      </div>
    {/if}
  </div>

  <div class="chat-input-area">
    <textarea
      bind:value={input}
      placeholder="Ask about your code..."
      onkeydown={handleKeydown}
      rows="3"
    ></textarea>
    <button class="send-btn" onclick={sendMessage} disabled={loading || !input.trim()}>
      Send
    </button>
  </div>

  <button class="settings-btn" onclick={() => showKeyInput = !showKeyInput}>
    API Key
  </button>
</div>

<style>
  .chat-container {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  .key-setup {
    padding: 12px;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12px;
  }

  .save-key-btn {
    background: var(--accent);
    color: var(--bg-tertiary);
    padding: 6px 12px;
    border-radius: 4px;
    font-weight: 600;
  }

  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .empty-chat {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    text-align: center;
    font-size: 13px;
    gap: 4px;
  }

  .hint {
    font-size: 11px;
  }

  .message {
    font-size: 13px;
    line-height: 1.5;
  }

  .message-header {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    margin-bottom: 4px;
    text-transform: uppercase;
  }

  .message.user .message-body {
    background: var(--bg-surface);
    padding: 8px 12px;
    border-radius: 8px;
  }

  .message.assistant .message-body {
    color: var(--text-primary);
    padding: 8px 0;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .typing {
    color: var(--text-muted);
    font-style: italic;
  }

  .chat-input-area {
    padding: 8px 12px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .chat-input-area textarea {
    resize: none;
    font-size: 13px;
    background: var(--bg-tertiary);
  }

  .send-btn {
    align-self: flex-end;
    background: var(--accent);
    color: var(--bg-tertiary);
    padding: 6px 16px;
    border-radius: 4px;
    font-weight: 600;
    font-size: 12px;
  }

  .send-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .send-btn:not(:disabled):hover {
    background: var(--accent-hover);
  }

  .settings-btn {
    padding: 6px 12px;
    font-size: 11px;
    color: var(--text-muted);
    border-top: 1px solid var(--border);
  }

  .settings-btn:hover {
    color: var(--text-primary);
    background: var(--bg-surface);
  }
</style>
