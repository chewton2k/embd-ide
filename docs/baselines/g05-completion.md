# Group 05 — Completion Summary

**Completed:** 2026-05-13
**Commits:** 3 (all prefixed `[G05]`)

## What was done

1. **Delete orphan projects by db_hash (C4)**
   - New Tauri command `knowledge_delete_by_hash(db_hash)`
   - Validates hash format (exactly 16 hex chars)
   - Defense-in-depth: verifies path is inside knowledge_dir()
   - Logs deletion via `log::info!`
   - Removes .db and sidecar files (-wal, -shm, -journal)
   - Frontend `deleteProjectByHash()` wrapper
   - KnowledgeListView: orphan projects use hash-based delete
   - Confirm state keyed by db_hash for uniqueness

2. **Client-side AI request size cap (M16)**
   - User message capped at 200,000 chars (~50K tokens)
   - Attached file contexts capped at 200,000 chars total
   - Over-cap shows assistant-role error message (no IPC fired)
   - Matches existing error-display convention

## What was deferred

- **M2 (Project root SSOT)**: Requires changing `set_project_root` return type and updating all frontend consumers. Mechanical but high-churn change.
- **M3 (Async mutex)**: Requires converting `ProjectRootState` from `std::sync::Mutex` to `tokio::sync::RwLock` and making all consumers async. Large mechanical change.
- **M6 (Reload consistency)**: Naming cleanup (`reloadFileContent` → `setEditorContentFromExternal`). Low priority.
- **M7 (Stale closure guard)**: Requires careful editor effect refactoring. Risk of regression.

## Test counts

- Added: 2 frontend tests (orphan delete + valid delete paths)
- Total: 57 (7 Rust + 50 frontend), all passing

## Bundle-size delta

Negligible (one new function export + size check logic in ai.ts).

## Behavioral diff

- Orphan projects in knowledge list can now be deleted (previously stuck)
- Oversized AI messages show a clear error instead of being sent to the API

## Reviewer sign-off

✅ Approved after addressing 2 items (log::info on delete, unit tests for both delete paths).

## New findings

None.

## Next group unblocked

Group 06 (Code Quality) is now unblocked — all prerequisite groups (00-05) are complete.
Group 04 (Frontend Performance) was already unblocked after G03.
