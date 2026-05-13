<script lang="ts">
  import {
    appearanceMode, editorTheme, uiFontSize, uiDensity,
    editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers,
    editorShowErrorLens,
    previewUrl,
    autosaveEnabled, autosaveDelay,
    maxRecentProjects, maxTabs,
    hiddenPatterns,
    EDITOR_THEMES, EDITOR_THEME_LABELS,
    type AppearanceMode, type EditorThemeId,
  } from '../../modules/stores';
  import { save, open } from '@tauri-apps/plugin-dialog';
  import { writeTextFile, readTextFile } from '@tauri-apps/plugin-fs';
  import SectionHeader from '../components/SectionHeader.svelte';

  let newPattern = $state('');
  let exportImportStatus = $state('');

  function addPattern() {
    const pat = newPattern.trim();
    if (!pat) return;
    if ($hiddenPatterns.some(p => p.pattern === pat)) { newPattern = ''; return; }
    hiddenPatterns.update(p => [...p, { pattern: pat, enabled: true }]);
    newPattern = '';
  }
  function removePattern(pat: string) {
    hiddenPatterns.update(p => p.filter(x => x.pattern !== pat));
  }
  function togglePattern(pat: string) {
    hiddenPatterns.update(p => p.map(x => x.pattern === pat ? { ...x, enabled: !x.enabled } : x));
  }

  const AUTOSAVE_DELAYS = [
    { label: '0.5s', value: 500 },
    { label: '1s',   value: 1000 },
    { label: '2s',   value: 2000 },
    { label: '5s',   value: 5000 },
  ];

  const EXPORT_KEYS = [
    'leo-autosave', 'leo-autosave-delay',
    'leo-editor-font-size', 'leo-editor-tab-size', 'leo-editor-word-wrap', 'leo-editor-line-numbers',
    'leo-editor-show-error-lens',
    'leo-terminal-font-size',
    'leo-terminal-mode', 'leo-terminal-panel-height',
    'leo-appearance', 'leo-editor-theme',
    'leo-ui-font-size', 'leo-ui-density',
    'leo-hidden-patterns',
    'leo-max-recent-projects', 'leo-max-tabs',
  ];

  async function exportSettings() {
    const data: Record<string, string> = {};
    for (const k of EXPORT_KEYS) {
      const v = localStorage.getItem(k);
      if (v !== null) data[k] = v;
    }
    const path = await save({ defaultPath: 'leo-settings.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!path) return;
    await writeTextFile(path, JSON.stringify(data, null, 2));
    exportImportStatus = 'Settings exported';
    setTimeout(() => exportImportStatus = '', 3000);
  }

  async function importSettings() {
    const path = await open({ filters: [{ name: 'JSON', extensions: ['json'] }], multiple: false, directory: false });
    if (!path) return;
    const data = JSON.parse(await readTextFile(path as string)) as Record<string, string>;
    for (const [k, v] of Object.entries(data)) {
      if (!k.startsWith('leo-') || k === 'leo-api-key') continue;
      localStorage.setItem(k, v);
    }
    location.reload();
  }
</script>

<div class="root">
  <SectionHeader
    title="General"
    description="Appearance, fonts, editor behavior, and app-wide preferences."
  />

  <!-- Appearance -->
  <div class="card" data-setting="appearance">
    <div class="card-head">
      <div class="card-title">Appearance</div>
      <div class="card-sub">Controls the IDE chrome. Editor colors are set separately below.</div>
    </div>
    <div class="appearance-grid">
      <button class="appearance-card" class:active={$appearanceMode === 'system'} onclick={() => appearanceMode.set('system')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="22" height="22">
          <rect x="2" y="3" width="20" height="14" rx="2" /><path d="M8 21h8 M12 17v4" />
        </svg>
        <span>System</span>
      </button>
      <button class="appearance-card" class:active={$appearanceMode === 'light'} onclick={() => appearanceMode.set('light')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="22" height="22">
          <circle cx="12" cy="12" r="5" /><path d="M12 1v2 M12 21v2 M4.22 4.22l1.42 1.42 M18.36 18.36l1.42 1.42 M1 12h2 M21 12h2 M4.22 19.78l1.42-1.42 M18.36 5.64l1.42-1.42" />
        </svg>
        <span>Light</span>
      </button>
      <button class="appearance-card" class:active={$appearanceMode === 'dark'} onclick={() => appearanceMode.set('dark')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="22" height="22">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79Z" />
        </svg>
        <span>Dark</span>
      </button>
    </div>
  </div>

  <!-- Editor Theme -->
  <div class="card" data-setting="editor-theme">
    <div class="card-head">
      <div class="card-title">Editor theme</div>
      <div class="card-sub">Syntax highlighting and editor background.</div>
    </div>
    <div class="rows" style="margin-top: 8px;">
      <select class="select" value={$editorTheme} onchange={(e) => editorTheme.set((e.currentTarget as HTMLSelectElement).value as EditorThemeId)}>
        {#each EDITOR_THEMES as t}
          <option value={t}>{EDITOR_THEME_LABELS[t]}</option>
        {/each}
      </select>
    </div>
  </div>

  <!-- Interface -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Interface</div>
    </div>
    <div class="rows">
      <div class="row" data-setting="ui-font-size">
        <div class="row-info">
          <div class="row-label">UI font size</div>
          <div class="row-help">Size of all in-app text.</div>
        </div>
        <div class="stepper">
          <button class="step-btn" onclick={() => uiFontSize.update(v => Math.max(11, v - 1))}>−</button>
          <span class="step-val">{$uiFontSize}px</span>
          <button class="step-btn" onclick={() => uiFontSize.update(v => Math.min(18, v + 1))}>+</button>
        </div>
      </div>
      <div class="row" data-setting="ui-density">
        <div class="row-info">
          <div class="row-label">Density</div>
          <div class="row-help">Vertical padding throughout the UI.</div>
        </div>
        <div class="pills">
          <button class="pill" class:active={$uiDensity === 'compact'} onclick={() => uiDensity.set('compact')}>Compact</button>
          <button class="pill" class:active={$uiDensity === 'comfortable'} onclick={() => uiDensity.set('comfortable')}>Comfortable</button>
        </div>
      </div>
    </div>
  </div>

  <!-- Editor -->
  <div class="card">
    <div class="card-head"><div class="card-title">Editor</div></div>
    <div class="rows">
      <div class="row" data-setting="editor-font-size">
        <div class="row-info"><div class="row-label">Font size</div></div>
        <div class="stepper">
          <button class="step-btn" onclick={() => editorFontSize.update(v => Math.max(10, v - 1))}>−</button>
          <span class="step-val">{$editorFontSize}px</span>
          <button class="step-btn" onclick={() => editorFontSize.update(v => Math.min(24, v + 1))}>+</button>
        </div>
      </div>
      <div class="row" data-setting="editor-tab-size">
        <div class="row-info"><div class="row-label">Tab size</div></div>
        <div class="pills">
          {#each [2, 4] as s}
            <button class="pill" class:active={$editorTabSize === s} onclick={() => editorTabSize.set(s)}>{s}</button>
          {/each}
        </div>
      </div>
      <div class="row" data-setting="editor-word-wrap">
        <div class="row-info"><div class="row-label">Word wrap</div></div>
        <button class="toggle" class:active={$editorWordWrap} onclick={() => editorWordWrap.update(v => !v)} aria-label="Toggle word wrap">
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>
      <div class="row" data-setting="editor-line-numbers">
        <div class="row-info"><div class="row-label">Line numbers</div></div>
        <button class="toggle" class:active={$editorLineNumbers} onclick={() => editorLineNumbers.update(v => !v)} aria-label="Toggle line numbers">
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>
      <div class="row" data-setting="editor-error-lens">
        <div class="row-info">
          <div class="row-label">Error lens</div>
          <div class="row-help">Inline syntax-error hints next to lines (JS, TS, C-family).</div>
        </div>
        <button class="toggle" class:active={$editorShowErrorLens} onclick={() => editorShowErrorLens.update(v => !v)} aria-label="Toggle error lens">
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>
    </div>
  </div>

  <!-- Preview -->
  <div class="card">
    <div class="card-head"><div class="card-title">Preview</div></div>
    <div class="rows">
      <div class="row" data-setting="preview-default-url">
        <div class="row-info">
          <div class="row-label">Default URL</div>
          <div class="row-help">The URL loaded when you open a new Preview tab.</div>
        </div>
        <input class="text-input" type="text" value={$previewUrl} onchange={(e) => previewUrl.set((e.currentTarget as HTMLInputElement).value)} spellcheck="false" />
      </div>
    </div>
  </div>

  <!-- Autosave -->
  <div class="card">
    <div class="card-head"><div class="card-title">Autosave</div></div>
    <div class="rows">
      <div class="row" data-setting="autosave-enabled">
        <div class="row-info">
          <div class="row-label">Enabled</div>
          <div class="row-help">Save modified files automatically after a short delay.</div>
        </div>
        <button class="toggle" class:active={$autosaveEnabled} onclick={() => autosaveEnabled.update(v => !v)} aria-label="Toggle autosave">
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>
      <div class="row" data-setting="autosave-delay">
        <div class="row-info"><div class="row-label">Delay</div></div>
        <div class="pills">
          {#each AUTOSAVE_DELAYS as opt}
            <button class="pill" class:active={$autosaveDelay === opt.value} onclick={() => autosaveDelay.set(opt.value)}>{opt.label}</button>
          {/each}
        </div>
      </div>
    </div>
  </div>

  <!-- Session -->
  <div class="card">
    <div class="card-head"><div class="card-title">Session</div></div>
    <div class="rows">
      <div class="row" data-setting="max-recent-projects">
        <div class="row-info"><div class="row-label">Max recent projects</div></div>
        <div class="stepper">
          <button class="step-btn" onclick={() => maxRecentProjects.update(v => Math.max(0, v - 1))}>−</button>
          <span class="step-val">{$maxRecentProjects}</span>
          <button class="step-btn" onclick={() => maxRecentProjects.update(v => Math.min(30, v + 1))}>+</button>
        </div>
      </div>
      <div class="row" data-setting="max-tabs">
        <div class="row-info"><div class="row-label">Max open tabs</div></div>
        <div class="stepper">
          <button class="step-btn" onclick={() => maxTabs.update(v => Math.max(1, v - 1))}>−</button>
          <span class="step-val">{$maxTabs}</span>
          <button class="step-btn" onclick={() => maxTabs.update(v => Math.min(30, v + 1))}>+</button>
        </div>
      </div>
    </div>
  </div>

  <!-- File visibility -->
  <div class="card" data-setting="hidden-patterns">
    <div class="card-head">
      <div class="card-title">File visibility</div>
      <div class="card-sub">Hide files and folders from the explorer. Supports exact names and <code>*.ext</code> globs.</div>
    </div>
    <div class="pattern-list">
      {#each $hiddenPatterns as item}
        <div class="pattern-row">
          <button class="toggle" class:active={item.enabled} onclick={() => togglePattern(item.pattern)} title={item.enabled ? 'Hidden' : 'Visible'}>
            <span class="track"><span class="thumb"></span></span>
          </button>
          <span class="pattern-name" class:disabled={!item.enabled}>{item.pattern}</span>
          <button class="delete-btn" onclick={() => removePattern(item.pattern)} title="Remove">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" width="12" height="12">
              <path d="M4 4l8 8M12 4l-8 8" />
            </svg>
          </button>
        </div>
      {/each}
    </div>
    <div class="add-row">
      <input bind:value={newPattern} class="add-input" placeholder="e.g. .vscode, *.log, dist" onkeydown={(e) => { if (e.key === 'Enter') addPattern(); }} />
      <button class="add-btn" onclick={addPattern}>Add</button>
    </div>
  </div>

  <!-- Backup -->
  <div class="card" data-setting="export-import-settings">
    <div class="card-head">
      <div class="card-title">Backup</div>
      <div class="card-sub">Export your settings to a file or import from a previous export. API keys are never exported.</div>
    </div>
    <div class="ex-row">
      <button class="primary-btn" onclick={exportSettings}>Export</button>
      <button class="secondary-btn" onclick={importSettings}>Import</button>
      {#if exportImportStatus}<span class="status">{exportImportStatus}</span>{/if}
    </div>
  </div>
</div>

<style>
  .root { display: flex; flex-direction: column; gap: 20px; }

  /* Card primitive */
  .card {
    background: color-mix(in srgb, var(--bg-tertiary) 60%, transparent);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .card-head { display: flex; flex-direction: column; gap: 4px; }
  .card-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: -0.1px;
  }
  .card-sub {
    font-size: 11.5px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .card-sub code {
    background: var(--bg-surface);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 10.5px;
    font-family: var(--font-mono);
  }

  .rows { display: flex; flex-direction: column; }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
  }
  .row + .row { border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent); }
  .row-info { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .row-label { font-size: 13px; color: var(--text-primary); }
  .row-help { font-size: 11px; color: var(--text-muted); line-height: 1.4; }

  /* Toggle */
  .toggle { padding: 0; background: none; border: none; cursor: pointer; flex-shrink: 0; }
  .track {
    display: block;
    width: 34px; height: 20px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    position: relative;
    transition: background 0.15s, border-color 0.15s;
  }
  .toggle.active .track { background: var(--success); border-color: var(--success); }
  .thumb {
    display: block;
    width: 14px; height: 14px;
    background: var(--text-muted);
    border-radius: 50%;
    position: absolute;
    top: 2px; left: 2px;
    transition: transform 0.15s, background 0.15s;
  }
  .toggle.active .thumb { transform: translateX(14px); background: #fff; }

  /* Stepper */
  .stepper {
    display: flex; align-items: center;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    flex-shrink: 0;
  }
  .step-btn {
    width: 28px; height: 28px;
    font-size: 14px; font-weight: 500;
    color: var(--text-secondary);
    background: var(--bg-surface);
    border: none; cursor: pointer;
    line-height: 1;
  }
  .step-btn:hover { background: var(--border); color: var(--text-primary); }
  .step-val {
    min-width: 56px; padding: 0 10px;
    text-align: center;
    font-size: 12px; color: var(--text-primary);
    background: var(--bg-secondary);
    height: 28px; line-height: 28px;
    font-variant-numeric: tabular-nums;
  }

  /* Pills */
  .pills {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    flex-shrink: 0;
  }
  .pill {
    padding: 6px 14px;
    font-size: 12px; font-weight: 500;
    color: var(--text-muted);
    background: var(--bg-secondary);
    border: none; border-right: 1px solid var(--border);
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }
  .pill:last-child { border-right: none; }
  .pill:hover { color: var(--text-primary); background: var(--bg-surface); }
  .pill.active { color: var(--text-primary); background: var(--bg-surface); border-color: var(--text-muted); font-weight: 600; }

  /* Theme grid */
  .appearance-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin-top: 10px;
  }

  .appearance-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 20px 12px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s, color 0.15s;
  }

  .appearance-card:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .appearance-card.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-secondary));
    color: var(--accent);
  }

  .appearance-card span {
    font-size: 12px;
    font-weight: 500;
  }

  .select {
    width: 100%; padding: 8px 12px; border-radius: 6px;
    background: var(--bg-secondary); color: var(--text-primary);
    border: 1px solid var(--border); font-size: 13px;
    cursor: pointer; appearance: auto;
  }
  .select:focus { border-color: var(--accent); outline: none; }

  .text-input {
    width: 180px; padding: 6px 10px; border-radius: 6px;
    background: var(--bg-secondary); color: var(--text-primary);
    border: 1px solid var(--border); font-size: 12px;
    font-family: var(--font-mono);
  }
  .text-input:focus { border-color: var(--accent); outline: none; }

  /* File visibility patterns */
  .pattern-list {
    display: flex; flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg-secondary);
    overflow: hidden;
  }
  .pattern-list:empty { display: none; }
  .pattern-row {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 12px;
  }
  .pattern-row + .pattern-row { border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent); }
  .pattern-row:hover { background: var(--bg-surface); }
  .pattern-name {
    flex: 1; font-size: 12px;
    color: var(--text-primary);
    font-family: var(--font-mono);
  }
  .pattern-name.disabled { color: var(--text-muted); text-decoration: line-through; }
  .delete-btn {
    color: var(--text-muted); padding: 4px;
    border-radius: 4px; border: none; background: none;
    display: flex; align-items: center; cursor: pointer;
    opacity: 0; transition: opacity 0.1s, background 0.1s, color 0.1s;
  }
  .pattern-row:hover .delete-btn { opacity: 1; }
  .delete-btn:hover { color: var(--error); background: color-mix(in srgb, var(--error) 12%, transparent); }
  .add-row { display: flex; gap: 8px; }
  .add-input {
    flex: 1;
    font-size: 12px; padding: 8px 12px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 6px; outline: none;
    font-family: var(--font-mono);
  }
  .add-input:focus { border-color: var(--accent); }
  .add-btn {
    background: var(--accent); color: var(--bg-tertiary);
    padding: 8px 18px; border-radius: 6px;
    font-size: 12px; font-weight: 600;
    border: none; cursor: pointer;
  }
  .add-btn:hover { opacity: 0.9; }

  /* Backup buttons */
  .ex-row { display: flex; align-items: center; gap: 10px; }
  .primary-btn {
    background: var(--accent); color: var(--bg-tertiary);
    padding: 8px 18px; border-radius: 6px;
    font-size: 12px; font-weight: 600;
    border: none; cursor: pointer;
  }
  .primary-btn:hover { opacity: 0.9; }
  .secondary-btn {
    background: var(--bg-surface); color: var(--text-primary);
    padding: 8px 18px; border-radius: 6px;
    font-size: 12px; font-weight: 600;
    border: 1px solid var(--border); cursor: pointer;
  }
  .secondary-btn:hover { background: var(--border); }
  .status { font-size: 11px; color: var(--success); }

  /*
   * Consolidated keyboard focus indicator.
   *
   * The earlier `:focus { ... outline: none; }` rules on inputs were
   * suppressing native outlines for mouse interaction; this rule adds
   * back a clean, theme-aware focus ring for keyboard users specifically.
   * `:focus-visible` is keyboard-only by browser heuristic, so mouse
   * clicks remain ringless. Listed at the end of the stylesheet so the
   * cascade beats the earlier `outline: none` rules.
   */
  .toggle:focus-visible,
  .step-btn:focus-visible,
  .pill:focus-visible,
  .appearance-card:focus-visible,
  .select:focus-visible,
  .text-input:focus-visible,
  .add-input:focus-visible,
  .add-btn:focus-visible,
  .primary-btn:focus-visible,
  .secondary-btn:focus-visible,
  .delete-btn:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 2px;
  }
</style>
