<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { get } from 'svelte/store';
  import {
    Send, Square, X, Minus, Maximize2, Paperclip, XCircle, Sparkles, History,
    FileText, Terminal as TerminalIcon, Search as SearchIcon, Pencil,
    AlertTriangle, CheckCircle2, ChevronDown, ChevronRight, Plus, Play,
  } from 'lucide-svelte';
  import {
    chatMessages, isStreaming, aiProvider, aiModel,
    sendStreamingMessage, cancelStream, clearChat, attachedFiles,
    type AiProvider,
    listConversations, loadConversation, saveConversationNow,
  } from '../../modules';
  import { showChat, activeFile } from '../../modules';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import {
    parseAssistantContent, parseUserContent, truncate, basename,
    type ChatBlock,
  } from '../../modules/ai/chatRenderer';

  // ── Window geometry ─────────────────────────────────────────────
  let x = $state(Math.max(100, window.innerWidth - 440));
  let y = $state(Math.max(100, window.innerHeight - 620));
  let width = $state(420);
  let height = $state(560);
  let minimized = $state(false);

  // ── Chat input ──────────────────────────────────────────────────
  let input = $state('');
  let inputEl = $state<HTMLTextAreaElement>(undefined!);
  let messagesEl = $state<HTMLDivElement>(undefined!);

  // ── Expanded tool-call / tool-result panels ─────────────────────
  // Keyed by message-index + block-index. Reset on conversation change.
  let expandedBlocks = $state<Record<string, boolean>>({});
  function toggleBlock(key: string) {
    expandedBlocks = { ...expandedBlocks, [key]: !expandedBlocks[key] };
  }

  // ── History dropdown ────────────────────────────────────────────
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
    expandedBlocks = {};
  }

  function newConversation() {
    saveConversationNow().finally(() => {
      clearChat();
      expandedBlocks = {};
      showHistory = false;
    });
  }

  // ── Drag / resize ───────────────────────────────────────────────
  let dragging = $state(false);
  let resizing = $state(false);
  let dragStart = { x: 0, y: 0, winX: 0, winY: 0 };
  let resizeStart = { x: 0, y: 0, w: 0, h: 0 };

  function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button, select, input')) return;
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

  function startResize(e: MouseEvent) {
    e.preventDefault();
    resizing = true;
    resizeStart = { x: e.clientX, y: e.clientY, w: width, h: height };
    window.addEventListener('mousemove', onResizeMove);
    window.addEventListener('mouseup', stopResize);
  }
  function onResizeMove(e: MouseEvent) {
    width = Math.max(320, resizeStart.w + e.clientX - resizeStart.x);
    height = Math.max(320, resizeStart.h + e.clientY - resizeStart.y);
  }
  function stopResize() {
    resizing = false;
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', stopResize);
  }

  // ── Provider / model menu ───────────────────────────────────────
  let selectedProvider = $state<AiProvider>(get(aiProvider));
  let selectedModel = $state(get(aiModel));
  let modelMenuOpen = $state(false);
  let modelMenuEl = $state<HTMLDivElement>();

  $effect(() => { const u = aiProvider.subscribe(v => selectedProvider = v); return u; });
  $effect(() => { const u = aiModel.subscribe(v => selectedModel = v); return u; });

  const MODELS: Record<AiProvider, { id: string; label: string }[]> = {
    openrouter: [
      { id: 'openrouter/auto',               label: 'Auto' },
      { id: 'anthropic/claude-sonnet-4-6',   label: 'Claude Sonnet 4.6' },
      { id: 'openai/gpt-5',                  label: 'GPT-5' },
      { id: 'google/gemini-2.5-pro',         label: 'Gemini 2.5 Pro' },
      { id: 'deepseek/deepseek-v3.1',        label: 'DeepSeek V3.1' },
    ],
    openai: [
      { id: 'gpt-5',       label: 'GPT-5' },
      { id: 'gpt-5-mini',  label: 'GPT-5 mini' },
      { id: 'o3',          label: 'o3' },
    ],
    anthropic: [
      { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
      { id: 'claude-haiku-4-5',  label: 'Claude Haiku 4.5' },
    ],
  };

  const PROVIDER_LABEL: Record<AiProvider, string> = {
    openrouter: 'OpenRouter',
    openai: 'OpenAI',
    anthropic: 'Anthropic',
  };

  const currentModelLabel = $derived(
    MODELS[selectedProvider].find(m => m.id === selectedModel)?.label ?? selectedModel
  );

  function pickModel(provider: AiProvider, modelId: string) {
    if (provider !== selectedProvider) {
      selectedProvider = provider;
      aiProvider.set(provider);
    }
    selectedModel = modelId;
    aiModel.set(modelId);
    modelMenuOpen = false;
  }

  function handleDocClick(e: MouseEvent) {
    if (modelMenuOpen && modelMenuEl && !modelMenuEl.contains(e.target as Node)) {
      modelMenuOpen = false;
    }
  }

  // ── Send ────────────────────────────────────────────────────────
  async function send() {
    const trimmed = input.trim();
    if (!trimmed || $isStreaming) return;

    let hasKey = false;
    try {
      const key = await invoke<string>('get_provider_key', { provider: selectedProvider });
      hasKey = !!key;
    } catch { /* ignore */ }
    if (!hasKey) {
      chatMessages.update(msgs => [
        ...msgs,
        { role: 'assistant', content: `No API key set for ${PROVIDER_LABEL[selectedProvider]}. Open Settings → Models.` },
      ]);
      return;
    }

    input = '';
    autoSizeInput();

    let fileContexts: { path: string; content: string }[] | undefined;
    const files = get(attachedFiles);
    if (files.length > 0) {
      fileContexts = [];
      for (const f of files) {
        try {
          const content = await invoke<string>('read_file_content', { path: f.path });
          fileContexts.push({ path: f.name, content });
        } catch { /* skip unreadable */ }
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

  // Auto-scroll while streaming
  $effect(() => { $chatMessages; scrollToBottom(); });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    }
  }

  function autoSizeInput() {
    if (!inputEl) return;
    inputEl.style.height = 'auto';
    const h = Math.min(200, inputEl.scrollHeight);
    inputEl.style.height = h + 'px';
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

  // ── Markdown rendering ──────────────────────────────────────────
  function renderProse(content: string): string {
    return DOMPurify.sanitize(marked.parse(content, { async: false }) as string);
  }

  function formatConvDate(epochSecs: number): string {
    const d = new Date(epochSecs * 1000);
    const now = new Date();
    const sameDay = d.toDateString() === now.toDateString();
    if (sameDay) return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
    const yesterday = new Date(now.getTime() - 86_400_000);
    if (d.toDateString() === yesterday.toDateString()) return 'Yesterday';
    return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }

  // Parse each message into blocks. Re-runs reactively as messages mutate.
  const parsedMessages = $derived(
    $chatMessages.map((msg, i) => ({
      role: msg.role,
      blocks: msg.role === 'user'
        ? parseUserContent(msg.content)
        : msg.role === 'assistant'
          ? parseAssistantContent(msg.content)
          : [{ kind: 'prose' as const, text: msg.content }],
      index: i,
    }))
  );

  /** Returns true if the assistant's last message is just an empty prose block
   *  (i.e. we're waiting for the first delta). Used to show a skeleton loader. */
  const waitingForFirstChunk = $derived.by(() => {
    if (!$isStreaming) return false;
    const last = $chatMessages[$chatMessages.length - 1];
    return !!last && last.role === 'assistant' && last.content.trim() === '';
  });

  function blockIcon(b: ChatBlock) {
    switch (b.kind) {
      case 'tool-read':         return FileText;
      case 'tool-search':       return SearchIcon;
      case 'tool-run':          return TerminalIcon;
      case 'tool-run-dangerous':return AlertTriangle;
      case 'tool-edit':         return Pencil;
      case 'tool-result':       return CheckCircle2;
      case 'activity':          return Sparkles;
      case 'error':             return AlertTriangle;
      default:                  return Sparkles;
    }
  }

  function blockTitle(b: ChatBlock): string {
    switch (b.kind) {
      case 'tool-read':         return 'Read file';
      case 'tool-search':       return 'Search';
      case 'tool-run':          return 'Run command';
      case 'tool-run-dangerous':return 'Dangerous command';
      case 'tool-edit':         return 'Proposed edit';
      case 'tool-result':       return b.label || 'Result';
      case 'activity':          return b.label || 'Activity';
      case 'error':             return b.label || 'Error';
      default:                  return '';
    }
  }
</script>

<svelte:document onmousedown={handleDocClick} />

{#if $showChat}
  <section
    class="chat"
    class:minimized
    class:dragging
    class:resizing
    style="left:{x}px;top:{y}px;width:{width}px;height:{minimized ? 'auto' : `${height}px`}"
    aria-label="Leo AI chat"
  >
    <!-- Title bar -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header class="titlebar" onmousedown={startDrag}>
      <div class="title">
        <Sparkles size={13} />
        <span>Leo AI</span>
      </div>
      <div class="title-actions">
        <button class="icon-btn" onclick={newConversation} title="New chat" aria-label="New chat">
          <Plus size={13} />
        </button>
        <button class="icon-btn" class:active={showHistory} onclick={toggleHistory} title="History" aria-label="History">
          <History size={13} />
        </button>
        <button class="icon-btn" onclick={() => minimized = !minimized} title={minimized ? 'Expand' : 'Minimize'} aria-label={minimized ? 'Expand' : 'Minimize'}>
          {#if minimized}<Maximize2 size={12} />{:else}<Minus size={12} />{/if}
        </button>
        <button class="icon-btn" onclick={() => { saveConversationNow(); showChat.set(false); }} title="Close" aria-label="Close">
          <X size={13} />
        </button>
      </div>
    </header>

    {#if !minimized}
      <!-- History -->
      {#if showHistory}
        <div class="history">
          <div class="history-head">
            <span>Recent</span>
            <button class="icon-btn small" onclick={() => showHistory = false} aria-label="Close history">
              <X size={11} />
            </button>
          </div>
          {#if conversations.length === 0}
            <div class="history-empty">No saved conversations yet.</div>
          {:else}
            <div class="history-list">
              {#each conversations as conv (conv.id)}
                <button class="history-item" onclick={() => selectConversation(conv.id)}>
                  <span class="history-title" title={conv.title}>{conv.title}</span>
                  <span class="history-date">{formatConvDate(conv.updated_at)}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <!-- Messages -->
      <div class="messages" bind:this={messagesEl} role="log" aria-live="polite">
        {#if $chatMessages.length === 0}
          <div class="empty">
            <div class="empty-icon"><Sparkles size={22} /></div>
            <p class="empty-title">Ask anything about your code</p>
            <p class="empty-hint">
              <Paperclip size={11} /> attach files for context —
              <kbd>Enter</kbd> to send, <kbd>Shift</kbd>+<kbd>Enter</kbd> for newline
            </p>
          </div>
        {/if}

        {#each parsedMessages as m (m.index)}
          {#if m.role === 'user'}
            {#each m.blocks as block, bi}
              {#if block.kind === 'prose'}
                <div class="row user">
                  <div class="bubble user-bubble">{block.text}</div>
                </div>
              {:else}
                {@const key = `${m.index}:${bi}`}
                {@const IconComp = blockIcon(block)}
                <div class="row tool">
                  <button
                    type="button"
                    class="tool-card"
                    class:has-detail={!!block.detail}
                    class:danger={block.kind === 'error'}
                    onclick={() => block.detail && toggleBlock(key)}
                    disabled={!block.detail}
                    aria-expanded={block.detail ? !!expandedBlocks[key] : undefined}
                  >
                    <span class="tool-icon"><IconComp size={12} /></span>
                    <span class="tool-head">
                      <span class="tool-kind">{blockTitle(block)}</span>
                      <span class="tool-target" title={block.text}>{block.text}</span>
                    </span>
                    {#if block.detail}
                      <span class="tool-chev">
                        {#if expandedBlocks[key]}<ChevronDown size={11} />{:else}<ChevronRight size={11} />{/if}
                      </span>
                    {/if}
                  </button>
                  {#if block.detail && expandedBlocks[key]}
                    <pre class="tool-detail">{truncate(block.detail, 4000)}</pre>
                  {/if}
                </div>
              {/if}
            {/each}
          {:else if m.role === 'assistant'}
            <div class="row assistant">
              <div class="assistant-head">
                <Sparkles size={11} />
                <span>Assistant</span>
              </div>
              <div class="assistant-body">
                {#each m.blocks as block, bi}
                  {#if block.kind === 'prose'}
                    {#if block.text.trim()}
                      <div class="prose">{@html renderProse(block.text)}</div>
                    {/if}
                  {:else}
                    {@const key = `${m.index}:${bi}`}
                    {@const IconComp = blockIcon(block)}
                    <button
                      type="button"
                      class="tool-card inline"
                      class:danger={block.kind === 'tool-run-dangerous' || block.kind === 'error'}
                      onclick={() => toggleBlock(key)}
                      aria-expanded={!!expandedBlocks[key]}
                    >
                      <span class="tool-icon"><IconComp size={12} /></span>
                      <span class="tool-head">
                        <span class="tool-kind">{blockTitle(block)}</span>
                        <span class="tool-target" title={block.text}>
                          {block.kind === 'tool-read' ? basename(block.text) : truncate(block.text, 80)}
                        </span>
                      </span>
                      {#if block.kind === 'tool-run' || block.kind === 'tool-run-dangerous'}
                        <span class="tool-run-badge"><Play size={9} /></span>
                      {/if}
                    </button>
                  {/if}
                {/each}
                {#if m.index === $chatMessages.length - 1 && waitingForFirstChunk}
                  <div class="skeleton" aria-label="Thinking">
                    <span></span><span></span><span></span>
                  </div>
                {/if}
              </div>
            </div>
          {/if}
        {/each}
      </div>

      <!-- Attached files -->
      {#if $attachedFiles.length > 0}
        <div class="chips">
          {#each $attachedFiles as file (file.path)}
            <span class="chip">
              <FileText size={10} />
              <span class="chip-name" title={file.path}>{file.name}</span>
              <button class="chip-remove" onclick={() => removeFile(file.path)} aria-label="Remove {file.name}">
                <XCircle size={11} />
              </button>
            </span>
          {/each}
        </div>
      {/if}

      <!-- Composer -->
      <div class="composer">
        <div class="composer-input">
          <textarea
            bind:this={inputEl}
            bind:value={input}
            oninput={autoSizeInput}
            onkeydown={handleKeydown}
            placeholder="Ask about your code…"
            rows="1"
            spellcheck="false"
          ></textarea>
        </div>
        <div class="composer-bar">
          <div class="composer-left">
            <button class="icon-btn" onclick={attachCurrentFile} title="Attach current file" aria-label="Attach current file">
              <Paperclip size={12} />
            </button>
            <div class="model-picker" bind:this={modelMenuEl}>
              <button class="model-btn" onclick={() => modelMenuOpen = !modelMenuOpen} aria-haspopup="menu" aria-expanded={modelMenuOpen}>
                <span class="model-provider">{PROVIDER_LABEL[selectedProvider]}</span>
                <span class="model-sep">·</span>
                <span class="model-name">{currentModelLabel}</span>
                <ChevronDown size={10} />
              </button>
              {#if modelMenuOpen}
                <div class="model-menu" role="menu">
                  {#each Object.entries(MODELS) as [prov, models] (prov)}
                    <div class="model-group">
                      <div class="model-group-head">{PROVIDER_LABEL[prov as AiProvider]}</div>
                      {#each models as m (m.id)}
                        <button
                          type="button"
                          role="menuitem"
                          class="model-item"
                          class:active={selectedProvider === prov && selectedModel === m.id}
                          onclick={() => pickModel(prov as AiProvider, m.id)}
                        >
                          {m.label}
                        </button>
                      {/each}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="composer-right">
            {#if $isStreaming}
              <button class="send-btn stop" onclick={cancelStream} title="Stop" aria-label="Stop generating">
                <Square size={11} />
              </button>
            {:else}
              <button class="send-btn" onclick={send} disabled={!input.trim()} title="Send" aria-label="Send message">
                <Send size={12} />
              </button>
            {/if}
          </div>
        </div>
      </div>

      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle" onmousedown={startResize} aria-hidden="true"></div>
    {/if}
  </section>
{/if}

<style>
  /* ── Shell ──────────────────────────────────────────────────── */
  .chat {
    position: fixed;
    z-index: 900;
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--bg-secondary) 60%, transparent);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 18px 50px rgba(0,0,0,0.45), 0 2px 4px rgba(0,0,0,0.25);
    overflow: hidden;
    min-width: 320px;
    min-height: 320px;
    font-family: var(--font-ui);
  }
  .chat.minimized { min-height: unset; }
  .chat.dragging, .chat.resizing { user-select: none; }

  /* ── Titlebar ──────────────────────────────────────────────── */
  .titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 34px;
    padding: 0 10px;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border);
    cursor: grab;
    flex-shrink: 0;
  }
  .titlebar:active { cursor: grabbing; }
  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: -0.1px;
  }
  .title :global(svg) { color: var(--accent); }
  .title-actions { display: flex; gap: 2px; }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px; height: 24px;
    border-radius: 5px;
    color: var(--text-muted);
    background: none;
    border: none;
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }
  .icon-btn:hover { background: var(--bg-surface); color: var(--text-primary); }
  .icon-btn.active { color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }
  .icon-btn.small { width: 20px; height: 20px; }

  /* ── Messages ──────────────────────────────────────────────── */
  .messages {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 14px 8px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    scrollbar-gutter: stable;
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--text-muted);
    text-align: center;
  }
  .empty-icon {
    width: 36px; height: 36px;
    display: flex; align-items: center; justify-content: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--accent);
    margin-bottom: 2px;
  }
  .empty-title { margin: 0; font-size: 13px; color: var(--text-secondary); font-weight: 500; }
  .empty-hint {
    margin: 0;
    font-size: 11px;
    display: inline-flex; align-items: center; gap: 5px;
    opacity: 0.8;
  }
  .empty-hint kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    padding: 1px 5px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text-secondary);
  }

  /* ── Row layouts ──────────────────────────────────────────── */
  .row { display: flex; flex-direction: column; gap: 4px; }
  .row.user { align-items: flex-end; }
  .row.tool { align-items: stretch; }

  /* User bubble — subtle, right-aligned */
  .bubble {
    max-width: 86%;
    padding: 8px 12px;
    font-size: 12.5px;
    line-height: 1.5;
    word-break: break-word;
    border-radius: 10px;
  }
  .user-bubble {
    background: color-mix(in srgb, var(--accent) 18%, var(--bg-surface));
    color: var(--text-primary);
    border: 1px solid color-mix(in srgb, var(--accent) 25%, transparent);
    border-bottom-right-radius: 4px;
  }

  /* Assistant — no bubble, just content with role indicator */
  .row.assistant { align-items: stretch; gap: 6px; }
  .assistant-head {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.3px;
    text-transform: uppercase;
  }
  .assistant-head :global(svg) { color: var(--accent); }
  .assistant-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12.5px;
    line-height: 1.55;
  }

  .prose { color: var(--text-primary); }
  .prose :global(p)   { margin: 4px 0; }
  .prose :global(p:first-child) { margin-top: 0; }
  .prose :global(p:last-child)  { margin-bottom: 0; }
  .prose :global(ul), .prose :global(ol) { padding-left: 18px; margin: 6px 0; }
  .prose :global(code) {
    font-family: var(--font-mono);
    font-size: 11.5px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
  }
  .prose :global(pre) {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px;
    overflow-x: auto;
    margin: 8px 0;
  }
  .prose :global(pre code) {
    background: none;
    border: none;
    padding: 0;
    font-size: 11.5px;
  }
  .prose :global(a) { color: var(--accent); text-decoration: none; }
  .prose :global(a:hover) { text-decoration: underline; }
  .prose :global(strong) { color: var(--text-primary); font-weight: 600; }
  .prose :global(h1), .prose :global(h2), .prose :global(h3) {
    font-size: 13px; font-weight: 600; margin: 8px 0 4px;
    color: var(--text-primary);
  }
  .prose :global(blockquote) {
    border-left: 2px solid var(--border);
    padding-left: 10px;
    color: var(--text-secondary);
    margin: 6px 0;
  }

  /* ── Tool cards ───────────────────────────────────────────── */
  .tool-card {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s;
  }
  .tool-card.inline { margin: 2px 0; }
  .tool-card:hover:not(:disabled) {
    background: var(--bg-surface);
    border-color: color-mix(in srgb, var(--border) 60%, var(--text-muted));
  }
  .tool-card:disabled {
    cursor: default;
  }
  .tool-card.danger {
    border-color: color-mix(in srgb, var(--warning, #d79921) 45%, var(--border));
    background: color-mix(in srgb, var(--warning, #d79921) 8%, var(--bg-tertiary));
  }
  .tool-card.has-detail:not(:disabled) {
    cursor: pointer;
  }
  .tool-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px; height: 20px;
    border-radius: 5px;
    background: var(--bg-surface);
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .tool-card.danger .tool-icon { color: var(--warning, #d79921); }
  .tool-head {
    display: flex;
    align-items: baseline;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }
  .tool-kind {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.1px;
    flex-shrink: 0;
  }
  .tool-target {
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tool-chev { color: var(--text-muted); display: flex; flex-shrink: 0; }
  .tool-run-badge {
    display: flex;
    align-items: center;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .tool-detail {
    font-family: var(--font-mono);
    font-size: 11px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 10px;
    margin: 0;
    max-height: 240px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text-secondary);
  }

  /* ── Skeleton / streaming indicator ──────────────────────── */
  .skeleton {
    display: inline-flex;
    gap: 4px;
    padding: 4px 0;
  }
  .skeleton span {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--text-muted);
    opacity: 0.35;
    animation: skeletonPulse 1.2s infinite ease-in-out;
  }
  .skeleton span:nth-child(2) { animation-delay: 0.15s; }
  .skeleton span:nth-child(3) { animation-delay: 0.3s; }
  @keyframes skeletonPulse {
    0%, 80%, 100% { opacity: 0.2; transform: scale(0.85); }
    40%           { opacity: 0.9; transform: scale(1); }
  }

  /* ── History panel ────────────────────────────────────────── */
  .history {
    border-bottom: 1px solid var(--border);
    max-height: 220px;
    overflow-y: auto;
    background: var(--bg-tertiary);
  }
  .history-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    position: sticky;
    top: 0;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border);
  }
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
    gap: 10px;
    padding: 8px 12px;
    font-size: 11.5px;
    color: var(--text-primary);
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
    cursor: pointer;
  }
  .history-item:hover { background: var(--bg-surface); }
  .history-title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .history-date { color: var(--text-muted); font-size: 10.5px; flex-shrink: 0; font-variant-numeric: tabular-nums; }

  /* ── Attached file chips ──────────────────────────────────── */
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 6px 10px 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 6px 3px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 5px;
    font-size: 11px;
    color: var(--text-secondary);
    max-width: 220px;
  }
  .chip :global(svg:first-child) { color: var(--text-muted); flex-shrink: 0; }
  .chip-name {
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip-remove {
    display: flex;
    background: none;
    border: none;
    color: var(--text-muted);
    padding: 0;
    cursor: pointer;
  }
  .chip-remove:hover { color: var(--text-primary); }

  /* ── Composer ─────────────────────────────────────────────── */
  .composer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px 10px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--bg-secondary);
  }
  .composer-input {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 8px;
    transition: border-color 0.12s;
  }
  .composer-input:focus-within {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  }
  .composer-input textarea {
    width: 100%;
    min-height: 20px;
    max-height: 200px;
    padding: 8px 10px;
    background: none;
    border: none;
    resize: none;
    font-family: var(--font-ui);
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--text-primary);
    outline: none;
    box-sizing: border-box;
    display: block;
  }
  .composer-input textarea::placeholder { color: var(--text-muted); }

  .composer-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }
  .composer-left { display: flex; align-items: center; gap: 2px; min-width: 0; }
  .composer-right { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }

  /* Model picker */
  .model-picker { position: relative; }
  .model-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 8px;
    background: none;
    border: 1px solid transparent;
    border-radius: 5px;
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s, color 0.12s;
    font-family: var(--font-ui);
    max-width: 220px;
  }
  .model-btn:hover {
    background: var(--bg-tertiary);
    border-color: var(--border);
    color: var(--text-primary);
  }
  .model-provider { color: var(--text-muted); font-weight: 500; }
  .model-sep { color: var(--text-muted); opacity: 0.6; }
  .model-name {
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .model-btn :global(svg) { color: var(--text-muted); flex-shrink: 0; }

  .model-menu {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    min-width: 220px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 4px;
    z-index: 100;
    box-shadow: 0 10px 30px rgba(0,0,0,0.35);
    max-height: 320px;
    overflow-y: auto;
  }
  .model-group { padding: 3px 0; }
  .model-group + .model-group {
    border-top: 1px solid var(--border);
    margin-top: 4px;
    padding-top: 6px;
  }
  .model-group-head {
    font-size: 9.5px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    font-weight: 600;
    color: var(--text-muted);
    padding: 3px 8px 4px;
  }
  .model-item {
    display: block;
    width: 100%;
    padding: 6px 8px;
    background: none;
    border: none;
    border-radius: 5px;
    color: var(--text-primary);
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
  }
  .model-item:hover { background: var(--bg-surface); }
  .model-item.active {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--text-primary);
    font-weight: 500;
  }

  /* Send / stop */
  .send-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px; height: 28px;
    border-radius: 6px;
    background: var(--accent);
    color: #fff;
    border: none;
    cursor: pointer;
    transition: opacity 0.12s, background 0.12s;
  }
  .send-btn:hover:not(:disabled) { opacity: 0.92; }
  .send-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .send-btn.stop { background: #d04545; }

  /* Resize handle */
  .resize-handle {
    position: absolute;
    bottom: 0;
    right: 0;
    width: 16px;
    height: 16px;
    cursor: nwse-resize;
  }
</style>
