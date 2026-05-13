<script lang="ts">
  import { fly, slide, fade } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { ChevronDown, ChevronRight, Trash2, FolderOpen, MessageSquare, File as FileIcon } from 'lucide-svelte';
  import {
    listConversations, deleteProjectConversations, deleteProjectByHash,
    shortProjectName, formatRelativeTime, formatBytes,
    type ProjectInfo, type ConversationSummary,
  } from '../../modules/knowledge';

  // ── Props ────────────────────────────────────────────────────

  interface Props {
    projects: ProjectInfo[];
    onOpenConversation: (projectRoot: string, conv: ConversationSummary) => void;
    onDeleteProject: (projectRoot: string) => void;
    /** Called after a batch-delete of one project's conversations. */
    onProjectChanged: () => void;
  }
  let { projects, onOpenConversation, onDeleteProject, onProjectChanged }: Props = $props();

  // ── State ────────────────────────────────────────────────────

  let expanded = $state<Record<string, boolean>>({});
  let conversations = $state<Record<string, ConversationSummary[]>>({});
  let loadingByProject = $state<Record<string, boolean>>({});
  let errorByProject = $state<Record<string, string>>({});
  let confirmClear = $state<Record<string, boolean>>({});
  let confirmDelete = $state<Record<string, boolean>>({});

  async function toggle(root: string) {
    if (expanded[root]) {
      expanded = { ...expanded, [root]: false };
      return;
    }
    expanded = { ...expanded, [root]: true };
    if (conversations[root] !== undefined) return;
    loadingByProject = { ...loadingByProject, [root]: true };
    try {
      const list = await listConversations(root);
      conversations = { ...conversations, [root]: list };
    } catch (e) {
      errorByProject = { ...errorByProject, [root]: String(e) };
    } finally {
      loadingByProject = { ...loadingByProject, [root]: false };
    }
  }

  async function confirmClearConversations(root: string) {
    if (!confirmClear[root]) {
      confirmClear = { ...confirmClear, [root]: true };
      return;
    }
    try {
      await deleteProjectConversations(root);
      conversations = { ...conversations, [root]: [] };
      confirmClear = { ...confirmClear, [root]: false };
      onProjectChanged();
    } catch (e) {
      errorByProject = { ...errorByProject, [root]: String(e) };
      confirmClear = { ...confirmClear, [root]: false };
    }
  }

  function requestDeleteProject(root: string, dbHash?: string) {
    const key = dbHash || root;
    if (!confirmDelete[key]) {
      confirmDelete = { ...confirmDelete, [key]: true };
      return;
    }
    confirmDelete = { ...confirmDelete, [key]: false };
    if (root === '(unknown)' && dbHash) {
      deleteProjectByHash(dbHash).then(() => onProjectChanged());
    } else {
      onDeleteProject(root);
    }
  }

  function cancelConfirms(key: string) {
    confirmClear = { ...confirmClear, [key]: false };
    confirmDelete = { ...confirmDelete, [key]: false };
  }

  function handleRowKey(e: KeyboardEvent, root: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggle(root);
    }
  }
</script>

