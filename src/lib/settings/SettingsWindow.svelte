<script lang="ts">
  import { currentThemeId, getTheme, uiFontSize, uiDensity } from '../modules/stores';
  import GeneralSection   from './sections/GeneralSection.svelte';
  import ShortcutsSection from './sections/ShortcutsSection.svelte';
  import ModelsSection    from './sections/ModelsSection.svelte';
  import AgentsSection    from './sections/AgentsSection.svelte';
  import AboutSection     from './sections/AboutSection.svelte';

  type TabId = 'general' | 'shortcuts' | 'models' | 'agents' | 'about';

  const TABS: { id: TabId; label: string; icon: string }[] = [
    { id: 'general',   label: 'General',   icon: 'M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z M19.4 13a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V19a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1A2 2 0 1 1 4.3 15l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1A2 2 0 1 1 7 2.3l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V1a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1A2 2 0 1 1 19.7 5l-.1.1a1.7 1.7 0 0 0-.3 1.8V7a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z' },
    { id: 'shortcuts', label: 'Shortcuts', icon: 'M2 8h20v8H2z M6 12h.01 M10 12h.01 M14 12h.01 M18 12h.01' },
    { id: 'models',    label: 'Models',    icon: 'M12 2v4 M12 18v4 M4.93 4.93l2.83 2.83 M16.24 16.24l2.83 2.83 M2 12h4 M18 12h4 M4.93 19.07l2.83-2.83 M16.24 7.76l2.83-2.83 M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z' },
    { id: 'agents',    label: 'Agents',    icon: 'M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M3 21v-1a6 6 0 0 1 6-6 6 6 0 0 1 6 6v1 M17 8a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z M14 14a4 4 0 0 1 8 0' },
    { id: 'about',     label: 'About',     icon: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z M12 8v4 M12 16h.01' },
  ];

  function parseInitialTab(): TabId {
    const hash = window.location.hash.replace(/^#\/?settings\/?/, '').replace(/^#/, '');
    const found = TABS.find(t => t.id === hash);
    return found ? found.id : 'general';
  }

  let activeTab = $state<TabId>(parseInitialTab());

  // Apply current theme as CSS variables on the document so this window
  // looks identical to the main editor.
  $effect(() => {
    const theme = getTheme($currentThemeId);
    const root = document.documentElement;
    root.style.setProperty('--bg-primary',      theme.colors.bgPrimary);
    root.style.setProperty('--bg-secondary',    theme.colors.bgSecondary);
    root.style.setProperty('--bg-tertiary',     theme.colors.bgTertiary);
    root.style.setProperty('--bg-surface',      theme.colors.bgSurface);
    root.style.setProperty('--border',          theme.colors.border);
    root.style.setProperty('--text-primary',    theme.colors.textPrimary);
    root.style.setProperty('--text-secondary',  theme.colors.textSecondary);
    root.style.setProperty('--text-muted',      theme.colors.textMuted);
    root.style.setProperty('--accent',          theme.colors.accent);
    root.style.setProperty('--success',         theme.colors.success);
    root.style.setProperty('--warning',         theme.colors.warning);
    root.style.setProperty('--error',           theme.colors.error);
    document.body.style.background = theme.colors.bgPrimary;
    document.body.style.color = theme.colors.textPrimary;
  });

  $effect(() => {
    document.documentElement.style.fontSize = `${$uiFontSize}px`;
  });

  function selectTab(id: TabId) {
    activeTab = id;
    history.replaceState(null, '', `#settings/${id}`);
  }
</script>

<div class="root" class:compact={$uiDensity === 'compact'}>
  <aside class="sidebar">
    <div class="sidebar-title">Settings</div>
    <nav>
      {#each TABS as tab}
        <button
          class="nav-btn"
          class:active={activeTab === tab.id}
          onclick={() => selectTab(tab.id)}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" width="16" height="16">
            <path d={tab.icon} />
          </svg>
          <span>{tab.label}</span>
        </button>
      {/each}
    </nav>
  </aside>

  <main class="content">
    <div class="content-inner">
      {#if activeTab === 'general'}
        <GeneralSection />
      {:else if activeTab === 'shortcuts'}
        <ShortcutsSection />
      {:else if activeTab === 'models'}
        <ModelsSection />
      {:else if activeTab === 'agents'}
        <AgentsSection />
      {:else if activeTab === 'about'}
        <AboutSection />
      {/if}
    </div>
  </main>
</div>

<style>
  :global(html, body) {
    margin: 0; padding: 0;
    height: 100%;
    overflow: hidden;
    font-family: var(--font-ui);
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: optimizeLegibility;
  }
  :global(#app) { height: 100%; }
  :global(*) { box-sizing: border-box; }
  :global(button) { font-family: inherit; }

  .root {
    display: grid;
    grid-template-columns: 240px 1fr;
    height: 100vh;
    background: var(--bg-primary);
    color: var(--text-primary);
  }

  .sidebar {
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    padding: 22px 14px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .sidebar-title {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.2px;
    color: var(--text-primary);
    padding: 0 10px;
  }
  nav { display: flex; flex-direction: column; gap: 2px; }
  .nav-btn {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 12px;
    background: none;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text-secondary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .nav-btn:hover { background: var(--bg-surface); color: var(--text-primary); }
  .nav-btn.active {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--accent);
  }
  .nav-btn svg { flex-shrink: 0; }

  .content {
    overflow-y: auto;
    background: var(--bg-primary);
  }
  .content-inner {
    max-width: 720px;
    padding: 40px 44px 64px;
  }

  .compact .content-inner { padding: 28px 32px 44px; }
  .compact .nav-btn { padding: 6px 10px; font-size: 12px; }
</style>
