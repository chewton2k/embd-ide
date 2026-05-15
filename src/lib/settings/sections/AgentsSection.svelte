<script lang="ts">
  import { ghostTextEnabled, ghostTextDelay, agentMaxStepsConfig, agentAutoApproveConfig, ghostTextModel, editModel } from '../../modules';
  import SectionHeader from '../components/SectionHeader.svelte';

  type PermLevel = 'allow' | 'ask' | 'deny';
  interface ToolPerm { id: string; label: string; icon: string; level: PermLevel; locked?: boolean }

  const STORAGE_KEY = 'leo-tool-permissions';

  function loadPerms(): Record<string, PermLevel> {
    try { const raw = localStorage.getItem(STORAGE_KEY); return raw ? JSON.parse(raw) : {}; }
    catch { return {}; }
  }

  const defaults: Record<string, PermLevel> = {
    read_file: 'allow', search: 'allow', list_dir: 'allow',
    edit_file: 'ask', run_command: 'ask', dangerous: 'deny',
  };

  let perms = $state<Record<string, PermLevel>>({ ...defaults, ...loadPerms() });

  function setLevel(id: string, level: PermLevel) {
    perms = { ...perms, [id]: level };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(perms));
  }

  const tools: ToolPerm[] = [
    { id: 'read_file', label: 'Read files', icon: 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z M14 2v6h6', level: perms.read_file },
    { id: 'search', label: 'Search / Grep', icon: 'M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16Z M21 21l-4.3-4.3', level: perms.search },
    { id: 'list_dir', label: 'List directories', icon: 'M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z', level: perms.list_dir },
    { id: 'edit_file', label: 'Edit files', icon: 'M12 20h9 M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z', level: perms.edit_file },
    { id: 'run_command', label: 'Run commands', icon: 'M4 17l6-6-6-6 M12 19h8', level: perms.run_command },
    { id: 'dangerous', label: 'Dangerous commands', icon: 'M12 9v4 M12 17h.01 M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z', level: perms.dangerous, locked: true },
  ];
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

  <!-- Model Routing -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Model Routing</div>
    </div>
    <div class="card-body">
      <p class="hint" style="margin-bottom: 8px;">Use a cheap/fast model for autocomplete and a strong model for edits.</p>
      <div class="routing-field" data-setting="ghost-text-model">
        <span class="field-label">Autocomplete model</span>
        <select class="select" value={$ghostTextModel} onchange={(e) => ghostTextModel.set((e.currentTarget as HTMLSelectElement).value)}>
          <option value="">Use default</option>
          <optgroup label="Anthropic">
            <option value="claude-haiku-4-5">Claude Haiku 4.5 — Fast & cheap</option>
            <option value="claude-sonnet-4-6">Claude Sonnet 4.6</option>
          </optgroup>
          <optgroup label="OpenAI">
            <option value="gpt-5-mini">GPT-5 mini — Fast & cheap</option>
            <option value="o4-mini">o4-mini — Fast reasoning</option>
          </optgroup>
          <optgroup label="OpenRouter">
            <option value="openrouter/auto">Auto</option>
            <option value="deepseek/deepseek-v3.1">DeepSeek V3.1 — Strong & cheap</option>
          </optgroup>
          <optgroup label="Local">
            <option value="llama3">Llama 3</option>
            <option value="codellama">Code Llama</option>
            <option value="mistral">Mistral</option>
            <option value="qwen2.5-coder">Qwen 2.5 Coder</option>
          </optgroup>
        </select>
      </div>
      <div class="routing-field" data-setting="edit-model">
        <span class="field-label">Edit model (Cmd+K)</span>
        <select class="select" value={$editModel} onchange={(e) => editModel.set((e.currentTarget as HTMLSelectElement).value)}>
          <option value="">Use default</option>
          <optgroup label="Anthropic">
            <option value="claude-opus-4-7">Claude Opus 4.7 — Most capable</option>
            <option value="claude-sonnet-4-6">Claude Sonnet 4.6 — Best for coding</option>
          </optgroup>
          <optgroup label="OpenAI">
            <option value="gpt-5">GPT-5 — Flagship</option>
            <option value="o3">o3 — Deep reasoning</option>
          </optgroup>
          <optgroup label="OpenRouter">
            <option value="openrouter/auto">Auto</option>
            <option value="anthropic/claude-sonnet-4.6">Claude Sonnet 4.6 (via OR)</option>
            <option value="openai/gpt-5">GPT-5 (via OR)</option>
          </optgroup>
          <optgroup label="Local">
            <option value="llama3">Llama 3</option>
            <option value="codellama">Code Llama</option>
            <option value="deepseek-coder-v2">DeepSeek Coder V2</option>
          </optgroup>
        </select>
      </div>
    </div>
  </div>

  <!-- Permission Scopes -->
  <div class="card">
    <div class="card-head">
      <div class="card-title">Tool Permissions</div>
    </div>
    <div class="card-body">
      <p class="hint" style="margin-bottom: 8px;">Control which tools the agent can use without asking.</p>
      <div class="perm-grid">
        {#each tools as tool (tool.id)}
          <div class="perm-row">
            <span class="perm-tool">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" width="14" height="14">
                <path d={tool.icon} />
              </svg>
              {tool.label}
            </span>
            {#if tool.locked}
              <span class="perm-level deny">Always block</span>
            {:else}
              <div class="perm-pills">
                <button class="perm-pill" class:active={perms[tool.id] === 'allow'} onclick={() => setLevel(tool.id, 'allow')}>Allow</button>
                <button class="perm-pill" class:active={perms[tool.id] === 'ask'} onclick={() => setLevel(tool.id, 'ask')}>Ask</button>
                <button class="perm-pill" class:active={perms[tool.id] === 'deny'} onclick={() => setLevel(tool.id, 'deny')}>Block</button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>

  <!-- Custom Agents -->
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

  .input-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 13px;
    color: var(--text-primary);
    gap: 12px;
    padding: 4px 0;
  }

  .text-input {
    width: 200px;
    padding: 5px 10px;
    font-size: 12px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-primary);
    outline: none;
  }
  .text-input:focus { border-color: var(--accent); }
  .text-input::placeholder { color: var(--text-muted); }

  .select {
    width: 100%; padding: 8px 12px; border-radius: 6px;
    background: var(--bg-secondary); color: var(--text-primary);
    border: 1px solid var(--border); font-size: 13px;
    cursor: pointer; appearance: auto;
  }
  .select:focus { border-color: var(--accent); outline: none; }

  .routing-field {
    display: flex; flex-direction: column; gap: 6px;
  }
  .routing-field + .routing-field { margin-top: 12px; }
  .field-label {
    font-size: 12px; font-weight: 500; color: var(--text-secondary);
  }

  .perm-grid { display: flex; flex-direction: column; gap: 6px; }
  .perm-row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 6px 10px; border-radius: 6px;
    background: var(--bg-secondary); font-size: 12px;
  }
  .perm-tool { display: flex; align-items: center; gap: 8px; color: var(--text-primary); }
  .perm-tool svg { color: var(--text-muted); flex-shrink: 0; }
  .perm-pills { display: flex; gap: 2px; }
  .perm-pill {
    padding: 3px 8px; border-radius: 4px; border: none;
    font-size: 10.5px; font-weight: 600; cursor: pointer;
    background: transparent; color: var(--text-muted);
    transition: background 0.12s, color 0.12s;
  }
  .perm-pill:hover { background: var(--bg-surface); color: var(--text-primary); }
  .perm-pill.active { background: var(--bg-surface); color: var(--text-primary); border: 1px solid var(--border); }
  .perm-level {
    font-size: 10.5px; font-weight: 600; padding: 2px 8px;
    border-radius: 8px; text-transform: uppercase; letter-spacing: 0.3px;
  }
  .perm-level.deny { background: color-mix(in srgb, var(--error) 15%, transparent); color: var(--error); }
</style>
