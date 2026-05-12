/**
 * Single source of truth for the "toggle the terminal" intent.
 *
 * Called from:
 *   - App.svelte's Cmd+` keyboard shortcut
 *   - TitleBar.svelte's terminal button
 *   - The status-bar terminal button (panel mode)
 *
 * Keeping the logic here means a behavior change only needs to happen
 * in one place; without this module, multiple call sites would have
 * silently divergent copies (the original bug that motivated extraction).
 *
 * The semantics are intentionally mode-aware:
 *   - panel mode: just toggles the panel's visibility, creating a first
 *     tab on demand if none exist.
 *   - tab mode: ensures a terminal tab exists and is focused; closes if
 *     the user is already focused on a terminal tab.
 */
import { get } from 'svelte/store';
import {
  showTerminal,
  terminalMode,
  terminalTabs,
  activeTerminalTabId,
  createTerminalSignal,
  terminalPath,
  isTerminalPath,
} from './stores/shell';
import { activeFilePath } from './stores/files';

export function toggleTerminal(): void {
  const tabs = get(terminalTabs);
  const mode = get(terminalMode);

  // ── Bottom-panel mode ──────────────────────────────────────────
  // The editor always stays visible; the panel simply hides/shows.
  if (mode === 'panel') {
    if (!get(showTerminal)) {
      showTerminal.set(true);
      createTerminalSignal.update(s => ({ count: s.count + 1, forceNew: false }));
    } else {
      showTerminal.set(false);
    }
    return;
  }

  // ── Legacy tab mode ────────────────────────────────────────────
  if (!get(showTerminal) || tabs.length === 0) {
    showTerminal.set(true);
    createTerminalSignal.update(s => ({ count: s.count + 1, forceNew: false }));
    return;
  }
  if (!isTerminalPath(get(activeFilePath))) {
    const tabId = get(activeTerminalTabId) ?? tabs[0].id;
    activeFilePath.set(terminalPath(tabId));
  } else {
    showTerminal.set(false);
  }
}
