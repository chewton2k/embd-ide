<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { Network, List as ListIcon, RefreshCw, Trash2 } from 'lucide-svelte';
  import SectionHeader from '../components/SectionHeader.svelte';
  import KnowledgeGraphView from '../../components/knowledge/KnowledgeGraphView.svelte';
  import KnowledgeListView from '../../components/knowledge/KnowledgeListView.svelte';
  import ConversationViewer from '../../components/knowledge/ConversationViewer.svelte';
  import {
    listProjects, deleteProject, deleteAllKnowledge, formatBytes,
    type ProjectInfo, type ConversationSummary,
  } from '../../modules/knowledge';

  // ── State ────────────────────────────────────────────────────

  let projects = $state<ProjectInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let reloadKey = $state(0);
  let view = $state<'list' | 'graph'>('list');
  let message = $state('');
  let confirmAll = $state(false);

  let selectedConv = $state<{
    projectRoot: string;
    id: string;
    title: string;
    updatedAt: number;
  } | null>(null);

  // ── Derived totals ───────────────────────────────────────────

  const totals = $derived({
    projects: projects.length,
    conversations: projects.reduce((acc, p) => acc + p.conversation_count, 0),
    files: projects.reduce((acc, p) => acc + p.file_count, 0),
    bytes: projects.reduce((acc, p) => acc + p.db_size_bytes, 0),
  });

  // ── Data loading ─────────────────────────────────────────────

  onMount(() => { refresh(); });

  async function refresh() {
    loading = true;
    error = null;
    try {
      projects = await listProjects();
      reloadKey++;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function flashMessage(text: string) {
    message = text;
    setTimeout(() => { if (message === text) message = ''; }, 3000);
  }

  // ── Action handlers ──────────────────────────────────────────

  function handleOpenConversation(projectRoot: string, conv: ConversationSummary) {
    selectedConv = {
      projectRoot,
      id: conv.id,
      title: conv.title,
      updatedAt: conv.updated_at,
    };
  }

  async function handleDeleteProject(projectRoot: string) {
    try {
      await deleteProject(projectRoot);
      flashMessage('Project knowledge deleted.');
      await refresh();
    } catch (e) {
      error = `Delete failed: ${String(e)}`;
    }
  }

  async function handleDeleteAll() {
    if (!confirmAll) {
      confirmAll = true;
      return;
    }
    confirmAll = false;
    try {
      await deleteAllKnowledge();
      flashMessage('All knowledge deleted.');
      await refresh();
    } catch (e) {
      error = `Delete all failed: ${String(e)}`;
    }
  }

  function onConversationDeleted() {
    selectedConv = null;
    flashMessage('Chat deleted.');
    refresh();
  }
</script>

<div class="root">
  <SectionHeader
    title="Knowledge"
    description="Every project's indexed code and saved AI conversations. Stored locally in SQLite under ~/.leo-ide/knowledge."
  />

  <!-- Totals — stagger-animated cards with hover lift. -->
  <div class="totals" data-setting="knowledge-index">
    {#each [
      { value: totals.projects,       label: 'Projects' },
      { value: totals.conversations,  label: 'Conversations' },
      { value: totals.files,          label: 'Indexed files' },
      { value: formatBytes(totals.bytes), label: 'Disk used' },
    ] as stat, i (stat.label)}
      <div
        class="total-card"
        in:fly={{ y: 8, duration: 260, delay: 40 + i * 55, easing: cubicOut }}
      >
        <div class="total-value">{stat.value}</div>
        <div class="total-label">{stat.label}</div>
      </div>
    {/each}
  </div>

  <!-- Actions row -->
  <div class="actions">
    <!-- View toggle with a sliding accent indicator behind the active tab. -->
    <div class="view-toggle" role="tablist" aria-label="Knowledge view">
      <span class="view-indicator" class:on-graph={view === 'graph'} aria-hidden="true"></span>
      <button
        type="button"
        role="tab"
        aria-selected={view === 'list'}
        class="view-btn"
        class:active={view === 'list'}
        onclick={() => view = 'list'}
        title="List view"
      >
        <ListIcon size={12} /> List
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={view === 'graph'}
        class="view-btn"
        class:active={view === 'graph'}
        onclick={() => view = 'graph'}
        title="Graph view"
      >
        <Network size={12} /> Graph
      </button>
    </div>

    <button
      type="button"
      class="action-btn"
      onclick={refresh}
      disabled={loading}
      title="Refresh"
      aria-label="Refresh knowledge"
    >
      <RefreshCw size={12} class={loading ? 'spin' : ''} />
      Refresh
    </button>

    <div class="danger-group" data-setting="knowledge-conversations">
      {#if confirmAll}
        <div class="confirm-row" in:fly={{ x: 6, duration: 160 }}>
          <span class="confirm">Wipe every project's knowledge?</span>
          <button class="action-btn danger" onclick={handleDeleteAll}>Confirm</button>
          <button class="action-btn" onclick={() => confirmAll = false}>Cancel</button>
        </div>
      {:else}
        <button
          type="button"
          class="action-btn danger-outline"
          onclick={handleDeleteAll}
          disabled={projects.length === 0}
          title="Delete every project's knowledge DB"
          in:fade={{ duration: 120 }}
        >
          <Trash2 size={12} /> Delete all
        </button>
      {/if}
    </div>
  </div>

  <!-- Banners with smooth slide-in. -->
  {#if message}
    <div class="banner info" in:fly={{ y: -6, duration: 160 }} out:fade={{ duration: 120 }}>
      {message}
    </div>
  {/if}
  {#if error}
    <div class="banner error" in:fly={{ y: -6, duration: 160 }} out:fade={{ duration: 120 }}>
      {error}
    </div>
  {/if}

  <!-- Main view with fade-through transition between list and graph. -->
  <div class="view-stage">
    {#if loading && projects.length === 0}
      <div class="status" in:fade={{ duration: 140 }}>
        <div class="skel-bar"></div>
        <div class="skel-bar"></div>
        <div class="skel-bar"></div>
      </div>
    {:else if view === 'list'}
      {#key reloadKey + '-list'}
        <div class="view-wrap" in:fade={{ duration: 180, delay: 40 }} out:fade={{ duration: 120 }}>
          <KnowledgeListView
            projects={projects}
            onOpenConversation={handleOpenConversation}
            onDeleteProject={handleDeleteProject}
            onProjectChanged={refresh}
          />
        </div>
      {/key}
    {:else}
      {#key reloadKey + '-graph'}
        <div class="view-wrap" in:fade={{ duration: 180, delay: 40 }} out:fade={{ duration: 120 }}>
          <KnowledgeGraphView
            projects={projects}
            onOpenConversation={handleOpenConversation}
            onDeleteProject={handleDeleteProject}
          />
        </div>
      {/key}
    {/if}
  </div>
</div>

{#if selectedConv}
  <ConversationViewer
    projectRoot={selectedConv.projectRoot}
    conversationId={selectedConv.id}
    conversationTitle={selectedConv.title}
    updatedAt={selectedConv.updatedAt}
    onClose={() => (selectedConv = null)}
    onDeleted={onConversationDeleted}
  />
{/if}

<style>
  .root { display: flex; flex-direction: column; gap: 16px; }

  /* ── Totals ──────────────────────────────────────────── */
  .totals {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }
  @media (max-width: 640px) {
    .totals { grid-template-columns: repeat(2, 1fr); }
  }

  .total-card {
    position: relative;
    background:
      linear-gradient(135deg,
        color-mix(in srgb, var(--accent) 5%, transparent),
        transparent 65%),
      var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px 16px;
    overflow: hidden;
    transition:
      transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1),
      border-color 180ms ease,
      box-shadow 180ms ease;
  }
  .total-card::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: linear-gradient(135deg,
      color-mix(in srgb, var(--accent) 18%, transparent),
      transparent 60%);
    opacity: 0;
    transition: opacity 200ms ease;
    pointer-events: none;
  }
  .total-card:hover {
    transform: translateY(-2px);
    border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
    box-shadow: 0 8px 22px color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .total-card:hover::before { opacity: 1; }

  .total-value {
    font-size: 22px;
    font-weight: 700;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
    position: relative;
  }
  .total-label {
    margin-top: 4px;
    font-size: 10.5px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.4px;
    position: relative;
  }

  /* ── Actions row ─────────────────────────────────────── */
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  /* View toggle with sliding indicator. */
  .view-toggle {
    position: relative;
    display: inline-flex;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 3px;
  }
  .view-indicator {
    position: absolute;
    top: 3px;
    left: 3px;
    width: calc(50% - 3px);
    height: calc(100% - 6px);
    background: var(--bg-surface);
    border-radius: 5px;
    box-shadow:
      0 1px 2px rgba(0, 0, 0, 0.18),
      0 0 0 1px color-mix(in srgb, var(--border) 80%, transparent);
    transition: transform 240ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .view-indicator.on-graph { transform: translateX(100%); }

  .view-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 14px;
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 500;
    border-radius: 5px;
    cursor: pointer;
    transition: color 160ms ease;
    z-index: 1;
  }
  .view-btn:hover { color: var(--text-primary); }
  .view-btn.active { color: var(--text-primary); }

  /* Plain action buttons. */
  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 7px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    transition:
      background 140ms ease,
      border-color 140ms ease,
      color 140ms ease,
      transform 120ms ease;
  }
  .action-btn:hover:not(:disabled) {
    background: var(--bg-surface);
    transform: translateY(-1px);
  }
  .action-btn:active:not(:disabled) { transform: translateY(0); }
  .action-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .action-btn.danger {
    background: color-mix(in srgb, var(--error, #f14c4c) 20%, transparent);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 42%, transparent);
    color: var(--error, #f14c4c);
  }
  .action-btn.danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--error, #f14c4c) 30%, transparent);
  }
  .action-btn.danger-outline:hover:not(:disabled) {
    color: var(--error, #f14c4c);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 45%, var(--border));
    background: color-mix(in srgb, var(--error, #f14c4c) 10%, transparent);
  }

  .danger-group { margin-left: auto; }
  .confirm-row { display: inline-flex; align-items: center; gap: 6px; }
  .confirm {
    font-size: 11px;
    color: var(--warning, #d79921);
    font-weight: 500;
    white-space: nowrap;
  }

  /* ── Banners ─────────────────────────────────────────── */
  .banner {
    padding: 9px 12px;
    border-radius: 8px;
    font-size: 11.5px;
    border: 1px solid transparent;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .banner.info {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border-color: color-mix(in srgb, var(--accent) 32%, transparent);
    color: var(--text-primary);
  }
  .banner.error {
    background: color-mix(in srgb, var(--error, #f14c4c) 10%, transparent);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 40%, transparent);
    color: var(--error, #f14c4c);
  }

  /* ── View stage (for fade-through) ───────────────────── */
  .view-stage { position: relative; min-height: 120px; }
  /* Layer both outgoing/incoming views on top of each other so the
     crossfade doesn't cause layout shift. */
  .view-stage > .view-wrap { width: 100%; }
  .view-stage > .view-wrap:not(:last-child) { position: absolute; inset: 0; }

  /* ── Loading skeleton ────────────────────────────────── */
  .status {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 0;
  }
  .skel-bar {
    height: 48px;
    border-radius: 10px;
    background: linear-gradient(90deg,
      var(--bg-tertiary) 0%,
      color-mix(in srgb, var(--bg-tertiary) 80%, var(--bg-surface)) 50%,
      var(--bg-tertiary) 100%);
    background-size: 220% 100%;
    animation: skelShimmer 1.4s ease-in-out infinite;
  }
  .skel-bar:nth-child(2) { animation-delay: 0.1s; }
  .skel-bar:nth-child(3) { animation-delay: 0.2s; }
  @keyframes skelShimmer {
    0%   { background-position: 120% 0; }
    100% { background-position: -120% 0; }
  }

  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { from { transform: rotate(0); } to { transform: rotate(360deg); } }

  @media (prefers-reduced-motion: reduce) {
    .total-card:hover { transform: none; }
    .action-btn:hover { transform: none; }
    .skel-bar { animation: none; }
    .view-indicator { transition: none; }
  }
</style>
