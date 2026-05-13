# Group 05 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass
- `svelte-check`: ✅ pass
- Full test suite: ✅ 55 tests (7 Rust + 48 frontend)

## Orphan project delete (C4)

`knowledge_delete_project` takes a `project_root: String` and derives the db_hash.
For orphan projects (where project_root is "(unknown)"), the frontend calls
`onDeleteProject("(unknown)")` which hashes "(unknown)" — this deletes the WRONG db
(or no db at all). There's no way to delete by db_hash directly.

## AI request size cap (M16)

`sendStreamingMessage` truncates conversation history when total > 400,000 chars
but does NOT cap individual user messages or file contexts. A user could paste
200KB of text or attach multiple large files with no guard.

## Scope decisions

- §5.1 (Delete orphan by hash): Implement
- §5.2 (Project root SSOT): Deferred — requires changing set_project_root return type
- §5.3 (Async mutex): Deferred — mechanical type change, low risk but high churn
- §5.4 (Reload consistency): Deferred — naming cleanup, low priority
- §5.5 (Stale closure guard): Deferred — requires careful editor effect refactoring
- §5.6 (AI request size cap): Implement
