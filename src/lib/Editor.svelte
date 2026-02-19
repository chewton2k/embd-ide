<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { get } from 'svelte/store';
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from '@codemirror/view';
  import { EditorState, Compartment } from '@codemirror/state';
  import { defaultKeymap, indentWithTab, history, historyKeymap } from '@codemirror/commands';
  import { javascript } from '@codemirror/lang-javascript';
  import { python } from '@codemirror/lang-python';
  import { html } from '@codemirror/lang-html';
  import { css } from '@codemirror/lang-css';
  import { json } from '@codemirror/lang-json';
  import { cpp } from '@codemirror/lang-cpp';
  import { java } from '@codemirror/lang-java';
  import { rust } from '@codemirror/lang-rust';
  import { go } from '@codemirror/lang-go';
  import { markdown } from '@codemirror/lang-markdown';
  import { php } from '@codemirror/lang-php';
  import { sql } from '@codemirror/lang-sql';
  import { xml } from '@codemirror/lang-xml';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { autocompletion } from '@codemirror/autocomplete';
  import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
  import { marked } from 'marked';
  import { updateFileContent, markFileSaved, autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers } from './stores.ts';

  let { filePath }: { filePath: string } = $props();
  let editorContainer: HTMLDivElement;
  let view: EditorView | null = null;
  let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let saving = $state(false);

  // Markdown preview
  let isMarkdown = $derived(/\.(md|mdx|markdown)$/i.test(filePath));
  let showPreview = $state(true);
  let previewHtml = $state('');

  // Compartments for dynamic reconfiguration
  const fontSizeComp = new Compartment();
  const tabSizeComp = new Compartment();
  const wordWrapComp = new Compartment();
  const lineNumbersComp = new Compartment();

  function getLanguage(path: string) {
    const ext = path.split('.').pop()?.toLowerCase();
    const name = path.split('/').pop()?.toLowerCase() || '';
    switch (ext) {
      case 'js': case 'jsx': return javascript();
      case 'ts': case 'tsx': return javascript({ typescript: true, jsx: ext?.includes('x') });
      case 'py': case 'pyw': case 'pyi': return python();
      case 'html': case 'svelte': case 'vue': return html();
      case 'css': case 'scss': case 'less': return css();
      case 'json': return json();
      case 'c': case 'h': case 'cpp': case 'cxx': case 'cc':
      case 'hpp': case 'hxx': case 'hh': case 'ino': return cpp();
      case 'java': case 'kt': case 'kts': return java();
      case 'rs': return rust();
      case 'go': return go();
      case 'md': case 'mdx': case 'markdown': return markdown();
      case 'php': return php();
      case 'sql': return sql();
      case 'xml': case 'xsl': case 'xsd': case 'svg': case 'plist': return xml();
      case 'yaml': case 'yml': case 'toml': case 'ini': case 'conf': case 'cfg': return null;
      case 'sh': case 'bash': case 'zsh': case 'fish': return null;
      case 'txt': case 'text': case 'log': case 'env': return null;
      case 'gitignore': case 'gitattributes': case 'editorconfig': return null;
      default:
        // Files with no extension (Makefile, Dockerfile, etc.)
        if (name === 'makefile') return null;
        if (name === 'dockerfile') return null;
        return null;
    }
  }

  function updatePreview(content: string) {
    if (isMarkdown && showPreview) {
      previewHtml = marked.parse(content) as string;
    }
  }

  function scheduleAutosave(path: string) {
    if (autosaveTimer) clearTimeout(autosaveTimer);
    if (!get(autosaveEnabled)) return;

    autosaveTimer = setTimeout(() => {
      saveFile(path);
    }, get(autosaveDelay));
  }

  async function loadFile(path: string) {
    // Clear any pending autosave for the previous file
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
      autosaveTimer = null;
    }

    try {
      const content = await invoke<string>('read_file_content', { path });
      createEditor(content, path);
      updatePreview(content);
    } catch (e) {
      createEditor(`// Error loading file: ${e}`, path);
    }
  }

  function createEditor(content: string, path: string) {
    if (view) view.destroy();

    const lang = getLanguage(path);
    const state = EditorState.create({
      doc: content,
      extensions: [
        lineNumbersComp.of(get(editorLineNumbers) ? lineNumbers() : []),
        highlightActiveLine(),
        drawSelection(),
        history(),
        autocompletion(),
        highlightSelectionMatches(),
        ...(lang ? [lang] : []),
        oneDark,
        fontSizeComp.of(EditorView.theme({
          '&': { fontSize: get(editorFontSize) + 'px' },
          '.cm-gutters': { fontSize: get(editorFontSize) + 'px' },
        })),
        tabSizeComp.of(EditorState.tabSize.of(get(editorTabSize))),
        wordWrapComp.of(get(editorWordWrap) ? EditorView.lineWrapping : []),
        keymap.of([
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
          indentWithTab,
          { key: 'Mod-s', run: () => { saveFile(path); return true; } }
        ]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            const content = update.state.doc.toString();
            updateFileContent(path, content);
            scheduleAutosave(path);
            updatePreview(content);
          }
        }),
        EditorView.theme({
          '&': { height: '100%' },
          '.cm-scroller': { overflow: 'auto' },
        }),
      ],
    });

    view = new EditorView({ state, parent: editorContainer });
  }

  async function saveFile(path: string) {
    if (!view || saving) return;
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
      autosaveTimer = null;
    }

    saving = true;
    const content = view.state.doc.toString();
    try {
      await invoke('write_file_content', { path, content });
      markFileSaved(path);
    } catch (e) {
      console.error('Failed to save:', e);
    }
    saving = false;
  }

  // Reactively reconfigure editor when settings change
  $effect(() => {
    const size = $editorFontSize;
    if (view) {
      view.dispatch({ effects: fontSizeComp.reconfigure(EditorView.theme({
        '&': { fontSize: size + 'px' },
        '.cm-gutters': { fontSize: size + 'px' },
      })) });
    }
  });

  $effect(() => {
    const size = $editorTabSize;
    if (view) {
      view.dispatch({ effects: tabSizeComp.reconfigure(EditorState.tabSize.of(size)) });
    }
  });

  $effect(() => {
    const wrap = $editorWordWrap;
    if (view) {
      view.dispatch({ effects: wordWrapComp.reconfigure(wrap ? EditorView.lineWrapping : []) });
    }
  });

  $effect(() => {
    const show = $editorLineNumbers;
    if (view) {
      view.dispatch({ effects: lineNumbersComp.reconfigure(show ? lineNumbers() : []) });
    }
  });

  onMount(() => {
    loadFile(filePath);
  });

  onDestroy(() => {
    // Save before destroying if there are pending changes
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
      if (view && get(autosaveEnabled)) {
        const content = view.state.doc.toString();
        invoke('write_file_content', { path: filePath, content }).then(() => {
          markFileSaved(filePath);
        });
      }
    }
    if (view) view.destroy();
  });

  $effect(() => {
    if (filePath && editorContainer) {
      loadFile(filePath);
    }
  });
