<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { watch } from '@tauri-apps/plugin-fs';
  import type { UnwatchFn } from '@tauri-apps/plugin-fs';
  import { get } from 'svelte/store';
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection, gutter, GutterMarker, ViewPlugin, Decoration, WidgetType } from '@codemirror/view';
  import type { DecorationSet, ViewUpdate } from '@codemirror/view';
  import { EditorState, Compartment, RangeSetBuilder, Transaction } from '@codemirror/state';
  import { defaultKeymap, indentWithTab, history, historyKeymap, cursorDocStart, cursorDocEnd, cursorLineBoundaryForward, cursorLineBoundaryBackward, selectDocStart, selectDocEnd, selectLineBoundaryForward, selectLineBoundaryBackward, cursorCharLeft, cursorCharRight, cursorLineUp, cursorLineDown, selectCharLeft, selectCharRight, selectLineUp, selectLineDown, deleteLine, cursorPageDown, cursorPageUp, toggleComment, moveLineUp, moveLineDown, copyLineDown, indentMore, indentLess, insertBlankLine, cursorMatchingBracket } from '@codemirror/commands';
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
  import { yaml } from '@codemirror/legacy-modes/mode/yaml';
  import { toml } from '@codemirror/legacy-modes/mode/toml';
  import { shell } from '@codemirror/legacy-modes/mode/shell';
  import { properties } from '@codemirror/legacy-modes/mode/properties';
  import { stex } from '@codemirror/legacy-modes/mode/stex';
  import { dockerFile } from '@codemirror/legacy-modes/mode/dockerfile';
  import { nginx } from '@codemirror/legacy-modes/mode/nginx';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { tags } from '@lezer/highlight';
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
  import { aiDiffExtension, addDiffEffect, clearDiffEffect, aiDiffField } from '../../modules/editor/aiDiffExtension';
  import { reanchorEditsForChanges, reanchorEditsForContent, projectEditsForDiffField } from '../../modules/editor/aiDiffAnchoring';
  import { shouldDispatchVersionUpdate } from '../../modules/editor/versionGate';
  import { bindAiDiffResolve } from '../../modules/editor/aiDiffEvents';
  import { ghostTextExtension } from '../../modules/editor/ghostText';
  import { scrollbarAnnotations, setScrollbarRanges, type ScrollbarRange } from '../../modules/editor/scrollbarAnnotations';
  import { pendingEdits, approveEdit, rejectEdit, addEdits } from '../../modules/ai/pendingEdits';
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
      'plum-dark': () => buildThemeExt(
        { bg: '#15121A', fg: '#E8E2D5', sel: '#4A3A5C', cursor: '#D4AE8A', gutter: '#1C1825', gutterFg: '#6A6080', line: '#241E2E' },
        { keyword: '#C79BBF', string: '#8EA88A', comment: '#6A6080', function: '#D4C697', variable: '#E8E2D5', number: '#C9956A', type: '#C79B78', operator: '#A8A09C' }
      ),
      'plum-light': () => buildThemeExt(
        { bg: '#F5EFE2', fg: '#2A2018', sel: '#C8B890', cursor: '#4A2640', gutter: '#EDE5D2', gutterFg: '#B0A48A', line: '#E2D8C1' },
        { keyword: '#7A3A6A', string: '#4A6B3E', comment: '#8A7E6A', function: '#7A5A14', variable: '#2A2018', number: '#8A4A1E', type: '#6E3E1A', operator: '#5E5346' }
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
      // Markdown / prose tags
      { tag: tags.heading, color: s.keyword, fontWeight: 'bold' },
      { tag: tags.heading1, color: s.keyword, fontWeight: 'bold' },
      { tag: tags.heading2, color: s.keyword, fontWeight: 'bold' },
      { tag: tags.heading3, color: s.keyword, fontWeight: 'bold' },
      { tag: tags.emphasis, fontStyle: 'italic', color: s.string },
      { tag: tags.strong, fontWeight: 'bold', color: s.function },
      { tag: tags.link, color: s.type, textDecoration: 'underline' },
      { tag: tags.url, color: s.type },
      { tag: tags.monospace, color: s.number },
      { tag: tags.strikethrough, textDecoration: 'line-through', color: s.comment },
      { tag: tags.quote, color: s.string, fontStyle: 'italic' },
      { tag: tags.meta, color: s.comment },
      { tag: tags.contentSeparator, color: s.comment },
      { tag: tags.processingInstruction, color: s.keyword },
      { tag: tags.labelName, color: s.function },
    ]));
    return [theme, highlight];
  }
  import { bracketMatching, indentOnInput, foldGutter, foldKeymap, syntaxTree, ensureSyntaxTree } from '@codemirror/language';
  import { autocompletion, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
  import { search, searchKeymap, highlightSelectionMatches, openSearchPanel, SearchQuery, getSearchQuery, setSearchQuery, findNext, findPrevious, replaceNext, replaceAll, closeSearchPanel, SearchCursor, selectNextOccurrence } from '@codemirror/search';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { updateFileContent, markFileSaved, autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers, editorShowErrorLens, editorVimMode, editorTheme, projectRoot, openFiles, registerFileRenameCallback, triggerSearchInFile, openPreviewSignal, activeFilePath } from '../../modules';
  import { vim } from '@replit/codemirror-vim';
  import { startInlineEdit, cancelInlineEdit, type InlineEditRequest } from '../../modules/ai/inlineEdit';
  import InlineEditPopover from './InlineEditPopover.svelte';
  import { open as openExternal } from '@tauri-apps/plugin-shell';
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
  let gitGutterTimer: ReturnType<typeof setTimeout> | null = null;
  let saving = $state(false);

  // Per-file undo/redo: cache EditorState keyed by file path
  const stateCache = new Map<string, EditorState>();
  // Per-file scroll position (EditorState doesn't include viewport scroll)
  const scrollCache = new Map<string, number>();
  let currentFilePath: string | null = null;

  // Per-path counter of the highest `openFiles` version we've already
  // pushed into the editor doc. Without this, the version $effect
  // re-fires on every openFiles change (e.g. the modified-flag flip
  // on the first keystroke after an AI accept) and reverts the
  // user's typing back to the post-accept content.
  const lastHandledVersion = new Map<string, number>();

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

  // Inline edit (Cmd+K)
  let inlineEditVisible = $state(false);
  let inlineEditTop = $state(0);
  let inlineEditLeft = $state(0);
  let inlineEditWidth = $state(320);
  let inlineEditSelection: { from: number; to: number; startLine: number; endLine: number; text: string } | null = null;

  // Compartments for dynamic reconfiguration
  const fontSizeComp = new Compartment();
  const tabSizeComp = new Compartment();
  const wordWrapComp = new Compartment();
  const lineNumbersComp = new Compartment();
  const errorLensComp = new Compartment();
  const themeComp = new Compartment();
  const vimComp = new Compartment();

  /** Debounced git gutter update — prevents subprocess pile-up during rapid tab switching. */
  function debouncedGitGutter(path: string) {
    if (gitGutterTimer) clearTimeout(gitGutterTimer);
    gitGutterTimer = setTimeout(() => updateGitGutter(path), 150);
  }

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
      const sbRanges: ScrollbarRange[] = ranges.map(r => ({
        kind: r.kind === 'add' ? 'add' : r.kind === 'del' ? 'del' : 'mod',
        start: r.start,
        end: r.end,
      }));
      view.dispatch({
        effects: [
          gitGutterComp.reconfigure(
            gutter({
              class: 'cm-git-gutter',
              markers: () => markers,
            })
          ),
          setScrollbarRanges.of(sbRanges),
        ],
      });
    } catch {
      // Not in a git repo or command failed — clear gutter and scrollbar
      if (view) {
        view.dispatch({
          effects: [gitGutterComp.reconfigure([]), setScrollbarRanges.of([])],
        });
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
      case 'yaml': case 'yml': return StreamLanguage.define(yaml);
      case 'toml': return StreamLanguage.define(toml);
      case 'ini': case 'conf': case 'cfg': case 'env': case 'properties':
        return StreamLanguage.define(properties);
      case 'sh': case 'bash': case 'zsh': case 'fish': return StreamLanguage.define(shell);
      case 'tex': case 'latex': case 'sty': case 'cls': return StreamLanguage.define(stex);
      case 'dockerfile': return StreamLanguage.define(dockerFile);
      case 'txt': case 'text': case 'log': return null;
      case 'gitignore': case 'gitattributes': case 'editorconfig':
        return StreamLanguage.define(properties);
      default:
        // Files with no extension (Makefile, Dockerfile, etc.)
        if (name === 'makefile') return StreamLanguage.define(shell);
        if (name === 'dockerfile') return StreamLanguage.define(dockerFile);
        if (name === '.env' || name.startsWith('.env.')) return StreamLanguage.define(properties);
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
      // Re-anchor pending edits via content search. mapPos is useless
      // here — we replaced the entire document — so we look up each
      // edit's `originalCode` as a line-aligned run in the new content.
      // Edits whose original content vanished (or appears multiple
      // times) are dropped; the user can re-request from the AI.
      reanchorPendingEditsForExternalContent(path, diskContent);
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
      scrollCache.set(currentFilePath, view.scrollDOM.scrollTop);
    }

    const cached = stateCache.get(path);
    if (cached) {
      // Restore cached state (preserves undo history)
      if (view) {
        view.setState(cached);
        // Restore scroll position (EditorState doesn't include viewport scroll)
        const savedScroll = scrollCache.get(path);
        if (savedScroll != null) {
          view.scrollDOM.scrollTop = savedScroll;
        }
      } else {
        view = new EditorView({ state: cached, parent: editorContainer });
        const savedScroll = scrollCache.get(path);
        if (savedScroll != null) {
          view.scrollDOM.scrollTop = savedScroll;
        }
      }
      currentFilePath = path;
      // Project pendingEdits onto the (possibly fresh) CM state — the
      // store didn't change but the projection target did.
      syncDiffFieldFromPendingEdits(get(pendingEdits));
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
      debouncedGitGutter(path);
      startWatching(path);

      // Background check: verify content against disk (handles external edits).
      // Skip when the file watcher is active — it will trigger reloadFromDisk
      // on any external change. This avoids an IPC + disk read per tab switch.
      if (!unwatchFn) {
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
              // doc.lines may have changed — re-anchor pending edits
              // against the new content via line-aligned search before
              // re-projecting onto the CM diff field.
              reanchorPendingEditsForExternalContent(path, diskContent);
              syncDiffFieldFromPendingEdits(get(pendingEdits));
            }
          }
        } catch {
          // File may have been deleted externally
        }
      }
    } else {
      // No cached state — load fresh from disk
      try {
        const content = await invoke<string>('read_file_content', { path });
        savedContentCache.set(path, content);
        createEditor(content, path);
        currentFilePath = path;
        syncDiffFieldFromPendingEdits(get(pendingEdits));
        updatePreview(content);
        debouncedGitGutter(path);
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
        // The error-stub document has no real content, so any pending
        // edits will fail the line-range filter and clear the field.
        // Calling the sync keeps the invariant explicit.
        syncDiffFieldFromPendingEdits(get(pendingEdits));
      }
    }
  }

  // ── Inline Edit (Cmd+K) ──

  function openInlineEdit(v: EditorView) {
    const sel = v.state.selection.main;
    if (sel.empty) return; // Require a selection

    const startLine = v.state.doc.lineAt(sel.from).number;
    const endLine = v.state.doc.lineAt(sel.to).number;
    const text = v.state.sliceDoc(sel.from, sel.to);

    // Position the popover above the selection start
    const coords = v.coordsAtPos(sel.from);
    if (!coords) return;
    const editorRect = editorContainer.getBoundingClientRect();

    inlineEditSelection = { from: sel.from, to: sel.to, startLine, endLine, text };
    inlineEditTop = coords.top - editorRect.top;
    inlineEditLeft = coords.left - editorRect.left;
    inlineEditWidth = Math.min(500, editorRect.width - 40);
    inlineEditVisible = true;
  }

  function closeInlineEdit() {
    inlineEditVisible = false;
    inlineEditSelection = null;
    cancelInlineEdit();
  }

  async function handleInlineEditSubmit(instruction: string) {
    if (!inlineEditSelection || !view) return;

    const request: InlineEditRequest = {
      selectedCode: inlineEditSelection.text,
      instruction,
      filePath,
      startLine: inlineEditSelection.startLine,
      endLine: inlineEditSelection.endLine,
    };

    const result = await startInlineEdit(request);

    if (result.success && result.newCode) {
      // Apply as a pending edit. Route through `addEdits` (the canonical
      // `pendingEdits` store) rather than dispatching `addDiffEffect`
      // directly to CM. The Editor's `pendingEdits.subscribe` below
      // projects pendingEdits onto the CM diff field, and the inline
      // Accept/Reject buttons resolve through `approveEdit` / `rejectEdit`
      // which consult that same store. Bypassing the store here would
      // make the Accept button a silent no-op (the original bug).
      //
      // `trusted: true` because the path is the user's currently-open
      // file (the Editor's `filePath` prop). The default untrusted
      // mode would silently drop the edit if the file lives outside
      // the project root (e.g. a recent file opened from a dialog).
      const edit = {
        id: `inline-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        filePath,
        startLine: inlineEditSelection.startLine,
        endLine: inlineEditSelection.endLine,
        originalCode: inlineEditSelection.text,
        newCode: result.newCode,
        status: 'pending' as const,
      };
      addEdits([edit], { trusted: true });
    }

    inlineEditVisible = false;
    inlineEditSelection = null;
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
        scrollbarAnnotations(),
        errorLensComp.of(hasErrorLens(path) && get(editorShowErrorLens) ? buildErrorLensPlugin() : []),
        vimComp.of(get(editorVimMode) ? vim() : []),
        keymap.of([
          ...closeBracketsKeymap,
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
          ...foldKeymap,
          indentWithTab,
          { key: 'Mod-s', run: () => { saveFile(path); return true; } },
          // Inline edit (Cmd+K)
          { key: 'Mod-k', run: (v) => { openInlineEdit(v); return true; } },
          // Common IDE shortcuts (VSCode/Zed/Xcode conventions)
          { key: 'Mod-/', run: toggleComment },
          { key: 'Mod-Shift-k', run: deleteLine },
          { key: 'Alt-ArrowUp', run: moveLineUp },
          { key: 'Alt-ArrowDown', run: moveLineDown },
          { key: 'Mod-Shift-d', run: copyLineDown },
          { key: 'Mod-d', run: selectNextOccurrence },
          { key: 'Mod-]', run: indentMore },
          { key: 'Mod-[', run: indentLess },
          { key: 'Mod-Enter', run: insertBlankLine },
          { key: 'Mod-Shift-\\', run: cursorMatchingBracket },
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
            // Re-anchor any pending AI edits' line numbers so they
            // track typing. Capture references from the (immutable)
            // ViewUpdate up front, then defer the store mutation to a
            // microtask: dispatching from inside an updateListener
            // would re-enter CM's transaction pipeline. By the time
            // the microtask runs the store still reflects pre-update
            // line numbers (only docChanged transactions reach this
            // branch and only one editor mutates this path's edits),
            // so the captured `oldDoc` is the right anchor.
            const oldDoc = update.startState.doc;
            const newDoc = update.state.doc;
            const changes = update.changes;
            queueMicrotask(() => {
              const all = get(pendingEdits);
              const fileEdits = all[path];
              if (!fileEdits || fileEdits.length === 0) return;
              const reanchored = reanchorEditsForChanges(fileEdits, oldDoc, newDoc, changes);
              // Reference-equality fast path: if every edit object is
              // unchanged AND no edit was dropped, skip the store
              // update to avoid a spurious subscriber notification.
              const unchanged = reanchored.length === fileEdits.length
                && reanchored.every((e, i) => e === fileEdits[i]);
              if (unchanged) return;
              pendingEdits.update(current => {
                // Staleness guard: if the bucket changed between our
                // `get(pendingEdits)` and this update callback (e.g.
                // a concurrent addEdits / removeApprovedEdits / rekey
                // landed during the microtask gap), `reanchored` was
                // computed from a stale snapshot. Bail and let the
                // next docChanged re-anchor against the fresh state.
                // Single-threaded JS makes this race impossible
                // *today*, but the guard is cheap and prevents a
                // future async refactor from silently corrupting line
                // numbers.
                if (current[path] !== fileEdits) return current;
                if (reanchored.length === 0) {
                  const { [path]: _, ...rest } = current;
                  return rest;
                }
                return { ...current, [path]: reanchored };
              });
            });
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

  $effect(() => {
    const enabled = $editorVimMode;
    if (view) {
      view.dispatch({ effects: vimComp.reconfigure(enabled ? vim() : []) });
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
  let unbindDiffResolve: (() => void) | null = null;

  onMount(() => {
    window.addEventListener('keydown', handleGlobalKeydown);
    unbindDiffResolve = bindAiDiffResolve(editorContainer, handleDiffResolve);

    // Register rename callback to update cache keys
    unregisterRenameCallback = registerFileRenameCallback((oldPath, newPath) => {
      const cached = stateCache.get(oldPath);
      if (cached) {
        stateCache.delete(oldPath);
        stateCache.set(newPath, cached);
      }
      const savedScroll = scrollCache.get(oldPath);
      if (savedScroll != null) {
        scrollCache.delete(oldPath);
        scrollCache.set(newPath, savedScroll);
      }
      const savedContent = savedContentCache.get(oldPath);
      if (savedContent !== undefined) {
        savedContentCache.delete(oldPath);
        savedContentCache.set(newPath, savedContent);
      }
      const versionHandled = lastHandledVersion.get(oldPath);
      if (versionHandled !== undefined) {
        lastHandledVersion.delete(oldPath);
        lastHandledVersion.set(newPath, versionHandled);
      }
      if (currentFilePath === oldPath) {
        currentFilePath = newPath;
        // Keep the invariant: every `currentFilePath` assignment is
        // followed by a sync. By the time this runs, the rename
        // callback registered in `pendingEdits.ts` (registered earlier
        // at module load) has already moved the bucket from `oldPath`
        // to `newPath`. The store-emit it triggered ran the
        // pendingEdits.subscribe with stale `currentFilePath = oldPath`,
        // dispatching `clearDiffEffect`. This sync re-projects with the
        // updated path and, if there are pending edits for the new
        // file, dispatches `addDiffEffect`. Both updates land within
        // the same synchronous tick, so there's no visible flicker.
        syncDiffFieldFromPendingEdits(get(pendingEdits));
      }
    });
  });

  // Handle accept/reject from the inline diff controls
  function handleDiffResolve({ id, action }: { id: string; action: 'approve' | 'reject' }) {
    if (action === 'approve') approveEdit(id);
    else rejectEdit(id);
  }

  /**
   * Project the canonical `pendingEdits` onto the CM `aiDiffField`
   * for the file currently in `view`. Sole writer of `addDiffEffect`
   * / `clearDiffEffect`. The pure `projectEditsForDiffField` helper
   * decides whether to dispatch, clear, or skip — the skip path
   * prevents no-op store notifications from cascading into CM
   * decoration recomputation.
   */
  function syncDiffFieldFromPendingEdits(allEdits: Record<string, import('../../modules/ai/editParser').EditProposal[]>) {
    if (!view) return;
    const path = currentFilePath;
    const current = view.state.field(aiDiffField);
    const fileEdits = path ? allEdits[path] : undefined;
    const projection = projectEditsForDiffField(fileEdits, current, view.state.doc.lines);
    if (projection.kind === 'unchanged') return;
    if (projection.kind === 'clear') {
      view.dispatch({ effects: clearDiffEffect.of(undefined) });
      return;
    }
    view.dispatch({ effects: addDiffEffect.of(projection.edits) });
  }

  /**
   * Re-anchor pending edits for `filePath` against a fresh content
   * snapshot using line-aligned content search. Called after wholesale
   * document replacements (file watcher reload, post-conflict disk
   * read), where mapPos through a `from: 0, to: doc.length` change
   * would map every old position to the boundary and corrupt the
   * line numbers.
   *
   * Pure-helper resolution rules: unique line-aligned occurrence
   * of `originalCode` re-anchors; zero or multiple occurrences drop
   * the edit (better lost than mis-applied).
   */
  function reanchorPendingEditsForExternalContent(filePath: string, newContent: string) {
    const all = get(pendingEdits);
    const fileEdits = all[filePath];
    if (!fileEdits || fileEdits.length === 0) return;
    const reanchored = reanchorEditsForContent(fileEdits, newContent);
    const unchanged = reanchored.length === fileEdits.length
      && reanchored.every((e, i) => e === fileEdits[i]);
    if (unchanged) return;
    pendingEdits.update(current => {
      // Staleness guard — if another update interleaved between our
      // get() and this callback, bail. Same rationale as the
      // keystroke re-anchor.
      if (current[filePath] !== fileEdits) return current;
      if (reanchored.length === 0) {
        const { [filePath]: _, ...rest } = current;
        return rest;
      }
      return { ...current, [filePath]: reanchored };
    });
  }

  // Sync pending AI edits into the editor's diff field whenever the
  // pendingEdits store changes.
  const unsubPendingEdits = pendingEdits.subscribe((allEdits) => {
    syncDiffFieldFromPendingEdits(allEdits);
  });

  onDestroy(() => {
    unsubPendingEdits();
    unbindDiffResolve?.();
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
    scrollCache.clear();
    savedContentCache.clear();
    if (view) view.destroy();
  });

  $effect(() => {
    if (filePath && editorContainer) {
      loadFile(filePath);
      // Focus the editor so the user can type immediately after clicking a tab.
      // Deferred to next frame so the editor DOM is fully mounted/restored.
      requestAnimationFrame(() => {
        if (view && currentFilePath === filePath) {
          view.focus();
        }
      });
    }
  });

  // React to store version changes (e.g. AI accept, file-watcher
  // reload, git discard). Gated per-path by `lastHandledVersion` so
  // we only dispatch on a fresh version bump — not on every openFiles
  // store update (the modified-flag flip on the first keystroke would
  // otherwise revert the user's typing).
  $effect(() => {
    const files = $openFiles;
    const path = currentFilePath;
    if (!view || !path) return;
    const file = files.find(f => f.path === path);
    if (!shouldDispatchVersionUpdate(file, lastHandledVersion.get(path) ?? 0)) return;
    // Only commit lastHandled AFTER we've actually dispatched. If the
    // currentFilePath !== filePath guard below skips the dispatch, we
    // leave lastHandled untouched so the *next* run on the right tab
    // can still react to this version.
    if (currentFilePath === filePath) {
      const editorContent = view.state.doc.toString();
      if (editorContent !== file!.content) {
        ignoreNextDocChange = true;
        const cursorPos = Math.min(view.state.selection.main.head, file!.content.length);
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: file!.content },
          selection: { anchor: cursorPos },
          annotations: Transaction.addToHistory.of(false),
        });
      }
      savedContentCache.set(filePath, file!.content);
      stateCache.delete(filePath);
      scrollCache.delete(filePath);
      updatePreview(file!.content);
      updateGitGutter(filePath);
      lastHandledVersion.set(path, file!.version);
    }
  });

  // Clean up cache entries for files that are no longer open
  $effect(() => {
    const files = $openFiles;
    const openPaths = new Set(files.map(f => f.path));
    const purge = (keys: IterableIterator<string>) => {
      for (const key of keys) {
        if (!openPaths.has(key)) {
          stateCache.delete(key);
          scrollCache.delete(key);
          savedContentCache.delete(key);
          lastHandledVersion.delete(key);
        }
      }
    };
    // Union of all per-path cache keys; a file may have entries in
    // some maps but not others (e.g. a freshly-opened file with a
    // version bump but no edited state yet).
    const allKeys = new Set<string>();
    for (const k of stateCache.keys()) allKeys.add(k);
    for (const k of scrollCache.keys()) allKeys.add(k);
    for (const k of savedContentCache.keys()) allKeys.add(k);
    for (const k of lastHandledVersion.keys()) allKeys.add(k);
    purge(allKeys.values());
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
    {#if inlineEditVisible}
      <InlineEditPopover
        top={inlineEditTop}
        left={inlineEditLeft}
        width={inlineEditWidth}
        onSubmit={handleInlineEditSubmit}
        onCancel={closeInlineEdit}
      />
    {/if}
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
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div class="md-preview-content" role="document" onclick={(e) => {
        const a = (e.target as HTMLElement).closest('a');
        if (a?.href) { e.preventDefault(); openExternal(a.href); }
      }} onkeydown={(e) => {
        if (e.key === 'Enter') {
          const a = (e.target as HTMLElement).closest('a');
          if (a?.href) { e.preventDefault(); openExternal(a.href); }
        }
      }}>
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
    user-select: none;
    -webkit-user-select: none;
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

  /* Scrollbar annotations overlay */
  .editor-wrapper :global(.cm-scrollbar-annotations) {
    position: absolute;
    top: 0;
    right: 0;
    width: 8px;
    height: 100%;
    pointer-events: none;
    z-index: 5;
  }

  .editor-wrapper :global(.cm-sb-mark) {
    position: absolute;
    right: 1px;
    width: 6px;
    min-height: 2px;
    border-radius: 1px;
  }

  .editor-wrapper :global(.cm-sb-add) {
    background: var(--success, #a6e3a1);
    opacity: 0.8;
  }

  .editor-wrapper :global(.cm-sb-mod) {
    background: var(--accent, #89b4fa);
    opacity: 0.8;
  }

  .editor-wrapper :global(.cm-sb-del) {
    background: var(--error, #f38ba8);
    opacity: 0.8;
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
    color: var(--settings-icon, #B34B3C);
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
    color: var(--settings-icon, #B34B3C);
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
    border-left: 3px solid var(--settings-icon, #B34B3C);
    margin: 0 0 12px;
    padding: 4px 16px;
    color: var(--text-secondary);
    background: color-mix(in srgb, var(--settings-icon, #B34B3C) 5%, transparent);
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
