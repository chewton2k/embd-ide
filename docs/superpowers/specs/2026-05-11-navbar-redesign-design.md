# Navbar Redesign

**Date:** 2026-05-11

## Overview

Reorganize the IDE chrome into two distinct rows and replace the statusbar's action buttons with a file path breadcrumb. The result mirrors VSCode's layout: a full-width toolbar row with tabs, a sidebar-scoped explorer header row, and a minimal statusbar.

---

## Row 1 — Toolbar (full width, new `Toolbar.svelte`)

**Layout (left → right):**
```
[≡ sidebar toggle] [tabs — flex 1] [search in file input] [terminal] [AI] [git] [autosave] [⚙ settings]
```

- **Sidebar toggle:** icon button on the far left. Clicking sets `sidebarVisible` (local App.svelte state) to true/false. Icon is a panel/hamburger icon.
- **Tabs:** `<Tabs />` component occupies all remaining horizontal space (`flex: 1; min-width: 0`).
- **Search in file:** text input placeholder "Search in file". On focus or Enter, increments a `triggerSearchInFile` writable store (exported from `stores.ts`). `Editor.svelte` watches this store and calls CodeMirror's `openSearchPanel` command when the value changes. This avoids prop drilling through the component tree. The input clears on blur if empty.
- **Action buttons (right side):** terminal toggle, AI chat toggle, git panel toggle, autosave toggle, settings gear. These are the exact buttons currently in the statusbar bottom-left. They import their state directly from stores (`showTerminal`, `showChat`, `showGit`, `autosaveEnabled`, `showSettings`).
- **Git branch** is removed from the statusbar and shown as a small badge next to the git button (tooltip or inline label).

**Props:**
- `sidebarVisible: boolean`
- `onToggleSidebar: () => void`
- `onSearchInFile: () => void`

All panel toggle state is read/written via stores — no additional props needed.

---

## Row 2 — Explorer header (sidebar width, existing `FileTree.svelte`)

No code changes. The existing `tree-header` div (folder icon + root name + new file / new folder / search files / open folder buttons) already lives at the top of the sidebar. It appears and disappears with the sidebar naturally.

The sidebar toggle in Row 1 controls `sidebarVisible` which gates the entire `.sidebar` div in App.svelte.

---

## Statusbar bottom

**Left side — breadcrumb (replaces all toggle buttons):**

Pill-style path segments derived reactively from `activeFilePath` and `projectRoot`:
```
[embd] › [src] › [lib] › [app.css]
```
- Segments are the path components of `activeFilePath` relative to `projectRoot`, split on `/`.
- First segment is the project root folder name.
- Rendered as small rounded chips separated by `›` dividers.
- Blank (no chips) when `activeFilePath` is null.
- Implemented inline in App.svelte using a derived computation (no new component needed).

**Right side — unchanged:**
- Save indicator (saved / unsaved dot)
- `embd v{appVersion}`

---

## Store changes (`stores.ts`)

Move `showChat` and `showGit` from local App.svelte `$state` into exported writable stores, consistent with `showTerminal` and `showSettings`:

```ts
export const showChat = writable<boolean>(false);
export const showGit = writable<boolean>(false);
```

The mutual-exclusion logic (opening chat closes git and vice versa) moves into the toggle functions — either helper functions exported from stores, or kept as handlers in App.svelte that write to both stores.

---

## Files changed

| File | Change |
|------|--------|
| `src/lib/stores.ts` | Add `showChat`, `showGit`, `triggerSearchInFile` writable stores |
| `src/lib/Toolbar.svelte` | New component — Row 1 toolbar |
| `src/App.svelte` | Use `<Toolbar>`, add `sidebarVisible` state, add breadcrumb to statusbar, remove old toggle buttons from statusbar |
| `src/lib/Editor.svelte` | Watch `triggerSearchInFile` store, call `openSearchPanel` on change |
| `src/lib/Tabs.svelte` | No changes |
| `src/lib/FileTree.svelte` | No changes |

---

## Constraints

- The sidebar resize handle must remain functional when sidebar is visible.
- Hiding the sidebar must also hide the resize handle (already gated by the `sidebar` div visibility).
- The toolbar height matches the current tabs bar height (`--density-tabs-height`, default 36px) so the overall layout height budget is unchanged.
- Search-in-file input should not submit a form or cause page reload — use `type="search"` or `type="text"` with `onkeydown` handler.
