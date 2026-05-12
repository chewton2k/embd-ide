<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import SectionHeader from '../components/SectionHeader.svelte';

  let projectRoot = $state<string | null>(null);
  let stats = $state<{ files: number; conversations: number; dbSize: string } | null>(null);
  let files = $state<{ path: string; language: string; exports: string }[]>([]);
  let conversations = $state<{ id: string; title: string; updated_at: number }[]>([]);
  let loading = $state(true);
  let message = $state('');

  onMount(async () => {
    // Get project root from localStorage (shared across windows)
    projectRoot = localStorage.getItem('leo-project-root');
    if (projectRoot) await loadData();
    loading = false;
  });

  async function loadData() {
    if (!projectRoot) return;
    try {
      await invoke('knowledge_init', { projectRoot });

      // Get conversations
      conversations = await invoke('knowledge_list_conversations', { projectRoot });

      // Get indexed files via context query (empty query returns all)
      files = await invoke('knowledge_get_context', { projectRoot, query: '', currentFile: null });

      // Estimate DB size
      const fileCount = files.length;
      const convCount = conversations.length;
      const estimatedKb = Math.round((fileCount * 0.5 + convCount * 2));
      stats = {
        files: fileCount,
        conversations: convCount,
        dbSize: estimatedKb > 1024 ? `${(estimatedKb / 1024).toFixed(1)} MB` : `${estimatedKb} KB`,
      };
    } catch (e) {
      stats = { files: 0, conversations: 0, dbSize: '0 KB' };
    }
  }

  async function clearConversations() {
    if (!projectRoot) return;
    try {
      await invoke('knowledge_delete_conversations', { projectRoot });
      conversations = [];
      message = 'All conversations deleted.';
      setTimeout(() => message = '', 3000);
      await loadData();
    } catch {}
  }

  async function reindexProject() {
    if (!projectRoot) return;
    message = 'Re-indexing...';
    try {
      await invoke('knowledge_index', { projectRoot });
      message = 'Re-index complete.';
      await loadData();
      setTimeout(() => message = '', 3000);
    } catch {
      message = 'Re-index failed.';
    }
  }
</script>

<div class="root">
  <SectionHeader
    title="Knowledge Graph"
    description="View and manage the AI's knowledge about your project. Data is stored locally in SQLite."
  />

  {#if loading}
    <div class="loading">Loading...</div>
  {:else if !projectRoot}
    <div class="empty">Open a project to see its knowledge graph.</div>
  {:else}
    <!-- Stats overview -->
    {#if stats}
      <div class="stats-grid">
        <div class="stat-card">
          <div class="stat-value">{stats.files}</div>
          <div class="stat-label">Indexed Files</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{stats.conversations}</div>
          <div class="stat-label">Conversations</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{stats.dbSize}</div>
          <div class="stat-label">Storage Used</div>
        </div>
      </div>
    {/if}

    <!-- Actions -->
    <div class="section">
      <div class="section-title">Actions</div>
      <div class="actions">
        <button class="btn" onclick={reindexProject} data-setting="knowledge-index">Re-index Project</button>
        <button class="btn btn-danger" onclick={clearConversations} data-setting="knowledge-conversations">Clear All Conversations</button>
      </div>
      {#if message}
        <div class="message">{message}</div>
      {/if}
    </div>

    <!-- Indexed files -->
    <div class="section">
      <div class="section-title">Indexed Files ({files.length})</div>
      <div class="file-list">
        {#each files as file}
          <div class="file-item">
            <span class="file-path">{file.path}</span>
            <span class="file-lang">{file.language}</span>
            {#if file.exports}
              <span class="file-exports">{file.exports}</span>
            {/if}
          </div>
        {/each}
        {#if files.length === 0}
          <div class="empty-list">No files indexed yet. Click "Re-index Project" to start.</div>
        {/if}
      </div>
    </div>

    <!-- Conversations -->
    <div class="section">
      <div class="section-title">Saved Conversations ({conversations.length})</div>
      <div class="conv-list">
        {#each conversations as conv}
          <div class="conv-item">
            <span class="conv-title">{conv.title}</span>
            <span class="conv-date">{new Date(conv.updated_at * 1000).toLocaleDateString()}</span>
          </div>
        {/each}
        {#if conversations.length === 0}
          <div class="empty-list">No conversations saved yet.</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .root { display: flex; flex-direction: column; gap: 24px; }

  .loading, .empty {
    color: var(--text-muted);
    font-size: 13px;
    padding: 20px 0;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }

  .stat-card {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px;
    text-align: center;
  }

  .stat-value {
    font-size: 22px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .stat-label {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .section { display: flex; flex-direction: column; gap: 8px; }

  .section-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .actions { display: flex; gap: 8px; }

  .btn {
    padding: 7px 14px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border);
  }

  .btn:hover { background: var(--border); }

  .btn-danger {
    color: var(--error, #f14c4c);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 30%, transparent);
  }

  .btn-danger:hover {
    background: color-mix(in srgb, var(--error, #f14c4c) 10%, transparent);
  }

  .message {
    font-size: 11px;
    color: var(--accent);
    padding: 4px 0;
  }

  .file-list, .conv-list {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 8px;
    max-height: 200px;
    overflow-y: auto;
  }

  .file-item, .conv-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }

  .file-item:last-child, .conv-item:last-child { border-bottom: none; }

  .file-path { flex: 1; color: var(--text-primary); font-family: var(--font-mono); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-lang { font-size: 10px; color: var(--text-muted); background: var(--bg-surface); padding: 1px 6px; border-radius: 4px; }
  .file-exports { font-size: 10px; color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 200px; }

  .conv-title { flex: 1; color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .conv-date { font-size: 10px; color: var(--text-muted); flex-shrink: 0; }

  .empty-list { padding: 16px; text-align: center; font-size: 11px; color: var(--text-muted); }
</style>
