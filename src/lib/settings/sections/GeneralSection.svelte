<script lang="ts">
  import {
    currentThemeId, THEMES, uiFontSize, uiDensity,
    editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers,
    terminalFontSize,
    autosaveEnabled, autosaveDelay,
    maxRecentProjects, maxTabs,
    hiddenPatterns,
  } from '../../stores';
  import { save, open } from '@tauri-apps/plugin-dialog';
  import { writeTextFile, readTextFile } from '@tauri-apps/plugin-fs';

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
    'embd-autosave', 'embd-autosave-delay',
    'embd-editor-font-size', 'embd-editor-tab-size', 'embd-editor-word-wrap', 'embd-editor-line-numbers',
    'embd-terminal-font-size',
    'embd-theme',
    'embd-ui-font-size', 'embd-ui-density',
    'embd-hidden-patterns',
    'embd-max-recent-projects', 'embd-max-tabs',
  ];

  async function exportSettings() {
    const data: Record<string, string> = {};
    for (const k of EXPORT_KEYS) {
      const v = localStorage.getItem(k);
      if (v !== null) data[k] = v;
    }
    const path = await save({ defaultPath: 'embd-settings.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
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
      if (!k.startsWith('embd-') || k === 'embd-api-key') continue;
      localStorage.setItem(k, v);
    }
    // Storage event will sync stores in the main window. Re-mirror locally.
    location.reload();
  }
</script>

<div class="section">
  <h3>Appearance</h3>
  <p class="desc">Pick a theme. Changes apply to all windows instantly.</p>
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

<div class="section">
  <h3>Interface</h3>
  <div class="row">
    <span class="label">UI font size</span>
    <div class="stepper">
      <button class="step-btn" onclick={() => uiFontSize.update(v => Math.max(11, v - 1))}>-</button>
      <span class="step-val">{$uiFontSize}px</span>
      <button class="step-btn" onclick={() => uiFontSize.update(v => Math.min(18, v + 1))}>+</button>
    </div>
  </div>
  <div class="row">
    <span class="label">Density</span>
    <div class="pills">
      <button class="pill" class:active={$uiDensity === 'compact'} onclick={() => uiDensity.set('compact')}>Compact</button>
      <button class="pill" class:active={$uiDensity === 'comfortable'} onclick={() => uiDensity.set('comfortable')}>Comfortable</button>
    </div>
  </div>
</div>

<div class="section">
  <h3>Editor</h3>
  <div class="row">
    <span class="label">Font size</span>
    <div class="stepper">
      <button class="step-btn" onclick={() => editorFontSize.update(v => Math.max(10, v - 1))}>-</button>
      <span class="step-val">{$editorFontSize}px</span>
      <button class="step-btn" onclick={() => editorFontSize.update(v => Math.min(24, v + 1))}>+</button>
    </div>
  </div>
  <div class="row">
    <span class="label">Tab size</span>
    <div class="pills">
      {#each [2, 4] as s}
        <button class="pill" class:active={$editorTabSize === s} onclick={() => editorTabSize.set(s)}>{s}</button>
      {/each}
    </div>
  </div>
  <div class="row">
    <span class="label">Word wrap</span>
    <button class="toggle" class:active={$editorWordWrap} onclick={() => editorWordWrap.update(v => !v)} aria-label="Toggle word wrap">
      <span class="track"><span class="thumb"></span></span>
    </button>
  </div>
  <div class="row">
    <span class="label">Line numbers</span>
    <button class="toggle" class:active={$editorLineNumbers} onclick={() => editorLineNumbers.update(v => !v)} aria-label="Toggle line numbers">
      <span class="track"><span class="thumb"></span></span>
    </button>
  </div>
</div>

<div class="section">
  <h3>Terminal</h3>
  <div class="row">
    <span class="label">Font size</span>
    <div class="stepper">
      <button class="step-btn" onclick={() => terminalFontSize.update(v => Math.max(10, v - 1))}>-</button>
      <span class="step-val">{$terminalFontSize}px</span>
      <button class="step-btn" onclick={() => terminalFontSize.update(v => Math.min(24, v + 1))}>+</button>
    </div>
  </div>
</div>

<div class="section">
  <h3>Autosave</h3>
  <div class="row">
    <span class="label">Enabled</span>
    <button class="toggle" class:active={$autosaveEnabled} onclick={() => autosaveEnabled.update(v => !v)} aria-label="Toggle autosave">
      <span class="track"><span class="thumb"></span></span>
    </button>
  </div>
  <div class="row">
    <span class="label">Delay</span>
    <div class="pills">
      {#each AUTOSAVE_DELAYS as opt}
        <button class="pill" class:active={$autosaveDelay === opt.value} onclick={() => autosaveDelay.set(opt.value)}>{opt.label}</button>
      {/each}
    </div>
  </div>
</div>

<div class="section">
  <h3>Session</h3>
  <div class="row">
    <span class="label">Max recent projects</span>
    <div class="stepper">
      <button class="step-btn" onclick={() => maxRecentProjects.update(v => Math.max(0, v - 1))}>-</button>
      <span class="step-val">{$maxRecentProjects}</span>
      <button class="step-btn" onclick={() => maxRecentProjects.update(v => Math.min(30, v + 1))}>+</button>
    </div>
  </div>
  <div class="row">
    <span class="label">Max open tabs</span>
    <div class="stepper">
      <button class="step-btn" onclick={() => maxTabs.update(v => Math.max(1, v - 1))}>-</button>
      <span class="step-val">{$maxTabs}</span>
      <button class="step-btn" onclick={() => maxTabs.update(v => Math.min(30, v + 1))}>+</button>
    </div>
  </div>
</div>

<div class="section">
  <h3>File visibility</h3>
  <p class="desc">Hide files and folders from the explorer. Supports exact names and <code>*.ext</code> glob patterns.</p>
  <div class="pattern-list">
    {#each $hiddenPatterns as item}
      <div class="pattern-row">
        <button class="toggle" class:active={item.enabled} onclick={() => togglePattern(item.pattern)} title={item.enabled ? 'Hidden' : 'Visible'}>
          <span class="track"><span class="thumb"></span></span>
        </button>
        <span class="pattern-name" class:disabled={!item.enabled}>{item.pattern}</span>
        <button class="delete-btn" onclick={() => removePattern(item.pattern)} title="Remove">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="12" height="12">
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

<div class="section">
  <h3>Backup</h3>
  <p class="desc">Export your settings to a file or import them from a previous export. The API key is never exported.</p>
  <div class="ex-row">
    <button class="primary-btn" onclick={exportSettings}>Export</button>
    <button class="secondary-btn" onclick={importSettings}>Import</button>
    {#if exportImportStatus}<span class="status">{exportImportStatus}</span>{/if}
  </div>
</div>

<style>
  .section { margin-bottom: 28px; }
  .section h3 {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    margin: 0 0 10px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
  }
  .desc {
    font-size: 11px;
    color: var(--text-muted);
    margin: -4px 0 12px;
    line-height: 1.5;
  }
  .desc code {
    background: var(--bg-surface);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 0;
  }
  .label { font-size: 13px; color: var(--text-primary); }

  .toggle { padding: 0; background: none; border: none; cursor: pointer; }
  .track {
    display: block;
    width: 32px; height: 18px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 9px;
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
    top: 1px; left: 1px;
    transition: transform 0.15s, background 0.15s;
  }
  .toggle.active .thumb { transform: translateX(14px); background: var(--bg-tertiary); }

  .stepper {
    display: flex; align-items: center;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
  }
  .step-btn {
    width: 28px; height: 26px;
    font-size: 13px; font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-surface);
    border: none; cursor: pointer;
  }
  .step-btn:hover { background: var(--border); color: var(--text-primary); }
  .step-val {
    min-width: 50px; padding: 0 8px;
    text-align: center;
    font-size: 12px; color: var(--text-primary);
    background: var(--bg-tertiary);
    height: 26px; line-height: 26px;
  }

  .pills {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
  }
  .pill {
    padding: 5px 14px;
    font-size: 12px; font-weight: 500;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border: none; border-right: 1px solid var(--border);
    cursor: pointer;
  }
  .pill:last-child { border-right: none; }
  .pill:hover { color: var(--text-primary); background: var(--bg-surface); }
  .pill.active { color: var(--bg-tertiary); background: var(--accent); }

  .theme-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
  .theme-card {
    display: flex; flex-direction: column; align-items: center; gap: 6px;
    padding: 8px; border-radius: 6px;
    border: 2px solid transparent;
    background: none; cursor: pointer;
    transition: border-color 0.15s;
  }
  .theme-card:hover { border-color: var(--border); }
  .theme-card.active { border-color: var(--accent); }
  .theme-preview {
    width: 100%; aspect-ratio: 16 / 10;
    border-radius: 4px; overflow: hidden;
    display: flex; flex-direction: column;
  }
  .tp-bar { height: 12%; display: flex; align-items: center; gap: 2px; padding: 0 3px; }
  .tp-dot { width: 3px; height: 3px; border-radius: 50%; }
  .tp-body { flex: 1; display: flex; overflow: hidden; }
  .tp-sidebar { width: 30%; padding: 3px 2px; display: flex; flex-direction: column; gap: 2px; }
  .tp-editor { flex: 1; padding: 3px 4px; display: flex; flex-direction: column; gap: 2px; }
  .tp-line { height: 2px; border-radius: 1px; opacity: 0.7; }
  .tp-statusbar { height: 6%; min-height: 2px; }
  .theme-name { font-size: 11px; color: var(--text-muted); }
  .active-name { color: var(--accent); font-weight: 600; }

  .pattern-list { display: flex; flex-direction: column; gap: 2px; margin-bottom: 10px; }
  .pattern-row {
    display: flex; align-items: center; gap: 10px;
    padding: 6px 8px; border-radius: 4px;
  }
  .pattern-row:hover { background: var(--bg-surface); }
  .pattern-name {
    flex: 1; font-size: 12px;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
  }
  .pattern-name.disabled { color: var(--text-muted); text-decoration: line-through; }
  .delete-btn {
    color: var(--text-muted); padding: 3px;
    border-radius: 3px; border: none; background: none;
    display: flex; align-items: center; cursor: pointer;
    opacity: 0; transition: opacity 0.1s;
  }
  .pattern-row:hover .delete-btn { opacity: 1; }
  .delete-btn:hover { color: #f38ba8; background: color-mix(in srgb, #f38ba8 10%, transparent); }
  .add-row { display: flex; gap: 8px; }
  .add-input {
    flex: 1;
    font-size: 12px; padding: 7px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 5px; outline: none;
    font-family: var(--font-mono, monospace);
  }
  .add-input:focus { border-color: var(--accent); }
  .add-btn {
    background: var(--accent); color: var(--bg-tertiary);
    padding: 7px 16px; border-radius: 5px;
    font-size: 12px; font-weight: 600;
    border: none; cursor: pointer;
  }
  .add-btn:hover { opacity: 0.9; }

  .ex-row { display: flex; align-items: center; gap: 10px; }
  .primary-btn {
    background: var(--accent); color: var(--bg-tertiary);
    padding: 7px 16px; border-radius: 5px;
    font-size: 12px; font-weight: 600;
    border: none; cursor: pointer;
  }
  .primary-btn:hover { opacity: 0.9; }
  .secondary-btn {
    background: var(--bg-surface); color: var(--text-primary);
    padding: 7px 16px; border-radius: 5px;
    font-size: 12px; font-weight: 600;
    border: 1px solid var(--border); cursor: pointer;
  }
  .secondary-btn:hover { background: var(--border); }
  .status { font-size: 11px; color: var(--success); }
</style>
