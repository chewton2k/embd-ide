# Group 04 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass
- `svelte-check`: ✅ pass
- Full test suite: ✅ 61 tests (7 Rust + 54 frontend)

## fileContentCache (H3)

Plain `Map<string, string>` with no eviction. Bounded in practice by maxTabs
(closeFile removes entries) but renameOpenFile and external renames could leak.

## Auto-scroll (H6)

`$effect(() => { $chatMessages; scrollToBottom(); })` — fires on every message
change regardless of user scroll position. Hijacks scroll when user is reading
earlier messages during streaming.

## openFiles store (H10)

Every mutation (markFileSaved, togglePin) reconstructs the array via `.map(...)`.
When the patched field already equals the new value, the store still notifies
subscribers (spurious reactive notifications).

## Scope decisions

- §5.1 (Git polling diff): Defer — requires FS watcher setup, complex
- §5.2 (LRU cache): Implement
- §5.3 (Memoize parsedMessages): Defer — touches FloatingChat reactivity deeply
- §5.4 (Debounced markdown): Defer — requires StreamingProse component
- §5.5 (Smart auto-scroll): Implement
- §5.6 (rAF drag/resize): Defer — App.svelte already fixed per spec note
- §5.7 (Conditional mousemove): Defer — requires FileTree template changes
- §5.8 (Map-based openFiles): Implement (patchFile helper)
- §5.9-5.11: Defer — Rust-side optimizations, CSS precompute
