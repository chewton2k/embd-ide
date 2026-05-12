<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { get } from 'svelte/store';
  import { Send, Square, X, Minus, Maximize2, Paperclip, XCircle, Sparkles, History, FileText, Terminal, Search, Pencil } from 'lucide-svelte';
  import {
    chatMessages, isStreaming, aiProvider, aiModel, apiKey, openaiApiKey, anthropicApiKey,
    sendStreamingMessage, cancelStream, clearChat, attachedFiles, type AiProvider, type ChatMessage,
    listConversations, loadConversation
  } from '../../modules/stores';
  import { projectRoot, showChat, activeFile, openFiles } from '../../modules/stores';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';

  // ── Window state ──
  let x = $state(Math.max(100, window.innerWidth - 420));
  let y = $state(Math.max(100, window.innerHeight - 560));
  let width = $state(400);
  let height = $state(500);
  let minimized = $state(false);

  // ── Input state ──
  let input = $state('');
  let messagesEl = $state<HTMLDivElement>(undefined!);

  // ── History state ──
  let showHistory = $state(false);
  let conversations = $state<{ id: string; title: string; updated_at: number }[]>([]);

  async function toggleHistory() {
    if (showHistory) { showHistory = false; return; }
    conversations = await listConversations();
    showHistory = true;
  }

  async function selectConversation(id: string) {
    await loadConversation(id);
    showHistory = false;
  }

  // ── Drag state ──
  let dragging = $state(false);
  let dragStart = { x: 0, y: 0, winX: 0, winY: 0 };

  // ── Resize state ──
  let resizing = $state(false);
  let resizeStart = { x: 0, y: 0, w: 0, h: 0 };

  // ── Provider/model — stay in sync with global stores ──
  let selectedProvider = $state<AiProvider>(get(aiProvider));
  let selectedModel = $state(get(aiModel));

  // Subscribe to store changes (e.g. from Settings)
  $effect(() => {
    const unsub = aiProvider.subscribe(v => { selectedProvider = v; });
    return unsub;
  });
  $effect(() => {
    const unsub = aiModel.subscribe(v => { selectedModel = v; });
    return unsub;
  });

  const MODELS: Record<AiProvider, { id: string; label: string }[]> = {
    openrouter: [
      { id: 'openrouter/auto', label: 'Auto' },
      { id: 'anthropic/claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
      { id: 'openai/gpt-5', label: 'GPT-5' },
      { id: 'google/gemini-2.5-pro', label: 'Gemini 2.5 Pro' },
      { id: 'deepseek/deepseek-v3.1', label: 'DeepSeek V3.1' },
    ],
    openai: [
      { id: 'gpt-5', label: 'GPT-5' },
      { id: 'gpt-5-mini', label: 'GPT-5 mini' },
      { id: 'o3', label: 'o3' },
    ],
    anthropic: [
      { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
      { id: 'claude-haiku-4-5', label: 'Claude Haiku 4.5' },
    ],
  };

  // ── Drag handlers ──
  function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button, select')) return;
    dragging = true;
    dragStart = { x: e.clientX, y: e.clientY, winX: x, winY: y };
    window.addEventListener('mousemove', onDragMove);
    window.addEventListener('mouseup', stopDrag);
  }
  function onDragMove(e: MouseEvent) {
    x = Math.max(0, Math.min(window.innerWidth - 100, dragStart.winX + e.clientX - dragStart.x));
    y = Math.max(0, Math.min(window.innerHeight - 40, dragStart.winY + e.clientY - dragStart.y));
  }
  function stopDrag() {
    dragging = false;
    window.removeEventListener('mousemove', onDragMove);
    window.removeEventListener('mouseup', stopDrag);
  }

  // ── Resize handlers ──
  function startResize(e: MouseEvent) {
    e.preventDefault();
    resizing = true;
    resizeStart = { x: e.clientX, y: e.clientY, w: width, h: height };
    window.addEventListener('mousemove', onResizeMove);
    window.addEventListener('mouseup', stopResize);
  }
  function onResizeMove(e: MouseEvent) {
    width = Math.max(300, resizeStart.w + e.clientX - resizeStart.x);
    height = Math.max(300, resizeStart.h + e.clientY - resizeStart.y);
  }
  function stopResize() {
    resizing = false;
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', stopResize);
  }

  // ── Send message ──
  async function send() {
    const trimmed = input.trim();
    if (!trimmed || $isStreaming) return;

    // Check API key from keychain directly (stores may be stale across windows)
    let hasKey = false;
    try {
      const key = await invoke<string>('get_provider_key', { provider: selectedProvider });
      hasKey = !!key;
    } catch { /* ignore */ }

    if (!hasKey) {
      chatMessages.update(msgs => [...msgs, { role: 'assistant', content: `No API key set for ${selectedProvider}. Go to Settings → Models.` }]);
      return;
    }

    input = '';

    // Read attached file contents
    let fileContexts: { path: string; content: string }[] | undefined;
    const files = get(attachedFiles);
    if (files.length > 0) {
      fileContexts = [];
      for (const f of files) {
        try {
          const content = await invoke<string>('read_file_content', { path: f.path });
          fileContexts.push({ path: f.name, content });
        } catch { /* skip unreadable files */ }
      }
    }

    await sendStreamingMessage(trimmed, fileContexts);
    scrollToBottom();
  }

  function scrollToBottom() {
    requestAnimationFrame(() => {
      if (messagesEl) messagesEl.scrollTop = messagesEl.scrollHeight;
    });
  }

  // Auto-scroll on new content
  $effect(() => {
    $chatMessages;
    scrollToBottom();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  function attachCurrentFile() {
    const path = get(activeFile);
    if (!path) return;
    const name = path.split('/').pop() || path;
    attachedFiles.update(files => {
      if (files.some(f => f.path === path)) return files;
      return [...files, { path, name }];
    });
  }

  function removeFile(path: string) {
    attachedFiles.update(files => files.filter(f => f.path !== path));
  }

  function onProviderChange() {
    aiProvider.set(selectedProvider);
    const models = MODELS[selectedProvider];
    if (!models.some(m => m.id === selectedModel)) {
      selectedModel = models[0].id;
      aiModel.set(selectedModel);
    }
  }

  function onModelChange() {
    aiModel.set(selectedModel);
  }

  function renderMd(content: string): string {
    return DOMPurify.sanitize(marked.parse(content, { async: false }) as string);
  }
</script>

{#if $showChat}
  <div
    class="floating-chat"
    class:minimized
    class:dragging
    class:resizing
    style="left:{x}px;top:{y}px;width:{width}px;height:{minimized ? 'auto' : `${height}px`}"
  >
    <!-- Title bar -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="chat-titlebar" onmousedown={startDrag}>
      <span class="chat-title"><Sparkles size={13} /> Leo AI</span>
      <div class="titlebar-actions">
        <button onclick={toggleHistory} title="Chat history" class:active={showHistory}>
          <History size={12} />
        </button>
        <button onclick={() => minimized = !minimized} title={minimized ? 'Expand' : 'Minimize'}>
          {#if minimized}<Maximize2 size={12} />{:else}<Minus size={12} />{/if}
        </button>
        <button onclick={() => showChat.set(false)} title="Close"><X size={12} /></button>
      </div>
    </div>

    {#if !minimized}
      <!-- History dropdown -->
      {#if showHistory}
        <div class="history-panel">
          <div class="history-header">
            <span>Recent Conversations</span>
            <button onclick={() => showHistory = false}><X size={11} /></button>
          </div>
          {#if conversations.length === 0}
            <div class="history-empty">No saved conversations yet.</div>
          {:else}
            <div class="history-list">
              {#each conversations as conv}
                <button class="history-item" onclick={() => selectConversation(conv.id)}>
                  <span class="history-title">{conv.title}</span>
                  <span class="history-date">{new Date(conv.updated_at * 1000).toLocaleDateString()}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <!-- Messages -->
      <div class="chat-messages" bind:this={messagesEl}>
        {#if $chatMessages.length === 0}
          <div class="chat-empty">
            <Sparkles size={24} />
            <p>Ask anything about your code.</p>
            <span class="chat-empty-hint">Attach files for context with 📎</span>
          </div>
        {/if}
        {#each $chatMessages as msg}
          <div class="chat-msg {msg.role}">
            {#if msg.role === 'user'}
              {#if msg.content.startsWith('[File content of')}
                <div class="activity-indicator">
                  <FileText size={11} /> <span>Read file</span>
                </div>
              {:else if msg.content.startsWith('[Executed:')}
                <div class="activity-indicator">
                  <Terminal size={11} /> <span>Ran command</span>
                </div>
              {:else if msg.content.startsWith('[Search results')}
                <div class="activity-indicator">
                  <Search size={11} /> <span>Searched files</span>
                </div>
              {:else if msg.content.startsWith('[Applied edit')}
                <div class="activity-indicator">
                  <Pencil size={11} /> <span>Applied edit</span>
                </div>
              {:else if msg.content.startsWith('[')}
                <div class="activity-indicator">
                  <Sparkles size={11} /> <span>{msg.content.slice(1, msg.content.indexOf(']'))}</span>
                </div>
              {:else}
                <div class="msg-bubble user">{msg.content}</div>
              {/if}
            {:else}
              <div class="msg-bubble assistant">{@html renderMd(msg.content)}</div>
            {/if}
          </div>
        {/each}
        {#if $isStreaming}
          <div class="streaming-bar">
            <div class="streaming-dot"></div>
            <span>Generating...</span>
          </div>
        {/if}
      </div>

      <!-- Attached files -->
      {#if $attachedFiles.length > 0}
        <div class="attached-files">
          {#each $attachedFiles as file}
            <span class="file-chip">
              {file.name}
              <button onclick={() => removeFile(file.path)}><XCircle size={10} /></button>
            </span>
          {/each}
        </div>
      {/if}

      <!-- Input -->
      <div class="chat-input-area">
        <div class="input-row">
          <button class="attach-btn" onclick={attachCurrentFile} title="Attach current file">
            <Paperclip size={13} />
          </button>
          <textarea
            bind:value={input}
            placeholder="Ask about your code..."
            onkeydown={handleKeydown}
            rows="2"
          ></textarea>
          {#if $isStreaming}
            <button class="send-btn stop" onclick={cancelStream} title="Stop">
              <Square size={12} />
            </button>
          {:else}
            <button class="send-btn" onclick={send} disabled={!input.trim()} title="Send">
              <Send size={12} />
            </button>
          {/if}
        </div>
        <div class="input-footer">
          <select bind:value={selectedProvider} onchange={onProviderChange}>
            <option value="openrouter">OpenRouter</option>
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
          </select>
          <select bind:value={selectedModel} onchange={onModelChange}>
            {#each MODELS[selectedProvider] as m}
              <option value={m.id}>{m.label}</option>
            {/each}
          </select>
          <button class="clear-btn" onclick={clearChat}>Clear</button>
        </div>
      </div>

      <!-- Resize handle -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle" onmousedown={startResize}></div>
    {/if}
  </div>
{/if}

<style>
  .floating-chat {
    position: fixed;
    z-index: 900;
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0,0,0,0.4);
    overflow: hidden;
    min-width: 300px;
    min-height: 300px;
  }

  .floating-chat.minimized {
    min-height: unset;
  }

  .floating-chat.dragging, .floating-chat.resizing {
    user-select: none;
  }

  .chat-titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border);
    cursor: grab;
    flex-shrink: 0;
  }

  .chat-titlebar:active { cursor: grabbing; }

  .chat-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .titlebar-actions {
    display: flex;
    gap: 4px;
  }

  .titlebar-actions button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .titlebar-actions button:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .chat-messages {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }

  .chat-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    font-size: 12px;
    gap: 8px;
    text-align: center;
  }

  .chat-empty-hint { font-size: 10px; opacity: 0.6; }

  .chat-msg { font-size: 12px; line-height: 1.5; }

  .msg-bubble {
    padding: 8px 12px;
    border-radius: 12px;
    max-width: 85%;
    word-break: break-word;
  }

  .msg-bubble.user {
    background: var(--accent);
    color: #fff;
    margin-left: auto;
    border-bottom-right-radius: 4px;
  }

  .msg-bubble.assistant {
    background: var(--bg-surface);
    color: var(--text-primary);
    border-bottom-left-radius: 4px;
    white-space: normal;
  }

  .msg-bubble.assistant :global(pre) { background: var(--bg-tertiary); padding: 8px; border-radius: 6px; overflow-x: auto; font-size: 11px; margin: 6px 0; }
  .msg-bubble.assistant :global(code) { font-family: var(--font-mono, monospace); font-size: 11px; }
  .msg-bubble.assistant :global(p) { margin: 4px 0; }
  .msg-bubble.assistant :global(ul), .msg-bubble.assistant :global(ol) { padding-left: 16px; margin: 4px 0; }

  .activity-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    font-size: 11px;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border-radius: 8px;
    width: fit-content;
    margin: 0 auto;
  }

  .streaming-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    font-size: 11px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    border-radius: 8px;
    width: fit-content;
  }

  .streaming-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1s infinite;
  }

  @keyframes pulse { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.4; transform: scale(0.8); } }

  /* History panel */
  .history-panel {
    border-bottom: 1px solid var(--border);
    max-height: 200px;
    overflow-y: auto;
    background: var(--bg-tertiary);
  }

  .history-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    position: sticky;
    top: 0;
    background: var(--bg-tertiary);
  }

  .history-header button { color: var(--text-muted); }

  .history-empty {
    padding: 16px;
    text-align: center;
    font-size: 11px;
    color: var(--text-muted);
  }

  .history-list { display: flex; flex-direction: column; }

  .history-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    font-size: 11px;
    color: var(--text-primary);
    cursor: pointer;
    border-bottom: 1px solid var(--border);
  }

  .history-item:hover { background: var(--bg-surface); }
  .history-title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .history-date { color: var(--text-muted); font-size: 10px; flex-shrink: 0; margin-left: 8px; }

  .titlebar-actions button.active { color: var(--accent); }

  .attached-files {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 4px 12px;
    border-top: 1px solid var(--border);
  }

  .file-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: var(--bg-surface);
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .file-chip button {
    display: flex;
    color: var(--text-muted);
    cursor: pointer;
  }

  .chat-input-area {
    padding: 8px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
  }

  .input-row {
    display: flex;
    align-items: flex-end;
    gap: 6px;
  }

  .input-row textarea {
    flex: 1;
    resize: none;
    font-size: 12px;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    min-height: 32px;
  }

  .attach-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 4px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .attach-btn:hover { background: var(--bg-surface); color: var(--text-primary); }

  .send-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 4px;
    background: var(--accent);
    color: var(--bg-primary);
    flex-shrink: 0;
  }

  .send-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .send-btn.stop { background: #f14c4c; }

  .input-footer {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .input-footer select {
    font-size: 10px;
    padding: 3px 4px;
    border-radius: 4px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border: 1px solid var(--border);
    flex: 1;
    min-width: 0;
  }

  .clear-btn {
    font-size: 10px;
    padding: 3px 8px;
    border-radius: 4px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .clear-btn:hover { background: var(--bg-surface); color: var(--text-primary); }

  .resize-handle {
    position: absolute;
    bottom: 0;
    right: 0;
    width: 16px;
    height: 16px;
    cursor: nwse-resize;
  }
</style>
