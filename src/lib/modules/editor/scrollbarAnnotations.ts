/**
 * Scrollbar annotations — renders colored tick marks on the scrollbar track
 * showing where git-changed lines are in the document. Similar to VSCode's
 * scrollbar decorations.
 *
 * Usage: include `scrollbarAnnotations()` in the editor extensions, then
 * dispatch `setScrollbarRanges.of(ranges)` whenever git diff data updates.
 */

import { StateEffect, StateField } from '@codemirror/state';
import { EditorView, ViewPlugin } from '@codemirror/view';
import type { ViewUpdate } from '@codemirror/view';

export interface ScrollbarRange {
  kind: 'add' | 'mod' | 'del';
  start: number;
  end: number;
}

/** Effect to update the scrollbar annotation ranges. */
export const setScrollbarRanges = StateEffect.define<ScrollbarRange[]>();

/** StateField holding the current set of ranges. */
const scrollbarRangesField = StateField.define<ScrollbarRange[]>({
  create: () => [],
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setScrollbarRanges)) return e.value;
    }
    return value;
  },
});

/** ViewPlugin that renders the overlay. */
const scrollbarPlugin = ViewPlugin.fromClass(
  class {
    dom: HTMLElement;
    private lastRanges: ScrollbarRange[] = [];
    private lastLines = 0;

    constructor(view: EditorView) {
      this.dom = document.createElement('div');
      this.dom.className = 'cm-scrollbar-annotations';
      this.dom.setAttribute('aria-hidden', 'true');
      // Insert into the scroller so it scrolls with content positioning
      const scroller = view.scrollDOM;
      scroller.style.position = 'relative';
      scroller.appendChild(this.dom);
      this.render(view);
    }

    update(update: ViewUpdate) {
      const ranges = update.state.field(scrollbarRangesField);
      const lines = update.state.doc.lines;
      if (ranges !== this.lastRanges || lines !== this.lastLines) {
        this.lastRanges = ranges;
        this.lastLines = lines;
        this.render(update.view);
      }
    }

    private render(view: EditorView) {
      const ranges = view.state.field(scrollbarRangesField);
      const totalLines = view.state.doc.lines;

      if (ranges.length === 0 || totalLines === 0) {
        this.dom.innerHTML = '';
        return;
      }

      // Build markers as a single innerHTML for performance
      let html = '';
      for (const r of ranges) {
        const topPct = ((r.start - 1) / totalLines) * 100;
        const heightPct = Math.max(((r.end - r.start + 1) / totalLines) * 100, 0.3);
        const cls = r.kind === 'add' ? 'add' : r.kind === 'del' ? 'del' : 'mod';
        html += `<div class="cm-sb-mark cm-sb-${cls}" style="top:${topPct.toFixed(2)}%;height:${heightPct.toFixed(2)}%"></div>`;
      }
      this.dom.innerHTML = html;
    }

    destroy() {
      this.dom.remove();
    }
  },
);

/** Extension bundle — include in editor extensions. */
export function scrollbarAnnotations() {
  return [scrollbarRangesField, scrollbarPlugin];
}
