<script lang="ts">
  import { RefreshCw, Globe, ExternalLink, PictureInPicture2 } from 'lucide-svelte';
  import { open as openExternal } from '@tauri-apps/plugin-shell';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { previewUrl } from '../../modules';
  import { log } from '../../modules/logging';
  import { get } from 'svelte/store';

  // ── Constants ──────────────────────────────────────────────────
  const STUCK_LOAD_TIMEOUT_MS = 6000;
  const POPUP_LABEL_PREFIX = 'preview-popup-';

  // ── State ──────────────────────────────────────────────────────
  const defaultUrl = get(previewUrl) || 'http://localhost:3000';
  let url = $state(defaultUrl);
  let inputValue = $state(defaultUrl);
  let nonce = $state(0);
  let loading = $state(false);

  /** True once the iframe has fired onload at least once for the current URL/nonce. */
  let loadedOnce = $state(false);
  /** True when the iframe hasn't fired onload within the timeout — likely X-Frame-Options. */
  let loadStuck = $state(false);
  let loadTimer: ReturnType<typeof setTimeout> | null = null;

  /** Tracks the previously-opened popup label so we can re-focus instead of re-spawning. */
  let activePopupLabel: string | null = null;

  // ── Helpers ────────────────────────────────────────────────────

  /**
   * Normalize + validate a user-entered URL.
   * Returns the canonical URL string or null if invalid / unsupported scheme.
   * Only http(s) is allowed to avoid opening arbitrary schemes (file://, javascript:, etc.).
   */
  function normalizeUrl(raw: string): string | null {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(trimmed) ? trimmed : 'http://' + trimmed;
    try {
      const parsed = new URL(withScheme);
      if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return null;
      return parsed.toString();
    } catch {
      return null;
    }
  }

  function clearLoadTimer() {
    if (loadTimer) { clearTimeout(loadTimer); loadTimer = null; }
  }

  function scheduleStuckCheck() {
    clearLoadTimer();
    loadTimer = setTimeout(() => {
      loadTimer = null;
      // onload for a frame-blocked iframe may or may not fire depending on browser;
      // when it does, it fires almost immediately with a blank doc. If we've been
      // spinning for the full timeout and no load event, assume it's blocked.
      if (!loadedOnce) loadStuck = true;
    }, STUCK_LOAD_TIMEOUT_MS);
  }

  function navigate() {
    const target = normalizeUrl(inputValue);
    if (!target) return;
    url = target;
    inputValue = target;
    nonce++;
    loading = true;
    loadedOnce = false;
    loadStuck = false;
    scheduleStuckCheck();
  }

  function reload() {
    if (!url) return;
    nonce++;
    loading = true;
    loadedOnce = false;
    loadStuck = false;
    scheduleStuckCheck();
  }

  function openInBrowser() {
    const target = normalizeUrl(url);
    if (!target) return;
    openExternal(target).catch((e) => log.error('Failed to open external', e));
  }

  /**
   * Opens the preview in a dedicated Tauri WebviewWindow. Unlike iframes, a native
   * webview is not subject to X-Frame-Options / CSP frame-ancestors restrictions,
   * so Next.js, SSR apps, and anything with frame-blocking headers render correctly.
   */
  async function openInPopup() {
    const target = normalizeUrl(url);
    if (!target) return;

    // Re-use the active popup if it's still alive.
    if (activePopupLabel) {
      try {
        const existing = await WebviewWindow.getByLabel(activePopupLabel);
        if (existing) {
          try { await existing.show(); } catch { /* ignore */ }
          try { await existing.setFocus(); } catch { /* ignore */ }
          return;
        }
      } catch { /* fall through and create a new one */ }
      activePopupLabel = null;
    }

    // Unique label so multiple popups don't clash if the user opens several.
    const label = `${POPUP_LABEL_PREFIX}${Date.now().toString(36)}`;
    try {
      const win = new WebviewWindow(label, {
        url: target,
        title: `Preview · ${new URL(target).host}`,
        width: 1100,
        height: 760,
        minWidth: 600,
        minHeight: 400,
        resizable: true,
        center: true,
        focus: true,
      });
      activePopupLabel = label;
      win.once('tauri://error', (e) => {
        log.error('Preview popup failed to open', e);
        activePopupLabel = null;
      });
      win.once('tauri://destroyed', () => {
        if (activePopupLabel === label) activePopupLabel = null;
      });
    } catch (e) {
      log.error('openInPopup failed', e);
      activePopupLabel = null;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') navigate();
  }

  function onLoad() {
    loading = false;
    loadedOnce = true;
    loadStuck = false;
    clearLoadTimer();
  }

  // Start the initial stuck-load check when the component mounts.
  $effect(() => {
    if (url && !loadedOnce) scheduleStuckCheck();
    return () => clearLoadTimer();
  });

  const isLocal = $derived.by(() => {
    try {
      const h = new URL(url).hostname;
      return h === 'localhost' || h === '127.0.0.1' || h === '0.0.0.0' || h === '[::1]';
    } catch {
      return false;
    }
  });
</script>

<div class="preview-panel">
  <div class="address-bar">
    <button class="bar-btn" onclick={reload} title="Reload" aria-label="Reload preview">
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
        autocapitalize="off"
        autocorrect="off"
      />
    </div>
    <button class="bar-btn" onclick={openInPopup} title="Open in Leo window (bypasses iframe restrictions)" aria-label="Open in new window">
      <PictureInPicture2 size={13} />
    </button>
    <button class="bar-btn" onclick={openInBrowser} title="Open in system browser" aria-label="Open in external browser">
      <ExternalLink size={13} />
    </button>
  </div>

  {#if loadStuck}
    <!-- Detected that the iframe didn't load within the timeout — most likely
         blocked by X-Frame-Options or CSP frame-ancestors (common on Next.js
         dev servers with middleware, auth pages, and production builds). -->
    <div class="block-hint">
      <span>
        This site is blocking iframe embedding (common for Next.js / SSR apps).
      </span>
      <button class="hint-action" onclick={openInPopup}>
        <PictureInPicture2 size={11} /> Open in window
      </button>
    </div>
  {:else if !isLocal && url}
    <div class="xfo-hint">
      Some sites block embedding. If the page is blank, open in a window.
    </div>
  {/if}

  <div class="iframe-container">
    {#if url}
      {#key `${url}#${nonce}`}
        <iframe
          src={url}
          title="Preview"
          class="preview-iframe"
          referrerpolicy="no-referrer-when-downgrade"
          sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox allow-modals allow-downloads allow-storage-access-by-user-activation allow-top-navigation-by-user-activation"
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

  .block-hint {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 12px;
    font-size: 11px;
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    border-bottom: 1px solid var(--border);
  }

  .hint-action {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: 4px;
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 11px;
    border: 1px solid var(--border);
    flex-shrink: 0;
  }
  .hint-action:hover {
    background: var(--bg-tertiary);
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
