<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { watch } from '@tauri-apps/plugin-fs';
  import type { UnwatchFn } from '@tauri-apps/plugin-fs';
  import { get } from 'svelte/store';
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection, gutter, GutterMarker, ViewPlugin, Decoration, WidgetType } from '@codemirror/view';
  import type { DecorationSet, ViewUpdate } from '@codemirror/view';
  import { EditorState, Compartment, RangeSetBuilder, Transaction } from '@codemirror/state';
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
  // @ts-ignore — no type declarations shipped
  import { scheme } from 'codemirror-lang-scheme';
  import { StreamLanguage } from '@codemirror/language';
  import { oCaml } from '@codemirror/legacy-modes/mode/mllike';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { tags } from '@lezer/highlight';
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
  import { aiDiffExtension, addDiffEffect, clearDiffEffect } from '../../modules/editor/aiDiffExtension';
  import { ghostTextExtension } from '../../modules/editor/ghostText';
  import { pendingEdits } from '../../modules/ai/pendingEdits';
  import { log } from '../../modules/logging';

  function buildEditorTheme(id: EditorThemeId): import('@codemirror/state').Extension {
    const themes: Record<EditorThemeId, () => import('@codemirror/state').Extension> = {
      'one-dark': () => oneDark,
      'dracula': () => buildThemeExt(
        { bg: '#282a36', fg: '#f8f8f2', sel: '#44475a', cursor: '#f8f8f2', gutter: '#21222c', gutterFg: '#6272a4', line: '#44475a33' },
        { keyword: '#ff79c6', string: '#f1fa8c', comment: '#6272a4', function: '#50fa7b', variable: '#f8f8f2', number: '#bd93f9', type: '#8be9fd', operator: '#ff79c6' }
      ),
      'github-dark': () => buildThemeExt(
        { bg: '#0d1117', fg: '#e6edf3', sel: '#264f78', cursor: '#e6edf3', gutter: '#010409', gutterFg: '#6e7681', line: '#161b2233' },
        { keyword: '#ff7b72', string: '#a5d6ff', comment: '#8b949e', function: '#d2a8ff', variable: '#e6edf3', number: '#79c0ff', type: '#79c0ff', operator: '#ff7b72' }
      ),
      'tokyo-night': () => buildThemeExt(
        { bg: '#1a1b26', fg: '#c0caf5', sel: '#33467c', cursor: '#c0caf5', gutter: '#16161e', gutterFg: '#565f89', line: '#292e4233' },
        { keyword: '#bb9af7', string: '#9ece6a', comment: '#565f89', function: '#7aa2f7', variable: '#c0caf5', number: '#ff9e64', type: '#2ac3de', operator: '#89ddff' }
      ),
      'nord': () => buildThemeExt(
        { bg: '#2e3440', fg: '#d8dee9', sel: '#434c5e', cursor: '#d8dee9', gutter: '#2b303b', gutterFg: '#616e88', line: '#3b425233' },
        { keyword: '#81a1c1', string: '#a3be8c', comment: '#616e88', function: '#88c0d0', variable: '#d8dee9', number: '#b48ead', type: '#8fbcbb', operator: '#81a1c1' }
      ),
      'catppuccin-mocha': () => buildThemeExt(
        { bg: '#1e1e2e', fg: '#cdd6f4', sel: '#45475a', cursor: '#cdd6f4', gutter: '#181825', gutterFg: '#6c7086', line: '#31324433' },
        { keyword: '#cba6f7', string: '#a6e3a1', comment: '#6c7086', function: '#89b4fa', variable: '#cdd6f4', number: '#fab387', type: '#94e2d5', operator: '#89dceb' }
      ),
      'rose-pine': () => buildThemeExt(
        { bg: '#191724', fg: '#e0def4', sel: '#403d52', cursor: '#e0def4', gutter: '#1f1d2e', gutterFg: '#6e6a86', line: '#26233a33' },
        { keyword: '#c4a7e7', string: '#f6c177', comment: '#6e6a86', function: '#9ccfd8', variable: '#e0def4', number: '#ebbcba', type: '#9ccfd8', operator: '#31748f' }
      ),
      'github-light': () => buildThemeExt(
        { bg: '#ffffff', fg: '#24292e', sel: '#c8c8fa', cursor: '#24292e', gutter: '#f6f8fa', gutterFg: '#8b949e', line: '#f6f8fa' },
        { keyword: '#cf222e', string: '#0a3069', comment: '#6e7781', function: '#8250df', variable: '#24292e', number: '#0550ae', type: '#0550ae', operator: '#cf222e' }
      ),
      'catppuccin-latte': () => buildThemeExt(
        { bg: '#eff1f5', fg: '#4c4f69', sel: '#bcc0cc', cursor: '#4c4f69', gutter: '#e6e9ef', gutterFg: '#8c8fa1', line: '#ccd0da33' },
        { keyword: '#8839ef', string: '#40a02b', comment: '#8c8fa1', function: '#1e66f5', variable: '#4c4f69', number: '#fe640b', type: '#179299', operator: '#04a5e5' }
      ),
      'solarized-light': () => buildThemeExt(
        { bg: '#fdf6e3', fg: '#657b83', sel: '#eee8d5', cursor: '#657b83', gutter: '#eee8d5', gutterFg: '#93a1a1', line: '#eee8d533' },
        { keyword: '#859900', string: '#2aa198', comment: '#93a1a1', function: '#268bd2', variable: '#657b83', number: '#d33682', type: '#b58900', operator: '#859900' }
      ),
    };
    return themes[id]();
  }

  type ThemeSpec = { bg: string; fg: string; sel: string; cursor: string; gutter: string; gutterFg: string; line: string };
  type SyntaxSpec = { keyword: string; string: string; comment: string; function: string; variable: string; number: string; type: string; operator: string };

  function buildThemeExt(t: ThemeSpec, s: SyntaxSpec): import('@codemirror/state').Extension {
    const theme = EditorView.theme({
      '&': { backgroundColor: t.bg, color: t.fg },
      '.cm-content': { caretColor: t.cursor },
      '.cm-cursor, .cm-dropCursor': { borderLeftColor: t.cursor },
      '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': { backgroundColor: t.sel },
      '.cm-gutters': { backgroundColor: t.gutter, color: t.gutterFg, borderRight: 'none' },
      '.cm-activeLineGutter': { backgroundColor: t.line },
      '.cm-activeLine': { backgroundColor: t.line },
    }, { dark: t.bg < '#808080' });
    const highlight = syntaxHighlighting(HighlightStyle.define([
      { tag: tags.keyword, color: s.keyword },
      { tag: tags.controlKeyword, color: s.keyword },
      { tag: tags.string, color: s.string },
      { tag: tags.comment, color: s.comment },
      { tag: tags.lineComment, color: s.comment },
      { tag: tags.blockComment, color: s.comment },
      { tag: tags.function(tags.variableName), color: s.function },
      { tag: tags.definition(tags.variableName), color: s.function },
      { tag: tags.variableName, color: s.variable },
      { tag: tags.number, color: s.number },
      { tag: tags.integer, color: s.number },
      { tag: tags.float, color: s.number },
      { tag: tags.typeName, color: s.type },
      { tag: tags.className, color: s.type },
      { tag: tags.operator, color: s.operator },
      { tag: tags.punctuation, color: s.variable },
      { tag: tags.propertyName, color: s.function },
      { tag: tags.bool, color: s.number },
      { tag: tags.null, color: s.number },
      { tag: tags.atom, color: s.number },
    ]));
    return [theme, highlight];
  }
  import { bracketMatching, indentOnInput, foldGutter, foldKeymap, syntaxTree, ensureSyntaxTree } from '@codemirror/language';
  import { autocompletion, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
  import { search, searchKeymap, highlightSelectionMatches, openSearchPanel, SearchQuery, getSearchQuery, setSearchQuery, findNext, findPrevious, replaceNext, replaceAll, closeSearchPanel, SearchCursor } from '@codemirror/search';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { updateFileContent, markFileSaved, autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers, editorShowErrorLens, editorTheme, projectRoot, openFiles, registerFileRenameCallback, triggerSearchInFile, openPreviewSignal, activeFilePath } from '../../modules';
  import type { EditorThemeId } from '../../modules/theme';

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

  // Error Lens widget — renders inline error message at end of line
  class ErrorWidget extends WidgetType {
    message: string;
    constructor(message: string) { super(); this.message = message; }
    toDOM() {
      const span = document.createElement('span');
      span.className = 'cm-error-lens';
      span.textContent = `  \u26A0 ${this.message}`;
      span.title = this.message;
      return span;
    }
    eq(other: ErrorWidget) { return this.message === other.message; }
    ignoreEvent() { return true; }
  }

  /** Format a snippet from an error node into a short, readable message. */
  function formatErrorSnippet(raw: string): string {
    // Take only the first non-empty line
    const firstLine = raw.split(/\r?\n/).find(l => l.trim()) ?? '';
    const trimmed = firstLine.trim();
    if (!trimmed) return '';
    const MAX = 48;
    return trimmed.length > MAX ? trimmed.slice(0, MAX - 1) + '\u2026' : trimmed;
  }

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

  // Per-file undo/redo: cache EditorState keyed by file path
  const stateCache = new Map<string, EditorState>();
  let currentFilePath: string | null = null;

  // File watcher for external changes
  let unwatchFn: UnwatchFn | null = null;
  let watchDebounce: ReturnType<typeof setTimeout> | null = null;
  let ignoreWatchUntil = 0;  // timestamp-based: ignore watcher events until this time
  let ignoreNextDocChange = false;

  // Per-file last-saved content (what's on disk as far as we know)
  const savedContentCache = new Map<string, string>();

  // Markdown preview
  let isMarkdown = $derived(/\.(md|mdx|markdown)$/i.test(filePath));
  let showPreview = $state(true);
  let previewHtml = $state('');

  // Compartments for dynamic reconfiguration
  const fontSizeComp = new Compartment();
  const tabSizeComp = new Compartment();
  const wordWrapComp = new Compartment();
  const lineNumbersComp = new Compartment();
  const errorLensComp = new Compartment();
  const themeComp = new Compartment();

  async function updateGitGutter(path: string) {
    if (!view) return;
    const root = get(projectRoot);
    if (!root || !path.startsWith(root)) return;
    const relPath = path.slice(root.length + 1);
    try {
      const ranges = await invoke<DiffRange[]>('git_diff_line_ranges', { repoPath: root, filePath: relPath });
      if (!view || currentFilePath !== path) return;
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

  // Custom search panel with match counter and replace count notification
  function createCustomSearchPanel(panelView: EditorView) {
    const dom = document.createElement('div');
    dom.className = 'cm-search cm-panel';

    // Search row
    const searchRow = document.createElement('div');
    searchRow.className = 'cm-search-row';

    const searchField = document.createElement('input');
    searchField.className = 'cm-textfield cm-search-field';
    searchField.setAttribute('main-field', 'true');
    searchField.setAttribute('autocapitalize', 'off');
    searchField.setAttribute('autocorrect', 'off');
    searchField.setAttribute('spellcheck', 'false');
    searchField.placeholder = 'Find';

    const matchInfo = document.createElement('span');
    matchInfo.className = 'cm-search-match-info';
    matchInfo.textContent = '';

    const prevBtn = document.createElement('button');
    prevBtn.className = 'cm-button cm-search-nav';
    prevBtn.textContent = '\u2191';
    prevBtn.title = 'Previous match (Shift+Enter)';

    const nextBtn = document.createElement('button');
    nextBtn.className = 'cm-button cm-search-nav';
    nextBtn.textContent = '\u2193';
    nextBtn.title = 'Next match (Enter)';

    // Toggle buttons
    const caseBtn = document.createElement('button');
    caseBtn.className = 'cm-button cm-search-toggle';
    caseBtn.textContent = 'Aa';
    caseBtn.title = 'Match case';

    const reBtn = document.createElement('button');
    reBtn.className = 'cm-button cm-search-toggle';
    reBtn.textContent = '.*';
    reBtn.title = 'Regex';

    const closeBtn = document.createElement('button');
    closeBtn.className = 'cm-button cm-search-close';
    closeBtn.setAttribute('aria-label', 'Close');
    closeBtn.textContent = '\u00d7';

    searchRow.append(searchField, matchInfo, prevBtn, nextBtn, caseBtn, reBtn, closeBtn);

    // Replace row
    const replaceRow = document.createElement('div');
    replaceRow.className = 'cm-search-row';

    const replaceField = document.createElement('input');
    replaceField.className = 'cm-textfield cm-search-field';
    replaceField.setAttribute('autocapitalize', 'off');
    replaceField.setAttribute('autocorrect', 'off');
    replaceField.setAttribute('spellcheck', 'false');
    replaceField.placeholder = 'Replace';

    const replaceBtn = document.createElement('button');
    replaceBtn.className = 'cm-button';
    replaceBtn.textContent = 'Replace';

    const replaceAllBtn = document.createElement('button');
    replaceAllBtn.className = 'cm-button';
    replaceAllBtn.textContent = 'Replace All';

    const replaceInfo = document.createElement('span');
    replaceInfo.className = 'cm-search-replace-info';
    replaceInfo.textContent = '';

    replaceRow.append(replaceField, replaceBtn, replaceAllBtn, replaceInfo);

    dom.append(searchRow, replaceRow);

    // State
    let caseSensitive = false;
    let regexp = false;
    let countTimer: ReturnType<typeof setTimeout> | null = null;

    function buildQuery(): SearchQuery {
      return new SearchQuery({
        search: searchField.value,
        replace: replaceField.value,
        caseSensitive,
        regexp,
      });
    }

    function countMatchesNow() {
      const state = panelView.state;
      const doc = state.doc;
      const searchText = searchField.value;

      if (!searchText) {
        matchInfo.textContent = '';
        return;
      }

      let totalMatches = 0;
      let currentIndex = 0;
      const cursorPos = state.selection.main.from;

      try {
        if (regexp) {
          let flags = 'g';
          if (!caseSensitive) flags += 'i';
          const re = new RegExp(searchText, flags);
          const content = doc.toString();
          let m;
          while ((m = re.exec(content)) !== null) {
            if (m[0].length === 0) { re.lastIndex++; continue; }
            totalMatches++;
            if (m.index <= cursorPos) currentIndex = totalMatches;
          }
        } else {
          const normalize = caseSensitive ? (s: string) => s : (s: string) => s.toLowerCase();
          const cursor = new SearchCursor(doc, searchText, 0, doc.length, normalize);
          cursor.next();
          while (!cursor.done) {
            totalMatches++;
            if (cursor.value.from <= cursorPos) currentIndex = totalMatches;
            cursor.next();
          }
        }
      } catch {
        matchInfo.textContent = 'Invalid';
        return;
      }

      if (totalMatches === 0) {
        matchInfo.textContent = 'No results';
      } else {
        if (currentIndex === 0) currentIndex = 1;
        matchInfo.textContent = `${currentIndex}/${totalMatches}`;
      }
    }

    // Debounced version — prevents blocking the main thread during rapid typing
    function scheduleCountMatches() {
      if (countTimer) clearTimeout(countTimer);
      countTimer = setTimeout(countMatchesNow, 80);
    }

    function commit() {
      panelView.dispatch({ effects: setSearchQuery.of(buildQuery()) });
      scheduleCountMatches();
    }

    searchField.addEventListener('input', commit);
    searchField.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        if (e.shiftKey) {
          findPrevious(panelView);
        } else {
          findNext(panelView);
        }
        scheduleCountMatches();
      } else if (e.key === 'Escape') {
        closeSearchPanel(panelView);
      }
    });

    replaceField.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        commit();
        replaceNext(panelView);
        scheduleCountMatches();
      } else if (e.key === 'Escape') {
        closeSearchPanel(panelView);
      }
    });

    prevBtn.addEventListener('click', () => {
      findPrevious(panelView);
      scheduleCountMatches();
    });

    nextBtn.addEventListener('click', () => {
      findNext(panelView);
      scheduleCountMatches();
    });

    caseBtn.addEventListener('click', () => {
      caseSensitive = !caseSensitive;
      caseBtn.classList.toggle('active', caseSensitive);
      commit();
    });

    reBtn.addEventListener('click', () => {
      regexp = !regexp;
      reBtn.classList.toggle('active', regexp);
      commit();
    });

    closeBtn.addEventListener('click', () => {
      closeSearchPanel(panelView);
    });

    replaceBtn.addEventListener('click', () => {
      commit();
      replaceNext(panelView);
      scheduleCountMatches();
    });

    replaceAllBtn.addEventListener('click', () => {
      commit();
      const searchText = searchField.value;
      if (!searchText) return;

      // Count matches before replacing
      let totalBefore = 0;
      const doc = panelView.state.doc;
      try {
        if (regexp) {
          let flags = 'g';
          if (!caseSensitive) flags += 'i';
          const re = new RegExp(searchText, flags);
          const content = doc.toString();
          let m;
          while ((m = re.exec(content)) !== null) {
            if (m[0].length === 0) { re.lastIndex++; continue; }
            totalBefore++;
          }
        } else {
          const normalize = caseSensitive ? (s: string) => s : (s: string) => s.toLowerCase();
          const cursor = new SearchCursor(doc, searchText, 0, doc.length, normalize);
          cursor.next();
          while (!cursor.done) {
            totalBefore++;
            cursor.next();
          }
        }
      } catch {
        // invalid regex
      }

      replaceAll(panelView);

      if (totalBefore > 0) {
        replaceInfo.textContent = `${totalBefore} replaced`;
        setTimeout(() => { replaceInfo.textContent = ''; }, 3000);
      }

      scheduleCountMatches();
    });

    // Close panel on Escape from anywhere within it
    dom.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        closeSearchPanel(panelView);
      }
    });

    // Pre-fill with selected text
    const sel = panelView.state.selection.main;
    if (!sel.empty) {
      const selectedText = panelView.state.sliceDoc(sel.from, sel.to);
      if (!selectedText.includes('\n')) {
        searchField.value = selectedText;
      }
    }

    // Initial count
    setTimeout(countMatchesNow, 0);

    return {
      dom,
      mount() {
        searchField.focus();
        if (searchField.value) commit();
      },
      update(update: ViewUpdate) {
        // Only re-count on doc or selection changes, debounced to avoid blocking typing
        if (update.docChanged || update.selectionSet) {
          scheduleCountMatches();
        }
      },
      top: true,
    };
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

  function hasErrorLens(path: string): boolean {
    // Languages whose Lezer parsers produce reliable error-recovery nodes.
    // JSX/TSX are included but heavily filtered below to avoid parser noise.
    return /\.(m?js|c?js|jsx|m?ts|c?ts|tsx|c|h|cpp|cxx|cc|hpp|hxx|hh|ino|java|kt|kts)$/i.test(path);
  }

  function buildErrorLensPlugin() {
    // Delay before a freshly-detected error is shown, to avoid flicker while typing.
    const SCAN_DELAY_MS = 450;

    function scanErrors(state: EditorState): DecorationSet {
      const builder = new RangeSetBuilder<Decoration>();
      // Wait for a fully-parsed tree (up to 300ms). syntaxTree() can return an
      // incomplete tree that produces false error nodes.
      const tree = ensureSyntaxTree(state, state.doc.length, 300);
      if (!tree) return Decoration.none;

      const seenLines = new Set<number>();
      const widgets: { pos: number; widget: Decoration }[] = [];

      tree.iterate({
        enter(node) {
          if (!node.type.isError) return;
          // Zero-length error nodes are recovery artifacts, not real errors.
          if (node.from === node.to) return;

          const raw = state.doc.sliceString(node.from, node.to);
          const formatted = formatErrorSnippet(raw);
          if (!formatted) return;

          // Skip tokens that are almost always parser recovery artifacts:
          // closing punctuation, lone JSX angle-brackets, leading dots, etc.
          if (/^[.}\])>;,]/.test(formatted) || /^[</>]+$/.test(formatted)) return;

          // Skip errors inside JSX scaffolding — these are almost always parser
          // recovery false positives rather than real user errors. Keep errors
          // inside JSXExpression (curly-brace embedded JS) since those are real.
          let parent = node.node.parent;
          while (parent) {
            const name = parent.type.name;
            if (
              name === 'JSXElement' ||
              name === 'JSXOpenTag' ||
              name === 'JSXAttribute' ||
              name === 'JSXEscape' ||
              name === 'JSXFragmentTag' ||
              name === 'JSXSelfClosingTag' ||
              name === 'JSXSpreadAttribute' ||
              name === 'JSXText'
            ) return;
            // Stop at script-level boundary.
            if (name === 'Script' || name === 'Program') break;
            parent = parent.parent;
          }

          const line = state.doc.lineAt(node.from);
          if (seenLines.has(line.number)) return;
          seenLines.add(line.number);

          const msg = `Unexpected: '${formatted}'`;
          widgets.push({
            pos: line.to,
            widget: Decoration.widget({ widget: new ErrorWidget(msg), side: 1 }),
          });
        },
      });

      widgets.sort((a, b) => a.pos - b.pos);
      for (const w of widgets) builder.add(w.pos, w.pos, w.widget);
      return builder.finish();
    }

    return ViewPlugin.fromClass(
      class {
        decorations: DecorationSet;
        private timer: ReturnType<typeof setTimeout> | null = null;
        private idle: number | null = null;

        constructor(view: EditorView) {
          this.decorations = Decoration.none;
          // Schedule the initial scan without blocking the first render.
          this.schedule(view, SCAN_DELAY_MS);
        }

        private schedule(view: EditorView, delay: number) {
          this.cancel();
          this.timer = setTimeout(() => {
            this.timer = null;
            // Prefer requestIdleCallback so heavy tree walks don't compete
            // with user input. Fallback to rAF when not supported.
            const run = () => {
              this.idle = null;
              try {
                this.decorations = scanErrors(view.state);
              } catch {
                this.decorations = Decoration.none;
              }
              view.requestMeasure();
            };
            const w = window as Window & {
              requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
            };
            if (typeof w.requestIdleCallback === 'function') {
              this.idle = w.requestIdleCallback(run, { timeout: 500 });
            } else {
              this.idle = requestAnimationFrame(run);
            }
          }, delay);
        }

        private cancel() {
          if (this.timer) { clearTimeout(this.timer); this.timer = null; }
          if (this.idle !== null) {
            const w = window as Window & { cancelIdleCallback?: (h: number) => void };
            if (typeof w.cancelIdleCallback === 'function') w.cancelIdleCallback(this.idle);
            else cancelAnimationFrame(this.idle);
            this.idle = null;
          }
        }

        update(update: ViewUpdate) {
          if (update.docChanged) {
            // Clear stale decorations immediately so they don't render on the
            // wrong lines during typing; the re-scan will restore any that remain.
            this.decorations = Decoration.none;
            this.schedule(update.view, SCAN_DELAY_MS);
          }
        }

        destroy() {
          this.cancel();
        }
      },
      { decorations: (v) => v.decorations }
    );
  }

  let previewTimer: ReturnType<typeof setTimeout> | null = null;

  function updatePreview(content: string) {
    if (previewTimer) {
      clearTimeout(previewTimer);
      previewTimer = null;
    }
    if (!isMarkdown || !showPreview) return;
    previewTimer = setTimeout(() => {
      previewHtml = DOMPurify.sanitize(marked.parse(content) as string);
    }, 300);
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
        // Ignore watcher events for a window after our own saves
        // (file systems often emit multiple events per write)
        if (Date.now() < ignoreWatchUntil) return;
        if (watchDebounce) clearTimeout(watchDebounce);
        watchDebounce = setTimeout(() => reloadFromDisk(path), 200);
      }, { recursive: false });
    } catch {
      // File may not exist yet or watching not supported
    }
  }

  async function reloadFromDisk(path: string) {
    if (!view || currentFilePath !== path) return;
    try {
      const diskContent = await invoke<string>('read_file_content', { path });
      // Re-check after await — user may have switched tabs
      if (!view || currentFilePath !== path) return;
      // Only reload if editor content differs from disk
      if (diskContent === view.state.doc.toString()) {
        // Disk matches editor — just update our saved cache
        savedContentCache.set(path, diskContent);
        return;
      }
      // If disk content matches what we last saved, this isn't an external
      // change — the user just typed ahead of the watcher. Don't overwrite.
      if (diskContent === savedContentCache.get(path)) return;

      // Preserve cursor position
      const cursorPos = Math.min(view.state.selection.main.head, diskContent.length);

      ignoreNextDocChange = true;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: diskContent },
        selection: { anchor: cursorPos },
        annotations: Transaction.addToHistory.of(false),
      });
      savedContentCache.set(path, diskContent);
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

    // Close any open search panel and save current editor state before switching
    if (view && currentFilePath) {
      closeSearchPanel(view);
      stateCache.set(currentFilePath, view.state);
    }

    const cached = stateCache.get(path);
    if (cached) {
      // Restore cached state (preserves undo history)
      if (view) {
        view.setState(cached);
      } else {
        view = new EditorView({ state: cached, parent: editorContainer });
      }
      currentFilePath = path;
      // The cached state captured the error-lens compartment at creation time,
      // which may now be stale (user toggled the setting, or different language).
      // Re-apply based on the current setting + file type.
      if (view) {
        view.dispatch({
          effects: errorLensComp.reconfigure(
            get(editorShowErrorLens) && hasErrorLens(path) ? buildErrorLensPlugin() : []
          ),
        });
      }
      updatePreview(cached.doc.toString());
      updateGitGutter(path);
      startWatching(path);

      // Background check: verify content against disk (handles external edits)
      // Compare disk against last SAVED content, not cached editor content.
      // If they match, disk hasn't changed — keep cached state with unsaved edits intact.
      try {
        const diskContent = await invoke<string>('read_file_content', { path });
        // Re-check after await — user may have switched tabs
        if (currentFilePath !== path || !view) return;
        const lastSaved = savedContentCache.get(path) ?? '';
        if (diskContent !== lastSaved) {
          // Disk changed externally — reload from disk
          savedContentCache.set(path, diskContent);
          if (diskContent !== view.state.doc.toString()) {
            ignoreNextDocChange = true;
            view.dispatch({
              changes: { from: 0, to: view.state.doc.length, insert: diskContent },
              annotations: Transaction.addToHistory.of(false),
            });
            markFileSaved(path);
            updatePreview(diskContent);
            updateGitGutter(path);
          }
        }
      } catch {
        // File may have been deleted externally
      }
    } else {
      // No cached state — load fresh from disk
      try {
        const content = await invoke<string>('read_file_content', { path });
        savedContentCache.set(path, content);
        createEditor(content, path);
        currentFilePath = path;
        updatePreview(content);
        updateGitGutter(path);
        startWatching(path);
      } catch (e) {
        const errStr = String(e);
        if (errStr.startsWith('FILE_TOO_LARGE:')) {
          const sizeInfo = errStr.replace('FILE_TOO_LARGE: ', '');
          createEditor(`// This file is too large to open in the editor (${sizeInfo}).\n// Use an external tool for files over 50 MB.`, path);
        } else {
          createEditor(`// Error loading file: ${e}`, path);
        }
        currentFilePath = path;
      }
    }
  }

  function buildState(content: string, path: string): EditorState {
    const lang = getLanguage(path);
    return EditorState.create({
      doc: content,
      extensions: [
        lineNumbersComp.of(get(editorLineNumbers) ? lineNumbers() : []),
        highlightActiveLine(),
        drawSelection(),
        EditorState.allowMultipleSelections.of(true),
        EditorView.clickAddsSelectionRange.of(e => e.altKey),
        history(),
        closeBrackets(),
        bracketMatching(),
        indentOnInput(),
        foldGutter(),
        autocompletion(),
        highlightSelectionMatches(),
        search({ createPanel: createCustomSearchPanel, top: true }),
        ...(lang ? [lang] : []),
        themeComp.of(buildEditorTheme(get(editorTheme))),
        fontSizeComp.of(EditorView.theme({
          '&': { fontSize: get(editorFontSize) + 'px' },
          '.cm-gutters': { fontSize: get(editorFontSize) + 'px' },
        })),
        tabSizeComp.of(EditorState.tabSize.of(get(editorTabSize))),
        wordWrapComp.of(get(editorWordWrap) ? EditorView.lineWrapping : []),
        gitGutterComp.of([]),
        errorLensComp.of(hasErrorLens(path) && get(editorShowErrorLens) ? buildErrorLensPlugin() : []),
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
        // AI extensions
        aiDiffExtension(),
        ghostTextExtension(),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            if (ignoreNextDocChange) {
              ignoreNextDocChange = false;
              return;
            }
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
  }

  function createEditor(content: string, path: string) {
    const state = buildState(content, path);

    if (view) {
      view.setState(state);
    } else {
      view = new EditorView({ state, parent: editorContainer });
    }
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
      // Ignore watcher events for 1.5s after save to handle
      // multiple FS events that many OS's emit per single write
      ignoreWatchUntil = Date.now() + 1500;
      await invoke('write_file_content', { path, content });
      savedContentCache.set(path, content);
      markFileSaved(path);
      updateGitGutter(path);
    } catch (e) {
      log.error('Failed to save', e);
      ignoreWatchUntil = 0;
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

  $effect(() => {
    const show = $editorShowErrorLens;
    const path = currentFilePath;
    if (view && path) {
      view.dispatch({
        effects: errorLensComp.reconfigure(
          show && hasErrorLens(path) ? buildErrorLensPlugin() : []
        ),
      });
    }
  });

  $effect(() => {
    const theme = $editorTheme;
    if (view) {
      view.dispatch({ effects: themeComp.reconfigure(buildEditorTheme(theme)) });
    }
  });

  function handleGlobalKeydown(e: KeyboardEvent) {
    // Cmd/Ctrl+F: focus editor and open search panel
    if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
      if (!view) return;

      // Only handle if this editor (or its search panel) contains focus,
      // or if no other editor has focus (fallback for sidebar/external focus)
      const active = document.activeElement;
      const thisEditorHasFocus = editorContainer?.contains(active) || view.hasFocus;
      const anyEditorHasFocus = !!active?.closest('.editor-wrapper');
      if (!thisEditorHasFocus && anyEditorHasFocus) return;

      e.preventDefault();

      // If the search panel is already open, just focus its input
      const existingPanel = editorContainer?.querySelector('.cm-panel.cm-search');
      if (existingPanel) {
        const searchInput = existingPanel.querySelector<HTMLInputElement>('input.cm-search-field[main-field]');
        if (searchInput) {
          searchInput.focus();
          searchInput.select();
        }
        return;
      }

      view.focus();
      openSearchPanel(view);
      // Disable autocapitalize on the search inputs
      requestAnimationFrame(() => {
        view?.dom.querySelectorAll('.cm-panel.cm-search input').forEach((input) => {
          input.setAttribute('autocapitalize', 'off');
          input.setAttribute('autocorrect', 'off');
        });
      });
    }
  }

  let unregisterRenameCallback: (() => void) | null = null;

  onMount(() => {
    window.addEventListener('keydown', handleGlobalKeydown);

    // Register rename callback to update cache keys
    unregisterRenameCallback = registerFileRenameCallback((oldPath, newPath) => {
      const cached = stateCache.get(oldPath);
      if (cached) {
        stateCache.delete(oldPath);
        stateCache.set(newPath, cached);
      }
      const savedContent = savedContentCache.get(oldPath);
      if (savedContent !== undefined) {
        savedContentCache.delete(oldPath);
        savedContentCache.set(newPath, savedContent);
      }
      if (currentFilePath === oldPath) {
        currentFilePath = newPath;
      }
    });
  });

  // Sync pending AI edits into the editor's diff field
  const unsubPendingEdits = pendingEdits.subscribe((allEdits) => {
    if (!view) return;
    const filePath = currentFilePath;
    if (!filePath) { view.dispatch({ effects: clearDiffEffect.of(undefined) }); return; }
    const fileEdits = allEdits[filePath];
    if (fileEdits && fileEdits.length > 0) {
      // Filter out edits with invalid line numbers
      const validEdits = fileEdits.filter(e =>
        e.startLine >= 1 && e.endLine <= view!.state.doc.lines
      );
      if (validEdits.length > 0) {
        view.dispatch({ effects: addDiffEffect.of(validEdits) });
      } else {
        view.dispatch({ effects: clearDiffEffect.of(undefined) });
      }
    } else {
      view.dispatch({ effects: clearDiffEffect.of(undefined) });
    }
  });

  onDestroy(() => {
    unsubPendingEdits();
    window.removeEventListener('keydown', handleGlobalKeydown);
    if (unregisterRenameCallback) unregisterRenameCallback();
    stopWatching();
    if (previewTimer) clearTimeout(previewTimer);
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
    stateCache.clear();
    savedContentCache.clear();
    if (view) view.destroy();
  });

  $effect(() => {
    if (filePath && editorContainer) {
      loadFile(filePath);
    }
  });

  // React to store version changes (e.g. git discard) — force editor to match store content
  $effect(() => {
    const file = $openFiles.find(f => f.path === filePath);
    if (!file || file.version === 0) return;
    // version > 0 means an external reload happened — push content into editor
    if (view && currentFilePath === filePath) {
      const editorContent = view.state.doc.toString();
      if (editorContent !== file.content) {
        ignoreNextDocChange = true;
        const cursorPos = Math.min(view.state.selection.main.head, file.content.length);
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: file.content },
          selection: { anchor: cursorPos },
          annotations: Transaction.addToHistory.of(false),
        });
      }
      savedContentCache.set(filePath, file.content);
      // Clear the cached state so it doesn't hold stale content
      stateCache.delete(filePath);
      updatePreview(file.content);
      updateGitGutter(filePath);
    }
  });

  // Clean up cache entries for files that are no longer open
  $effect(() => {
    const files = $openFiles;
    const openPaths = new Set(files.map(f => f.path));
    for (const cachedPath of stateCache.keys()) {
      if (!openPaths.has(cachedPath)) {
        stateCache.delete(cachedPath);
        savedContentCache.delete(cachedPath);
      }
    }
  });

  // Open search panel when triggerSearchInFile store increments
  $effect(() => {
    const trigger = $triggerSearchInFile;
    if (trigger === 0) return;
    if (!view) return;

    // Only open in the focused/active editor — matches handleGlobalKeydown guard
    const active = document.activeElement;
    const thisEditorHasFocus = editorContainer?.contains(active) || view.hasFocus;
    const anyEditorHasFocus = !!active?.closest('.editor-wrapper');
    if (!thisEditorHasFocus && anyEditorHasFocus) return;

    // If panel already open, just focus its input
    const existingPanel = editorContainer?.querySelector('.cm-panel.cm-search');
    if (existingPanel) {
      const searchInput = existingPanel.querySelector<HTMLInputElement>('input.cm-search-field[main-field]');
      if (searchInput) {
        searchInput.focus();
        searchInput.select();
      }
      return;
    }

    view.focus();
    openSearchPanel(view);
    requestAnimationFrame(() => {
      view?.dom.querySelectorAll('.cm-panel.cm-search input').forEach((input) => {
        input.setAttribute('autocapitalize', 'off');
        input.setAttribute('autocorrect', 'off');
      });
    });
  });

  let lastPreviewTrigger = 0;
  $effect(() => {
    const trigger = $openPreviewSignal;
    if (trigger === 0 || trigger === lastPreviewTrigger) return;
    lastPreviewTrigger = trigger;
    if (!view || $activeFilePath !== filePath || !isMarkdown) return;
    showPreview = true;
    updatePreview(view.state.doc.toString());
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
    color: var(--text-secondary);
    font-weight: 500;
  }
  .editor-wrapper :global(.cm-lineNumbers .cm-gutterElement) {
    color: var(--text-muted);
    font-weight: 500;
  }
  .editor-wrapper :global(.cm-activeLineGutter) {
    background: color-mix(in srgb, var(--accent) 10%, transparent) !important;
    color: var(--text-primary) !important;
    font-weight: 600 !important;
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

  .editor-wrapper :global(.cm-error-lens) {
    color: var(--error, #f38ba8);
    opacity: 0.75;
    font-style: italic;
    font-size: 0.88em;
    padding-left: 2em;
    pointer-events: none;
    user-select: none;
    white-space: pre;
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
    font-family: var(--font-mono);
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

  /* Custom search panel styles */
  .editor-wrapper :global(.cm-panel.cm-search) {
    background: var(--bg-secondary, #1e1e2e);
    border-bottom: 1px solid var(--border, #45475a);
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }

  .editor-wrapper :global(.cm-search-row) {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .editor-wrapper :global(.cm-search-field) {
    flex: 1;
    min-width: 120px;
    max-width: 260px;
    background: var(--bg-primary, #11111b);
    color: var(--text-primary, #cdd6f4);
    border: 1px solid var(--border, #45475a);
    border-radius: 4px;
    padding: 3px 8px;
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }

  .editor-wrapper :global(.cm-search-field:focus) {
    border-color: var(--accent, #89b4fa);
  }

  .editor-wrapper :global(.cm-search-match-info) {
    font-size: 11px;
    color: var(--text-muted, #a6adc8);
    min-width: 70px;
    text-align: center;
    white-space: nowrap;
  }

  .editor-wrapper :global(.cm-search-replace-info) {
    font-size: 11px;
    color: var(--success, #a6e3a1);
    white-space: nowrap;
    margin-left: 4px;
  }

  .editor-wrapper :global(.cm-search .cm-button) {
    background: var(--bg-surface, #313244);
    color: var(--text-primary, #cdd6f4);
    border: 1px solid var(--border, #45475a);
    border-radius: 4px;
    padding: 2px 8px;
    font-size: 11px;
    cursor: pointer;
    line-height: 1.4;
    white-space: nowrap;
  }

  .editor-wrapper :global(.cm-search .cm-button:hover) {
    background: var(--bg-tertiary, #45475a);
  }

  .editor-wrapper :global(.cm-search-nav) {
    padding: 2px 5px !important;
    font-size: 13px !important;
    font-weight: bold;
  }

  .editor-wrapper :global(.cm-search-toggle) {
    font-size: 11px !important;
    opacity: 0.5;
  }

  .editor-wrapper :global(.cm-search-toggle.active) {
    opacity: 1;
    background: var(--accent, #89b4fa) !important;
    color: var(--bg-primary, #11111b) !important;
    border-color: var(--accent, #89b4fa) !important;
  }

  .editor-wrapper :global(.cm-search-close) {
    margin-left: auto;
    font-size: 16px !important;
    padding: 0 6px !important;
    line-height: 1.2;
  }
</style>
