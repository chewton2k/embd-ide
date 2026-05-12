<script lang="ts">
  import { apiKey, openaiApiKey, anthropicApiKey, aiModel, aiProvider, type AiProvider } from '../../stores';
  import { invoke } from '@tauri-apps/api/core';
  import { get } from 'svelte/store';
  import type { Writable } from 'svelte/store';

  type Provider = {
    id: AiProvider;
    label: string;
    blurb: string;
    placeholder: string;
    docsUrl: string;
    store: Writable<string>;
    models: { id: string; label: string }[];
  };

  const PROVIDERS: Provider[] = [
    {
      id: 'openrouter',
      label: 'OpenRouter',
      blurb: 'Single key, hundreds of models across providers. Includes free tier.',
      placeholder: 'sk-or-...',
      docsUrl: 'https://openrouter.ai/keys',
      store: apiKey,
      models: [
        { id: 'openrouter/auto',                   label: 'Auto (best for prompt)' },
        { id: 'anthropic/claude-sonnet-4',         label: 'Claude Sonnet 4' },
        { id: 'anthropic/claude-haiku-4',          label: 'Claude Haiku 4' },
        { id: 'openai/gpt-4o',                     label: 'GPT-4o' },
        { id: 'openai/gpt-4o-mini',                label: 'GPT-4o mini' },
        { id: 'google/gemini-2.5-pro-preview',     label: 'Gemini 2.5 Pro' },
        { id: 'meta-llama/llama-3.3-70b-instruct', label: 'Llama 3.3 70B' },
      ],
    },
    {
      id: 'openai',
      label: 'OpenAI',
      blurb: 'Direct access to OpenAI’s GPT models.',
      placeholder: 'sk-...',
      docsUrl: 'https://platform.openai.com/api-keys',
      store: openaiApiKey,
      models: [
        { id: 'gpt-4o',          label: 'GPT-4o' },
        { id: 'gpt-4o-mini',     label: 'GPT-4o mini' },
        { id: 'gpt-4-turbo',     label: 'GPT-4 Turbo' },
        { id: 'o1-mini',         label: 'o1-mini' },
      ],
    },
    {
      id: 'anthropic',
      label: 'Anthropic',
      blurb: 'Direct access to Claude models. Recommended for coding.',
      placeholder: 'sk-ant-...',
      docsUrl: 'https://console.anthropic.com/settings/keys',
      store: anthropicApiKey,
      models: [
        { id: 'claude-3-5-sonnet-latest', label: 'Claude 3.5 Sonnet' },
        { id: 'claude-3-5-haiku-latest',  label: 'Claude 3.5 Haiku' },
        { id: 'claude-3-opus-latest',     label: 'Claude 3 Opus' },
      ],
    },
  ];

  function providerOf(id: AiProvider): Provider {
    return PROVIDERS.find(p => p.id === id) ?? PROVIDERS[0];
  }

  const configured = $derived<Record<AiProvider, boolean>>({
    openrouter: $apiKey.trim().length > 0,
    openai:     $openaiApiKey.trim().length > 0,
    anthropic:  $anthropicApiKey.trim().length > 0,
  });
  const configuredCount = $derived(
    (configured.openrouter ? 1 : 0) + (configured.openai ? 1 : 0) + (configured.anthropic ? 1 : 0)
  );

  // Local UI state for each provider card.
  type CardState = { input: string; show: boolean; status: string };
  let cards = $state<Record<AiProvider, CardState>>({
    openrouter: { input: get(apiKey),           show: false, status: '' },
    openai:     { input: get(openaiApiKey),     show: false, status: '' },
    anthropic:  { input: get(anthropicApiKey),  show: false, status: '' },
  });

  async function saveKey(p: Provider) {
    const next = cards[p.id].input.trim();
    p.store.set(next);
    try { await invoke('set_provider_key', { provider: p.id, key: next }); } catch {}
    cards[p.id].status = next ? 'API key saved' : 'API key cleared';
    setTimeout(() => cards[p.id].status = '', 2500);
  }

  async function clearKey(p: Provider) {
    cards[p.id].input = '';
    p.store.set('');
    try { await invoke('set_provider_key', { provider: p.id, key: '' }); } catch {}
    cards[p.id].status = 'API key cleared';
    setTimeout(() => cards[p.id].status = '', 2500);
  }

  function selectDefault(provider: AiProvider, model: string) {
    aiProvider.set(provider);
    aiModel.set(model);
  }
</script>

