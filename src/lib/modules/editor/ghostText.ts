import { EditorView, Decoration, WidgetType, type DecorationSet } from '@codemirror/view';
import { StateField, StateEffect } from '@codemirror/state';
import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { aiProvider, aiModel } from '../ai/ai';

// ── Effects ──

const setSuggestion = StateEffect.define<string>();
const clearSuggestion = StateEffect.define<void>();

// ── Widget ──

class GhostTextWidget extends WidgetType {
  constructor(readonly text: string) { super(); }
  toDOM() {
    const span = document.createElement('span');
    span.className = 'cm-ghost-text';
    span.textContent = this.text;
    return span;
  }
  eq(other: GhostTextWidget) { return other.text === this.text; }
}

// ── State ──

const ghostTextField = StateField.define<{ text: string; pos: number } | null>({
  create() { return null; },
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setSuggestion)) return { text: e.value, pos: tr.state.selection.main.head };
      if (e.is(clearSuggestion)) return null;
    }
    // Clear on any document change or cursor move
    if (tr.docChanged || tr.selection) return null;
    return value;
  },
});

const ghostTextDecoration = EditorView.decorations.compute([ghostTextField], (state) => {
  const ghost = state.field(ghostTextField);
  if (!ghost || !ghost.text) return Decoration.none;
  return Decoration.set([
    Decoration.widget({ widget: new GhostTextWidget(ghost.text), side: 1 }).range(ghost.pos),
  ]);
});

// ── Debounced trigger ──

let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let enabled = true;
let debounceMs = 450;

const ghostTextPlugin = EditorView.updateListener.of((update) => {
  if (!enabled) return;
  if (!update.docChanged) return;

  // Clear existing suggestion
  update.view.dispatch({ effects: clearSuggestion.of(undefined) });

  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    requestCompletion(update.view);
  }, debounceMs);
});

async function requestCompletion(view: EditorView) {
  const state = view.state;
  const pos = state.selection.main.head;
  const line = state.doc.lineAt(pos);

  // Don't trigger at start of line or in empty files
  if (line.text.trim().length < 2) return;
  if (state.doc.length < 10) return;

  // Get context: current line + 15 lines above/below
  const lineNum = line.number;
  const startLine = Math.max(1, lineNum - 15);
  const endLine = Math.min(state.doc.lines, lineNum + 15);
  let context = '';
  for (let i = startLine; i <= endLine; i++) {
    const l = state.doc.line(i);
    context += (i === lineNum ? l.text.slice(0, pos - line.from) + '█' : l.text) + '\n';
  }

  try {
    const response = await invoke<string>('ai_chat', {
      request: {
        prompt: `Complete the code at the cursor position (█). Return ONLY the completion text, nothing else. No explanation, no markdown, just the raw code that should be inserted at the cursor.\n\nCode:\n${context}`,
        context: null,
        model: get(aiModel),
        provider: get(aiProvider),
      },
    });

    const suggestion = response.trim().split('\n')[0]; // Take first line only
    if (suggestion && suggestion.length > 0 && suggestion.length < 200) {
      // Only apply if cursor hasn't moved
      if (view.state.selection.main.head === pos) {
        view.dispatch({ effects: setSuggestion.of(suggestion) });
      }
    }
  } catch { /* ignore completion failures */ }
}

// ── Keymap: Tab to accept, Escape to dismiss ──

const ghostTextKeymap = EditorView.domEventHandlers({
  keydown(event: KeyboardEvent, view: EditorView) {
    const ghost = view.state.field(ghostTextField);
    if (!ghost) return false;

    if (event.key === 'Tab') {
      event.preventDefault();
      view.dispatch({
        changes: { from: ghost.pos, insert: ghost.text },
        effects: clearSuggestion.of(undefined),
      });
      return true;
    }

    if (event.key === 'Escape') {
      view.dispatch({ effects: clearSuggestion.of(undefined) });
      return true;
    }

    return false;
  },
});

// ── Theme ──

const ghostTextTheme = EditorView.baseTheme({
  '.cm-ghost-text': {
    color: '#666',
    fontStyle: 'italic',
    opacity: '0.6',
  },
});

// ── Export ──

export function ghostTextExtension() {
  return [ghostTextField, ghostTextDecoration, ghostTextPlugin, ghostTextKeymap, ghostTextTheme];
}

export function setGhostTextEnabled(value: boolean) {
  enabled = value;
}

export function setGhostTextDelay(ms: number) {
  debounceMs = ms;
}
