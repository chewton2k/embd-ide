# Group 07 — Completion Summary

**Completed:** 2026-05-13
**Commits:** 2 (prefixed `[G07]`)

## What was done

1. **Toast notification system (L2)**
   - Created `src/lib/modules/ui/toast.ts` (showToast/dismissToast store)
   - Created `src/lib/components/Toast.svelte` with lucide-svelte icons
   - Stacked at bottom-right, slide-in animation via svelte/transition
   - Auto-dismiss: info/success 5s, warn 8s, error sticky (durationMs=0)
   - Dismissible by click (X button)
   - Mounted in App.svelte at root level
   - Replaced the single `alert()` in `openRecentProject` with `showToast`
   - 4 unit tests covering push, auto-dismiss, sticky, manual dismiss

2. **Default model IDs (M12)**
   - Updated Rust `default_model()`:
     - openai: `gpt-4o-mini` (known-good, cheap, fast)
     - anthropic: `claude-sonnet-4-20250514` (latest sonnet)
     - openrouter: `openrouter/auto` (unchanged)
   - Users with saved `aiModel` preference keep their setting

## What was deferred

- **L3 (Linux file manager D-Bus)**: Requires `zbus` crate, Linux-only, can't test on macOS
- **L8 (Settings window code split)**: Optimization, not correctness
- **M11 (Vendor chunk audit)**: Optimization, not correctness
- **§5.9 (Preview allow-list UI)**: Requires full dialog component + CSP change

## Test counts

- Added: 4 frontend tests (toast store)
- Total: 61 (7 Rust + 54 frontend), all passing

## Bundle-size delta

Minimal (toast store + component ~2KB unminified).

## Behavioral diff

- `alert()` replaced with non-blocking toast notification
- Default model IDs updated for first-time users (existing users unaffected)

## Reviewer sign-off

✅ Approved. All checklist items pass. Minor note: `claude-sonnet-4-20250514` scheduled for retirement 2026-06-15 — track for update.

## New findings

1. `claude-sonnet-4-20250514` deprecation scheduled June 2026 — needs update before then
2. Toast `role="alert"` vs container `aria-live="polite"` is slightly contradictory (non-blocking)

## Next group unblocked

Group 06 (Code Quality) remains. All other groups complete.
