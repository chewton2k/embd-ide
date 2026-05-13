<script lang="ts">
  import type { AiProvider } from '../../modules';
  import ProviderIcon from './ProviderIcon.svelte';
  import { open as openUrl } from '@tauri-apps/plugin-shell';

  interface Props {
    provider: AiProvider;
    label: string;
    placeholder: string;
    keyPrefix?: string;
    docsUrl: string;
    currentKey: string;
    onSave: (value: string) => void | Promise<void>;
    onClear: () => void | Promise<void>;
  }

  let {
    provider, label, placeholder, keyPrefix = '',
    docsUrl, currentKey, onSave, onClear,
  }: Props = $props();

  let editing = $state(false);
  let userInitiatedEdit = $state(false);
  let draft = $state('');
  let show = $state(false);
  let error = $state('');
  let saving = $state(false);

  // Reactively sync editing state when currentKey changes (e.g. async keychain load).
  // Only auto-flip if the user hasn't manually started editing.
  $effect(() => {
    if (currentKey.length > 0 && !userInitiatedEdit) {
      editing = false;
    } else if (currentKey.length === 0 && !userInitiatedEdit) {
      editing = true;
    }
  });

  const isConfigured = $derived(currentKey.trim().length > 0);

  function maskKey(k: string): string {
    if (!k) return '';
    if (k.length <= 8) return '••••••••';
    return k.slice(0, 4) + '•'.repeat(Math.min(12, k.length - 8)) + k.slice(-4);
  }

  function startEdit() {
    draft = currentKey;
    error = '';
    userInitiatedEdit = true;
    editing = true;
  }

  function cancelEdit() {
    draft = '';
    error = '';
    show = false;
    userInitiatedEdit = false;
    if (isConfigured) editing = false;
  }

  async function save() {
    const v = draft.trim();
    if (!v) { error = 'Key cannot be empty.'; return; }
    if (keyPrefix && !v.startsWith(keyPrefix)) {
      error = `${label} keys usually start with "${keyPrefix}".`;
      return;
    }
    saving = true;
    try {
      await onSave(v);
      editing = false;
      show = false;
      draft = '';
      userInitiatedEdit = false;
    } finally {
      saving = false;
    }
  }

  async function clear() {
    await onClear();
    userInitiatedEdit = true;
    editing = true;
    draft = '';
  }
</script>

<div class="card" class:configured={isConfigured}>
  <div class="head">
    <div class="ident">
      <ProviderIcon {provider} size={16} />
      <span class="label">{label}</span>
    </div>
    <span class="badge" class:on={isConfigured}>
      {isConfigured ? 'Configured' : 'Not configured'}
    </span>
  </div>

  {#if !editing}
    <div class="masked">{maskKey(currentKey)}</div>
    <div class="actions">
      <button class="btn btn-secondary" onclick={startEdit}>Edit</button>
      <button class="btn btn-danger" onclick={clear}>Remove</button>
      <button class="link" onclick={() => openUrl(docsUrl).catch(() => {})}>Get key →</button>
    </div>
  {:else}
    <div class="key-row">
      <input
        class="key-input"
        type={show ? 'text' : 'password'}
        bind:value={draft}
        placeholder={placeholder}
        spellcheck="false"
        autocomplete="off"
        onkeydown={(e) => { if (e.key === 'Enter') save(); if (e.key === 'Escape') cancelEdit(); }}
      />
      <button class="ghost" onclick={() => show = !show}>
        {show ? 'Hide' : 'Show'}
      </button>
    </div>
    {#if error}<div class="error">{error}</div>{/if}
    <div class="actions">
      <button class="btn btn-primary" onclick={save} disabled={saving || !draft.trim()}>
        {saving ? 'Saving…' : 'Save'}
      </button>
      {#if isConfigured}
        <button class="btn btn-secondary" onclick={cancelEdit}>Cancel</button>
      {/if}
      <button class="link" onclick={() => openUrl(docsUrl).catch(() => {})}>Get key →</button>
    </div>
  {/if}
</div>

<style>
  .card {
    background: color-mix(in srgb, var(--bg-tertiary) 70%, transparent);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }
  .card:hover {
    border-color: color-mix(in srgb, var(--accent) 22%, var(--border));
  }
  .card.configured { border-color: color-mix(in srgb, var(--success) 35%, var(--border)); }
  .card.configured:hover {
    box-shadow: 0 4px 14px color-mix(in srgb, var(--success) 12%, transparent);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .ident { display: flex; align-items: center; gap: 8px; }
  .label { font-size: 13px; font-weight: 600; color: var(--text-primary); }
  .badge {
    font-size: 10px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 2px 8px;
    border-radius: 10px;
    background: var(--bg-surface);
    color: var(--text-muted);
    border: 1px solid var(--border);
  }
  .badge.on {
    background: color-mix(in srgb, var(--success) 18%, transparent);
    color: var(--success);
    border-color: color-mix(in srgb, var(--success) 35%, transparent);
  }

  .masked {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-secondary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 7px 10px;
    user-select: none;
  }

  .key-row { display: flex; gap: 6px; }
  .key-input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 7px 10px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 6px;
    outline: none;
  }
  .key-input:focus { border-color: var(--accent); }
  .ghost {
    background: var(--bg-surface);
    color: var(--text-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0 12px;
    font-size: 11px;
    cursor: pointer;
  }
  .ghost:hover { color: var(--text-primary); background: var(--border); }

  .error {
    font-size: 11px;
    color: var(--error);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .btn {
    padding: 5px 12px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.12s ease, border-color 0.12s ease, transform 0.12s ease, opacity 0.12s ease;
  }
  .btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .btn:not(:disabled):active { transform: translateY(1px); }
  .btn:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 1px;
  }
  .btn-primary {
    background: var(--accent);
    color: var(--bg-tertiary);
  }
  .btn-primary:not(:disabled):hover { opacity: 0.9; }
  .btn-secondary {
    background: var(--bg-surface);
    color: var(--text-primary);
    border-color: var(--border);
  }
  .btn-secondary:hover { background: var(--border); }
  .btn-danger {
    background: transparent;
    color: var(--error);
    border-color: color-mix(in srgb, var(--error) 30%, transparent);
  }
  .btn-danger:hover {
    background: color-mix(in srgb, var(--error) 12%, transparent);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
  }
  .link {
    margin-left: auto;
    font-size: 11px;
    color: var(--accent);
    background: none;
    border: none;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
    transition: background 0.12s ease;
  }
  .link:hover {
    text-decoration: underline;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .link:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 1px;
  }
</style>
