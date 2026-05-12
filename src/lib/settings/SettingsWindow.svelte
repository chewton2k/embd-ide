<script lang="ts">
  import { appearanceMode, uiFontSize, uiDensity } from '../modules/stores';
  import GeneralSection   from './sections/GeneralSection.svelte';
  import ShortcutsSection from './sections/ShortcutsSection.svelte';
  import ModelsSection    from './sections/ModelsSection.svelte';
  import AgentsSection    from './sections/AgentsSection.svelte';
  import AboutSection     from './sections/AboutSection.svelte';

  type TabId = 'general' | 'shortcuts' | 'models' | 'agents' | 'about';

  const TABS: { id: TabId; label: string; icon: string; keywords: string }[] = [
    { id: 'general',   label: 'General',   keywords: 'appearance theme editor font tab autosave density hidden patterns terminal', icon: 'M4 21v-7 M4 10V3 M12 21v-9 M12 8V3 M20 21v-5 M20 12V3 M1 14h6 M9 8h6 M17 16h6' },
    { id: 'shortcuts', label: 'Shortcuts', keywords: 'keyboard keybindings hotkeys', icon: 'M2 8h20v8H2z M6 12h.01 M10 12h.01 M14 12h.01 M18 12h.01' },
    { id: 'models',    label: 'Models',    keywords: 'ai api key openrouter openai anthropic provider', icon: 'M12 2v4 M12 18v4 M4.93 4.93l2.83 2.83 M16.24 16.24l2.83 2.83 M2 12h4 M18 12h4 M4.93 19.07l2.83-2.83 M16.24 7.76l2.83-2.83 M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z' },
    { id: 'agents',    label: 'Agents',    keywords: 'assistant chat', icon: 'M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M3 21v-1a6 6 0 0 1 6-6 6 6 0 0 1 6 6v1 M17 8a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z M14 14a4 4 0 0 1 8 0' },
    { id: 'about',     label: 'About',     keywords: 'version info', icon: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z M12 8v4 M12 16h.01' },
  ];

  function parseInitialTab(): TabId {
    const hash = window.location.hash.replace(/^#\/?settings\/?/, '').replace(/^#/, '');
    const found = TABS.find(t => t.id === hash);
    return found ? found.id : 'general';
  }

  let activeTab = $state<TabId>(parseInitialTab());
  let searchQuery = $state('');

  const filteredTabs = $derived(
    searchQuery.trim()
      ? TABS.filter(t => {
          const q = searchQuery.toLowerCase();
          return t.label.toLowerCase().includes(q) || t.keywords.includes(q);
        })
      : TABS
  );

  // Auto-select first matching tab when search narrows results
  $effect(() => {
    if (searchQuery.trim() && filteredTabs.length > 0 && !filteredTabs.find(t => t.id === activeTab)) {
      activeTab = filteredTabs[0].id;
    }
  });

  // Apply appearance mode to this window
  $effect(() => {
    const mode = $appearanceMode;
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    if (mode === 'light') root.classList.add('light');
    else if (mode === 'dark') root.classList.add('dark');
    else {
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      root.classList.add(prefersDark ? 'dark' : 'light');
    }
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
    <div class="search-box">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13">
        <circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" />
      </svg>
      <input type="text" placeholder="Search settings..." bind:value={searchQuery} />
    </div>
    <nav>
      {#each filteredTabs as tab}
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

  .search-box {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--text-muted);
  }
  .search-box input {
    flex: 1;
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    color: var(--text-primary);
    outline: none;
  }
  .search-box input::placeholder { color: var(--text-muted); }
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
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-surface));
    border-color: color-mix(in srgb, var(--accent) 25%, transparent);
    color: var(--text-primary);
    font-weight: 600;
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
