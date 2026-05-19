# Group 07 — UX Polish

> **Status:** Not started
> **Risk:** Low — surface-level polish behind stable behavior
> **Effort:** Small (1–2 days)
> **Depends on:** Group 00 (logging + tests). May land in parallel
>                 with later stages of Group 06 since the surfaces
>                 are mostly disjoint.

## 1. Goal

Replace the rough edges that make the app feel pre-production: a
native `alert()` jarring against the dark IDE chrome, a Linux file-
manager fallback that doesn't actually highlight the target file, an
expensive `color-mix` recomputed every paint, a settings window that
ships the same code twice, a vendor catch-all chunk that re-bloats
silently, and stale model defaults that point at non-existent IDs.

These are individually small. Together they are what separates "demo"
from "production".

## 2. Audit references

- **L2** — `alert()` in `App.svelte::openRecentProject` is jarring.
- **L3** — `reveal_in_file_manager` Linux path doesn't select the
  file.
- **L4** — `color-mix` recomputed every paint in CSS hot paths.
  (NOTE: This is owned by Group 04 — already covered there.)
- **L6** — `todo.md` has 50+ items mixed with prompt instructions.
- **L7** — `recentProjects` paths can grow unbounded if users open
  many projects (clamped at 30 already; documentation only).
- **L8** — Settings window shares the main bundle.
- **M11** — Catch-all `vendor` chunk in `vite.config.ts`
  `manualChunks` re-bloats silently when new heavy deps are added.
- **M12** — Default models in Rust (`gpt-5-mini`,
  `claude-sonnet-4-6`) don't exist as real model IDs.

This group also picks up the Preview-allow-list prompt UI that
Group 01 deferred (CSP `frame-src https://*` tightening).

## 3. Preservation guarantees

- All current dialogs, prompts, and notifications keep their meaning.
  Replacements look different but communicate the same information
  with the same call-to-action.
- All current settings panels work identically.
- Every persisted setting still loads with its prior default.
- Default model IDs change, but a user with a saved model preference
  keeps that preference. Only first-time users (no preference yet)
  see the new default.
- Bundle layout may change; total bundle size MUST NOT regress > 2%.
- Settings window cold-open time should improve (route-level split).

## 4. Pre-flight

1. Capture the current `alert()` flow on macOS/Linux/Windows
   screenshots — for the visual diff.
2. Capture the Linux `xdg-open` behavior on a live distro VM
   (Ubuntu, Fedora) to see what currently happens.
