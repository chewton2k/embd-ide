# Group 05 — Knowledge UX & Data Integrity

> **Status:** Not started
> **Risk:** Medium — touches the project-root state machine and
>            knowledge persistence, which is exactly where the
>            chat-save bug originated
> **Effort:** Medium (2 days)
> **Depends on:** Group 00 (logging + tests)

## 1. Goal

Finish the work started by the chat-save fix in the previous session
and harden the knowledge module end-to-end: orphan project rows
become deletable from the UI, the project-root state machine has a
single canonical source of truth, the async/sync mutex mismatch is
resolved, the editor's reactive feedback loop on file content is
cleaned up consistently, and stale-closure footguns in the file-
loading path are removed.

The chat-save bug we just fixed was a symptom; this group fixes the
underlying class of bug.

## 2. Audit references

- **C4** — `(unknown project)` rows in knowledge listing have no
  delete affordance.
- **M2** — Frontend `projectRoot` store and Rust `ProjectRootState`
  can diverge.
- **M3** — `ProjectRootState` uses `std::sync::Mutex` but is locked
  from async commands (potential thread blocking).
- **M6** — `reloadFileContent` writes content into the reactive
  store; inconsistent with the cache approach introduced last
  session.
- **M7** — Editor's `$openFiles.find()` in an effect has a stale-
  closure risk during tab switches.
- **M16** — No client-side cap on AI streaming request body size.

## 3. Preservation guarantees

- All currently-listed knowledge projects continue to appear in the
  list with the same metadata (file count, conversation count, size,
  last-updated).
- Conversation save / load / list / delete from the UI works
  identically for valid projects.
- Existing knowledge DBs continue to be readable; no destructive
  schema migrations.
- Opening a project, editing files, saving, switching tabs — all
  current flows are preserved.
- Git "discard changes" continues to reload the editor with the
  on-disk content.

## 4. Pre-flight

1. **Reproduce the orphan-project case.** Manually create a
   `~/.leo-ide/knowledge/<somehash>.db` without a `project_meta`
   entry (or with `project_root = '(unknown)'`). Confirm:
   - It appears in the knowledge list as `(unknown project)`.
   - The "Delete" button does NOT remove it (or removes the wrong
     thing). Document the exact failure.
2. **Reproduce the M2 divergence.** Open a project via a symlinked
   path (`ln -s real-project sym-project; open sym-project`).
   Inspect:
   - Frontend `projectRoot` store value.
   - Rust `ProjectRootState` value (via debug log or a debug Tauri
     command).
   - Whether they're equal.
3. **Probe M7 stale closure.** Open file A, type a few characters,
   while it's saving switch to file B. Observe whether the post-save
   `$effect` ever targets the wrong file (look for any incorrect
   editor content).

## 5. Implementation tasks

### 5.1 Delete orphan projects by db_hash (C4)

Files: `src-tauri/src/modules/knowledge/mod.rs`,
`src/lib/components/knowledge/KnowledgeListView.svelte`,
`src/lib/modules/knowledge/knowledge.ts`.

**New Tauri command:**

```rust
#[tauri::command]
pub async fn knowledge_delete_by_hash(
    db_hash: String,
    state: tauri::State<'_, Arc<KnowledgeState>>,
) -> Result<(), String>;
```

- Validates that `db_hash` matches `^[a-f0-9]{16}$`.
- Constructs the path: `knowledge_dir().join(format!("{}.db", db_hash))`.
- Verifies the file is inside `knowledge_dir()` (defense-in-depth).
- Drops the cached connection if it was for that DB.
- Removes the `.db` file and any `-wal`, `-shm`, `-journal` sidecars.
- Logs the action via Group 00 logger.

**Frontend:**

`knowledge.ts` exports:

```ts
export async function deleteProjectByHash(dbHash: string): Promise<void>;
```

`KnowledgeListView.svelte`:

