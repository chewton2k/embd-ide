<script lang="ts">
  import { apiKey, openaiApiKey, anthropicApiKey, aiModel, aiProvider, type AiProvider } from '../../modules';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import type { Writable } from 'svelte/store';
  import SectionHeader from '../components/SectionHeader.svelte';
  import ProviderIcon from '../components/ProviderIcon.svelte';
  import ProviderKeyCard from '../components/ProviderKeyCard.svelte';

  type Model = { id: string; label: string; hint: string };
  type Provider = {
    id: AiProvider;
    label: string;
    placeholder: string;
    keyPrefix: string;
    docsUrl: string;
    store: Writable<string>;
    models: Model[];
  };

  const PROVIDERS: Provider[] = [
    {
      id: 'anthropic',
      label: 'Anthropic',
      placeholder: 'sk-ant-...',
      keyPrefix: 'sk-ant-',
      docsUrl: 'https://console.anthropic.com/settings/keys',
      store: anthropicApiKey,
      models: [
        { id: 'claude-opus-4-7',          label: 'Claude Opus 4.7',    hint: 'Most capable' },
        { id: 'claude-sonnet-4-6',        label: 'Claude Sonnet 4.6',  hint: 'Best for coding' },
        { id: 'claude-haiku-4-5',         label: 'Claude Haiku 4.5',   hint: 'Fast & cheap' },
      ],
    },
    {
      id: 'openai',
      label: 'OpenAI',
      placeholder: 'sk-...',
      keyPrefix: 'sk-',
      docsUrl: 'https://platform.openai.com/api-keys',
      store: openaiApiKey,
      models: [
        { id: 'gpt-5',         label: 'GPT-5',         hint: 'Flagship multimodal' },
        { id: 'gpt-5-mini',    label: 'GPT-5 mini',    hint: 'Fast & cheap' },
        { id: 'o3',            label: 'o3',            hint: 'Deep reasoning' },
        { id: 'o4-mini',       label: 'o4-mini',       hint: 'Fast reasoning' },
      ],
    },
    {
      id: 'openrouter',
      label: 'OpenRouter',
      placeholder: 'sk-or-...',
      keyPrefix: 'sk-or-',
      docsUrl: 'https://openrouter.ai/keys',
      store: apiKey,
      models: [
        { id: 'openrouter/auto',                    label: 'Auto',                 hint: 'Pick per prompt' },
        { id: 'anthropic/claude-opus-4.7',          label: 'Claude Opus 4.7',      hint: 'Most capable' },
        { id: 'anthropic/claude-sonnet-4.6',        label: 'Claude Sonnet 4.6',    hint: 'Best for coding' },
        { id: 'openai/gpt-5',                       label: 'GPT-5',                hint: 'Routed via OR' },
        { id: 'openai/o3',                          label: 'o3',                   hint: 'Deep reasoning' },
        { id: 'google/gemini-2.5-pro',              label: 'Gemini 2.5 Pro',       hint: 'Long context' },
        { id: 'x-ai/grok-4',                        label: 'Grok 4',               hint: 'Realtime + reasoning' },
        { id: 'deepseek/deepseek-v3.1',             label: 'DeepSeek V3.1',        hint: 'Strong + cheap' },
        { id: 'meta-llama/llama-4-maverick',        label: 'Llama 4 Maverick',     hint: 'Open weights' },
      ],
    },
  ];

  const configured = $derived<Record<AiProvider, boolean>>({
    openrouter: $apiKey.trim().length > 0,
    openai:     $openaiApiKey.trim().length > 0,
    anthropic:  $anthropicApiKey.trim().length > 0,
  });
  const configuredCount = $derived(
    (configured.openrouter ? 1 : 0) + (configured.openai ? 1 : 0) + (configured.anthropic ? 1 : 0)
  );

  function providerOf(modelId: string): Provider | null {
    for (const p of PROVIDERS) if (p.models.some(m => m.id === modelId)) return p;
    return null;
  }
  function modelOf(modelId: string): Model | null {
    for (const p of PROVIDERS) {
      const m = p.models.find(m => m.id === modelId);
      if (m) return m;
    }
    return null;
  }

  const currentProvider = $derived(providerOf($aiModel) ?? PROVIDERS[0]);
  const currentModel    = $derived(modelOf($aiModel) ?? currentProvider.models[0]);

  let dropdownOpen = $state(false);
  let dropdownEl: HTMLDivElement;

  function toggleDropdown() { dropdownOpen = !dropdownOpen; }

  function selectModel(p: Provider, m: Model) {
    aiProvider.set(p.id);
    aiModel.set(m.id);
    dropdownOpen = false;
  }

  function handleDocClick(e: MouseEvent) {
    if (!dropdownOpen) return;
    if (dropdownEl && !dropdownEl.contains(e.target as Node)) dropdownOpen = false;
  }
  onMount(() => {
    document.addEventListener('mousedown', handleDocClick);

    // Load keys from OS keychain into stores (needed because settings is a separate window)
    (async () => {
      for (const p of PROVIDERS) {
        try {
          const key: string = await invoke('get_provider_key', { provider: p.id });
          if (key) p.store.set(key);
        } catch { /* ignore */ }
      }
    })();

    return () => document.removeEventListener('mousedown', handleDocClick);
  });

  async function saveKey(p: Provider, value: string) {
    p.store.set(value);
    try { await invoke('set_provider_key', { provider: p.id, key: value }); } catch { /* Legitimate: keyring may be unavailable */ }
  }
  async function clearKey(p: Provider) {
    p.store.set('');
    try { await invoke('set_provider_key', { provider: p.id, key: '' }); } catch { /* Legitimate: keyring may be unavailable */ }
    // If the cleared provider owned the default model, pick another configured one.
    if (currentProvider.id === p.id) {
      const next = PROVIDERS.find(x => configured[x.id]) ?? PROVIDERS[0];
      aiProvider.set(next.id);
      aiModel.set(next.models[0].id);
    }
  }
