# Group 02 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass
- `svelte-check`: ✅ pass
- Full test suite: ✅ 55 tests (7 Rust + 48 frontend)

## PTY child handling (C8)

Current `PtyInstance` holds `writer` and `master` only. The `Child` returned by
`spawn_command` is dropped immediately after `process_id()` is read. `kill_terminal`
only removes the session from the HashMap — it does NOT kill the child process.

## File size limits (C9)

`read_file_content` calls `fs::read_to_string` with no size check.
`write_file_content` calls `fs::write` with no content length check.
`read_file_binary` calls `fs::read` with no size check.
Opening a 100MB+ file would freeze the IDE.

## Conversation save (M17)

`knowledge_save_conversation` uses `INSERT OR REPLACE` with no generation counter.
Two concurrent saves can race; the last one to complete wins regardless of which
had newer content.

## M14 (SQLite connection cache)

`KnowledgeState` holds a `db: Mutex<Option<Connection>>` but every command opens
a fresh connection. Deferring M14 to keep this group focused on the higher-impact items.
