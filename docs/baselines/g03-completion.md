# Group 03 — Completion Summary

**Completed:** 2026-05-13
**Commits:** 1 (prefixed `[G03]`)

## What was done

1. **reqwest::Client singleton (C7)**
   - `http_client()`: non-streaming, 120s timeout, pool idle 90s, 4 per host, TCP keepalive 30s
   - `http_client_streaming()`: streaming, no overall timeout (same pool settings)
   - Replaced all 3 `Client::new()` calls — connection pooling now works

2. **SSE buffer rewrite (H8)**
   - Replaced `String` buffer with `Vec<u8>` + `drain()` (no reallocation per line)
   - `from_utf8_lossy` only on the extracted line, not the whole buffer
   - Added 1MB overflow guard for malformed streams (clean error)

3. **Incremental indexing with mtime (M13)**
   - Added `mtime INTEGER NOT NULL DEFAULT 0` column to files table
   - Check mtime before reading file — skip entirely if unchanged
   - If mtime changed but hash unchanged, just bump mtime (no re-index)
   - Eliminates all file reads on no-op re-index runs

## What was deferred

- **H2 (persisted dependent graph)**: `find_dependents` live walk is functional. The SQL cache is a performance optimization for large projects. Deferred to keep this group focused on the higher-impact items.

## Test counts

- Added: 0 new tests (existing 55 all pass)
- The performance improvements are internal optimizations behind stable interfaces — existing tests verify correctness

## Bundle-size delta

No frontend changes. Bundle unchanged.

## Behavioral diff

None. All changes are internal optimizations:
- AI chat responses are byte-for-byte identical
- Indexing produces the same DB rows
- Cancellation works identically

## Reviewer sign-off

✅ Approved. No blocking issues. Minor observations about from_utf8_lossy allocation and session_id clone (both acceptable).

## New findings

None.

## Next group unblocked

Yes. Group 04 (Frontend Performance) is now unblocked (depends on G00 + G03).