</script>

<div class="editor-root" class:md-split={isMarkdown && showPreview}>
  <div class="editor-pane">
    <div class="editor-wrapper" bind:this={editorContainer}></div>
    {#if isMarkdown && !showPreview}
      <button class="md-open-preview" onclick={() => showPreview = true} title="Open preview">
        <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
          <path d="M8 3C4.5 3 1.5 5.5 1 8c.5 2.5 3.5 5 7 5s6.5-2.5 7-5c-.5-2.5-3.5-5-7-5z" />
          <circle cx="8" cy="8" r="2" />
        </svg>
      </button>
    {/if}
  </div>
  {#if isMarkdown && showPreview}
    <div class="md-divider"></div>
    <div class="md-preview-pane">
      <div class="md-preview-header">
        <span class="md-preview-title">Preview</span>
        <button class="md-preview-toggle" onclick={() => showPreview = false} title="Close preview">
          <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M4 4l8 8M12 4l-8 8" />
          </svg>
        </button>
      </div>
      <div class="md-preview-content">
        {@html previewHtml}
      </div>
    </div>
  {/if}
</div>

<style>
  .editor-wrapper {
    height: 100%;
    width: 100%;
    overflow: hidden;
  }

  .editor-wrapper :global(.cm-editor) {
    height: 100%;
  }

  .editor-wrapper :global(.cm-gutters) {
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
  }

  /* Root container — always present */
  .editor-root {
    height: 100%;
    width: 100%;
    overflow: hidden;
  }

  .editor-root.md-split {
    display: flex;
  }

  .editor-pane {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    position: relative;
    height: 100%;
  }

  .md-divider {
    width: 1px;
    background: var(--border);
    flex-shrink: 0;
  }

  .md-preview-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-primary);
  }

  .md-preview-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .md-preview-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .md-preview-toggle {
    color: var(--text-muted);
    padding: 2px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    transition: all 0.1s;
  }

  .md-preview-toggle:hover {
    color: var(--text-primary);
    background: var(--bg-surface);
  }

  .md-preview-content {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px;
    font-size: 14px;
    line-height: 1.7;
    color: var(--text-primary);
  }

  /* Markdown rendered styles */
  .md-preview-content :global(h1) {
    font-size: 1.8em;
    font-weight: 700;
    margin: 0 0 16px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
    color: var(--text-primary);
  }

  .md-preview-content :global(h2) {
    font-size: 1.4em;
    font-weight: 600;
    margin: 24px 0 12px;
    padding-bottom: 6px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    color: var(--text-primary);
  }

  .md-preview-content :global(h3) {
    font-size: 1.15em;
    font-weight: 600;
    margin: 20px 0 8px;
    color: var(--text-primary);
  }

  .md-preview-content :global(h4),
  .md-preview-content :global(h5),
  .md-preview-content :global(h6) {
    font-size: 1em;
    font-weight: 600;
    margin: 16px 0 8px;
    color: var(--text-secondary);
  }

  .md-preview-content :global(p) {
    margin: 0 0 12px;
  }

  .md-preview-content :global(a) {
    color: var(--accent);
    text-decoration: none;
  }

  .md-preview-content :global(a:hover) {
    text-decoration: underline;
  }

  .md-preview-content :global(strong) {
    font-weight: 600;
    color: var(--text-primary);
  }

  .md-preview-content :global(code) {
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    background: var(--bg-tertiary);
    padding: 2px 5px;
    border-radius: 3px;
    font-size: 0.88em;
    color: var(--accent);
  }

  .md-preview-content :global(pre) {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 14px 16px;
    overflow-x: auto;
    margin: 0 0 16px;
  }

  .md-preview-content :global(pre code) {
    background: none;
    padding: 0;
    color: var(--text-primary);
    font-size: 13px;
    line-height: 1.5;
  }

  .md-preview-content :global(blockquote) {
    border-left: 3px solid var(--accent);
    margin: 0 0 12px;
    padding: 4px 16px;
    color: var(--text-secondary);
    background: color-mix(in srgb, var(--accent) 5%, transparent);
    border-radius: 0 4px 4px 0;
  }

  .md-preview-content :global(ul),
  .md-preview-content :global(ol) {
    margin: 0 0 12px;
    padding-left: 24px;
  }

  .md-preview-content :global(li) {
    margin: 4px 0;
  }

  .md-preview-content :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 24px 0;
  }

  .md-preview-content :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 0 0 16px;
    font-size: 13px;
  }

  .md-preview-content :global(th),
  .md-preview-content :global(td) {
    border: 1px solid var(--border);
    padding: 8px 12px;
    text-align: left;
  }

  .md-preview-content :global(th) {
    background: var(--bg-secondary);
    font-weight: 600;
  }

  .md-preview-content :global(tr:nth-child(even)) {
    background: color-mix(in srgb, var(--bg-secondary) 30%, transparent);
  }

  .md-preview-content :global(img) {
    max-width: 100%;
    border-radius: 6px;
    margin: 8px 0;
  }

  .md-preview-content :global(input[type="checkbox"]) {
    margin-right: 6px;
  }

  /* Open preview button (shown when preview is closed) */
  .md-open-preview {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text-muted);
    padding: 4px 6px;
    border-radius: 4px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 4px;
    transition: all 0.15s;
    z-index: 10;
    opacity: 0.7;
  }

  .md-open-preview:hover {
    opacity: 1;
    color: var(--text-primary);
    background: var(--bg-surface);
  }
</style>
