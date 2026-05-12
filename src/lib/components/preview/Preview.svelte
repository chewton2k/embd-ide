<script lang="ts">
  import { RefreshCw, Globe, ExternalLink } from 'lucide-svelte';
  import { open as openExternal } from '@tauri-apps/plugin-shell';
  import { previewUrl } from '../../modules/stores';
  import { get } from 'svelte/store';

  const defaultUrl = get(previewUrl) || 'http://localhost:3000';
  let url = $state(defaultUrl);
  let inputValue = $state(defaultUrl);
  let nonce = $state(0);
  let loading = $state(false);

  function navigate() {
    let target = inputValue.trim();
    if (!target) return;
    if (!/^https?:\/\//.test(target)) target = 'http://' + target;
    url = target;
    inputValue = target;
    nonce++;
    loading = true;
  }

  function reload() {
    nonce++;
    loading = true;
  }

  function openInBrowser() {
    if (url) openExternal(url).catch(() => {});
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') navigate();
  }

  function onLoad() { loading = false; }

  const isLocal = $derived(() => {
    try {
      const h = new URL(url).hostname;
      return h === 'localhost' || h === '127.0.0.1' || h === '0.0.0.0' || h === '[::1]';
    } catch { return false; }
  });
</script>

<div class="preview-panel">
  <div class="address-bar">
    <button class="bar-btn" onclick={reload} title="Reload">
      <RefreshCw size={13} class={loading ? 'spinning' : ''} />
    </button>
    <div class="url-input-wrap">
      <Globe size={12} />
      <input
        type="text"
        class="url-input"
        bind:value={inputValue}
        onkeydown={handleKeydown}
        placeholder="Enter URL (e.g. localhost:3000)"
        spellcheck="false"
      />
    </div>
    <button class="bar-btn" onclick={openInBrowser} title="Open in browser">
      <ExternalLink size={13} />
    </button>
  </div>

  {#if !isLocal() && url}
    <div class="xfo-hint">
      Some sites block embedding. If the page is blank, use the external link button.
    </div>
  {/if}

  <div class="iframe-container">
    {#if url}
      {#key `${url}#${nonce}`}
        <iframe
          src={url}
          title="Preview"
          class="preview-iframe"
          sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals allow-downloads"
          allow="clipboard-read; clipboard-write; fullscreen"
          onload={onLoad}
        ></iframe>
      {/key}
    {:else}
      <div class="empty-state">
        <Globe size={28} />
        <p class="empty-title">Nothing to preview</p>
        <p class="empty-desc">Enter a URL above to preview your running dev server.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .preview-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-primary);
  }

  .address-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .bar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px; height: 26px;
    border-radius: 5px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .bar-btn:hover { background: var(--bg-surface); color: var(--text-primary); }

  .url-input-wrap {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 20px;
    color: var(--text-muted);
  }
  .url-input-wrap:focus-within { border-color: var(--text-muted); }

  .url-input {
    flex: 1;
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    color: var(--text-primary);
    outline: none;
    font-family: var(--font-mono);
  }
  .url-input::placeholder { color: var(--text-muted); }

  .xfo-hint {
    padding: 5px 12px;
    font-size: 11px;
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 8%, transparent);
    border-bottom: 1px solid var(--border);
  }

  .iframe-container {
    flex: 1;
    min-height: 0;
    background: #fff;
  }

  .preview-iframe {
    width: 100%;
    height: 100%;
    border: none;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 10px;
    color: var(--text-muted);
    background: var(--bg-primary);
  }
  .empty-title { font-size: 14px; font-weight: 500; color: var(--text-secondary); }
  .empty-desc { font-size: 12px; max-width: 280px; text-align: center; line-height: 1.5; }

  :global(.spinning) { animation: spin 0.8s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
