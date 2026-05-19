# Group 02 — Resource & Robustness

> **Status:** Not started
> **Risk:** Low — bounded, additive guards
> **Effort:** Small (1 day)
> **Depends on:** Group 00 (logging + tests)

## 1. Goal

Close resource-leak and DoS-by-misclick pathways: PTY child processes
that orphan when terminals close, file reads/writes with no size cap
that can lock up the IDE, an unused SQLite connection cache that
adds confusion, and a non-monotonic conversation save path that can
let a stale write win over a fresh one.

## 2. Audit references

- **C8** — PTY child not explicitly killed on `kill_terminal`.
- **C9** — `read_file_content` / `write_file_content` have no size limit.
- **M14** — `KnowledgeState` holds a `Connection` that no command uses.
- **M17** — Conversation save has no generation counter; out-of-order
  writes can lose data.

## 3. Preservation guarantees

- Opening any text file under the size cap works exactly as before.
- Files over the cap fail gracefully with a clear, actionable error;
  the IDE does NOT freeze or OOM.
- Closing a terminal continues to clear it from the UI immediately;
  the user sees no change in latency.
- The PTY child process actually exits within a second of the
  terminal closing (today, in some cases, it does not).
- Existing conversations remain intact across the SQLite refactor.
- Save-conversation success/failure semantics from the user's POV
  don't change.

## 4. Pre-flight

1. **Capture PTY orphan baseline.** On macOS:
   - Open a terminal in the IDE.
   - In that terminal, run `sleep 999`.
   - Close the terminal (X button).
   - On the host: `ps aux | grep sleep` — observe whether the child
     process exits. Document.
   - Repeat with `vim`, `top`, `nano` — interactive programs that
     trap signals differently.
2. **Capture file-size baseline.** Try to open:
   - A 100 MB log file. Document what happens (likely UI freeze).
   - A 1 GB file. Document.
   - A 4 KB normal file. Document (works fine; this is the
     no-regression case).
3. **Capture conversation save behavior.** Send 5 quick consecutive
   chat messages, observe whether all are persisted, in order.

## 5. Implementation tasks

### 5.1 PTY child kill on session close (C8)

File: `src-tauri/src/modules/shell/mod.rs`.

**Current state:**

`PtyInstance` holds `writer` and `master`. The `Child` returned by
`spawn_command` is dropped immediately after `process_id()` is read.
This is the bug: the `Child` handle is what owns the kill capability.

**New state:**

```rust
pub struct PtyInstance {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    pid: Option<u32>,
}
```

**Behavior:**

- `kill_terminal(id)`:
  - If the session is found, take its `child` out, call
    `child.kill()`, then spawn a 500ms wait task that calls
    `child.wait()` to reap. Don't block the command on the wait.
  - Drop the master/writer to close the PTY.
- App shutdown: install a `tauri::RunEvent::Exit` handler in `lib.rs`
  that iterates `TerminalManager::sessions` and kills each child.
  This ensures clean exit even when the user quits the app without
  closing terminals.

**Logging:**

- `log::info` on every kill ("terminal {id} killed, pid={pid}").
- `log::warn` if `wait` returns an error after `kill`.

**Test plan:**

- Unit test (Rust) using a fake `Child` impl that records
  `kill`/`wait` calls. Confirm `kill_terminal` triggers `kill` and
  later `wait`.
- Integration smoke (manual): replay the `sleep 999` baseline from
  §4.1; confirm the host process is gone within 1s after closing
  the terminal.
- Regression: rapid open/close 20 terminals in 5 seconds; confirm
  no zombie processes via `ps`.

### 5.2 File size limits (C9)

Files: `src-tauri/src/modules/fs/mod.rs`, frontend
`src/lib/components/editor/Editor.svelte` and adjacent.

**Constants:**

```rust
const MAX_TEXT_FILE_BYTES: u64 = 50 * 1024 * 1024;   // 50 MB
const MAX_BINARY_FILE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB (base64-encoded)
```

These match Zed/VSCode behavior (Zed: 50 MB hard, VSCode: 50 MB
warning).

**Backend:**

