/**
 * State machine for the three drag-to-resize gutters in the IDE shell:
 *
 *   sidebar (left edge of the file tree → editor)
 *   chat    (right edge of the editor → floating chat)
 *   git     (right edge of the editor → git panel)
 *
 * Extracted from `App.svelte` so the host component just owns the
 * three width `$state` cells. The factory returns a `startDrag(target)`
 * handler that the gutter elements bind to `onmousedown`.
 *
 * Implementation notes preserved from the original:
 *
 *  - Pointer events arrive at up to ~240Hz on high-refresh displays.
 *    We coalesce into one width update per animation frame to avoid
 *    pegging layout on every move event.
 *  - The cursor + user-select global styles are toggled on `body` so
 *    the cursor stays "col-resize" everywhere on the page during the
 *    drag (otherwise it flickers when moving over child elements that
 *    have their own `cursor:` rule).
 *  - On `destroy()` we cancel any pending rAF and detach window
 *    listeners so the resizer can be torn down cleanly with the host
 *    component (idempotent).
 *
 * Bounds are passed in by the caller so tests can use small numbers
 * without hard-coding the production min/max.
 */

export interface PanelBounds {
  min: number;
  max: number;
}

export type PanelTarget = 'sidebar' | 'chat' | 'git';

export interface PanelResizerOptions {
  sidebar: PanelBounds;
  chat: PanelBounds;
  git: PanelBounds;
  /** Called from rAF; should write the new width to component state. */
  setSidebarWidth: (w: number) => void;
  setChatWidth: (w: number) => void;
  setGitWidth: (w: number) => void;
  /**
   * Returns the current viewport width. Defaults to `window.innerWidth`
   * but is injectable for testing.
   */
  viewportWidth?: () => number;
  /** Optional notifier for the component's `dragging` state cell. */
  onDragStateChange?: (target: PanelTarget | null) => void;
  /** rAF replacement for testing. Defaults to `window.requestAnimationFrame`. */
  raf?: (cb: FrameRequestCallback) => number;
  cancelRaf?: (handle: number) => void;
}

export interface PanelResizer {
  /** Returns a `mousedown` handler scoped to the given gutter. */
  startDrag(target: PanelTarget): (e: MouseEvent) => void;
  /** True while a drag is in progress (mirrors the underlying state). */
  isDragging(): PanelTarget | null;
  /** Detach any active window listeners. Safe to call repeatedly. */
  destroy(): void;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function createPanelResizer(opts: PanelResizerOptions): PanelResizer {
  const viewportWidth = opts.viewportWidth ?? (() => window.innerWidth);
  const raf =
    opts.raf ??
    (typeof window !== 'undefined' ? window.requestAnimationFrame.bind(window) : null);
  const cancelRaf =
    opts.cancelRaf ??
    (typeof window !== 'undefined' ? window.cancelAnimationFrame.bind(window) : null);

  let dragging: PanelTarget | null = null;
  let pendingX = 0;
  let rafHandle: number | null = null;

  function applyWidth() {
    rafHandle = null;
    if (!dragging) return;
    const x = pendingX;
    switch (dragging) {
      case 'sidebar':
        opts.setSidebarWidth(clamp(x, opts.sidebar.min, opts.sidebar.max));
        break;
      case 'chat':
        opts.setChatWidth(clamp(viewportWidth() - x, opts.chat.min, opts.chat.max));
        break;
      case 'git':
        opts.setGitWidth(clamp(viewportWidth() - x, opts.git.min, opts.git.max));
        break;
    }
  }

  function onMove(e: MouseEvent) {
    pendingX = e.clientX;
    if (rafHandle !== null) return;
    if (raf) {
      rafHandle = raf(applyWidth);
    } else {
      // No rAF available (e.g. SSR / test envs without a window) —
      // apply synchronously.
      applyWidth();
    }
  }

  function detachWindowListeners() {
    if (typeof window === 'undefined') return;
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
  }

  function onUp() {
    dragging = null;
    if (rafHandle !== null && cancelRaf) {
      cancelRaf(rafHandle);
      rafHandle = null;
    }
    if (typeof document !== 'undefined') {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
    detachWindowListeners();
    opts.onDragStateChange?.(null);
  }

  function startDrag(target: PanelTarget): (e: MouseEvent) => void {
    return (e: MouseEvent) => {
      e.preventDefault();
      dragging = target;
      opts.onDragStateChange?.(target);
      if (typeof document !== 'undefined') {
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
      }
      if (typeof window !== 'undefined') {
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
      }
    };
  }

  function destroy() {
    if (rafHandle !== null && cancelRaf) {
      cancelRaf(rafHandle);
      rafHandle = null;
    }
    detachWindowListeners();
    if (typeof document !== 'undefined' && dragging) {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
    dragging = null;
  }

  return {
    startDrag,
    isDragging: () => dragging,
    destroy,
  };
}
