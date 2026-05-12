<script lang="ts">
  /**
   * Terminal-specific settings.
   *
   * Promoted to its own settings tab so the configuration surface for
   * the terminal can grow (font face, scrollback, custom shell, profile
   * presets, etc.) without further crowding the General tab.
   *
   * Reuses the same `.card / .rows / .row / .pills / .stepper` primitive
   * styles as GeneralSection — duplicated here because each section in
   * this codebase owns its own scoped CSS.
   */
  import { terminalMode, terminalFontSize } from '../../modules/stores';
  import SectionHeader from '../components/SectionHeader.svelte';
</script>

<div class="root">
  <SectionHeader
    title="Terminal"
    description="Layout and typography for the integrated terminal."
  />

  <!-- Layout (Tab vs Bottom panel). -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Layout</div>
      <div class="card-sub">
        Choose where terminal sessions appear. The "Tab" option mixes
        them in with file tabs at the top of the editor; "Bottom panel"
        docks them in a resizable panel below the editor like VSCode,
        Xcode and Zed. Switching layout closes any open terminals so
        they can restart cleanly in the new container.
      </div>
    </div>
    <div class="rows">
      <div class="row" data-setting="terminal-mode">
        <div class="row-info">
          <div class="row-label">Surface</div>
          <div class="row-help">Where new terminal sessions open.</div>
        </div>
        <div class="pills">
          <button
            class="pill"
            class:active={$terminalMode === 'tab'}
            onclick={() => terminalMode.set('tab')}
            aria-pressed={$terminalMode === 'tab'}
            title="Terminals open in the main editor tab bar"
          >Tab</button>
          <button
            class="pill"
            class:active={$terminalMode === 'panel'}
            onclick={() => terminalMode.set('panel')}
            aria-pressed={$terminalMode === 'panel'}
            title="Terminals dock in a resizable bottom panel"
          >Bottom panel</button>
        </div>
      </div>
    </div>
  </div>

  <!-- Typography. -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Typography</div>
    </div>
    <div class="rows">
      <div class="row" data-setting="terminal-font-size">
        <div class="row-info">
          <div class="row-label">Font size</div>
          <div class="row-help">Applies to all open terminal panes immediately.</div>
        </div>
        <div class="stepper">
          <button class="step-btn" onclick={() => terminalFontSize.update(v => Math.max(10, v - 1))} aria-label="Decrease terminal font size">−</button>
          <span class="step-val">{$terminalFontSize}px</span>
          <button class="step-btn" onclick={() => terminalFontSize.update(v => Math.min(24, v + 1))} aria-label="Increase terminal font size">+</button>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .root { display: flex; flex-direction: column; gap: 20px; }

  /* Card primitive (mirrors GeneralSection — see file header). */
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

  .rows { display: flex; flex-direction: column; }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
  }
  /* `.row + .row` border is intentionally omitted — currently each card has
     a single row. Re-add when a card gains multiple rows. */
  .row-info { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .row-label { font-size: 13px; color: var(--text-primary); }
  .row-help { font-size: 11px; color: var(--text-muted); line-height: 1.4; }

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
  .pill.active {
    color: var(--text-primary);
    background: var(--bg-surface);
    border-color: var(--text-muted);
    font-weight: 600;
  }
</style>