- `read_file_content`: `fs::metadata(&path)?.len()`. If over cap,
  return `Err("FILE_TOO_LARGE: <bytes> bytes; limit <cap>")`. Use
  a typed error prefix so the frontend can render a specific UI.
- `read_file_binary`: same pattern with `MAX_BINARY_FILE_BYTES`.
- `write_file_content`: cap the *content* arg length at 50 MB
  before writing; if over, return `Err("CONTENT_TOO_LARGE: ...")`.
  This prevents pathological frontend bugs from writing huge files.

**Frontend:**

- `Editor.svelte::loadFile`: detect the `FILE_TOO_LARGE:` prefix in
  the error string. Show a viewer with: file size, "Open as binary
  in external app", "View first 1 MB" (does a separate
  `read_file_partial` call — out of scope here, just the UI scaffolding;
  for now, just show a diagnostic).
- The "View first 1 MB" path requires a new Tauri command
  `read_file_content_partial(path, max_bytes)`. Implement it in this
  group with the same `validate_path` flow.

**Test plan:**

- Rust unit: with a tempfile of N+1 bytes, `read_file_content`
  returns the typed error.
- Rust unit: with a tempfile under cap, contents round-trip.
- Frontend: mock `invoke('read_file_content')` to throw
  `FILE_TOO_LARGE: ...`; confirm the editor renders the size-warning
  UI and does not enter a broken state.
- Manual: open a deliberately-large log file; confirm the IDE
  remains responsive.

### 5.3 SQLite connection cache cleanup (M14)

File: `src-tauri/src/modules/knowledge/mod.rs`.

**Current state:**

`KnowledgeState { db: Mutex<Option<Connection>> }`. Set in
`knowledge_init`. Read by no command — every other command opens a
fresh connection.

**Decision:** keep the cache, USE it in every command. SQLite
connection open is fast but locks the file briefly; reusing the
handle is cleaner and gives us one place to enforce pragmas
(`PRAGMA journal_mode=WAL` for concurrent reads while writing).

**Refactor:**

- Add `set_pragmas(&conn)` helper that sets
  `journal_mode=WAL`, `synchronous=NORMAL`, `temp_store=MEMORY`,
  `busy_timeout=5000`.
- Call it once in `knowledge_init` after opening the connection.
- Every command takes `state: tauri::State<'_, Arc<KnowledgeState>>`
  and uses `state.db.lock().await.as_ref()` instead of opening a
  new connection.
