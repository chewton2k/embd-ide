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
  import { updateFileContent, markFileSaved, autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers } from './stores.ts';

  let { filePath }: { filePath: string } = $props();
  let editorContainer: HTMLDivElement;
  let view: EditorView | null = null;
  let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let saving = $state(false);

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
            updateFileContent(path, update.state.doc.toString());
            scheduleAutosave(path);
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

<div class="editor-wrapper" bind:this={editorContainer}></div>

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
</style>
