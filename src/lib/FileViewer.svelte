<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import DOMPurify from 'dompurify';
  import * as pdfjsLib from 'pdfjs-dist';

  // Point PDF.js to its bundled worker
  pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
    'pdfjs-dist/build/pdf.worker.mjs',
    import.meta.url
  ).href;

  let { filePath }: { filePath: string } = $props();

  let dataUrl = $state<string | null>(null);
  let assetUrl = $state<string | null>(null);
  let svgContent = $state<string | null>(null);
  let pdfData = $state<Uint8Array | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let zoom = $state(100);
  let fileSize = $state('');
  let pdfContainer: HTMLDivElement;
  let pdfPageCount = $state(0);

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
      ico: 'image/x-icon', svg: 'image/svg+xml',
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
    assetUrl = null;
    svgContent = null;
    pdfData = null;
    pdfPageCount = 0;
    zoom = 100;

    const type = getFileType(path);

    try {
      if (type === 'svg') {
        const content = await invoke<string>('read_file_content', { path });
        svgContent = DOMPurify.sanitize(content, { USE_PROFILES: { svg: true, svgFilters: true } });
        fileSize = formatSize(new Blob([content]).size);
      } else if (type === 'pdf') {
        const base64 = await invoke<string>('read_file_binary', { path });
        const raw = atob(base64);
        const bytes = new Uint8Array(raw.length);
        for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
        pdfData = bytes;
        fileSize = formatSize(bytes.length);
      } else if (type === 'video' || type === 'audio') {
        assetUrl = convertFileSrc(path);
        const base64 = await invoke<string>('read_file_binary', { path });
        fileSize = formatSize(Math.floor(base64.length * 0.75));
      } else {
        const base64 = await invoke<string>('read_file_binary', { path });
        const mime = getMimeType(path);
        dataUrl = `data:${mime};base64,${base64}`;
        fileSize = formatSize(Math.floor(base64.length * 0.75));
      }
    } catch (e) {
      error = `Failed to load file: ${e}`;
    }

    loading = false;
  }

  async function renderPdf(data: Uint8Array, scale: number) {
    if (!pdfContainer) return;
    pdfContainer.innerHTML = '';

    try {
      const pdf = await pdfjsLib.getDocument({ data }).promise;
      pdfPageCount = pdf.numPages;

      for (let i = 1; i <= pdf.numPages; i++) {
        const page = await pdf.getPage(i);
        const viewport = page.getViewport({ scale: scale / 100 * 1.5 });

        const canvas = document.createElement('canvas');
        canvas.width = viewport.width;
        canvas.height = viewport.height;
        canvas.style.display = 'block';
        canvas.style.marginBottom = '8px';
        canvas.style.borderRadius = '4px';
        canvas.style.boxShadow = '0 1px 4px rgba(0,0,0,0.3)';

        const ctx = canvas.getContext('2d')!;
        await page.render({ canvasContext: ctx, viewport }).promise;

        pdfContainer.appendChild(canvas);
      }
    } catch (e) {
      error = `Failed to render PDF: ${e}`;
    }
  }

  // Re-render PDF when data or zoom changes
  $effect(() => {
    if (pdfData && pdfContainer) {
      renderPdf(pdfData, zoom);
    }
  });

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
      <span class="viewer-meta">{fileSize}{pdfPageCount > 0 ? ` - ${pdfPageCount} page${pdfPageCount !== 1 ? 's' : ''}` : ''}</span>
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

      {:else if getFileType(filePath) === 'pdf' && pdfData}
        <div class="pdf-scroll" bind:this={pdfContainer}></div>

      {:else if getFileType(filePath) === 'video' && assetUrl}
        <video controls src={assetUrl} class="media-player">
          <track kind="captions" />
        </video>

      {:else if getFileType(filePath) === 'audio' && assetUrl}
        <audio controls src={assetUrl} class="audio-player">
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

  .pdf-scroll {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    height: 100%;
    overflow-y: auto;
    padding: 12px;
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
