<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { chatMessages, apiKey, aiModel, activeFile, openFiles, type ChatMessage } from './stores';
  import { get } from 'svelte/store';

  let input = $state('');
  let loading = $state(false);
  let messagesContainer: HTMLDivElement;
  let showKeyInput = $state(false);
  let keyInput = $state(get(apiKey));
  let selectedModel = $state(get(aiModel));

  const freeModels = [
    { id: 'openrouter/free', label: 'OpenRouter Auto (Free)' },
    { id: 'nvidia/nemotron-3-nano-30b-a3b:free', label: 'Nemotron Nano 30B' },
    { id: 'stepfun/step-3.5-flash:free', label: 'Step 3.5 Flash' },
    { id: 'liquid/lfm-2.5-1.2b-instruct:free', label: 'LFM 2.5 Instruct' },
    { id: 'arcee-ai/trinity-large-preview:free', label: 'Arcee Trinity Large' },
  ];

  const paidModels = [
    { id: 'anthropic/claude-sonnet-4', label: 'Claude Sonnet 4' },
    { id: 'anthropic/claude-haiku-4', label: 'Claude Haiku 4' },
    { id: 'openai/gpt-4o', label: 'GPT-4o' },
    { id: 'openai/gpt-4o-mini', label: 'GPT-4o Mini' },
    { id: 'google/gemini-2.5-pro-preview', label: 'Gemini 2.5 Pro' },
  ];

  async function saveKey() {
    apiKey.set(keyInput);
    // Store key on backend so it's not sent on every request
    try { await invoke('set_api_key', { key: keyInput }); } catch {}
    showKeyInput = false;
  }

  const slashCommands: Record<string, string> = {
    '/tableflip': '(╯°□°)╯︵ ┻━┻',
    '/tableunflip': '┬─┬ノ( º _ ºノ)',
    '/shrug': '¯\\_(ツ)_/¯',
    '/lenny': '( ͡° ͜ʖ ͡°)',
    '/disapproval': 'ಠ_ಠ',
    '/sparkles': '✧・゚: *✧・゚:*',
    '/bear': 'ʕ•ᴥ•ʔ',
    '/fight': '(\u0E07\u0027\u0300-\u0027\u0301)\u0E07',
    '/magic': '(ﾉ◕ヮ◕)ﾉ*:・゚✧',
    '/rage': '(ノಠ益ಠ)ノ彡┻━┻',
  };

  async function sendMessage() {
    const trimmed = input.trim();
    if (!trimmed || loading) return;

    const cmd = slashCommands[trimmed.toLowerCase()];
    if (cmd) {
      const msg: ChatMessage = { role: 'user', content: cmd };
      chatMessages.update(msgs => [...msgs, msg]);
      input = '';
      scrollToBottom();
      return;
    }

    const key = get(apiKey);
    if (!key) {
      showKeyInput = true;
      return;
    }

    const userMsg: ChatMessage = { role: 'user', content: trimmed };
    chatMessages.update(msgs => [...msgs, userMsg]);
    const prompt = trimmed;
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
        request: { prompt, context: context || null, model: selectedModel }
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
      <p>Enter your OpenRouter API key:</p>
      <input
        type="password"
        bind:value={keyInput}
        placeholder="sk-or-..."
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
    <div class="input-footer">
      <select
        class="model-select"
        bind:value={selectedModel}
        onchange={() => aiModel.set(selectedModel)}
      >
        <optgroup label="Free">
          {#each freeModels as m}
            <option value={m.id}>{m.label}</option>
          {/each}
        </optgroup>
        <optgroup label="Paid">
          {#each paidModels as m}
            <option value={m.id}>{m.label}</option>
          {/each}
        </optgroup>
      </select>
      <button class="send-btn" onclick={sendMessage} disabled={loading || !input.trim()}>
        Send
      </button>
    </div>
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

  .input-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .model-select {
    flex: 1;
    font-size: 11px;
    padding: 4px 6px;
    border-radius: 4px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border: 1px solid var(--border);
    min-width: 0;
  }

  .send-btn {
    background: var(--accent);
    color: var(--bg-tertiary);
    padding: 6px 16px;
    border-radius: 4px;
    font-weight: 600;
    font-size: 12px;
    flex-shrink: 0;
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
