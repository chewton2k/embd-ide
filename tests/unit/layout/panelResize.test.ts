import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest';
import {
  createPanelResizer,
  type PanelTarget,
} from '$lib/modules/layout/panelResize';

interface RafQueue {
  flush(): void;
  pending(): number;
}

function setupSyncRaf(): RafQueue {
  const queued: FrameRequestCallback[] = [];
  // Stash a queue + monkey-patch window's rAF so the resizer calls it.
  // Tests then call `flush()` to advance the frame.
  Object.defineProperty(globalThis, 'requestAnimationFrame', {
    configurable: true,
    value: (cb: FrameRequestCallback): number => {
      queued.push(cb);
      return queued.length;
    },
  });
  Object.defineProperty(globalThis, 'cancelAnimationFrame', {
    configurable: true,
    value: () => {
      // No-op; tests call `flush()` explicitly.
    },
  });
  return {
    flush() {
      const batch = queued.splice(0);
      for (const cb of batch) cb(performance.now());
    },
    pending() {
      return queued.length;
    },
  };
}

describe('createPanelResizer', () => {
  let raf: RafQueue;
  let setSidebar: ReturnType<typeof vi.fn>;
  let setChat: ReturnType<typeof vi.fn>;
  let setGit: ReturnType<typeof vi.fn>;
  let stateChanges: (PanelTarget | null)[];

  beforeEach(() => {
    raf = setupSyncRaf();
    setSidebar = vi.fn();
    setChat = vi.fn();
    setGit = vi.fn();
    stateChanges = [];
  });

  afterEach(() => {
    // Make sure mouseup listeners installed by a test don't leak.
    window.dispatchEvent(new MouseEvent('mouseup'));
  });

  function build(viewportWidth = 1000) {
    return createPanelResizer({
      sidebar: { min: 100, max: 400 },
      chat: { min: 200, max: 500 },
      git: { min: 250, max: 600 },
      setSidebarWidth: setSidebar,
      setChatWidth: setChat,
      setGitWidth: setGit,
      viewportWidth: () => viewportWidth,
      onDragStateChange: (t) => stateChanges.push(t),
    });
  }

  it('starts a sidebar drag and updates width on mousemove', () => {
    const r = build();
    r.startDrag('sidebar')(new MouseEvent('mousedown'));
    expect(r.isDragging()).toBe('sidebar');
    expect(stateChanges).toEqual(['sidebar']);

    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 250 }));
    raf.flush();
    expect(setSidebar).toHaveBeenCalledWith(250);
  });

  it('clamps sidebar width to its bounds', () => {
    const r = build();
    r.startDrag('sidebar')(new MouseEvent('mousedown'));

    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 50 }));
    raf.flush();
    expect(setSidebar).toHaveBeenLastCalledWith(100);

    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 9999 }));
    raf.flush();
    expect(setSidebar).toHaveBeenLastCalledWith(400);
  });

  it('chat drag width = viewportWidth - clientX, clamped', () => {
    const r = build(1000);
    r.startDrag('chat')(new MouseEvent('mousedown'));

    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 700 }));
    raf.flush();
    expect(setChat).toHaveBeenCalledWith(300);

    // Off the right side → clamp to min.
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 990 }));
    raf.flush();
    expect(setChat).toHaveBeenLastCalledWith(200);
  });

  it('git drag uses the git bounds independently of chat bounds', () => {
    const r = build(1000);
    r.startDrag('git')(new MouseEvent('mousedown'));

    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 600 }));
    raf.flush();
    expect(setGit).toHaveBeenCalledWith(400);
    expect(setChat).not.toHaveBeenCalled();
    expect(setSidebar).not.toHaveBeenCalled();
  });

  it('coalesces multiple mousemoves into a single rAF', () => {
    const r = build();
    r.startDrag('sidebar')(new MouseEvent('mousedown'));

    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 200 }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 220 }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 240 }));
    expect(raf.pending()).toBe(1);

    raf.flush();
    // Only the latest pointer position is applied.
    expect(setSidebar).toHaveBeenCalledTimes(1);
    expect(setSidebar).toHaveBeenLastCalledWith(240);
  });

  it('mouseup ends the drag and detaches listeners', () => {
    const r = build();
    r.startDrag('sidebar')(new MouseEvent('mousedown'));
    expect(r.isDragging()).toBe('sidebar');

    window.dispatchEvent(new MouseEvent('mouseup'));
    expect(r.isDragging()).toBe(null);
    expect(stateChanges).toEqual(['sidebar', null]);

    // Subsequent moves must NOT trigger a width update.
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 999 }));
    raf.flush();
    expect(setSidebar).not.toHaveBeenCalled();
  });

  it('toggles document.body cursor + userSelect during drag', () => {
    const r = build();
    document.body.style.cursor = '';
    document.body.style.userSelect = '';

    r.startDrag('chat')(new MouseEvent('mousedown'));
    expect(document.body.style.cursor).toBe('col-resize');
    expect(document.body.style.userSelect).toBe('none');

    window.dispatchEvent(new MouseEvent('mouseup'));
    expect(document.body.style.cursor).toBe('');
    expect(document.body.style.userSelect).toBe('');
  });

  it('destroy() detaches listeners even if a drag is in progress', () => {
    const r = build();
    r.startDrag('sidebar')(new MouseEvent('mousedown'));

    r.destroy();
    expect(r.isDragging()).toBe(null);

    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 222 }));
    raf.flush();
    expect(setSidebar).not.toHaveBeenCalled();
  });

  it('startDrag prevents default on mousedown', () => {
    const r = build();
    const ev = new MouseEvent('mousedown', { cancelable: true });
    const spy = vi.spyOn(ev, 'preventDefault');
    r.startDrag('git')(ev);
    expect(spy).toHaveBeenCalled();
  });
});
