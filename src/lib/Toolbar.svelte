<script lang="ts">
  import Tabs from './Tabs.svelte';
  import { showTerminal, showSettings, autosaveEnabled, showChat, showGit, gitBranch, triggerSearchInFile, toggleChatPanel, toggleGitPanel } from './stores';

  let { sidebarVisible, onToggleSidebar }: {
    sidebarVisible: boolean;
    onToggleSidebar: () => void;
  } = $props();

  function triggerSearch() {
    triggerSearchInFile.update(n => n + 1);
  }
</script>

<div class="toolbar">
  <button
    type="button"
    class="toolbar-btn sidebar-toggle"
    class:active={sidebarVisible}
    onclick={onToggleSidebar}
    title="Toggle sidebar"
    aria-label="Toggle sidebar"
  >
    <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
      <rect x="1" y="1" width="4" height="14" rx="1" opacity={sidebarVisible ? 1 : 0.4}/>
      <rect x="7" y="1" width="8" height="14" rx="1"/>
    </svg>
  </button>

  <div class="tabs-wrapper">
    <Tabs />
  </div>

  <div class="toolbar-right">
    <button type="button" class="toolbar-search-btn" onclick={triggerSearch} title="Search in file (Cmd/Ctrl+F)">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="12" height="12">
        <circle cx="7" cy="7" r="4.5"/>
        <path d="M10.5 10.5L14 14"/>
      </svg>
      <span>Search in file</span>
    </button>

    <div class="toolbar-divider"></div>

    <button
      type="button"
      class="toolbar-btn"
      class:active={$showTerminal}
      onclick={() => showTerminal.update(v => !v)}
      title="Toggle terminal (Ctrl+`)"
      aria-label="Toggle terminal"
      aria-pressed={$showTerminal}
    >
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="13" height="13">
        <rect x="1" y="2" width="14" height="12" rx="2"/>
        <path d="M4 6.5l3 2-3 2"/>
        <path d="M9 10.5h3"/>
      </svg>
    </button>

    <button
      type="button"
      class="toolbar-btn"
      class:active={$showChat}
      onclick={toggleChatPanel}
      title="Toggle AI chat (Ctrl+L)"
      aria-label="Toggle AI chat"
      aria-pressed={$showChat}
    >
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="13" height="13">
        <path d="M14 5H2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1h2v2l3-2h7a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1z"/>
      </svg>
    </button>

    <button
      type="button"
      class="toolbar-btn"
      class:active={$showGit}
      onclick={toggleGitPanel}
      title="Toggle source control (Ctrl+G)"
      aria-label="Toggle source control"
      aria-pressed={$showGit}
    >
      <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
        <path d="M14.7 7.3L8.7 1.3a1 1 0 0 0-1.4 0L5.7 2.9l1.8 1.8A1.2 1.2 0 0 1 9 5.9v4.3a1.2 1.2 0 1 1-1-.1V6.1L6.3 7.8a1.2 1.2 0 1 1-.9-.5l1.8-1.8-1.8-1.8L1.3 7.3a1 1 0 0 0 0 1.4l6 6a1 1 0 0 0 1.4 0l6-6a1 1 0 0 0 0-1.4z"/>
      </svg>
      {#if $gitBranch}
        <span class="branch-label">{$gitBranch}</span>
      {/if}
    </button>

    <button
      type="button"
      class="toolbar-btn autosave-btn"
      class:active={$autosaveEnabled}
      onclick={() => autosaveEnabled.update(v => !v)}
      title="Toggle autosave"
      aria-label="Toggle autosave"
      aria-pressed={$autosaveEnabled}
    >
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="13" height="13">
        <path d="M13 5l-5 5-2-2"/>
        <rect x="1" y="1" width="14" height="14" rx="2"/>
      </svg>
    </button>

    <button
      type="button"
      class="toolbar-btn gear-btn"
      class:active={$showSettings}
      onclick={() => showSettings.update(v => !v)}
      title="Settings"
      aria-label="Settings"
      aria-pressed={$showSettings}
    >
      <svg viewBox="0 0 16 14" fill="currentColor" width="13" height="13">
        <path d="M8 1l1.3.8.8-.5 1 1-.5.8.5 1H12.5v1.4l-.8.5.2 1 .9.5-.3 1.2-1 .1-.3 1 .6.8-.7 1.1-1-.3-.7.8.1 1L8 13l-1.3-.8-.8.5-1-1 .5-.8-.5-1H3.5V8.5l.8-.5-.2-1-.9-.5.3-1.2 1-.1.3-1-.6-.8.7-1.1 1 .3.7-.8L6.5 2 8 1zm0 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z"/>
      </svg>
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    height: var(--density-tabs-height, 36px);
    overflow: hidden;
    flex-shrink: 0;
  }

  .tabs-wrapper {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
  }

  /* Remove the tabs-bar's own bottom border — the toolbar provides it */
  .tabs-wrapper :global(.tabs-bar) {
    border-bottom: none;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 8px;
    flex-shrink: 0;
    height: 100%;
  }

  .toolbar-divider {
    width: 1px;
    height: 16px;
    background: var(--border);
    margin: 0 4px;
    flex-shrink: 0;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 11px;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }

  .toolbar-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .toolbar-btn.active {
    color: var(--accent);
  }

  .sidebar-toggle {
    padding: 4px 8px;
    margin-left: 4px;
    border-right: 1px solid var(--border);
    border-radius: 0;
    height: 100%;
  }

  .toolbar-search-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.1s, color 0.1s;
    margin-right: 6px;
  }

  .toolbar-search-btn:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .branch-label {
    font-size: 11px;
    max-width: 80px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

</style>
