<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import DOMPurify from 'dompurify';

  let { filePath }: { filePath: string } = $props();

  let dataUrl = $state<string | null>(null);
  let svgContent = $state<string | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let zoom = $state(100);
  let fileSize = $state('');

  function getFileType(path: string): 'image' | 'svg' | 'pdf' | 'video' | 'audio' | 'unknown' {
    const ext = path.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'png': case 'jpg': case 'jpeg': case 'gif': case 'webp': case 'bmp': case 'ico':
        return 'image';
      case 'svg':
        return 'svg';
      case 'pdf':
        return 'pdf';
      case 'mp4': case 'webm': case 'mov':
        return 'video';
      case 'mp3': case 'wav': case 'ogg': case 'flac':
        return 'audio';
      default:
        return 'unknown';
    }
  }

  function getMimeType(path: string): string {
    const ext = path.split('.').pop()?.toLowerCase();
    const mimes: Record<string, string> = {
      png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg',
      gif: 'image/gif', webp: 'image/webp', bmp: 'image/bmp',
      ico: 'image/x-icon', svg: 'image/svg+xml', pdf: 'application/pdf',
      mp4: 'video/mp4', webm: 'video/webm', mov: 'video/quicktime',
      mp3: 'audio/mpeg', wav: 'audio/wav', ogg: 'audio/ogg', flac: 'audio/flac',
    };
    return mimes[ext || ''] || 'application/octet-stream';
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function loadFile(path: string) {
    loading = true;
    error = null;
    dataUrl = null;
    svgContent = null;
    zoom = 100;

    const type = getFileType(path);

    try {
      if (type === 'svg') {
        // Load SVG as text so we can render it inline
        const content = await invoke<string>('read_file_content', { path });
        svgContent = DOMPurify.sanitize(content, { USE_PROFILES: { svg: true, svgFilters: true } });
        fileSize = formatSize(new Blob([content]).size);
      } else {
        const base64 = await invoke<string>('read_file_binary', { path });
        const mime = getMimeType(path);
        dataUrl = `data:${mime};base64,${base64}`;
        // Estimate file size from base64 length
        fileSize = formatSize(Math.floor(base64.length * 0.75));
      }
    } catch (e) {
      error = `Failed to load file: ${e}`;
    }

    loading = false;
  }

  function zoomIn() { zoom = Math.min(500, zoom + 25); }
  function zoomOut() { zoom = Math.max(25, zoom - 25); }
  function zoomReset() { zoom = 100; }

  onMount(() => { loadFile(filePath); });

  $effect(() => {
    if (filePath) loadFile(filePath);
  });
</script>

<div class="viewer">
  {#if loading}
    <div class="viewer-status">Loading...</div>
  {:else if error}
    <div class="viewer-status error">{error}</div>
  {:else}
    <!-- Toolbar -->
    <div class="viewer-toolbar">
      <span class="viewer-filename">{filePath.split('/').pop()}</span>
      <span class="viewer-meta">{fileSize}</span>
      <div class="viewer-controls">
        <button onclick={zoomOut} title="Zoom out">-</button>
        <button onclick={zoomReset} title="Reset zoom">{zoom}%</button>
        <button onclick={zoomIn} title="Zoom in">+</button>
      </div>
    </div>

    <!-- Content -->
    <div class="viewer-content">
      {#if getFileType(filePath) === 'svg' && svgContent}
        <div
          class="svg-container"
          style="transform: scale({zoom / 100}); transform-origin: center center;"
        >
          {@html svgContent}
        </div>

      {:else if getFileType(filePath) === 'image' && dataUrl}
        <img
          src={dataUrl}
          alt={filePath.split('/').pop()}
          style="max-width: {zoom}%; max-height: {zoom}%;"
          draggable="false"
        />

      {:else if getFileType(filePath) === 'pdf' && dataUrl}
        <iframe
          src={dataUrl}
          title={filePath.split('/').pop()}
          class="pdf-frame"
        ></iframe>

      {:else if getFileType(filePath) === 'video' && dataUrl}
        <video controls src={dataUrl} class="media-player">
          <track kind="captions" />
        </video>

      {:else if getFileType(filePath) === 'audio' && dataUrl}
        <audio controls src={dataUrl} class="audio-player">
          <track kind="captions" />
        </audio>

      {:else}
        <div class="viewer-status">Preview not available for this file type</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .viewer {
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-primary);
    overflow: hidden;
  }

  .viewer-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 14px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    font-size: 11px;
  }

  .viewer-filename {
    color: var(--text-primary);
    font-weight: 500;
  }

  .viewer-meta {
    color: var(--text-muted);
  }

  .viewer-controls {
    margin-left: auto;
    display: flex;
    gap: 2px;
  }

  .viewer-controls button {
    padding: 2px 8px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-surface);
  }

  .viewer-controls button:hover {
    color: var(--text-primary);
    background: var(--border);
  }

  .viewer-content {
    flex: 1;
    overflow: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .viewer-content img {
    object-fit: contain;
    image-rendering: auto;
    background: repeating-conic-gradient(#2a2a3a 0% 25%, #1e1e2e 0% 50%) 50% / 16px 16px;
    border-radius: 4px;
  }

  .svg-container {
    transition: transform 0.15s ease;
  }

  .svg-container :global(svg) {
    max-width: 100%;
    max-height: 100%;
  }

  .pdf-frame {
    width: 100%;
    height: 100%;
    border: none;
    border-radius: 4px;
  }

  .media-player {
    max-width: 100%;
    max-height: 100%;
    border-radius: 4px;
    outline: none;
  }

  .audio-player {
    width: 400px;
    max-width: 100%;
  }

  .viewer-status {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    font-size: 12px;
  }

  .viewer-status.error {
    color: var(--error);
  }
</style>