<div class="section">
  <h3>Default model</h3>
  <p class="desc">The chat panel uses this by default. You can still switch per-message.</p>
  <div class="row">
    <span class="label">Provider</span>
    <div class="pills">
      {#each PROVIDERS as p}
        <button
          class="pill"
          class:active={$aiProvider === p.id}
          onclick={() => {
            aiProvider.set(p.id);
            aiModel.set(p.models[0].id);
          }}
        >{p.label}</button>
      {/each}
    </div>
  </div>
  <div class="row">
    <span class="label">Model</span>
    <select
      class="select"
      value={$aiModel}
      onchange={(e) => selectDefault($aiProvider, (e.currentTarget as HTMLSelectElement).value)}
    >
      {#each providerOf($aiProvider).models as m}
        <option value={m.id}>{m.label}</option>
      {/each}
    </select>
  </div>
</div>

<div class="section">
  <h3>Providers</h3>
  <p class="desc">{configuredCount} of {PROVIDERS.length} configured. Keys are stored locally and only sent to the matching provider.</p>

  <div class="provider-list">
    {#each PROVIDERS as p}
      {@const isConfigured = configured[p.id]}
      <div class="provider-card">
        <div class="provider-head">
          <div>
            <div class="provider-name">{p.label}</div>
            <div class="provider-blurb">{p.blurb}</div>
          </div>
          <div class="provider-status" class:on={isConfigured}>
            <span class="dot"></span>{isConfigured ? 'Configured' : 'Not configured'}
          </div>
        </div>
        <div class="key-row">
          <input
            class="key-input"
            type={cards[p.id].show ? 'text' : 'password'}
            bind:value={cards[p.id].input}
            placeholder={p.placeholder}
            spellcheck="false"
            autocomplete="off"
          />
          <button class="ghost-btn" onclick={() => cards[p.id].show = !cards[p.id].show}>
            {cards[p.id].show ? 'Hide' : 'Show'}
          </button>
        </div>
        <div class="actions">
          <button class="primary-btn" onclick={() => saveKey(p)} disabled={!cards[p.id].input.trim()}>Save</button>
          <button class="secondary-btn" onclick={() => clearKey(p)} disabled={!isConfigured}>Clear</button>
          <a class="link" href={p.docsUrl} target="_blank" rel="noopener">Get key →</a>
          {#if cards[p.id].status}<span class="status">{cards[p.id].status}</span>{/if}
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .section { margin-bottom: 28px; }
  .section h3 {
    font-size: 11px; font-weight: 600;
    color: var(--text-secondary);
    margin: 0 0 10px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
  }
  .desc { font-size: 11px; color: var(--text-muted); margin: -4px 0 12px; line-height: 1.5; }
  .row { display: flex; align-items: center; justify-content: space-between; padding: 6px 0; }
  .label { font-size: 13px; color: var(--text-primary); }

  .pills { display: flex; border: 1px solid var(--border); border-radius: 5px; overflow: hidden; }
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

  .select {
    font-size: 12px;
    padding: 6px 10px;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 5px;
    min-width: 240px;
  }

  .provider-list { display: flex; flex-direction: column; gap: 12px; }
  .provider-card {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 14px 16px;
  }
  .provider-head {
    display: flex; justify-content: space-between; align-items: flex-start;
    margin-bottom: 10px;
    gap: 12px;
  }
  .provider-name { font-size: 13px; font-weight: 600; color: var(--text-primary); }
  .provider-blurb { font-size: 11px; color: var(--text-muted); margin-top: 3px; }
  .provider-status {
    font-size: 11px;
    color: var(--text-muted);
    display: flex; align-items: center; gap: 6px;
    flex-shrink: 0;
  }
  .provider-status.on { color: var(--success); }
  .dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--text-muted);
  }
  .provider-status.on .dot { background: var(--success); }

  .key-row { display: flex; gap: 8px; margin-bottom: 12px; }
  .key-input {
    flex: 1;
    font-family: var(--font-mono, monospace);
    font-size: 12px;
    padding: 7px 10px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 5px;
    outline: none;
  }
  .key-input:focus { border-color: var(--accent); }
  .ghost-btn {
    background: var(--bg-surface);
    color: var(--text-secondary);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 0 12px;
    font-size: 11px;
    cursor: pointer;
  }
  .ghost-btn:hover { color: var(--text-primary); background: var(--border); }

  .actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .primary-btn {
    background: var(--accent); color: var(--bg-tertiary);
    padding: 6px 14px; border-radius: 5px;
    font-size: 12px; font-weight: 600;
    border: none; cursor: pointer;
  }
  .primary-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .secondary-btn {
    background: var(--bg-surface); color: var(--text-primary);
    border: 1px solid var(--border);
    padding: 6px 14px; border-radius: 5px;
    font-size: 12px; font-weight: 600;
    cursor: pointer;
  }
  .secondary-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .link {
    font-size: 11px; color: var(--accent);
    text-decoration: none;
    margin-left: auto;
  }
  .link:hover { text-decoration: underline; }
  .status { font-size: 11px; color: var(--success); }
</style>
