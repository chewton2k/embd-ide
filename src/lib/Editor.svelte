<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { watch } from '@tauri-apps/plugin-fs';
  import type { UnwatchFn } from '@tauri-apps/plugin-fs';
  import { get } from 'svelte/store';
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection, gutter, GutterMarker } from '@codemirror/view';
  import { EditorState, Compartment, RangeSetBuilder } from '@codemirror/state';
  import { defaultKeymap, indentWithTab, history, historyKeymap, cursorDocStart, cursorDocEnd, cursorLineBoundaryForward, cursorLineBoundaryBackward, selectDocStart, selectDocEnd, selectLineBoundaryForward, selectLineBoundaryBackward, cursorCharLeft, cursorCharRight, cursorLineUp, cursorLineDown, selectCharLeft, selectCharRight, selectLineUp, selectLineDown, deleteLine, cursorPageDown, cursorPageUp } from '@codemirror/commands';
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
  import { prolog } from 'codemirror-lang-prolog';
  import { scheme } from 'codemirror-lang-scheme';
  import { StreamLanguage } from '@codemirror/language';
  import { oCaml } from '@codemirror/legacy-modes/mode/mllike';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { autocompletion, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
  import { bracketMatching, indentOnInput, foldGutter, foldKeymap } from '@codemirror/language';
  import { searchKeymap, highlightSelectionMatches, openSearchPanel } from '@codemirror/search';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { updateFileContent, markFileSaved, autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers, projectRoot } from './stores.ts';

  let { filePath }: { filePath: string } = $props();

  // Git gutter markers
  class AddedMarker extends GutterMarker {
    toDOM() {
      const el = document.createElement('div');
      el.className = 'cm-git-added';
      return el;
    }
  }
  class ModifiedMarker extends GutterMarker {
    toDOM() {
      const el = document.createElement('div');
      el.className = 'cm-git-modified';
      return el;
    }
  }
  class DeletedMarker extends GutterMarker {
    toDOM() {
      const el = document.createElement('div');
      el.className = 'cm-git-deleted';
      return el;
    }
  }

  const addedMarker = new AddedMarker();
  const modifiedMarker = new ModifiedMarker();
  const deletedMarker = new DeletedMarker();

  interface DiffRange {
    kind: string;
    start: number;
    end: number;
  }

  const gitGutterComp = new Compartment();
  let editorContainer: HTMLDivElement;
  let view: EditorView | null = null;
  let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let saving = $state(false);

  // File watcher for external changes
  let unwatchFn: UnwatchFn | null = null;
  let watchDebounce: ReturnType<typeof setTimeout> | null = null;
  let lastSavedContent: string = '';
  let ignoreNextWatch = false;

  // Markdown preview
  let isMarkdown = $derived(/\.(md|mdx|markdown)$/i.test(filePath));
  let showPreview = $state(true);
  let previewHtml = $state('');

  // Compartments for dynamic reconfiguration
  const fontSizeComp = new Compartment();
  const tabSizeComp = new Compartment();
  const wordWrapComp = new Compartment();
  const lineNumbersComp = new Compartment();

  async function updateGitGutter(path: string) {
    if (!view) return;
    const root = get(projectRoot);
    if (!root || !path.startsWith(root)) return;
    const relPath = path.slice(root.length + 1);
    try {
      const ranges = await invoke<DiffRange[]>('git_diff_line_ranges', { repoPath: root, filePath: relPath });
      if (!view) return;
      const doc = view.state.doc;
      const builder = new RangeSetBuilder<GutterMarker>();
      // RangeSetBuilder requires positions in ascending order
      const marks: { pos: number; marker: GutterMarker }[] = [];
      for (const r of ranges) {
        const marker = r.kind === 'add' ? addedMarker : r.kind === 'del' ? deletedMarker : modifiedMarker;
        for (let line = r.start; line <= r.end; line++) {
          if (line >= 1 && line <= doc.lines) {
            const pos = doc.line(line).from;
            marks.push({ pos, marker });
          }
        }
      }
      marks.sort((a, b) => a.pos - b.pos);
      for (const m of marks) {
        builder.add(m.pos, m.pos, m.marker);
      }
      const markers = builder.finish();
      view.dispatch({
        effects: gitGutterComp.reconfigure(
          gutter({
            class: 'cm-git-gutter',
            markers: () => markers,
          })
        ),
      });
    } catch {
      // Not in a git repo or command failed — clear gutter
      if (view) {
        view.dispatch({ effects: gitGutterComp.reconfigure([]) });
      }
    }
  }

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
      case 'pl': case 'pro': case 'prolog': return prolog();
      case 'scm': case 'ss': case 'rkt': return scheme();
      case 'ml': case 'mli': case 'ocaml': return StreamLanguage.define(oCaml);
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
      previewHtml = DOMPurify.sanitize(marked.parse(content) as string);
    }
  }

  function scheduleAutosave(path: string) {
    if (autosaveTimer) clearTimeout(autosaveTimer);
    if (!get(autosaveEnabled)) return;

    autosaveTimer = setTimeout(() => {
      saveFile(path);
    }, get(autosaveDelay));
  }

  async function stopWatching() {
    if (unwatchFn) {
      unwatchFn();
      unwatchFn = null;
    }
    if (watchDebounce) {
      clearTimeout(watchDebounce);
      watchDebounce = null;
    }
  }

  async function startWatching(path: string) {
    await stopWatching();
    try {
      unwatchFn = await watch(path, () => {
        if (ignoreNextWatch) {
          ignoreNextWatch = false;
          return;
        }
        if (watchDebounce) clearTimeout(watchDebounce);
        watchDebounce = setTimeout(() => reloadFromDisk(path), 200);
      }, { recursive: false });
    } catch {
      // File may not exist yet or watching not supported
    }
  }

  async function reloadFromDisk(path: string) {
    if (!view) return;
    try {
      const diskContent = await invoke<string>('read_file_content', { path });
      // Only reload if content actually changed from what we know
      if (diskContent === lastSavedContent) return;
      if (diskContent === view.state.doc.toString()) return;

      // Preserve cursor position
      const cursorPos = Math.min(view.state.selection.main.head, diskContent.length);

      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: diskContent },
        selection: { anchor: cursorPos },
      });
      lastSavedContent = diskContent;
      markFileSaved(path);
      updatePreview(diskContent);
      updateGitGutter(path);
    } catch {
      // File might have been deleted
    }
  }

  async function loadFile(path: string) {
    // Clear any pending autosave for the previous file
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
      autosaveTimer = null;
    }

    try {
      const content = await invoke<string>('read_file_content', { path });
      lastSavedContent = content;
      createEditor(content, path);
      updatePreview(content);
      // Load git gutter after editor is created
      updateGitGutter(path);
      // Start watching for external changes
      startWatching(path);
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
        closeBrackets(),
        bracketMatching(),
        indentOnInput(),
        foldGutter(),
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
        gitGutterComp.of([]),
        keymap.of([
          ...closeBracketsKeymap,
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
          ...foldKeymap,
          indentWithTab,
          { key: 'Mod-s', run: () => { saveFile(path); return true; } },
          // Emacs navigation
          { key: 'Ctrl-a', run: cursorLineBoundaryBackward, shift: selectLineBoundaryBackward },
          { key: 'Ctrl-e', run: cursorLineBoundaryForward, shift: selectLineBoundaryForward },
          { key: 'Ctrl-f', run: cursorCharRight, shift: selectCharRight },
          { key: 'Ctrl-b', run: cursorCharLeft, shift: selectCharLeft },
          { key: 'Ctrl-p', run: cursorLineUp, shift: selectLineUp },
          { key: 'Ctrl-n', run: cursorLineDown, shift: selectLineDown },
          { key: 'Ctrl-v', run: cursorPageDown },
          // Top/bottom of file
          { key: 'Mod-Up', run: cursorDocStart, shift: selectDocStart },
          { key: 'Mod-Down', run: cursorDocEnd, shift: selectDocEnd },
          // Delete entire line (Cmd+Backspace)
          { key: 'Mod-Backspace', run: deleteLine },
          // Emacs kill line (Ctrl-k): delete from cursor to end of line
          { key: 'Ctrl-k', run: (view) => {
            const { state } = view;
            const range = state.selection.main;
            const line = state.doc.lineAt(range.head);
            const from = range.head;
            const to = from === line.to && line.to < state.doc.length ? line.to + 1 : line.to;
            if (from === to) return false;
            view.dispatch({ changes: { from, to } });
            return true;
          }},
          // Transpose characters (Ctrl-t)
          { key: 'Ctrl-t', run: (view) => {
            const { state } = view;
            const pos = state.selection.main.head;
            if (pos <= 0 || pos >= state.doc.length) return false;
            const before = state.doc.sliceString(pos - 1, pos);
            const after = state.doc.sliceString(pos, pos + 1);
            view.dispatch({ changes: { from: pos - 1, to: pos + 1, insert: after + before } });
            return true;
          }},
          // Delete word backward (Ctrl-w / Alt-Backspace)
          { key: 'Alt-Backspace', run: (view) => {
            const { state } = view;
            const pos = state.selection.main.head;
            if (pos === 0) return false;
            const text = state.doc.sliceString(0, pos);
            const match = text.match(/(?:\s+|\w+|[^\s\w]+)$/);
            const deleteFrom = match ? pos - match[0].length : pos - 1;
            view.dispatch({ changes: { from: deleteFrom, to: pos } });
            return true;
          }},
          // Delete word forward (Alt-d)
          { key: 'Alt-d', run: (view) => {
            const { state } = view;
            const pos = state.selection.main.head;
            if (pos >= state.doc.length) return false;
            const text = state.doc.sliceString(pos);
            const match = text.match(/^(?:\s+|\w+|[^\s\w]+)/);
            const deleteTo = match ? pos + match[0].length : pos + 1;
            view.dispatch({ changes: { from: pos, to: deleteTo } });
            return true;
          }},
          // Word movement (Alt-f forward, Alt-b backward)
          { key: 'Alt-f', run: (view) => {
            const { state } = view;
            const pos = state.selection.main.head;
            const text = state.doc.sliceString(pos);
            const match = text.match(/^(?:\s*\w+|\s*[^\s\w]+|\s+)/);
            const newPos = match ? pos + match[0].length : pos;
            view.dispatch({ selection: { anchor: newPos } });
            return true;
          }},
          { key: 'Alt-b', run: (view) => {
            const { state } = view;
            const pos = state.selection.main.head;
            const text = state.doc.sliceString(0, pos);
            const match = text.match(/(?:\w+\s*|[^\s\w]+\s*|\s+)$/);
            const newPos = match ? pos - match[0].length : pos;
            view.dispatch({ selection: { anchor: newPos } });
            return true;
          }},
          // Go to top/bottom of file (Alt-< / Alt->)
          { key: 'Alt-<', run: cursorDocStart },
          { key: 'Alt->', run: cursorDocEnd },
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
      ignoreNextWatch = true;
      await invoke('write_file_content', { path, content });
      lastSavedContent = content;
      markFileSaved(path);
      updateGitGutter(path);
    } catch (e) {
      console.error('Failed to save:', e);
      ignoreNextWatch = false;
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

  function handleGlobalKeydown(e: KeyboardEvent) {
    // Cmd/Ctrl+F: focus editor and open search panel
    if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
      if (view) {
        e.preventDefault();
        view.focus();
        openSearchPanel(view);
        // Disable autocapitalize on the search inputs
        requestAnimationFrame(() => {
          view.dom.querySelectorAll('.cm-panel.cm-search input').forEach((input) => {
            input.setAttribute('autocapitalize', 'off');
            input.setAttribute('autocorrect', 'off');
          });
        });
      }
    }
  }

  onMount(() => {
    loadFile(filePath);
    window.addEventListener('keydown', handleGlobalKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleGlobalKeydown);
    stopWatching();
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

  .editor-wrapper :global(.cm-git-gutter) {
    width: 6px;
    padding: 0 1px;
  }

  .editor-wrapper :global(.cm-git-added) {
    width: 3px;
    height: 100%;
    background: var(--success, #a6e3a1);
    border-radius: 1px;
  }

  .editor-wrapper :global(.cm-git-modified) {
    width: 3px;
    height: 100%;
    background: var(--accent, #89b4fa);
    border-radius: 1px;
  }

  .editor-wrapper :global(.cm-git-deleted) {
    width: 3px;
    height: 100%;
    background: var(--error, #f38ba8);
    border-radius: 1px;
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
