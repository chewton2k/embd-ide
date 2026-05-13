<script lang="ts">
  import { appearanceMode, uiFontSize, uiDensity } from '../modules';
  import GeneralSection   from './sections/GeneralSection.svelte';
  import TerminalSection  from './sections/TerminalSection.svelte';
  import ShortcutsSection from './sections/ShortcutsSection.svelte';
  import ModelsSection    from './sections/ModelsSection.svelte';
  import AgentsSection    from './sections/AgentsSection.svelte';
  import KnowledgeSection from './sections/KnowledgeSection.svelte';
  import AboutSection     from './sections/AboutSection.svelte';
  import {
    SETTINGS_TABS,
    matchesQuery,
    searchSettings,
    type SettingsTabId,
    type SettingsSearchResult,
  } from './searchIndex';

  // ── State ─────────────────────────────────────────────────────────

  function parseInitialTab(): SettingsTabId {
    const hash = window.location.hash.replace(/^#\/?settings\/?/, '').replace(/^#/, '');
    const found = SETTINGS_TABS.find(t => t.id === hash);
    return found ? found.id : 'general';
  }

  let activeTab = $state<SettingsTabId>(parseInitialTab());
  let searchQuery = $state('');
  let contentEl = $state<HTMLElement | undefined>();

  // Tabs matching the query (used when the user types a tab name like "General").
  const filteredTabs = $derived(
    searchQuery.trim()
      ? SETTINGS_TABS.filter(t => matchesQuery(`${t.label} ${t.keywords}`, searchQuery))
      : SETTINGS_TABS
  );

  // Individual settings matching the query (the main search results).
  const settingResults = $derived<SettingsSearchResult[]>(searchSettings(searchQuery));

  // ── Navigation + highlight ───────────────────────────────────────

  const HIGHLIGHT_CLASS = 'setting-flash';
  const HIGHLIGHT_DURATION_MS = 1400;

  /** Wait for the next animation frame (after the tab's DOM has mounted). */
  function nextFrame(): Promise<void> {
    return new Promise(r => requestAnimationFrame(() => r()));
  }

  /**
   * Scroll the given setting into view inside the content pane and briefly
   * highlight it. Safe to call even if the anchor hasn't mounted yet — this
   * retries a couple of frames before giving up.
   */
  async function flashSetting(anchor: string) {
    if (!contentEl) return;
    let el: HTMLElement | null = null;
    // Retry across a few frames — the target section may not be mounted yet
    // immediately after switching tabs.
    for (let attempt = 0; attempt < 6 && !el; attempt++) {
      el = contentEl.querySelector<HTMLElement>(`[data-setting="${CSS.escape(anchor)}"]`);
      if (el) break;
      await nextFrame();
    }
    if (!el) return;
    el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    el.classList.remove(HIGHLIGHT_CLASS);
    // Force reflow so the class re-addition restarts the animation.
    void el.offsetWidth;
    el.classList.add(HIGHLIGHT_CLASS);
    window.setTimeout(() => el?.classList.remove(HIGHLIGHT_CLASS), HIGHLIGHT_DURATION_MS);
  }

  function selectTab(id: SettingsTabId) {
    activeTab = id;
    history.replaceState(null, '', `#settings/${id}`);
  }

  async function jumpToResult(result: SettingsSearchResult) {
    if (activeTab !== result.tab.id) selectTab(result.tab.id);
    searchQuery = ''; // Close the results list — the user navigated.
    await flashSetting(result.entry.anchor);
  }

  function handleSearchKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && settingResults.length > 0) {
      e.preventDefault();
      jumpToResult(settingResults[0]);
    } else if (e.key === 'Escape') {
      searchQuery = '';
    }
  }

  // Auto-select the first matching tab when search narrows results and the
  // user's active tab no longer appears in the list.
  $effect(() => {
    if (searchQuery.trim() && filteredTabs.length > 0 && !filteredTabs.find(t => t.id === activeTab)) {
      activeTab = filteredTabs[0].id;
    }
  });

  // ── Window theming / sizing ──────────────────────────────────────

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
</script>

