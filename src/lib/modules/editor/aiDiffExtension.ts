import {
  StateField,
  StateEffect,
  RangeSetBuilder,
  type Extension,
} from '@codemirror/state';
import {
  Decoration,
  type DecorationSet,
  EditorView,
  WidgetType,
  GutterMarker,
  gutter,
} from '@codemirror/view';
import type { EditProposal } from '../ai/editParser';

// ── Effects ──

export const addDiffEffect = StateEffect.define<EditProposal[]>();
export const clearDiffEffect = StateEffect.define<void>();
export const resolveDiffEffect = StateEffect.define<{ id: string; action: 'approve' | 'reject' }>();

// ── State ──

export const aiDiffField = StateField.define<EditProposal[]>({
  create() { return []; },
  update(edits, tr) {
    for (const e of tr.effects) {
      if (e.is(addDiffEffect)) return e.value; // replace, not append
      if (e.is(clearDiffEffect)) return [];
      if (e.is(resolveDiffEffect)) return edits.filter(edit => edit.id !== e.value.id);
    }
    return edits;
  },
});

// ── Decorations ──

const deletedLineDeco = Decoration.line({ class: 'cm-ai-deleted-line' });
const addedLineDeco = Decoration.line({ class: 'cm-ai-added-line' });

class AddedCodeWidget extends WidgetType {
  constructor(readonly code: string, readonly editId: string) { super(); }
  toDOM() {
    const wrap = document.createElement('div');
    wrap.className = 'cm-ai-added-block';
    wrap.textContent = this.code;
    return wrap;
  }
  eq(other: AddedCodeWidget) { return other.editId === this.editId && other.code === this.code; }
}

class DiffControlsWidget extends WidgetType {
  constructor(readonly editId: string) { super(); }
  toDOM(view: EditorView) {
    const wrap = document.createElement('span');
    wrap.className = 'cm-ai-diff-controls';

    const approve = document.createElement('button');
    approve.className = 'cm-ai-btn cm-ai-btn-approve';
    approve.textContent = '✓ Accept';
    approve.onclick = () => {
      view.dispatch({ effects: resolveDiffEffect.of({ id: this.editId, action: 'approve' }) });
      // Dispatch custom event for the Svelte layer to handle the actual file edit
      view.dom.dispatchEvent(new CustomEvent('ai-diff-resolve', { detail: { id: this.editId, action: 'approve' }, bubbles: true }));
    };

    const reject = document.createElement('button');
    reject.className = 'cm-ai-btn cm-ai-btn-reject';
    reject.textContent = '✗ Reject';
    reject.onclick = () => {
      view.dispatch({ effects: resolveDiffEffect.of({ id: this.editId, action: 'reject' }) });
      view.dom.dispatchEvent(new CustomEvent('ai-diff-resolve', { detail: { id: this.editId, action: 'reject' }, bubbles: true }));
    };

    wrap.appendChild(approve);
    wrap.appendChild(reject);
    return wrap;
  }
  eq(other: DiffControlsWidget) { return other.editId === this.editId; }
}

// ── Gutter ──

class AiGutterMarker extends GutterMarker {
  toDOM() {
    const el = document.createElement('span');
    el.className = 'cm-ai-gutter-marker';
    el.textContent = '✦';
    return el;
  }
}

const aiGutterMarker = new AiGutterMarker();

const aiGutter = gutter({
  class: 'cm-ai-gutter',
  markers(view) {
    const edits = view.state.field(aiDiffField);
    const builder = new RangeSetBuilder<GutterMarker>();
    for (const edit of edits) {
      if (edit.status !== 'pending') continue;
      const startLine = Math.max(1, edit.startLine);
      const endLine = Math.min(view.state.doc.lines, edit.endLine || startLine);
      for (let l = startLine; l <= endLine; l++) {
        const line = view.state.doc.line(l);
        builder.add(line.from, line.from, aiGutterMarker);
      }
    }
    return builder.finish();
  },
});

// ── Decoration builder ──

const diffDecorations = EditorView.decorations.compute([aiDiffField], (state) => {
  const edits = state.field(aiDiffField);
  if (edits.length === 0) return Decoration.none;
  const builder = new RangeSetBuilder<Decoration>();

  for (const edit of edits) {
    if (edit.status !== 'pending') continue;
    const startLine = Math.max(1, edit.startLine);
    const endLine = Math.min(state.doc.lines, edit.endLine || startLine);
    if (startLine > state.doc.lines || endLine < 1) continue;

    // Mark original lines as deleted
    for (let l = startLine; l <= endLine; l++) {
      const line = state.doc.line(l);
      builder.add(line.from, line.from, deletedLineDeco);
    }

    // Add widget showing new code after the last deleted line
    const lastLine = state.doc.line(endLine);
    builder.add(lastLine.to, lastLine.to, Decoration.widget({
      widget: new AddedCodeWidget(edit.newCode, edit.id),
      side: 1,
      block: true,
    }));

    // Add controls widget
    builder.add(lastLine.to, lastLine.to, Decoration.widget({
      widget: new DiffControlsWidget(edit.id),
      side: 1,
      block: true,
    }));
  }

  try {
    return builder.finish();
  } catch {
    return Decoration.none;
  }
});

// ── Theme ──

const aiDiffTheme = EditorView.baseTheme({
  '.cm-ai-deleted-line': {
    backgroundColor: 'rgba(255, 0, 0, 0.08)',
    textDecoration: 'line-through',
    opacity: '0.6',
  },
  '.cm-ai-added-block': {
    backgroundColor: 'rgba(0, 200, 0, 0.08)',
    borderLeft: '3px solid #4ec9b0',
    padding: '4px 8px',
    fontFamily: 'inherit',
    fontSize: 'inherit',
    whiteSpace: 'pre',
    margin: '2px 0',
  },
  '.cm-ai-diff-controls': {
    display: 'flex',
    gap: '6px',
    padding: '4px 8px',
    margin: '2px 0',
  },
  '.cm-ai-btn': {
    padding: '3px 10px',
    borderRadius: '4px',
    fontSize: '11px',
    fontWeight: '600',
    cursor: 'pointer',
    border: '1px solid transparent',
  },
  '.cm-ai-btn-approve': {
    backgroundColor: 'rgba(78, 201, 176, 0.15)',
    color: '#4ec9b0',
    border: '1px solid rgba(78, 201, 176, 0.3)',
  },
  '.cm-ai-btn-approve:hover': {
    backgroundColor: 'rgba(78, 201, 176, 0.25)',
  },
  '.cm-ai-btn-reject': {
    backgroundColor: 'rgba(241, 76, 76, 0.1)',
    color: '#f14c4c',
    border: '1px solid rgba(241, 76, 76, 0.3)',
  },
  '.cm-ai-btn-reject:hover': {
    backgroundColor: 'rgba(241, 76, 76, 0.2)',
  },
  '.cm-ai-gutter-marker': {
    color: '#4a9eff',
    fontSize: '12px',
  },
});

// ── Export extension ──

export function aiDiffExtension(): Extension {
  return [aiDiffField, diffDecorations, aiGutter, aiDiffTheme];
}
