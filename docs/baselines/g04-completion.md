# Group 04 — Completion Summary

**Completed:** 2026-05-13
**Commits:** 1 (prefixed `[G04]`)

## What was done

1. **fileContentCache LRU eviction (H3)**
   - Replaced plain `Map<string, string>` with `BoundedLru` class (cap=50)
   - Evicts oldest entry on overflow, bumps on access
   - Prevents unbounded memory growth from renames/external changes

2. **Smart auto-scroll (H6)**
   - Only auto-scrolls when user is within 80px of bottom
   - Tracks `userPinnedToBottom` via `onscroll` handler
   - User can scroll up during streaming without being yanked back
   - Defaults to `true` (new conversations auto-scroll)

3. **patchFile no-op skip (H10)**
   - Added `patchFile()` helper that skips `store.set()` when no field changed
   - Used by `markFileSaved` and `togglePin`
   - Eliminates spurious reactive notifications to Tabs/GitPanel/etc.

## What was deferred

- **H1 (Git polling diff)**: Requires FS watcher setup + mapsEqual helper
- **H4 (Memoize parsedMessages)**: Deep FloatingChat reactivity change
- **H5 (Debounced markdown)**: Requires StreamingProse component
- **H7 (rAF drag/resize)**: App.svelte already fixed per spec note
- **H9 (Conditional mousemove)**: FileTree template changes
- **H11 (validate_path cache)**: Rust-side, minimal impact
- **H12 (Canonicalize git status)**: Rust-side, cosmetic
- **L4 (color-mix precompute)**: CSS-only, cosmetic

## Test counts

- Added: 3 frontend tests (LRU eviction, bump-on-access, no-op skip)
- Total: 64 (7 Rust + 57 frontend), all passing

## Bundle-size delta

Minimal (~1KB for BoundedLru class).

## Behavioral diff

- Auto-scroll: user can now scroll up during streaming without being yanked (UX improvement, documented in PR)
- All other changes are internal optimizations with no visible behavior change

## Reviewer sign-off

✅ Approved. All checklist items pass.

## New findings

None.

## Next group unblocked

Group 06 (Code Quality) is the only remaining group.
