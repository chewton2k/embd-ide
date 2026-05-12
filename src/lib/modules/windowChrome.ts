/**
 * Tracks the OS platform and live window-fullscreen state for chrome
 * decisions like "should we reserve space for the macOS traffic-light
 * buttons?".
 *
 * - `isMac` is a static boolean computed once from `navigator.platform`.
 * - `isFullscreen` is a Svelte writable that mirrors the current Tauri
 *   window's fullscreen state. The store is updated on:
 *     * cold start (read once)
 *     * window resize events (Tauri emits these on enter/leave fullscreen)
 *     * native `fullscreenchange` DOM events as a belt-and-suspenders
 *
 * This file is intentionally tiny and side-effect-free at import time;
 * `installWindowChromeWatchers()` performs the wiring during the host
 * component's `onMount`.
 */
import { writable, type Writable } from 'svelte/store';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** True on macOS. Computed once — platform doesn't change at runtime. */
export const isMac: boolean =
  typeof navigator !== 'undefined' &&
  /Mac|iPhone|iPad|iPod/.test(navigator.platform);

/** Live fullscreen flag for the current Tauri window. */
export const isFullscreen: Writable<boolean> = writable<boolean>(false);

/**
 * Wire up listeners that keep `isFullscreen` in sync.
 * Call from a component's `onMount`. Returns a teardown function.
 */
export async function installWindowChromeWatchers(): Promise<() => void> {
  const win = getCurrentWindow();

  async function refresh() {
    try {
      isFullscreen.set(await win.isFullscreen());
    } catch {
      // Best-effort: some platforms (or sub-windows) may not expose this.
    }
  }

  await refresh();

  // Tauri emits resize events when the window is resized — including
  // entering and leaving fullscreen on macOS — so this is our primary
  // signal. Note: on macOS the resize fires AFTER the ~0.7s fullscreen
  // animation completes, so during the animation our `isFullscreen`
  // state is briefly stale. This is the same limitation other Tauri /
  // Electron apps (VSCode, Zed) have on macOS and is acceptable for
  // chrome decisions like toolbar padding.
  const unlistenResize = await win.onResized(() => refresh());

  // Defense in depth: also listen for the DOM's `fullscreenchange`
  // event in case the user triggers it via the WebView itself.
  const onFsChange = () => { refresh(); };
  document.addEventListener('fullscreenchange', onFsChange);

  return () => {
    unlistenResize();
    document.removeEventListener('fullscreenchange', onFsChange);
  };
}