- For each `ProjectInfo`, if `project.project_root === '(unknown)'`,
  the delete button calls `deleteProjectByHash(project.db_hash)`
  instead of the project-root-based delete.
- For valid projects, the existing `onDeleteProject(root)` flow is
  unchanged.
- The UI shows the orphan rows with a tooltip: "This project's
  original folder is unknown. You can still delete its data."

**Tests:**

- Rust unit: create a DB without `project_meta`, call the new
  command, assert the file is gone.
- Rust security: pass `../../foo` as `db_hash`, assert error and no
  filesystem effect.
- Vitest: orphan row click invokes `invoke('knowledge_delete_by_hash')`
  with the right hash.

### 5.2 Single source of truth for project root (M2)

Files: `src-tauri/src/modules/fs/mod.rs`,
`src/lib/components/filetree/FileTree.svelte`,
`src/lib/modules/git/git.ts`.

**Decision:** the canonical project root is owned by Rust. The
frontend store mirrors it, but does not generate it.

**Backend change:**

`set_project_root` returns the canonicalized path:

```rust
#[tauri::command]
pub fn set_project_root(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<String, String>;  // returns canonical path
```

**Frontend change:**

`FileTree.svelte::openFolderByPath` now waits for the canonical path
back from Rust before populating the store:

```ts
async function openFolderByPath(path: string, restoreSession = true) {
  if (rootPath) await saveSessionNow(rootPath);
  // Set Rust state first, get back the canonical path.
  const canonical = await invoke<string>('set_project_root', { path });
  rootPath = canonical;
  projectRoot.set(canonical);
  // ... rest unchanged ...
}
```

This eliminates the divergence: every consumer of `projectRoot`
sees the same canonical path that Rust stored.

**Compatibility note:** `recentProjects` already stores raw paths.
That's fine — when the user clicks a recent project, `openFolderByPath`
calls `set_project_root` which canonicalizes; the canonical path is
what gets persisted in `projectRoot` and used for knowledge lookup.

**Tests:**

- Rust: `set_project_root("/tmp/foo")` returns `/private/tmp/foo`
  (macOS prefix expansion).
- Frontend: mock `invoke('set_project_root')` to return a canonical
  path; assert the store ends up with that canonical path.

### 5.3 Async-safe project root state (M3)

File: `src-tauri/src/modules/fs/mod.rs`.

**Current:** `pub type ProjectRootState = Arc<Mutex<Option<PathBuf>>>;`
where `Mutex` is `std::sync::Mutex`.

**Problem:** async Tauri commands lock this mutex. On contention the
tokio worker thread blocks instead of yielding. Rare in practice
(the lock is held for microseconds) but architecturally incorrect.

**Fix:** use `parking_lot::Mutex` for synchronous fast-paths (already
fast and not async-aware, but lockless reads via fast-path) — OR use
`tokio::sync::RwLock` since most accesses are reads.

**Decision:** `tokio::sync::RwLock`.

```rust
pub type ProjectRootState = Arc<tokio::sync::RwLock<Option<PathBuf>>>;
```

All synchronous Tauri commands that take `ProjectRootState` must
become `async fn` and use `.read().await` / `.write().await`.

**Audit every consumer:**

- `set_project_root` (sync → async)
- `validate_path` (sync helper) — change to async, propagate
- `validate_repo_path` (sync helper) — same
- `validate_knowledge_root` (sync helper) — same
- All commands that call these helpers — already async in most
  cases; for the few sync ones, convert.

This is a large mechanical change. Sub-agent should:

- Convert in one focused commit, all at once.
- Run `cargo check` after to confirm.
- Run the full test suite to confirm no behavioral change.

### 5.4 Reconcile reloadFileContent with the cache (M6)

File: `src/lib/modules/explorer/files.ts`,
`src/lib/components/editor/Editor.svelte`.

