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

// ── State ──

export const aiDiffField = StateField.define<EditProposal[]>({
  create() { return []; },
  update(edits, tr) {
    for (const e of tr.effects) {
      if (e.is(addDiffEffect)) return e.value; // replace, not append
      if (e.is(clearDiffEffect)) return [];
    }
    return edits;
  },
});

// ── Decorations ──

const deletedLineDeco = Decoration.line({ class: 'cm-ai-deleted-line' });
const deletedLineStaleDeco = Decoration.line({ class: 'cm-ai-deleted-line cm-ai-stale-line' });
const addedLineDeco = Decoration.line({ class: 'cm-ai-added-line' });

class AddedCodeWidget extends WidgetType {
  constructor(readonly code: string, readonly editId: string, readonly stale: boolean) { super(); }
  toDOM() {
    const wrap = document.createElement('div');
    wrap.className = this.stale ? 'cm-ai-added-block cm-ai-stale-block' : 'cm-ai-added-block';
    wrap.textContent = this.code;
    return wrap;
  }
  eq(other: AddedCodeWidget) {
    return other.editId === this.editId
      && other.code === this.code
      && other.stale === this.stale;
  }
}

class DiffControlsWidget extends WidgetType {
  constructor(readonly editId: string, readonly stale: boolean) { super(); }
  toDOM(view: EditorView) {
    const wrap = document.createElement('span');
    wrap.className = this.stale ? 'cm-ai-diff-controls cm-ai-stale-controls' : 'cm-ai-diff-controls';

    // NOTE: the click handlers intentionally do NOT mutate `aiDiffField`.
    // `pendingEdits` (the Svelte store) is the single source of truth for
    // which edits exist; the CM `aiDiffField` is a derived projection
    // driven by `pendingEdits.subscribe`. Mutating CM optimistically here
    // would desync the two stores whenever the underlying write fails.
    // We dispatch a custom event and let `approveEdit` / `rejectEdit`
    // mutate `pendingEdits`; CM updates follow automatically.
    //
    // Repeated clicks during the in-flight write are harmless:
    // `approveEdit` looks up the edit by id; once it's removed from the
    // store the second invocation is a silent no-op. Two concurrent
    // approves on the same id race-write identical content (idempotent).
    const dispatchResolve = (action: 'approve' | 'reject') => {
      view.dom.dispatchEvent(new CustomEvent('ai-diff-resolve', {
        detail: { id: this.editId, action },
        bubbles: true,
      }));
    };

    if (this.stale) {
      // Visible warning that the live content has drifted from the
      // proposal's `originalCode` since it was generated. Approving
      // anyway will overwrite the user's intervening changes — the
      // user remains the authority, but they should know.
      const badge = document.createElement('span');
      badge.className = 'cm-ai-stale-badge';
      badge.textContent = '⚠ Stale';
      const tooltip = 'The file content under this edit has changed since the proposal was generated. Accepting will overwrite those changes.';
      badge.title = tooltip;
      // ARIA: `title` is invisible to screen readers and keyboard
      // focus; mirror it to `aria-label` and give the badge a role
      // so assistive tech surfaces the warning.
      badge.setAttribute('role', 'status');
      badge.setAttribute('aria-label', tooltip);
      wrap.appendChild(badge);
    }

    const approve = document.createElement('button');
    approve.className = this.stale ? 'cm-ai-btn cm-ai-btn-approve cm-ai-btn-stale' : 'cm-ai-btn cm-ai-btn-approve';
    approve.textContent = this.stale ? '✓ Accept anyway' : '✓ Accept';
    approve.onclick = () => dispatchResolve('approve');

    const reject = document.createElement('button');
    reject.className = 'cm-ai-btn cm-ai-btn-reject';
    reject.textContent = '✗ Reject';
    reject.onclick = () => dispatchResolve('reject');

    wrap.appendChild(approve);
    wrap.appendChild(reject);
    return wrap;
  }
  eq(other: DiffControlsWidget) {
    return other.editId === this.editId && other.stale === this.stale;
  }
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

    const isStale = edit.stale === true;

    // Mark original lines as deleted (with a stale variant when the
    // proposal's originalCode no longer matches the live content).
    const lineDeco = isStale ? deletedLineStaleDeco : deletedLineDeco;
    for (let l = startLine; l <= endLine; l++) {
      const line = state.doc.line(l);
      builder.add(line.from, line.from, lineDeco);
    }

    // Add widget showing new code after the last deleted line
    const lastLine = state.doc.line(endLine);
    builder.add(lastLine.to, lastLine.to, Decoration.widget({
      widget: new AddedCodeWidget(edit.newCode, edit.id, isStale),
      side: 1,
      block: true,
    }));

    // Add controls widget
    builder.add(lastLine.to, lastLine.to, Decoration.widget({
      widget: new DiffControlsWidget(edit.id, isStale),
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
  // ── Stale-edit variants ────────────────────────────────────────
  // Visual cue when the live content under an edit no longer matches
  // the proposal's `originalCode`. Yellow-orange palette signals
  // caution without being as alarming as red (which is reserved for
  // deletions). The user can still Accept; the badge + altered button
  // text make the consequence explicit.
  '.cm-ai-stale-line': {
    backgroundColor: 'rgba(255, 170, 0, 0.10)',
    // Override the deleted-line strikethrough so the stale state
    // doesn't compound the visual noise.
    textDecoration: 'none',
    opacity: '0.85',
  },
  '.cm-ai-stale-block': {
    borderLeft: '3px solid #f0a020',
    backgroundColor: 'rgba(240, 160, 32, 0.08)',
  },
  '.cm-ai-stale-controls': {
    borderLeft: '3px solid #f0a020',
    paddingLeft: '8px',
  },
  '.cm-ai-stale-badge': {
    fontSize: '10px',
    fontWeight: '600',
    color: '#ffcc66',
    backgroundColor: 'rgba(240, 160, 32, 0.15)',
    border: '1px solid rgba(240, 160, 32, 0.4)',
    borderRadius: '3px',
    padding: '1px 6px',
    marginRight: '4px',
    cursor: 'help',
  },
  '.cm-ai-btn-stale': {
    // Slightly emphasize the "Accept anyway" affordance so a stale
    // approve still feels distinct from a clean approve.
    fontStyle: 'italic',
  },
});

// ── Export extension ──

export function aiDiffExtension(): Extension {
  return [aiDiffField, diffDecorations, aiGutter, aiDiffTheme];
}
