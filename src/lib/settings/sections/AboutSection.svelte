<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion, getTauriVersion } from '@tauri-apps/api/app';
  import { open as openUrl } from '@tauri-apps/plugin-shell';
  import { Bug, ExternalLink } from 'lucide-svelte';
  import Icon from '@iconify/svelte';

  let appVersion = $state('—');
  let tauriVersion = $state('—');
  const userAgent = typeof navigator !== 'undefined' ? navigator.userAgent : '';
  const osPlatform = /Mac/i.test(userAgent) ? 'macOS'
    : /Win/i.test(userAgent) ? 'Windows'
    : /Linux/i.test(userAgent) ? 'Linux'
    : 'Unknown';
  const osArch = /arm64|aarch64/i.test(userAgent) ? 'arm64'
    : /x86_64|x64|Win64/i.test(userAgent) ? 'x64'
    : '—';

  onMount(async () => {
    try { appVersion = await getVersion(); } catch {}
    try { tauriVersion = await getTauriVersion(); } catch {}
  });

  async function openExternal(url: string) {
    try { await openUrl(url); } catch {}
  }
</script>

<div class="hero" data-setting="about">
  <img src="/leo.png" alt="leo" class="logo" />
  <div class="title">leo</div>
  <div class="subtitle">A minimal Tauri-based code IDE.</div>
  <div class="version-pill">
    <span class="version-dot" aria-hidden="true"></span>
    <span>Version {appVersion}</span>
  </div>
</div>

<div class="grid">
  <div class="cell"><span class="k">Platform</span><span class="v">{osPlatform} ({osArch})</span></div>
  <div class="cell"><span class="k">Tauri</span><span class="v">{tauriVersion}</span></div>
  <div class="cell"><span class="k">Bundle ID</span><span class="v">com.leo.ide</span></div>
  <div class="cell"><span class="k">License</span><span class="v">Apache 2.0</span></div>
</div>

<div class="links">
  <button class="link-btn" onclick={() => openExternal('https://github.com/')}>
    <Icon icon="simple-icons:github" width={13} height={13} />
    <span>GitHub</span>
    <ExternalLink size={11} class="external" />
  </button>
  <button class="link-btn" onclick={() => openExternal('https://github.com/')}>
    <Bug size={13} />
    <span>Report an issue</span>
    <ExternalLink size={11} class="external" />
  </button>
</div>

<style>
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 28px 0 28px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 24px;
  }
  .logo {
    width: 64px; height: 64px;
    border-radius: 14px;
    margin-bottom: 14px;
  }
  .title {
    font-size: 22px; font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.5px;
  }
  .subtitle {
    font-size: 12px;
    color: var(--text-muted);
    margin: 4px 0 14px;
  }

  /* Version pill — replaces the previous monospace string with a small
     status-style chip so the hero feels like a real about pane. */
  .version-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    font-size: 11px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .version-dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--success, var(--accent));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success, var(--accent)) 18%, transparent);
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--border);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
  }
  .cell {
    background: var(--bg-tertiary);
    padding: 12px 14px;
    display: flex; justify-content: space-between; align-items: center;
  }
  .k {
    font-size: 10.5px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 600;
  }
  .v {
    font-size: 12px;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
  }

  .links {
    display: flex; gap: 10px;
    margin-top: 22px;
    justify-content: center;
    flex-wrap: wrap;
  }
  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border);
    padding: 7px 14px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease, transform 0.12s ease;
  }
  .link-btn:hover {
    background: var(--bg-tertiary);
    border-color: color-mix(in srgb, var(--accent) 28%, var(--border));
    transform: translateY(-1px);
  }
  .link-btn:active { transform: translateY(0); }
  .link-btn:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 2px;
  }
  .link-btn :global(.external) {
    color: var(--text-muted);
    margin-left: 2px;
  }
</style>