- If `state.db` is `None` when a command is called (e.g. command
  fires before `knowledge_init`), the command lazily opens a
  connection using the project_root path it received, applies
  pragmas, and stashes it. This makes the commands robust to
  init-order races (related to Group 05's M2/M3 work).
- `knowledge_delete_project` and `knowledge_delete_all_projects`
  drop the cached connection BEFORE touching the file (existing
  behavior — preserved).

**Migration concern:** changing journal mode rewrites the DB on
first open. Document this; old DBs are auto-upgraded by SQLite —
no user action needed.

**Test plan:**

- Round-trip a save/list/load conversation flow using the cached
  connection.
- Concurrent reads: spawn 4 simulated concurrent `knowledge_get_context`
  calls; confirm no `database is locked` errors with WAL on.
- After `knowledge_delete_project`, the cache is dropped and the
  next command lazily reopens.

### 5.4 Conversation save generation counter (M17)

File: `src-tauri/src/modules/knowledge/mod.rs`,
`src/lib/modules/ai/ai.ts`.

**Problem:** `saveConversationNow` doesn't track in-flight writes.
Two saves dispatched 100ms apart can race; whichever the SQLite
write completes for second wins, even if it had older content.

**Fix:** monotonic generation counter on the frontend, included in
the IPC payload. Backend uses `WHERE updated_at < ?new_ts AND
(SELECT COALESCE((SELECT generation FROM conversations WHERE id =
?), 0)) < ?new_gen` — only write if our generation is newer.

**Schema migration:**

- Add `generation INTEGER NOT NULL DEFAULT 0` column to
  `conversations` table. SQLite `ALTER TABLE ADD COLUMN` is online
  and safe.
- `init_schema` runs the migration idempotently.

**Frontend:**

- Add `let saveGen = 0` to `ai.ts`. Increment on every
  `saveConversationNow` invocation. Pass it to the Tauri command.

**Backend command shape change:**

```rust
#[tauri::command]
pub async fn knowledge_save_conversation(
    project_root: String,
    id: String,
    title: String,
    messages: String,
    generation: u64,   // NEW
    // ...
) -> Result<bool, String>;  // returns true if this write won
```

The frontend logs (via Group 00 logger) when a save was rejected as
stale; the conversation is saved by a later (winning) call.

**Test plan:**

- Frontend: dispatch two `saveConversationNow` calls back-to-back;
  confirm the IPC receives both with monotonically increasing
  `generation` values.
- Backend: simulate out-of-order arrival (call save with
  generation=2 first, then generation=1); confirm the second is
  rejected and the row's content is from gen=2.
- Round-trip: existing single-save flow continues to work.

### 5.5 Logging migration

Every new error/branch in §5.1–§5.4 logs through the Group 00
logger. No `console.*` calls.

## 6. Test plan (cumulative)

In addition to per-task tests:

- **PTY orphan suite:** the manual ps-grep flow from §4.1 becomes a
  documented release smoke test in `docs/release-smoke-tests.md`.
- **File-size suite:** `tests/integration/file-size.test.ts` covers
  the typed error round-trip end-to-end.
- **Conversation save race suite:** `tests/integration/conversation-save.test.ts`
  drives the generation-counter behavior.
- **Behavioral baseline replay** (from G00): every step identical.

## 7. Code review checklist

- [ ] `PtyInstance` retains the child handle; `kill_terminal`
      actually calls `kill()`.
- [ ] App shutdown handler kills all live PTY children. Manual
      verification with `ps`.
- [ ] No file-read path bypasses the size check. `grep` for every
      caller of `fs::read_to_string` and `fs::read` in the project
      root.
- [ ] Frontend handles `FILE_TOO_LARGE:` and `CONTENT_TOO_LARGE:`
      error prefixes — no untyped error reaches the user as raw
      text.
- [ ] WAL mode pragma is set inside the same transaction as schema
      init — no race window where the DB is in default-rollback mode
      while another command tries to open it.
- [ ] Generation counter migration is idempotent: running on an
      existing DB adds the column with default 0 and doesn't drop
      data.
- [ ] Saving a conversation with a stale generation does NOT
      overwrite a fresh row, and surfaces a `log::warn`.
- [ ] No new `unwrap`. No new silent `Err -> Ok` conversions.
- [ ] Behavioral baseline replay passes.

## 8. Rollback

- §5.1: revert `PtyInstance` shape change. Existing kill-on-drop
  semantics return.
- §5.2: revert size caps. Pre-existing behavior (UI freeze on huge
  files) returns.
- §5.3: revert WAL pragma; SQLite reverts to rollback journal on
  next open. Existing DBs work either way.
- §5.4: schema column stays (harmless); revert frontend generation
  counter to send 0 always; backend rejection logic short-circuits
  when generation is 0 on both sides.

All rollbacks are non-destructive.

## 9. Out of scope

- **Streaming/chunked reads for huge files.** A "view file in
  pages" feature is a separate UX project.
- **PTY session limit policy.** `MAX_SESSIONS = 10` stays.
- **Knowledge DB compaction.** `cleanup_old_data` already runs;
  improving its policy is a future task.
- **WAL checkpoint scheduling.** SQLite's auto-checkpoint default is
  fine; tuning is out of scope.

## 10. Notes for the implementing agent

- `portable_pty::Child` does not implement `Drop`-based kill. You
  must explicitly call `kill()`.
- WAL mode requires that the directory containing the DB is
  writable (it creates `-wal` and `-shm` sidecars). The
  `~/.leo-ide/knowledge/` dir is, so this is fine.
- Conversation message JSON can be large (50+ message threads).
  The `messages: String` column is fine for now; if it grows, move
  to a separate table — out of scope here.
- The `FILE_TOO_LARGE:` prefix convention is also useful for future
  typed errors. Document it in `docs/error-codes.md` as part of
  this group.