3. Capture bundle size per chunk.
4. Document which model IDs are currently fetched in the Settings
   → Models view; confirm they fail with a 404 if no user
   preference has been saved (this is the bug we're fixing).

## 5. Implementation tasks

### 5.1 Toast notification system (L2)

Files: new `src/lib/components/Toast.svelte`,
`src/lib/modules/ui/toast.ts`. Replace `alert()` in
`src/App.svelte`.

**Toast store:**

```ts
export interface ToastEntry {
  id: number;
  level: 'info' | 'warn' | 'error' | 'success';
  message: string;
  action?: { label: string; onClick: () => void };
  durationMs: number;
}

export const toasts = writable<ToastEntry[]>([]);

let nextId = 1;

export function showToast(t: Omit<ToastEntry, 'id'>): number {
  const id = nextId++;
  toasts.update(list => [...list, { id, ...t }]);
  if (t.durationMs > 0) {
    setTimeout(() => dismissToast(id), t.durationMs);
  }
  return id;
}

export function dismissToast(id: number): void {
  toasts.update(list => list.filter(t => t.id !== id));
}
```

**Toast component:**

- Stacked at bottom-right of the IDE.
- Each toast has the appropriate lucide-svelte icon
  (`Info`, `AlertTriangle`, `XCircle`, `CheckCircle2`).
- Optional action button (e.g. "Undo").
- Auto-dismiss based on `durationMs`; default 5000 for info/success,
  8000 for warn, 0 (sticky) for error.
- Slide-in animation using existing `svelte/transition`.

**Mount in App.svelte** at the root.

**Replace `alert()` in `openRecentProject`:**

```ts
showToast({
  level: 'warn',
  message: `Project folder no longer exists: ${project.path}`,
  durationMs: 6000,
});
```

**Tests:**

- Vitest: showToast pushes onto the store; auto-dismiss after the
  duration; manual dismiss removes by id.

### 5.2 Linux file-manager select via D-Bus (L3)

File: `src-tauri/src/modules/fs/mod.rs::reveal_in_file_manager`.

**Current Linux fallback:** `xdg-open <parent_dir>`. Opens the
folder; doesn't select the file.

**New approach:** use the `org.freedesktop.FileManager1` D-Bus
interface, which file managers (Nautilus, Dolphin, Thunar, Nemo,
PCManFM) implement. Method: `ShowItems`.

Add the `zbus` crate (Linux only, gated by cfg):

```toml
[target.'cfg(target_os = "linux")'.dependencies]
zbus = "5"
```

Implementation (Linux only):

```rust
#[cfg(target_os = "linux")]
async fn reveal_via_dbus(path: &str) -> Result<(), String> {
    use zbus::Connection;
    let conn = Connection::session().await.map_err(|e| e.to_string())?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.freedesktop.FileManager1",
        "/org/freedesktop/FileManager1",
        "org.freedesktop.FileManager1",
    ).await.map_err(|e| e.to_string())?;
    let uri = format!("file://{}", path);
    proxy.call::<_, _, ()>("ShowItems", &(vec![uri], "")).await.map_err(|e| e.to_string())?;
    Ok(())
}
```

Fall through to the existing `xdg-open` if D-Bus isn't available.

**Tests:**

- Manual on Linux VM with each major file manager.
- Cargo cfg-gated test that skips on non-Linux.

### 5.3 Defer L4 (already in Group 04)

L4 is implemented in Group 04 §5.11. No work here.

### 5.4 Clean up `todo.md` (L6)

File: `/Users/.../leo/todo.md`.

**Process:**

- Move closed-out items to `docs/changelog.md` or just delete.
- Move forward-looking items to GitHub Issues (or, if the project
  isn't using Issues, into `docs/roadmap.md`).
- Move prompt instructions out of the file entirely (they belong
  in `CLAUDE.md` or per-conversation prompts, not a tracker).

The result: `todo.md` is either deleted or reduced to a minimal
"see roadmap.md" pointer.

**Tests:** none (documentation cleanup).

### 5.5 Recent-project bound documentation (L7)

File: `docs/limitations.md` (create if needed).

The bound is fine (30 projects × 20 files = ~600 entries in
state.json). Document the cap and how to reset it (delete
state.json).

**Tests:** none.

### 5.6 Settings window code split (L8)

File: `src/main.ts`, `vite.config.ts`.

**Current:** main bundle includes both `App.svelte` and
`SettingsWindow.svelte`. Loading either route pays for both.

**Fix:** dynamic import for `SettingsWindow`:

```ts
import { mount } from 'svelte'
import './app.css'

const target = document.getElementById('app')!
const isSettings = window.location.hash.startsWith('#settings')

if (isSettings) {
  const { default: SettingsWindow } = await import('./lib/settings/SettingsWindow.svelte');
  mount(SettingsWindow, { target });
} else {
  const { default: App } = await import('./App.svelte');
  mount(App, { target });
}

// Iconify deferred load (existing) ...
```

Vite/Rollup will produce two route chunks. Settings window cold
open should be measurably faster.

**Bundle measurement:**

- Before/after: each route's load size, including shared deps.

**Tests:**

- Manual: open the settings window from the menu; first-paint
  time recorded.

### 5.7 Audit catch-all vendor chunk (M11)

File: `vite.config.ts::manualChunks`.

**Current:** unmatched `node_modules` deps land in a generic
`vendor` chunk. The audit noted this is currently 590 KB.

**Process:**

1. After build, inspect `dist/assets/vendor-*.js` and identify which
   packages contributed (use `vite-bundle-visualizer` or `rollup-plugin-visualizer`
   added as a devDep — small, focused, well-known).
2. For any package over 100 KB, give it a named chunk in
   `manualChunks`.
3. Document the resulting layout in `vite.config.ts` comments.

The end state: no single dep > 100 KB lives in the catch-all. Future
heavy deps surface in build output and force an explicit decision.

**Tests:**

- Build size diff.

### 5.8 Default model IDs (M12)

Files: `src-tauri/src/modules/ai/mod.rs::default_model`,
`src/lib/components/ai/FloatingChat.svelte::MODELS`.

**Current Rust defaults:** `gpt-5-mini`, `claude-sonnet-4-6`,
`openrouter/auto`.

**Fix:** these need to be model IDs that actually work today.

- OpenRouter: `openrouter/auto` is correct (it's a meta-model that
  picks a real one). Keep.
- OpenAI: switch to a known-good ID. Check the latest `/v1/models`
  list and choose a sensible cheap-and-fast default. As of the
  audit timeframe, `gpt-4o-mini` is appropriate. The implementing
  agent should verify with a test API call before committing.
- Anthropic: switch to a known-good ID; `claude-3-5-sonnet-20241022`
  or whichever is current. Verify before committing.

**Frontend MODELS list:** sync with the Rust defaults. Same model
IDs in both places.

**Migration:** users with a previously-saved `aiModel` setting keep
their setting (no migration). Only first-time users see the new
default. Verified by leaving the existing `persistedString` logic
untouched.

**Tests:**

- Vitest: `aiModel` store reads from localStorage if set, otherwise
  the new default.
- Manual: send a "ping" message with each provider's default;
  confirm 200 OK.

### 5.9 Preview allow-list UI (CSP tightening, gated from Group 01)

Files: new `src/lib/components/preview/AllowDialog.svelte`,
`src/lib/modules/preview/allowList.ts`.

**Behavior:**

- New persisted setting `previewAllowList: string[]` (default `[]`).
- When a non-localhost preview URL is loaded:
  - If host in allow-list → load.
  - Otherwise → render a confirmation dialog: "Allow leo to display
    `<host>`?" with [Once] [Always] [Cancel].
  - "Always" appends to allow-list and persists.
- Existing localhost preview behavior unchanged.

**Once Allow-list UI ships, tighten CSP:**

Change `tauri.conf.json` `frame-src` to drop `https://*`. The user-
controlled allow-list is enforced at the JS layer (Preview
component refuses to load disallowed hosts).

This is the deferred half of Group 01 §5.4.

**Tests:**

- Vitest: allow-list logic unit-tested.
- Manual: load a non-localhost preview, confirm prompt; choose
  "Always", confirm subsequent loads bypass the prompt.

## 6. Test plan (cumulative)

- Per-task tests above.
- **Visual regression** on the smoke flow: screenshots before/after,
  no unexpected differences.
- **Bundle size diff** documented.
- **Settings cold-open time** documented (before vs after split).

## 7. Code review checklist

- [ ] Toast component uses lucide-svelte icons. No emoji. No
      ad-hoc SVG.
- [ ] Toasts are dismissible by both click and timeout. Multiple
      toasts stack visibly.
- [ ] No `alert()` / `confirm()` / `prompt()` calls remain in the
      frontend (`grep -rE "alert\(|confirm\(|prompt\(" src/`).
- [ ] Linux D-Bus path is gated by `cfg(target_os = "linux")`;
      builds on macOS and Windows untouched.
- [ ] D-Bus call falls through to `xdg-open` if proxy fails.
- [ ] Settings window route-split measurably reduces main bundle
      size; numbers in PR.
- [ ] Vendor-chunk visualization committed (or generated on demand
      via a documented command).
- [ ] Default model IDs verified against live APIs at PR time.
      Document the verification in the PR description.
- [ ] Allow-list dialog blocks the load until the user decides.
      Cancel does NOT load. Once allows for one navigation only.
- [ ] CSP tightening only ships AFTER the allow-list UI lands.
      Tested as a single coherent feature.
- [ ] No new `console.*`. No new emoji. No new untyped errors.
- [ ] Behavioral baseline replay passes.

## 8. Rollback

Each task is independently revertable.

- §5.1: restore `alert()`. Old behavior returns.
- §5.2: drop the D-Bus path; xdg-open fallback returns.
- §5.4: restore todo.md.
- §5.6: revert dynamic import; main bundle returns.
- §5.7: revert chunk splits; current vendor chunk returns.
- §5.8: revert model defaults. Existing user preferences unaffected.
- §5.9: revert allow-list AND keep the CSP wildcard. Atomic
  rollback — never leave the strict CSP without the prompt UI.

## 9. Out of scope

- **Full design system / token rebuild.** Out of scope.
- **Animation library.** Existing `svelte/transition` is enough.
- **Localization (i18n).** Out of scope.
- **Toast theming per user setting.** Out of scope.
- **Sound notifications.** Out of scope.

## 10. Notes for the implementing agent

- The toast component is the kind of thing where consistency
  matters — every future "I need to tell the user something"
  surface should use it. Document the API in
  `docs/ui-patterns.md`.
- Linux D-Bus integration assumes a session bus; on a server
  install (no graphical session) the fallback to `xdg-open`
  should still work. Test in that scenario.
- For the Settings split, ensure CSS is still loaded for the
  Settings route. Vite's CSS-per-entry handling should do this
  automatically; verify in the build output.
- The default-model fix (M12) is sensitive to the API providers'
  current model lineups. The implementing agent must verify with
  a real (cheap) API call before committing — listing the model
  IDs is not enough since the providers can deprecate without
  warning.
- The allow-list dialog should NOT be a native browser confirm.
  Use the same toast/dialog pattern from §5.1 for visual
  consistency.