{#if projects.length === 0}
  <div class="empty" in:fade={{ duration: 180 }}>
    <div class="empty-icon"><FolderOpen size={28} /></div>
    <p class="empty-title">No knowledge indexed yet</p>
    <p class="empty-hint">Open a project and chat with the AI — conversations will appear here.</p>
  </div>
{:else}
  <div class="list" role="list">
    {#each projects as project, i (project.db_hash)}
      {@const root = project.project_root}
      {@const isExpanded = !!expanded[root]}
      {@const convs = conversations[root] ?? []}

      <!-- Project row, animates in with stagger. -->
      <div
        class="project"
        class:expanded={isExpanded}
        role="listitem"
        in:fly={{ y: 8, duration: 240, delay: 30 + i * 50, easing: cubicOut }}
      >
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="project-head"
          role="button"
          tabindex="0"
          aria-expanded={isExpanded}
          onclick={() => toggle(root)}
          onkeydown={(e) => handleRowKey(e, root)}
        >
          <span class="chev" aria-hidden="true">
            {#if isExpanded}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
          </span>

          <span class="project-name" title={root}>{shortProjectName(root)}</span>

          <span class="project-meta">
            <span class="pill"><MessageSquare size={10} /> {project.conversation_count}</span>
            <span class="pill"><FileIcon size={10} /> {project.file_count}</span>
            <span class="pill">{formatBytes(project.db_size_bytes)}</span>
            <span class="meta-time">{formatRelativeTime(project.last_updated)}</span>
          </span>

          <div
            class="project-actions"
            role="toolbar"
            aria-label="Project actions"
            tabindex="-1"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
          >
            {#if confirmDelete[project.db_hash]}
              <div class="confirm-group" in:fly={{ x: 6, duration: 140 }}>
                <span class="confirm">Delete project?</span>
                <button class="mini-btn danger" onclick={() => requestDeleteProject(root, project.db_hash)}>Confirm</button>
                <button class="mini-btn" onclick={() => cancelConfirms(project.db_hash)}>Cancel</button>
              </div>
            {:else if confirmClear[root]}
              <div class="confirm-group" in:fly={{ x: 6, duration: 140 }}>
                <span class="confirm">
                  Delete {project.conversation_count} chat{project.conversation_count === 1 ? '' : 's'}?
                </span>
                <button class="mini-btn danger" onclick={() => confirmClearConversations(root)}>Confirm</button>
                <button class="mini-btn" onclick={() => cancelConfirms(project.db_hash)}>Cancel</button>
              </div>
            {:else}
              <div class="action-group" in:fade={{ duration: 120 }}>
                {#if project.conversation_count > 0}
                  <button
                    type="button"
                    class="mini-btn"
                    onclick={() => confirmClearConversations(root)}
                    title="Delete all conversations in this project"
                    aria-label="Delete all conversations in {shortProjectName(root)}"
                  >
                    Clear chats
                  </button>
                {/if}
                <button
                  type="button"
                  class="mini-btn danger-outline"
                  onclick={() => requestDeleteProject(root, project.db_hash)}
                  title="Delete this project's knowledge DB"
                  aria-label="Delete {shortProjectName(root)}"
                >
                  <Trash2 size={11} /> Delete
                </button>
              </div>
            {/if}
          </div>
        </div>

        {#if isExpanded}
          <div
            class="conv-list"
            transition:slide={{ duration: 220, easing: cubicOut }}
          >
            {#if loadingByProject[root]}
              <div class="conv-empty">
                <div class="mini-skeleton"><span></span><span></span><span></span></div>
                <span>Loading conversations…</span>
              </div>
            {:else if errorByProject[root]}
              <div class="conv-empty error">{errorByProject[root]}</div>
            {:else if convs.length === 0}
              <div class="conv-empty">No conversations saved for this project yet.</div>
            {:else}
              {#each convs as conv, ci (conv.id)}
                <div
                  class="conv-item"
                  in:fly={{ y: 4, duration: 200, delay: ci * 25, easing: cubicOut }}
                >
                  <button
                    type="button"
                    class="conv-open"
                    onclick={() => onOpenConversation(root, conv)}
                    title={conv.title}
                  >
                    <span class="conv-dot" aria-hidden="true"></span>
                    <MessageSquare size={11} />
                    <span class="conv-title">{conv.title}</span>
                    <span class="conv-time">{formatRelativeTime(conv.updated_at)}</span>
                  </button>
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  /* ── Empty state ─────────────────────────────────────── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 56px 20px;
    color: var(--text-muted);
    text-align: center;
  }
  .empty-icon {
    width: 52px; height: 52px;
    border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--accent);
    animation: emptyBreathe 3s ease-in-out infinite;
  }
  @keyframes emptyBreathe {
    0%, 100% { transform: scale(1); opacity: 0.9; }
    50%      { transform: scale(1.05); opacity: 1; }
  }
  .empty-title { margin: 0; font-size: 13px; color: var(--text-secondary); font-weight: 500; }
  .empty-hint  { margin: 0; font-size: 11.5px; max-width: 340px; line-height: 1.5; }

  /* ── List ────────────────────────────────────────────── */
  .list { display: flex; flex-direction: column; gap: 10px; }

  .project {
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-tertiary);
    overflow: hidden;
    transition:
      background 180ms ease,
      border-color 180ms ease,
      box-shadow 180ms ease,
      transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .project:hover {
    border-color: color-mix(in srgb, var(--accent) 28%, var(--border));
    box-shadow: 0 6px 18px color-mix(in srgb, var(--accent) 6%, transparent);
    transform: translateY(-1px);
  }
  .project.expanded {
    background: var(--bg-secondary);
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
    box-shadow: 0 8px 22px color-mix(in srgb, var(--accent) 8%, transparent);
  }

  /* ── Row head ────────────────────────────────────────── */
  .project-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 12px;
    cursor: pointer;
    user-select: none;
    transition: background 140ms ease;
  }
  .project-head:hover { background: color-mix(in srgb, var(--bg-surface) 60%, transparent); }
  .project-head:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: -2px;
  }

  .chev {
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    flex-shrink: 0;
    transition: color 140ms ease, transform 200ms ease;
  }
  .project.expanded .chev { color: var(--accent); }

  .project-name {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .project-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    flex-shrink: 0;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 7px;
    border-radius: 12px;
    background: var(--bg-surface);
    color: var(--text-muted);
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    border: 1px solid transparent;
    transition: background 140ms ease, color 140ms ease, border-color 140ms ease;
  }
  .project.expanded .pill {
    border-color: color-mix(in srgb, var(--border) 50%, transparent);
  }
  .meta-time {
    font-size: 10.5px;
    color: var(--text-muted);
    margin-left: 4px;
    font-variant-numeric: tabular-nums;
  }

  /* ── Action buttons ──────────────────────────────────── */
  .project-actions {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    min-width: 0;
  }
  .action-group,
  .confirm-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .confirm {
    font-size: 11px;
    color: var(--warning, #d79921);
    font-weight: 500;
    white-space: nowrap;
  }

  .mini-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 9px;
    font-size: 11px;
    font-weight: 500;
    border-radius: 6px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background 140ms ease,
      color 140ms ease,
      border-color 140ms ease,
      transform 120ms ease;
  }
  .mini-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    transform: translateY(-1px);
  }
  .mini-btn:active { transform: translateY(0); }

  .mini-btn.danger {
    background: color-mix(in srgb, var(--error, #f14c4c) 20%, transparent);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 42%, transparent);
    color: var(--error, #f14c4c);
  }
  .mini-btn.danger:hover {
    background: color-mix(in srgb, var(--error, #f14c4c) 30%, transparent);
  }
  .mini-btn.danger-outline { color: var(--text-muted); }
  .mini-btn.danger-outline:hover {
    color: var(--error, #f14c4c);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 45%, var(--border));
    background: color-mix(in srgb, var(--error, #f14c4c) 10%, transparent);
  }

  /* ── Conversation list (expanded) ────────────────────── */
  .conv-list {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
    background:
      linear-gradient(to bottom, color-mix(in srgb, var(--accent) 3%, transparent), transparent 60%),
      var(--bg-secondary);
  }
  .conv-item {
    border-top: 1px solid color-mix(in srgb, var(--border) 35%, transparent);
  }
  .conv-item:first-child { border-top: none; }

  .conv-open {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 14px 9px 34px; /* align with chev column */
    background: none;
    border: none;
    color: var(--text-primary);
    text-align: left;
    font-size: 12px;
    cursor: pointer;
    transition: background 120ms ease, padding 180ms ease;
  }
  .conv-open:hover {
    background: color-mix(in srgb, var(--bg-surface) 70%, transparent);
    padding-left: 38px;
  }
  .conv-dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--accent);
    opacity: 0.5;
    flex-shrink: 0;
    transition: opacity 140ms ease, transform 140ms ease;
  }
  .conv-open:hover .conv-dot {
    opacity: 1;
    transform: scale(1.3);
  }
  .conv-open :global(svg:first-of-type) { color: var(--text-muted); flex-shrink: 0; }
  .conv-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .conv-time {
    color: var(--text-muted);
    font-size: 10.5px;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .conv-empty {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px 12px 34px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .conv-empty.error { color: var(--error, #f14c4c); }

  .mini-skeleton {
    display: inline-flex;
    gap: 4px;
  }
  .mini-skeleton span {
    width: 5px; height: 5px;
    border-radius: 50%;
    background: var(--text-muted);
    opacity: 0.35;
    animation: skeletonPulse 1.2s infinite ease-in-out;
  }
  .mini-skeleton span:nth-child(2) { animation-delay: 0.15s; }
  .mini-skeleton span:nth-child(3) { animation-delay: 0.3s; }
  @keyframes skeletonPulse {
    0%, 80%, 100% { opacity: 0.2; transform: scale(0.8); }
    40%           { opacity: 0.95; transform: scale(1); }
  }

  @media (prefers-reduced-motion: reduce) {
    .empty-icon { animation: none; }
    .project { transition: none; }
    .project:hover { transform: none; }
    .mini-btn:hover { transform: none; }
    .conv-open:hover { padding-left: 34px; }
  }
</style>
