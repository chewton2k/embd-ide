# Group 06 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass
- `svelte-check`: ✅ pass
- Full test suite: ✅ 64 tests (7 Rust + 57 frontend)

## Silent catches (M1)

59 silent catch blocks across 22 files. Targeted subset for this group:
- GitPanel.svelte: 7 catches (5 should-warn, 2 legitimate)
- FloatingChat.svelte: 2 catches (should-warn)

## Dead code (L1)

ChatPanel.svelte is not imported anywhere (confirmed via grep).
Only references are comments mentioning "ChatPanel" and the `toggleChatPanel` function name.

## Scope decisions

- §5.1 (Silent catches): Implement targeted subset (GitPanel + FloatingChat)
- §5.2 (Split App.svelte): Defer — highest risk, most error-prone
- §5.3 (Consolidate git stores): Defer — large refactor
- §5.4 (Symlinks in file tree): Defer — requires Rust changes
- §5.5 (Remove ChatPanel): Implement
- §5.6 (Update CLAUDE.md): Skip — file is in .gitignore
- §5.7 (Consolidate icon libraries): Defer — visual audit needed
