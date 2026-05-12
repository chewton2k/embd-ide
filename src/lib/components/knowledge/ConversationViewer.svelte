<script lang="ts">
  import { onMount } from 'svelte';
  import { X, Trash2, FileText, Terminal as TerminalIcon, Search as SearchIcon, Pencil, AlertTriangle, CheckCircle2, ChevronDown, ChevronRight, Sparkles } from 'lucide-svelte';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import {
    loadConversation, deleteConversation, shortProjectName, formatRelativeTime,
    type KnowledgeMessage,
  } from '../../modules/knowledge';
  import {
    parseAssistantContent, parseUserContent, truncate, basename,
    type ChatBlock,
  } from '../../modules/ai/chatRenderer';

  // ── Props ─────────────────────────────────────────────────────

  interface Props {
    projectRoot: string;
    conversationId: string;
    conversationTitle: string;
    updatedAt: number;
    /** Called when the viewer should close (X button, backdrop click, Esc). */
    onClose: () => void;
    /** Called after a successful delete so the parent can refresh its list. */
    onDeleted: () => void;
  }

  let { projectRoot, conversationId, conversationTitle, updatedAt, onClose, onDeleted }: Props = $props();

  // ── State ─────────────────────────────────────────────────────

  let messages = $state<KnowledgeMessage[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let deleting = $state(false);
  let confirmDelete = $state(false);
  let expandedBlocks = $state<Record<string, boolean>>({});

  onMount(async () => {
    try {
      messages = await loadConversation(projectRoot, conversationId);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function toggleBlock(key: string) {
    expandedBlocks = { ...expandedBlocks, [key]: !expandedBlocks[key] };
  }

  async function handleDelete() {
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    deleting = true;
    try {
      await deleteConversation(projectRoot, conversationId);
      onDeleted();
    } catch (e) {
      error = `Failed to delete: ${String(e)}`;
      deleting = false;
      confirmDelete = false;
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  function renderProse(content: string): string {
    return DOMPurify.sanitize(marked.parse(content, { async: false }) as string);
  }

  // Reuse the icon + title logic from the chat renderer so this viewer
  // looks the same as the live chat.
  function blockIcon(b: ChatBlock) {
    switch (b.kind) {
      case 'tool-read':          return FileText;
      case 'tool-search':        return SearchIcon;
      case 'tool-run':           return TerminalIcon;
      case 'tool-run-dangerous': return AlertTriangle;
      case 'tool-edit':          return Pencil;
      case 'tool-result':        return CheckCircle2;
      case 'activity':           return Sparkles;
      case 'error':              return AlertTriangle;
      default:                   return Sparkles;
    }
  }

  function blockTitle(b: ChatBlock): string {
    switch (b.kind) {
      case 'tool-read':          return 'Read file';
      case 'tool-search':        return 'Search';
      case 'tool-run':           return 'Run command';
      case 'tool-run-dangerous': return 'Dangerous command';
      case 'tool-edit':          return 'Proposed edit';
      case 'tool-result':        return b.label || 'Result';
      case 'activity':           return b.label || 'Activity';
      case 'error':              return b.label || 'Error';
      default:                   return '';
    }
  }

  const parsed = $derived(
    messages.map((msg, i) => ({
      role: msg.role,
      blocks: msg.role === 'user'
        ? parseUserContent(msg.content)
        : msg.role === 'assistant'
          ? parseAssistantContent(msg.content)
          : [{ kind: 'prose' as const, text: msg.content }],
      index: i,
    }))
  );
</script>

<svelte:window onkeydown={handleKey} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" onclick={onClose} role="presentation">
  <div
    class="viewer"
    role="dialog"
    aria-modal="true"
    aria-labelledby="conv-title"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
  >
    <header class="head">
      <div class="head-title">
        <span class="title-text" id="conv-title" title={conversationTitle}>{conversationTitle}</span>
        <span class="title-meta">
          {shortProjectName(projectRoot)} · {formatRelativeTime(updatedAt)}
        </span>
      </div>
      <div class="head-actions">
        {#if confirmDelete}
          <span class="confirm-label">Delete this chat?</span>
          <button class="btn btn-danger" onclick={handleDelete} disabled={deleting}>
            {deleting ? 'Deleting…' : 'Confirm'}
          </button>
          <button class="btn" onclick={() => confirmDelete = false} disabled={deleting}>Cancel</button>
        {:else}
          <button class="icon-btn danger" onclick={handleDelete} title="Delete chat" aria-label="Delete chat">
            <Trash2 size={13} />
          </button>
          <button class="icon-btn" onclick={onClose} title="Close" aria-label="Close">
            <X size={14} />
          </button>
        {/if}
      </div>
    </header>

    <div class="body" role="log" aria-live="polite">
      {#if loading}
        <div class="status">Loading conversation…</div>
      {:else if error}
        <div class="status error">{error}</div>
      {:else if messages.length === 0}
        <div class="status">This conversation is empty.</div>
      {:else}
        {#each parsed as m (m.index)}
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
                    </button>
                  {/if}
                {/each}
              </div>
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.52);
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 40px 24px;
    animation: fade 0.12s ease-out;
  }
  @keyframes fade { from { opacity: 0; } to { opacity: 1; } }

  .viewer {
    display: flex;
    flex-direction: column;
    width: 100%;
    max-width: 780px;
    height: 100%;
    max-height: 820px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 24px 72px rgba(0, 0, 0, 0.5);
    overflow: hidden;
    font-family: var(--font-ui);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-tertiary);
    flex-shrink: 0;
  }
  .head-title {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title-text {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title-meta {
    font-size: 10.5px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .head-actions { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px; height: 24px;
    border: none;
    border-radius: 5px;
    color: var(--text-muted);
    background: none;
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }
  .icon-btn:hover { background: var(--bg-surface); color: var(--text-primary); }
  .icon-btn.danger:hover { color: var(--error, #f14c4c); }

  .confirm-label {
    font-size: 11px;
    color: var(--warning, #d79921);
    font-weight: 500;
  }
  .btn {
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 500;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-primary);
    cursor: pointer;
  }
  .btn:hover:not(:disabled) { background: var(--bg-tertiary); }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-danger {
    background: color-mix(in srgb, var(--error, #f14c4c) 18%, transparent);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 38%, transparent);
    color: var(--error, #f14c4c);
  }
  .btn-danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--error, #f14c4c) 28%, transparent);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    scrollbar-gutter: stable;
  }

  .status {
    text-align: center;
    color: var(--text-muted);
    font-size: 12px;
    padding: 40px 0;
  }
  .status.error { color: var(--error, #f14c4c); }

  /* Match FloatingChat styling as closely as possible so the viewer feels
     like a playback of the same conversation. */
  .row { display: flex; flex-direction: column; gap: 4px; }
  .row.user { align-items: flex-end; }
  .row.tool { align-items: stretch; }

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
  .prose :global(pre code) { background: none; border: none; padding: 0; font-size: 11.5px; }
  .prose :global(a) { color: var(--accent); text-decoration: none; }
  .prose :global(a:hover) { text-decoration: underline; }
  .prose :global(strong) { color: var(--text-primary); font-weight: 600; }
  .prose :global(blockquote) {
    border-left: 2px solid var(--border);
    padding-left: 10px;
    color: var(--text-secondary);
    margin: 6px 0;
  }

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
  .tool-card:disabled { cursor: default; }
  .tool-card.danger {
    border-color: color-mix(in srgb, var(--warning, #d79921) 45%, var(--border));
    background: color-mix(in srgb, var(--warning, #d79921) 8%, var(--bg-tertiary));
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
  .tool-head { display: flex; align-items: baseline; gap: 6px; flex: 1; min-width: 0; }
  .tool-kind {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
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
</style>
