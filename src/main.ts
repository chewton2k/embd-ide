import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import SettingsWindow from './lib/settings/SettingsWindow.svelte'
import { log } from './lib/modules/logging'

const target = document.getElementById('app')!
const isSettings = window.location.hash.startsWith('#settings')

// Mount the app first so the user sees the UI as quickly as possible.
const app = mount(isSettings ? SettingsWindow : App, { target })

/**
 * Iconify icon collections.
 *
 * The vscode-icons + simple-icons JSON sets weigh roughly 8 MB combined
 * after minification. Loading them eagerly would block the main bundle
 * download / parse and balloon time-to-interactive.
 *
 * Strategy:
 *  1. Mount the app first (above) so the UI is responsive immediately.
 *  2. Dynamically import the collections after mount so Vite gives them
 *     their own chunks that load in parallel with no startup cost.
 *  3. Until they're registered, Iconify falls back to fetching specific
 *     icons from its public API (api.iconify.design, allowed by the
 *     app's CSP). In production that fallback window is sub-frame for
 *     local chunks, so users effectively never see the gap.
 *
 * Errors during the deferred load are caught and logged but never
 * surface to the user — icons gracefully fall back to remote fetch.
 */
async function loadIconCollectionsDeferred(): Promise<void> {
  // Use idle callback when available so we don't compete with the
  // browser's first paint / layout work.
  const schedule: (cb: () => void) => void =
    typeof requestIdleCallback === 'function'
      ? (cb) => requestIdleCallback(cb, { timeout: 1500 })
      : (cb) => setTimeout(cb, 0)

  schedule(async () => {
    try {
      const [iconify, vscode, simple] = await Promise.all([
        import('@iconify/svelte'),
        import('./lib/icons/vscode-icons-subset.json'),
        import('./lib/icons/simple-icons-subset.json'),
      ])
      const addCollection = iconify.addCollection
      const vscodeData = (vscode as { default?: unknown }).default ?? vscode
      const simpleData = (simple as { default?: unknown }).default ?? simple
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      addCollection(vscodeData as any)
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      addCollection(simpleData as any)
    } catch (err) {
      log.warn('Iconify collections failed to load locally', err);
    }
  })
}

void loadIconCollectionsDeferred()

export default app
