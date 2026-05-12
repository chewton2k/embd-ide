<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion, getTauriVersion } from '@tauri-apps/api/app';
  import { open as openUrl } from '@tauri-apps/plugin-shell';

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

<div class="hero">
  <img src="/embd.png" alt="embd" class="logo" />
  <div class="title">embd</div>
  <div class="subtitle">A minimal Tauri-based code IDE.</div>
  <div class="version">Version {appVersion}</div>
</div>

<div class="grid">
  <div class="cell"><span class="k">Platform</span><span class="v">{osPlatform} ({osArch})</span></div>
  <div class="cell"><span class="k">Tauri</span><span class="v">{tauriVersion}</span></div>
  <div class="cell"><span class="k">Bundle ID</span><span class="v">com.embd.ide</span></div>
  <div class="cell"><span class="k">License</span><span class="v">MIT</span></div>
</div>

<div class="links">
  <button class="link-btn" onclick={() => openExternal('https://github.com/')}>GitHub</button>
  <button class="link-btn" onclick={() => openExternal('https://github.com/')}>Report an issue</button>
</div>

<style>
  .hero {
    text-align: center;
    padding: 24px 0 28px;
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
    margin: 4px 0 10px;
  }
  .version {
    font-size: 11px;
    color: var(--text-secondary);
    font-family: var(--font-mono, monospace);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--border);
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .cell {
    background: var(--bg-tertiary);
    padding: 12px 14px;
    display: flex; justify-content: space-between; align-items: center;
  }
  .k { font-size: 11px; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.4px; }
  .v { font-size: 12px; color: var(--text-primary); font-family: var(--font-mono, monospace); }
  .links {
    display: flex; gap: 10px;
    margin-top: 24px;
    justify-content: center;
  }
  .link-btn {
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border);
    padding: 7px 16px;
    border-radius: 5px;
    font-size: 12px;
    cursor: pointer;
  }
  .link-btn:hover { background: var(--border); }
</style>