</script>

<div class="root">
  <SectionHeader
    title="Models"
    description="Bring your own keys. They are stored securely in your OS keychain and sent only to the matching provider."
  />

  <!-- Default model -->
  <div class="block" data-setting="default-model">
    <div class="block-head"><span class="block-label">Default model</span></div>
    <div class="dropdown" bind:this={dropdownEl}>
      <button class="dropdown-trigger" onclick={toggleDropdown} class:open={dropdownOpen}>
        <span class="trigger-left">
          <ProviderIcon provider={currentProvider.id} size={14} />
          <span class="trigger-model">{currentModel.label}</span>
          <span class="trigger-hint">· {currentModel.hint}</span>
        </span>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12" class="chevron" class:flipped={dropdownOpen}>
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>

      {#if dropdownOpen}
        <div class="dropdown-menu" role="menu">
          {#each PROVIDERS as p}
            {@const hasKey = configured[p.id]}
            <div class="menu-group">
              <div class="menu-group-head">
                <ProviderIcon provider={p.id} size={11} />
                <span>{p.label}</span>
                {#if !hasKey}<span class="no-key">no key</span>{/if}
              </div>
              {#each p.models as m}
                {@const active = m.id === $aiModel}
                <button
                  class="menu-item"
                  class:active
                  disabled={!hasKey}
                  onclick={() => hasKey && selectModel(p, m)}
                >
                  <span class="mi-label">{m.label}</span>
                  <span class="mi-hint">{m.hint}</span>
                  {#if active}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" width="11" height="11" class="check">
                      <path d="M20 6L9 17l-5-5" />
                    </svg>
                  {/if}
                </button>
              {/each}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- API keys -->
  <div class="block">
    <div class="block-head">
      <span class="block-label">API keys</span>
      <span class="block-meta">{configuredCount} of {PROVIDERS.length} configured</span>
    </div>
    <div class="key-grid">
      {#each PROVIDERS as p}
        <div data-setting={`${p.id}-api-key`}>
          <ProviderKeyCard
            provider={p.id}
            label={p.label}
            placeholder={p.placeholder}
            keyPrefix={p.keyPrefix}
            docsUrl={p.docsUrl}
            currentKey={p.id === 'openrouter' ? $apiKey : p.id === 'openai' ? $openaiApiKey : $anthropicApiKey}
            onSave={(v) => saveKey(p, v)}
            onClear={() => clearKey(p)}
          />
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .root { display: flex; flex-direction: column; gap: 24px; }

  .block { display: flex; flex-direction: column; gap: 8px; }
  .block-head {
    display: flex; align-items: baseline; justify-content: space-between;
    gap: 8px;
  }
  .block-label {
    font-size: 11px; font-weight: 500;
    color: var(--text-muted);
    letter-spacing: 0.2px;
  }
  .block-meta {
    font-size: 10.5px;
    color: var(--text-muted);
  }

  /* Dropdown */
  .dropdown { position: relative; }
  .dropdown-trigger {
    width: 100%;
    height: 36px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s;
  }
  .dropdown-trigger:hover { background: var(--bg-surface); }
  .dropdown-trigger.open { border-color: var(--accent); }
  .trigger-left { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .trigger-model { font-weight: 500; }
  .trigger-hint { color: var(--text-muted); font-size: 11.5px; }
  .chevron { color: var(--text-muted); transition: transform 0.15s; }
  .chevron.flipped { transform: rotate(180deg); }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
    padding: 4px;
    z-index: 20;
    max-height: 360px;
    overflow-y: auto;
  }

  .menu-group { padding: 4px 0; }
  .menu-group + .menu-group { border-top: 1px solid var(--border); margin-top: 2px; padding-top: 8px; }
  .menu-group-head {
    display: flex; align-items: center; gap: 6px;
    padding: 4px 10px 6px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .no-key {
    margin-left: auto;
    font-size: 9.5px;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 500;
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 15%, transparent);
    padding: 1px 6px;
    border-radius: 8px;
  }

  .menu-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
    font-size: 12px;
  }
  .menu-item:hover:not(:disabled) { background: var(--bg-surface); }
  .menu-item:disabled { opacity: 0.4; cursor: not-allowed; }
  .menu-item.active { background: color-mix(in srgb, var(--accent) 18%, transparent); }
  .mi-label { font-weight: 500; }
  .mi-hint { color: var(--text-muted); font-size: 10.5px; margin-left: auto; }
  .menu-item.active .mi-hint { margin-left: 0; }
  .menu-item .check { margin-left: auto; color: var(--accent); }

  /* Provider key grid */
  .key-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  @media (max-width: 720px) {
    .key-grid { grid-template-columns: 1fr; }
  }
</style>