**Current:** the cache from the previous session's fix bypasses the
reactive `openFiles.content` field on every keystroke. But
`reloadFileContent` (called from `GitPanel` after a `git checkout
--`) writes content into the reactive store AND the cache, which
fires the editor's `$effect` and dispatches a CodeMirror replace.

This is intentional behavior (we want the editor to update on
reload), but the design is inconsistent: write-via-cache for
keystrokes, write-via-store for reloads.

**Cleanup:** introduce a single helper:

```ts
export function setEditorContentFromExternal(path: string, content: string): void {
  fileContentCache.set(path, content);
  // Bump version so the editor's $effect picks it up.
  openFiles.update(files => files.map(
    f => f.path === path
      ? { ...f, content, modified: false, version: f.version + 1 }
      : f
  ));
}
```

Document clearly:

- `updateFileContent` = user keystroke; never write to reactive
  store except to flip `modified`.
- `setEditorContentFromExternal` = git/external reload; writes
  full content + bumps version.
- `reloadFileContent` is renamed to `setEditorContentFromExternal`
  for clarity. Update the one caller in `GitPanel.svelte`.

**Tests:**

- Round-trip: edit content, then reload via the helper, confirm
  editor reflects the reloaded content and `modified` is false.

### 5.5 Stale-closure guard in editor effect (M7)

File: `src/lib/components/editor/Editor.svelte`.

**Current:**

```ts
$effect(() => {
  const file = $openFiles.find(f => f.path === filePath);
  if (!file || file.version === 0) return;
  if (view && currentFilePath === filePath) {
    // ... dispatch
  }
});
```

If the user switches tabs between effect-trigger and effect-run,
`filePath` (closure-captured) and `currentFilePath` (mutable) can
disagree.

**Fix:** check `currentFilePath` BEFORE accessing `file.content`,
and re-read `file` based on `currentFilePath`:

```ts
$effect(() => {
  const _ = $openFiles; // subscribe
  const path = currentFilePath;
  if (!view || !path) return;
  const file = get(openFiles).find(f => f.path === path);
  if (!file || file.version === 0) return;
  // ... dispatch — the file is guaranteed to match currentFilePath
});
```

Subscribing to `$openFiles` to re-trigger but reading the freshest
value via `get()` avoids the staleness.

**Tests:**

- Synthetic: rapidly switch between two open files while issuing
  `setEditorContentFromExternal` calls; confirm the editor never
  shows file A's content with file B selected.

### 5.6 Client-side AI request size cap (M16)

File: `src/lib/modules/ai/ai.ts`.

**Current:** `sendStreamingMessage` truncates conversation history
when total chars > 400_000 but doesn't cap individual file contexts
or the user message itself.

**Fix:** add a per-message size check:

```ts
const MAX_USER_MESSAGE_CHARS = 200_000; // ~50K tokens
const MAX_CONTEXT_CHARS = 200_000;

if (userContent.length > MAX_USER_MESSAGE_CHARS) {
  chatMessages.update(msgs => [
    ...msgs,
    { role: 'assistant', content: `Message too large (${userContent.length} chars; limit ${MAX_USER_MESSAGE_CHARS}). Trim the message and try again.` },
  ]);
  return;
}

if (effectiveContexts) {
  const total = effectiveContexts.reduce((s, c) => s + c.content.length, 0);
  if (total > MAX_CONTEXT_CHARS) {
    chatMessages.update(msgs => [
      ...msgs,
      { role: 'assistant', content: `Attached files exceed size limit (${total} chars; limit ${MAX_CONTEXT_CHARS}). Remove some attachments and try again.` },
    ]);
    return;
  }
}
```

Surface as a chat message rather than throwing; that's the existing
error-display convention.

**Tests:**

- Vitest: stub `sendStreamingMessage` inputs with oversized content;
  assert no IPC call is made and an assistant-role error message
  is appended.

## 6. Test plan (cumulative)

Per-task tests above plus:

- **Integration: orphan flow.** Set up a knowledge dir with a mix
  of valid and orphan DBs; load the knowledge view; delete an
  orphan via the UI; reload; confirm only that DB is gone.
- **Integration: project-root canonicalization.** End-to-end via a
  symlinked project: open via the symlink, confirm the chat saves
  to the right DB, the file watcher works, and conversation
  loading round-trips.
- **Race smoke.** Tight loop of `openFolderByPath` calls (different
  projects in rapid succession). No crashes, no orphan watchers,
  knowledge state always consistent with the most recent project.
- **Behavioral baseline replay** passes.

## 7. Code review checklist

- [ ] `knowledge_delete_by_hash` validates the hash format, refuses
      anything outside `knowledge_dir()`, logs the action.
- [ ] Orphan UI path uses `db_hash` for delete; valid path uses
      `project_root`. Unit test covers both branches.
- [ ] `set_project_root` returns the canonical path. Frontend
      stores that value. No code path writes to `projectRoot`
      with a non-canonical value.
- [ ] `ProjectRootState` is `tokio::sync::RwLock`. Every consumer
      uses async lock methods. No `std::sync::Mutex` left in this
      surface.
- [ ] `setEditorContentFromExternal` is the one place that writes
      `content` into the reactive `openFiles` store (other than
      `addFile`). Verified by `grep`.
- [ ] Editor `$effect` reads `currentFilePath` first, never targets
      a file other than the active one. Stale-closure test passes.
- [ ] AI client-side size cap rejects with a clear assistant
      message; no IPC fired for over-cap inputs.
- [ ] No `console.*`. Logging via Group 00 logger for non-user-
      facing diagnostic output.
- [ ] All existing knowledge tests from Group 00 still pass.
- [ ] Behavioral baseline replay passes.

## 8. Rollback

- §5.1 (delete-by-hash) is purely additive; revert the new command
  + UI branch. Orphan rows return to undeletable but everything
  else is unaffected.
- §5.2 (canonical return): if frontend code paths break, the
  workaround is for the frontend to canonicalize on its own (it
  doesn't have an API to do this perfectly, but for the M2 issue
  the symptom is rare).
- §5.3 (RwLock): biggest risk surface. Revert is a wholesale
  revert of the type and all `await` calls. If issues are seen
  *after* deployment, document and revert; existing data is
  unaffected.
- §5.4, §5.5, §5.6: trivial revert.

No data migration, no destructive schema change.

## 9. Out of scope

- **Per-conversation per-message versioning.** Generation counter
  is in Group 02. This group doesn't touch that surface again.
- **Knowledge graph view improvements.** UX-level work; covered in
  the broader UX polish (Group 07 if it makes the cut, otherwise
  separate).
- **Per-DB encryption at rest.** Conversations contain user
  questions and code; encrypting them is a future feature. For
  now, file permissions + OS-level FDE are the protection.
- **Knowledge schema versioning system.** When we eventually break
  schema compat, we'll need real migrations. Out of scope here;
  current ALTER TABLE additions are handled via try-and-ignore.

## 10. Notes for the implementing agent

- The RwLock conversion (§5.3) is the most invasive change.
  Sequence: change the type → run `cargo check` → fix every
  compile error one by one, all in a single commit (so the project
  is never broken at HEAD).
- For §5.2, double-check the `recentProjects` flow: when restoring
  a session, the path passed in is the previously-stored raw path,
  not canonical. Make sure `openFolderByPath` handles that
  consistently. The flow should be: take whatever the user gives
  → call `set_project_root` → use the returned canonical
  everywhere downstream.
- Don't conflate this group with the chat-save fix from the
  previous session. That fix is already in. This group builds on
  it; do not undo or rewrite the canonical/raw fallback in
  `validate_knowledge_root`.
- The `knowledge_delete_by_hash` command should NOT canonicalize
  or check filesystem-existence beyond what's needed for safety.
  Treat the hash as opaque; regex-validate, then act.
