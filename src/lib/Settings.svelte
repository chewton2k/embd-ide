<script lang="ts">
  import {
    showSettings, hiddenPatterns,
    autosaveEnabled, autosaveDelay,
    editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers,
    terminalFontSize,
    currentThemeId, THEMES, uiFontSize, uiDensity,
  } from './stores.ts';

  let newPattern = $state('');

  function addPattern() {
    const pat = newPattern.trim();
    if (!pat) return;
    if ($hiddenPatterns.some(p => p.pattern === pat)) {
      newPattern = '';
      return;
    }
    hiddenPatterns.update(patterns => [...patterns, { pattern: pat, enabled: true }]);
    newPattern = '';
  }

  function removePattern(pattern: string) {
    hiddenPatterns.update(patterns => patterns.filter(p => p.pattern !== pattern));
  }

  function togglePattern(pattern: string) {
    hiddenPatterns.update(patterns =>
      patterns.map(p => p.pattern === pattern ? { ...p, enabled: !p.enabled } : p)
    );
  }

  function close() {
    showSettings.set(false);
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }

  const AUTOSAVE_DELAYS = [
    { label: '0.5s', value: 500 },
    { label: '1s', value: 1000 },
    { label: '2s', value: 2000 },
    { label: '5s', value: 5000 },
  ];
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" onclick={handleBackdropClick}>
  <div class="modal">
    <div class="modal-header">
      <h2>Settings</h2>
      <button class="close-btn" onclick={close}>
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="14" height="14">
          <path d="M4 4l8 8M12 4l-8 8" />
        </svg>
      </button>
    </div>

    <div class="modal-body">

      <!-- Theme -->
      <section class="section">
        <h3>Theme</h3>
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
      </section>

      <div class="divider"></div>

      <!-- UI -->
      <section class="section">
        <h3>Interface</h3>
        <div class="setting-row">
          <span class="setting-label">UI Font Size</span>
          <div class="stepper">
            <button class="stepper-btn" onclick={() => uiFontSize.update(v => Math.max(11, v - 1))}>-</button>
            <span class="stepper-value">{$uiFontSize}px</span>
            <button class="stepper-btn" onclick={() => uiFontSize.update(v => Math.min(18, v + 1))}>+</button>
          </div>
        </div>
        <div class="setting-row">
          <span class="setting-label">Density</span>
          <div class="pill-group">
            <button class="pill" class:active={$uiDensity === 'compact'} onclick={() => uiDensity.set('compact')}>Compact</button>
            <button class="pill" class:active={$uiDensity === 'comfortable'} onclick={() => uiDensity.set('comfortable')}>Comfortable</button>
          </div>
        </div>
      </section>

      <div class="divider"></div>

      <!-- Editor -->
      <section class="section">
        <h3>Editor</h3>
        <div class="setting-row">
          <span class="setting-label">Font Size</span>
          <div class="stepper">
            <button class="stepper-btn" onclick={() => editorFontSize.update(v => Math.max(10, v - 1))}>-</button>
            <span class="stepper-value">{$editorFontSize}px</span>
            <button class="stepper-btn" onclick={() => editorFontSize.update(v => Math.min(24, v + 1))}>+</button>
          </div>
        </div>
        <div class="setting-row">
          <span class="setting-label">Tab Size</span>
          <div class="pill-group">
            {#each [2, 4] as size}
              <button
                class="pill"
                class:active={$editorTabSize === size}
                onclick={() => editorTabSize.set(size)}
              >{size}</button>
            {/each}
          </div>
        </div>
        <div class="setting-row">
          <span class="setting-label">Word Wrap</span>
          <button
            class="toggle-btn"
            class:active={$editorWordWrap}
            onclick={() => editorWordWrap.update(v => !v)}
          >
            <span class="toggle-track"><span class="toggle-thumb"></span></span>
          </button>
        </div>
        <div class="setting-row">
          <span class="setting-label">Line Numbers</span>
          <button
            class="toggle-btn"
            class:active={$editorLineNumbers}
            onclick={() => editorLineNumbers.update(v => !v)}
          >
            <span class="toggle-track"><span class="toggle-thumb"></span></span>
          </button>
        </div>
      </section>

      <div class="divider"></div>

      <!-- Autosave -->
      <section class="section">
        <h3>Autosave</h3>
        <div class="setting-row">
          <span class="setting-label">Enabled</span>
          <button
            class="toggle-btn"
            class:active={$autosaveEnabled}
            onclick={() => autosaveEnabled.update(v => !v)}
          >
            <span class="toggle-track"><span class="toggle-thumb"></span></span>
          </button>
        </div>
        <div class="setting-row">
          <span class="setting-label">Delay</span>
          <div class="pill-group">
            {#each AUTOSAVE_DELAYS as opt}
              <button
                class="pill"
                class:active={$autosaveDelay === opt.value}
                onclick={() => autosaveDelay.set(opt.value)}
              >{opt.label}</button>
            {/each}
          </div>
        </div>
      </section>

      <div class="divider"></div>

      <!-- Terminal -->
      <section class="section">
        <h3>Terminal</h3>
        <div class="setting-row">
          <span class="setting-label">Font Size</span>
          <div class="stepper">
            <button class="stepper-btn" onclick={() => terminalFontSize.update(v => Math.max(10, v - 1))}>-</button>
            <span class="stepper-value">{$terminalFontSize}px</span>
            <button class="stepper-btn" onclick={() => terminalFontSize.update(v => Math.min(24, v + 1))}>+</button>
          </div>
        </div>
      </section>

      <div class="divider"></div>

      <!-- File Visibility -->
      <section class="section">
        <h3>File Visibility</h3>
        <p class="section-desc">Hide files and folders from the explorer. Supports exact names and <code>*.ext</code> glob patterns.</p>

        <div class="pattern-list">
          {#each $hiddenPatterns as item}
            <div class="pattern-row">
              <button
                class="toggle-btn"
                class:active={item.enabled}
                onclick={() => togglePattern(item.pattern)}
                title={item.enabled ? 'Hidden (click to show)' : 'Visible (click to hide)'}
              >
                <span class="toggle-track"><span class="toggle-thumb"></span></span>
              </button>
              <span class="pattern-name" class:disabled={!item.enabled}>{item.pattern}</span>
              <button class="delete-btn" onclick={() => removePattern(item.pattern)} title="Remove pattern">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="12" height="12">
                  <path d="M4 4l8 8M12 4l-8 8" />
                </svg>
              </button>
            </div>
          {/each}
        </div>

        <div class="add-row">
          <input
            bind:value={newPattern}
            class="add-input"
            placeholder="e.g. .vscode, *.log, dist"
            onkeydown={(e) => { if (e.key === 'Enter') addPattern(); }}
          />
          <button class="add-btn" onclick={addPattern}>Add</button>
        </div>
      </section>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 440px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
  }

  .close-btn {
    color: var(--text-muted);
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-surface);
  }

  .modal-body {
    padding: 16px 18px;
    overflow-y: auto;
  }

  .divider {
    height: 1px;
    background: var(--border);
    margin: 14px 0;
  }

  /* Sections */
  .section h3 {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    margin: 0 0 10px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
  }

  .section-desc {
    font-size: 11px;
    color: var(--text-muted);
    margin: -6px 0 10px;
    line-height: 1.4;
  }

  .section-desc code {
    background: var(--bg-surface);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
  }

  /* Generic setting row */
  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 0;
  }

  .setting-label {
    font-size: 12px;
    color: var(--text-primary);
  }

  /* Toggle switch */
  .toggle-btn {
    padding: 0;
    flex-shrink: 0;
  }

  .toggle-track {
    display: block;
    width: 28px;
    height: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    position: relative;
    transition: background 0.15s, border-color 0.15s;
  }

  .toggle-btn.active .toggle-track {
    background: var(--accent);
    border-color: var(--accent);
  }

  .toggle-thumb {
    display: block;
    width: 12px;
    height: 12px;
    background: var(--text-muted);
    border-radius: 50%;
    position: absolute;
    top: 1px;
    left: 1px;
    transition: transform 0.15s, background 0.15s;
  }

  .toggle-btn.active .toggle-thumb {
    transform: translateX(12px);
    background: var(--bg-tertiary);
  }

  /* Stepper */
  .stepper {
    display: flex;
    align-items: center;
    gap: 0;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
  }

  .stepper-btn {
    width: 26px;
    height: 24px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-surface);
  }

  .stepper-btn:hover {
    background: var(--border);
    color: var(--text-primary);
  }

  .stepper-value {
    width: 44px;
    text-align: center;
    font-size: 12px;
    color: var(--text-primary);
    background: var(--bg-tertiary);
    height: 24px;
    line-height: 24px;
  }

  /* Pill group */
  .pill-group {
    display: flex;
    gap: 0;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
  }

  .pill {
    padding: 4px 12px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border-right: 1px solid var(--border);
  }

  .pill:last-child {
    border-right: none;
  }

  .pill:hover {
    color: var(--text-primary);
    background: var(--bg-surface);
  }

  .pill.active {
    color: var(--bg-tertiary);
    background: var(--accent);
  }

  /* Color swatches */
  .color-swatches {
    display: flex;
    gap: 6px;
  }

  .swatch {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid transparent;
    transition: border-color 0.15s, transform 0.1s;
  }

  .swatch:hover {
    transform: scale(1.15);
  }

  .swatch.active {
    border-color: var(--text-primary);
  }

  /* Pattern list (file visibility) */
  .pattern-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 10px;
  }

  .pattern-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 6px;
    border-radius: 4px;
  }

  .pattern-row:hover {
    background: var(--bg-surface);
  }

  .pattern-name {
    flex: 1;
    font-size: 12px;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
  }

  .pattern-name.disabled {
    color: var(--text-muted);
    text-decoration: line-through;
  }

  .delete-btn {
    color: var(--text-muted);
    padding: 2px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    opacity: 0;
    transition: opacity 0.1s;
  }

  .pattern-row:hover .delete-btn {
    opacity: 1;
  }

  .delete-btn:hover {
    color: #f38ba8;
    background: color-mix(in srgb, #f38ba8 10%, transparent);
  }

  .add-row {
    display: flex;
    gap: 6px;
  }

  .add-input {
    flex: 1;
    font-size: 12px;
    padding: 6px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 5px;
    outline: none;
    font-family: var(--font-mono, monospace);
  }

  .add-input:focus {
    border-color: var(--accent);
  }

  .add-btn {
    background: var(--accent);
    color: var(--bg-tertiary);
    padding: 6px 14px;
    border-radius: 5px;
    font-size: 12px;
    font-weight: 600;
  }

  .add-btn:hover {
    opacity: 0.9;
  }

  /* Theme grid */
  .theme-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .theme-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 6px;
    border-radius: 6px;
    border: 2px solid transparent;
    transition: border-color 0.15s;
  }

  .theme-card:hover {
    border-color: var(--border);
  }

  .theme-card.active {
    border-color: var(--accent);
  }

  .theme-preview {
    width: 100%;
    aspect-ratio: 16 / 10;
    border-radius: 4px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .tp-bar {
    height: 12%;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 3px;
  }

  .tp-dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
  }

  .tp-body {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  .tp-sidebar {
    width: 30%;
    padding: 3px 2px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tp-editor {
    flex: 1;
    padding: 3px 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tp-line {
    height: 2px;
    border-radius: 1px;
    opacity: 0.7;
  }

  .tp-statusbar {
    height: 6%;
    min-height: 2px;
  }

  .theme-name {
    font-size: 10px;
    color: var(--text-muted);
    text-align: center;
    line-height: 1.2;
  }

  .active-name {
    color: var(--accent);
    font-weight: 600;
  }
</style>
