<script lang="ts">
  import {
    currentThemeId, THEMES, uiFontSize, uiDensity,
    editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers,
    terminalFontSize,
    autosaveEnabled, autosaveDelay,
    maxRecentProjects, maxTabs,
    hiddenPatterns,
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
    'leo-terminal-font-size',
    'leo-theme',
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

  <!-- Appearance / Theme -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Theme</div>
      <div class="card-sub">Color scheme for the app. Applies instantly across all windows.</div>
    </div>
    <div class="theme-grid">
      {#each THEMES as theme}
        <button
          class="theme-card"
          class:active={$currentThemeId === theme.id}
          onclick={() => currentThemeId.set(theme.id)}
        >
          <div class="theme-preview">
            <div class="tp-bar" style="background: {theme.colors.bgSecondary}; border-bottom: 1px solid {theme.colors.border};">
              <span class="tp-dot" style="background: {theme.colors.error}"></span>
              <span class="tp-dot" style="background: {theme.colors.warning}"></span>
              <span class="tp-dot" style="background: {theme.colors.success}"></span>
            </div>
            <div class="tp-body" style="background: {theme.colors.bgPrimary};">
              <div class="tp-sidebar" style="background: {theme.colors.bgSecondary}; border-right: 1px solid {theme.colors.border};">
                <div class="tp-line" style="background: {theme.colors.textMuted}; width: 60%;"></div>
                <div class="tp-line" style="background: {theme.colors.accent}; width: 75%;"></div>
                <div class="tp-line" style="background: {theme.colors.textMuted}; width: 50%;"></div>
              </div>
              <div class="tp-editor">
                <div class="tp-line" style="background: {theme.colors.textMuted}; width: 80%;"></div>
                <div class="tp-line" style="background: {theme.colors.accent}; width: 60%;"></div>
                <div class="tp-line" style="background: {theme.colors.textSecondary}; width: 70%;"></div>
                <div class="tp-line" style="background: {theme.colors.textMuted}; width: 45%;"></div>
              </div>
            </div>
            <div class="tp-statusbar" style="background: {theme.colors.accent};"></div>
          </div>
          <span class="theme-name" class:active-name={$currentThemeId === theme.id}>{theme.name}</span>
        </button>
      {/each}
    </div>
  </div>

  <!-- Interface -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Interface</div>
    </div>
    <div class="rows">
      <div class="row">
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
      <div class="row">
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
      <div class="row">
        <div class="row-info"><div class="row-label">Font size</div></div>
        <div class="stepper">
          <button class="step-btn" onclick={() => editorFontSize.update(v => Math.max(10, v - 1))}>−</button>
          <span class="step-val">{$editorFontSize}px</span>
          <button class="step-btn" onclick={() => editorFontSize.update(v => Math.min(24, v + 1))}>+</button>
        </div>
      </div>
      <div class="row">
        <div class="row-info"><div class="row-label">Tab size</div></div>
        <div class="pills">
          {#each [2, 4] as s}
            <button class="pill" class:active={$editorTabSize === s} onclick={() => editorTabSize.set(s)}>{s}</button>
          {/each}
        </div>
      </div>
      <div class="row">
        <div class="row-info"><div class="row-label">Word wrap</div></div>
        <button class="toggle" class:active={$editorWordWrap} onclick={() => editorWordWrap.update(v => !v)} aria-label="Toggle word wrap">
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>
      <div class="row">
        <div class="row-info"><div class="row-label">Line numbers</div></div>
        <button class="toggle" class:active={$editorLineNumbers} onclick={() => editorLineNumbers.update(v => !v)} aria-label="Toggle line numbers">
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>
    </div>
  </div>

  <!-- Terminal -->
  <div class="card">
    <div class="card-head"><div class="card-title">Terminal</div></div>
    <div class="rows">
      <div class="row">
        <div class="row-info"><div class="row-label">Font size</div></div>
        <div class="stepper">
          <button class="step-btn" onclick={() => terminalFontSize.update(v => Math.max(10, v - 1))}>−</button>
          <span class="step-val">{$terminalFontSize}px</span>
          <button class="step-btn" onclick={() => terminalFontSize.update(v => Math.min(24, v + 1))}>+</button>
        </div>
      </div>
    </div>
  </div>

  <!-- Autosave -->
  <div class="card">
    <div class="card-head"><div class="card-title">Autosave</div></div>
    <div class="rows">
      <div class="row">
        <div class="row-info">
          <div class="row-label">Enabled</div>
          <div class="row-help">Save modified files automatically after a short delay.</div>
        </div>
        <button class="toggle" class:active={$autosaveEnabled} onclick={() => autosaveEnabled.update(v => !v)} aria-label="Toggle autosave">
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>
      <div class="row">
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
      <div class="row">
        <div class="row-info"><div class="row-label">Max recent projects</div></div>
        <div class="stepper">
          <button class="step-btn" onclick={() => maxRecentProjects.update(v => Math.max(0, v - 1))}>−</button>
          <span class="step-val">{$maxRecentProjects}</span>
          <button class="step-btn" onclick={() => maxRecentProjects.update(v => Math.min(30, v + 1))}>+</button>
        </div>
      </div>
      <div class="row">
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
  <div class="card">
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
  <div class="card">
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
  .toggle.active .track { background: var(--accent); border-color: var(--accent); }
  .thumb {
    display: block;
    width: 14px; height: 14px;
    background: var(--text-muted);
    border-radius: 50%;
    position: absolute;
    top: 2px; left: 2px;
    transition: transform 0.15s, background 0.15s;
  }
  .toggle.active .thumb { transform: translateX(14px); background: var(--bg-tertiary); }

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
  .pill.active { color: var(--bg-tertiary); background: var(--accent); }

  /* Theme grid */
  .theme-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
  .theme-card {
    display: flex; flex-direction: column; align-items: center; gap: 8px;
    padding: 10px 8px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
  }
  .theme-card:hover { background: var(--bg-surface); }
  .theme-card.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-secondary));
  }
  .theme-preview {
    width: 100%; aspect-ratio: 16 / 10;
    border-radius: 5px; overflow: hidden;
    display: flex; flex-direction: column;
    border: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  }
  .tp-bar { height: 12%; display: flex; align-items: center; gap: 2px; padding: 0 4px; }
  .tp-dot { width: 3px; height: 3px; border-radius: 50%; }
  .tp-body { flex: 1; display: flex; overflow: hidden; }
  .tp-sidebar { width: 30%; padding: 4px 3px; display: flex; flex-direction: column; gap: 2px; }
  .tp-editor { flex: 1; padding: 4px 5px; display: flex; flex-direction: column; gap: 2px; }
  .tp-line { height: 2px; border-radius: 1px; opacity: 0.8; }
  .tp-statusbar { height: 6%; min-height: 2px; }
  .theme-name { font-size: 11.5px; color: var(--text-secondary); font-weight: 500; }
  .active-name { color: var(--accent); font-weight: 600; }

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
</style>