<div class="root" class:compact={$uiDensity === 'compact'}>
  <aside class="sidebar">
    <div class="sidebar-title">Settings</div>
    <div class="search-box">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13">
        <circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" />
      </svg>
      <input
        type="text"
        placeholder="Search settings..."
        bind:value={searchQuery}
        onkeydown={handleSearchKey}
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
      />
    </div>

    {#if searchQuery.trim()}
      <!-- Fine-grained search results: individual settings that match. -->
      {#if settingResults.length === 0}
        <div class="no-results">No settings match "{searchQuery}"</div>
      {:else}
        <div class="result-list" role="listbox" aria-label="Matching settings">
          {#each settingResults as r (r.entry.anchor)}
            <button
              class="result"
              role="option"
              aria-selected={activeTab === r.tab.id}
              onclick={() => jumpToResult(r)}
              title={`${r.tab.label}${r.entry.group ? ' · ' + r.entry.group : ''} · ${r.entry.label}`}
            >
              <span class="result-label">{r.entry.label}</span>
              <span class="result-path">
                {r.tab.label}{#if r.entry.group && r.entry.group !== r.tab.label} · {r.entry.group}{/if}
              </span>
            </button>
          {/each}
        </div>
      {/if}

      {#if filteredTabs.length > 0}
        <div class="result-divider">Sections</div>
        <nav>
          {#each filteredTabs as tab (tab.id)}
            <button
              class="nav-btn"
              class:active={activeTab === tab.id}
              onclick={() => { selectTab(tab.id); searchQuery = ''; }}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" width="16" height="16">
                <path d={tab.icon} />
              </svg>
              <span>{tab.label}</span>
            </button>
          {/each}
        </nav>
      {/if}
    {:else}
      <nav>
        {#each SETTINGS_TABS as tab (tab.id)}
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
    {/if}
  </aside>

  <main class="content" bind:this={contentEl}>
    <div class="content-inner">
      {#if activeTab === 'general'}
        <GeneralSection />
      {:else if activeTab === 'terminal'}
        <TerminalSection />
      {:else if activeTab === 'shortcuts'}
        <ShortcutsSection />
      {:else if activeTab === 'models'}
        <ModelsSection />
      {:else if activeTab === 'agents'}
        <AgentsSection />
      {:else if activeTab === 'knowledge'}
        <KnowledgeSection />
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

  /* Highlight flash applied to the searched-for setting row. Scoped globally
     because individual section components render the [data-setting] targets
     in their own scopes. */
  :global(.setting-flash) {
    animation: settingFlashKeyframes 1.4s ease-out;
    border-radius: 8px;
  }
  @keyframes settingFlashKeyframes {
    0%   { box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 50%, transparent); background: color-mix(in srgb, var(--accent) 16%, transparent); }
    70%  { box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 20%, transparent); background: color-mix(in srgb, var(--accent) 8%, transparent); }
    100% { box-shadow: 0 0 0 0 transparent; background: transparent; }
  }

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
    transition: border-color 0.12s ease, background 0.12s ease;
  }
  .search-box:focus-within {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
    background: var(--bg-surface);
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

  .no-results {
    padding: 10px 12px;
    font-size: 11.5px;
    color: var(--text-muted);
    line-height: 1.5;
  }

  .result-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .result {
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: flex-start;
    text-align: left;
    padding: 7px 10px;
    background: none;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text-primary);
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease;
  }
  .result:hover {
    background: var(--bg-surface);
    border-color: color-mix(in srgb, var(--border) 70%, transparent);
  }
  .result:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 1px;
  }
  .result-label {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-primary);
    line-height: 1.2;
  }
  .result-path {
    font-size: 10.5px;
    color: var(--text-muted);
    line-height: 1.2;
  }

  .result-divider {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    color: var(--text-muted);
    padding: 6px 10px 2px;
    margin-top: 4px;
    border-top: 1px solid var(--border);
  }

  /*
   * Sidebar nav. Active state uses an accent left bar plus subtle
   * background tint — clearer than the previous border-only treatment,
   * and matches the visual weight of the Knowledge section's
   * highlighted controls.
   */
  .nav-btn {
    position: relative;
    display: flex; align-items: center; gap: 10px;
    padding: 8px 12px;
    background: none;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text-secondary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition:
      background 0.12s ease,
      color 0.12s ease,
      border-color 0.12s ease;
  }
  .nav-btn::before {
    content: '';
    position: absolute;
    left: 0;
    top: 6px;
    bottom: 6px;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
    opacity: 0;
    transition: opacity 0.12s ease;
  }
  .nav-btn:hover { background: var(--bg-surface); color: var(--text-primary); }
  .nav-btn:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 1px;
  }
  .nav-btn.active {
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-surface));
    border-color: color-mix(in srgb, var(--accent) 25%, transparent);
    color: var(--text-primary);
    font-weight: 600;
  }
  .nav-btn.active::before { opacity: 1; }
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
