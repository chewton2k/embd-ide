/**
 * Helpers for opening secondary Tauri webview windows.
 *
 * Today: just the Settings window. Anchored here so the launching
 * logic (focus existing instance, otherwise create one) is one
 * import away from any caller (toolbar, status bar, menu, command
 * palette).
 *
 * The Settings window is a standalone webview pointed at the same
 * SPA bundle with `#settings` in the URL, which the bootstrap in
 * `main.ts` reads to render `<SettingsWindow>` instead of `<App>`.
 */
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { log } from '../logging';

const SETTINGS_LABEL = 'settings';

/**
 * Open the Settings window. If it already exists, just focus it.
 * Errors are logged via the structured logger; this function never
 * throws.
 */
export async function openSettingsWindow(): Promise<void> {
  try {
    const existing = await WebviewWindow.getByLabel(SETTINGS_LABEL);
    if (existing) {
      // Either of these may fail on platforms where the window is in
      // an unusual state (e.g. minimized/hidden) — non-fatal: the
      // window is still there and reachable from the dock.
      try { await existing.show(); } catch { /* Legitimate: show may fail when already visible */ }
      try { await existing.setFocus(); } catch { /* Legitimate: focus may fail on hidden window */ }
      return;
    }
    const win = new WebviewWindow(SETTINGS_LABEL, {
      url: 'index.html#settings',
      title: 'Settings',
      width: 900,
      height: 640,
      minWidth: 720,
      minHeight: 480,
      resizable: true,
      center: true,
      focus: true,
    });
    win.once('tauri://error', (e) => {
      log.error('Failed to open settings window', e);
    });
  } catch (e) {
    log.error('openSettingsWindow failed', e);
  }
}
