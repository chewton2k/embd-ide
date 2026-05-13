<script lang="ts">
  import { ghostTextEnabled, ghostTextDelay, agentMaxStepsConfig, agentAutoApproveConfig } from '../../modules';
  import SectionHeader from '../components/SectionHeader.svelte';
</script>

<div class="root">
  <SectionHeader
    title="AI Preferences"
    description="Configure inline completions, agent behavior, and code editing settings."
  />

  <!-- Ghost Text / Autocomplete -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Inline Completions (Ghost Text)</div>
    </div>
    <div class="card-body">
      <label class="toggle-row" data-setting="inline-completions">
        <span>Enable inline suggestions</span>
        <button class="toggle" class:on={$ghostTextEnabled} onclick={() => ghostTextEnabled.update(v => !v)} aria-label="Toggle inline suggestions">
          <span class="toggle-knob"></span>
        </button>
      </label>
      <label class="slider-row" data-setting="inline-completion-delay">
        <span>Trigger delay</span>
        <div class="slider-group">
          <input type="range" min="200" max="1500" step="50" bind:value={$ghostTextDelay} />
          <span class="slider-value">{$ghostTextDelay}ms</span>
        </div>
      </label>
    </div>
  </div>

  <!-- Agent Settings -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Agent Mode</div>
    </div>
    <div class="card-body">
      <label class="slider-row" data-setting="agent-max-steps">
        <span>Max steps per run</span>
        <div class="slider-group">
          <input type="range" min="3" max="25" step="1" bind:value={$agentMaxStepsConfig} />
          <span class="slider-value">{$agentMaxStepsConfig}</span>
        </div>
      </label>
      <label class="toggle-row" data-setting="agent-auto-approve">
        <span>Auto-approve edits</span>
        <button class="toggle" class:on={$agentAutoApproveConfig} onclick={() => agentAutoApproveConfig.update(v => !v)} aria-label="Toggle auto-approve">
          <span class="toggle-knob"></span>
        </button>
      </label>
      <p class="hint">When enabled, the agent applies code changes without asking. Use with caution.</p>
    </div>
  </div>

  <!-- Placeholder for custom agents -->
  <div class="card muted">
    <div class="card-head">
      <div class="card-title">Custom Agents</div>
      <span class="badge">Coming Soon</span>
    </div>
    <div class="card-body">
      <p class="hint">Build personalized AI agents with custom personas, instructions, and tool access.</p>
    </div>
  </div>
</div>

<style>
  .root { display: flex; flex-direction: column; gap: 20px; }

  .card {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
  }

  .card.muted { opacity: 0.6; }

  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .card-title { font-size: 13px; font-weight: 600; color: var(--text-primary); }

  .badge {
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 8px;
    background: var(--bg-surface);
    color: var(--text-muted);
    border: 1px solid var(--border);
  }

  .card-body {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 13px;
    color: var(--text-primary);
    cursor: pointer;
    padding: 4px 0;
  }

  /* Toggle — visually matches the General section's toggle so users
     get the same feel across tabs. Uses a soft surface track with a
     muted dot in the off state, accent track + white knob when on. */
  .toggle {
    position: relative;
    width: 34px;
    height: 20px;
    border-radius: 10px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease;
    flex-shrink: 0;
    padding: 0;
  }
  .toggle:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 2px;
  }
  .toggle.on {
    background: var(--success);
    border-color: var(--success);
  }

  .toggle-knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text-muted);
    transition: transform 0.15s ease, background 0.15s ease;
  }
  .toggle.on .toggle-knob {
    transform: translateX(14px);
    background: #fff;
  }

  .slider-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 13px;
    color: var(--text-primary);
    gap: 12px;
    padding: 4px 0;
  }

  .slider-group {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* Native range with a tighter, accent-colored track. We deliberately
     keep this minimal — heavier custom-rendered sliders look out of
     place against the rest of the section's neutral palette. */
  .slider-group input[type="range"] {
    width: 130px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .slider-group input[type="range"]:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 4px;
    border-radius: 6px;
  }

  .slider-value {
    font-size: 11px;
    color: var(--text-muted);
    min-width: 48px;
    text-align: right;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .hint {
    font-size: 11px;
    color: var(--text-muted);
    margin: 0;
    line-height: 1.5;
  }
</style>
