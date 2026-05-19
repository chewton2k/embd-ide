# Group 03 — Performance: Backend

> **Status:** Not started
> **Risk:** Low — internal optimizations behind stable interfaces
> **Effort:** Medium (2 days)
> **Depends on:** Group 00 (logging + tests)

## 1. Goal

Eliminate the avoidable backend latency in the AI streaming path,
the indexer's full re-walk on every invocation, and the SSE buffer
allocation pattern that quadratically scales with chunk count.

The user-visible payoff: AI chat responses start ~100–300 ms faster
on each turn, large projects index incrementally instead of from
scratch, and high-throughput streams don't stutter.

## 2. Audit references

- **C7** — `reqwest::Client::new()` per request defeats connection
  pooling.
- **H8** — SSE buffer drain pattern allocates a new String per line.
- **H2** — `find_dependents` reads every project file every call.
- **M13** — `knowledge_index` re-hashes unchanged files on every run
  (file is read even when the hash check would short-circuit the DB
  write).

## 3. Preservation guarantees

- AI chat semantics: the message returned to the frontend is byte-
  for-byte identical to the previous behavior given identical model
  output. Streaming chunk boundaries may differ (irrelevant to the
  UI), but the concatenated final text matches.
- Cancellation works: `ai_chat_cancel` interrupts an in-flight
  stream within the same latency budget as before.
- Indexing produces the same DB rows for the same project state.
  Order-of-insertion may change; final row content is identical.
- The `find_dependents` Tauri command returns the same set of
  dependents for the same project (cache may be cold the first
  time; after this group, it's served from the SQL cache).
- HTTP error handling unchanged — the same `API error <status>:
  <body>` strings reach the frontend.

## 4. Pre-flight

1. **AI chat latency baseline.** Capture time-to-first-byte for a
   `ping`-style chat message ("respond with the word OK") on each
   provider. Record p50/p95 over 10 runs.
2. **Indexing baseline.** On a 5,000-file project (use `~/.cargo`
   if no real project that large is available, or generate one with
   `mkdir -p` + sequenced fixture files in tests):
   - Time `knowledge_init` end-to-end.
   - Time the second `knowledge_init` immediately after — currently
     this is also a full walk.
3. **Streaming buffer baseline.** Send a chat message that elicits
   a long response (>2000 tokens). Measure CPU during streaming
   via `top` / Activity Monitor.
4. **Find-dependents baseline.** Open a file diagram on a known
   1000+ file project; time the call.

## 5. Implementation tasks

### 5.1 `reqwest::Client` singleton (C7)

File: `src-tauri/src/modules/ai/mod.rs`.

**Add a static client** with sensible production defaults:

```rust
use std::sync::OnceLock;
use std::time::Duration;

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .pool_max_idle_per_host(4)
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120)) // total request timeout
            .build()
            .expect("reqwest client build")
    })
}
```

Replace every `let client = reqwest::Client::new();` in `ai/mod.rs`
with `let client = http_client();`. Confirm the borrow lifetime
works — `&'static reqwest::Client` is fine for spawn'd tasks.

**Note on the streaming timeout:** `reqwest::ClientBuilder::timeout`
applies to the entire request including the body stream. For
streaming completions we explicitly want a long-running response;
either:
- (a) set `timeout` only on non-streaming `call_blocking`, OR
- (b) set the client-level timeout high (120s) and rely on the
  cancellation path to cut early.

Choose (a): use one client for non-streaming and another (or no
timeout) for streaming. Implementation:

```rust
fn http_client_streaming() -> &'static reqwest::Client { /* no .timeout(...) */ }
```

Both clients share the same pool; this is a config-only difference.

**Tests:**

- Unit: build the client successfully under a `#[test]` (smoke).
- Integration: with a local mock HTTP server (use `wiremock` —
  evaluate vs. just a tiny `tokio::net` listener; `wiremock` is
  already widely used in the Rust ecosystem; if not pulled in, hand-
  roll a 50-line mock server in tests, no new dep).
- Latency: measure two consecutive non-streaming calls; second call
  is faster (connection reuse). Document the speedup.

### 5.2 SSE buffer rewrite (H8)

File: `src-tauri/src/modules/ai/mod.rs::stream_response`.

**Current:**

```rust
let mut buffer = String::new();
// ...
buffer.push_str(&String::from_utf8_lossy(&chunk));
while let Some(line_end) = buffer.find('\n') {
    let line = buffer[..line_end].trim_end_matches('\r').to_string();
    buffer = buffer[line_end + 1..].to_string();   // <-- realloc every line
    // ...
}
```

**New:**

```rust
let mut buffer: Vec<u8> = Vec::with_capacity(8192);
// ...
buffer.extend_from_slice(&chunk);
loop {
    let Some(line_end) = buffer.iter().position(|&b| b == b'\n') else { break };
    let line_bytes = &buffer[..line_end];
    let line = std::str::from_utf8(
        line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes)
    ).map_err(|_| "Invalid UTF-8 in SSE stream".to_string())?;
    // ... parse `line` ...
    buffer.drain(..line_end + 1);
}
```

`Vec::drain` is O(remaining length) but doesn't reallocate. For
typical SSE chunks (few hundred bytes), the win is small per line
but compounds over thousands of lines.

**Edge cases:**

- Buffer exceeds a reasonable size with no newline (malformed
  stream): cap at 1 MB; if exceeded, return an error and abort the
  stream. Log via `log::error`.
- Trailing partial line at stream end: handled by the existing
  loop-and-break pattern; verify nothing is lost on the final
  chunk.

**Tests:**

- Unit on the line-extraction loop with a fixture stream of known
  lines.
- Property test: split a known SSE response into chunks of varied
  sizes (1 byte, 10 bytes, 1 KB), feed through the parser; assert
  the same final delta sequence regardless of chunk boundaries.

### 5.3 Persisted dependent graph (H2)

File: `src-tauri/src/modules/graph/mod.rs`,
`src-tauri/src/modules/knowledge/mod.rs`.

**Current:** `find_dependents` walks the project, reads every JS/TS/
Svelte/Rust/Python file, scans for imports of the target. O(N × M).

**New approach:** during `knowledge_index`, record imports for each
file in a new SQLite table. `find_dependents` becomes a single SQL
query.

**Schema:**

```sql
CREATE TABLE IF NOT EXISTS file_imports (
    importer_path TEXT NOT NULL,
    imported_path TEXT NOT NULL,    -- normalized relative path or module name
    PRIMARY KEY (importer_path, imported_path)
);
CREATE INDEX IF NOT EXISTS idx_file_imports_target ON file_imports(imported_path);
```

**Indexer change:**

- During each file's index step, in addition to current
  `summary`/`exports` extraction, parse imports using the existing
  `extract_module_path` helper from `graph/mod.rs`.
- Insert into `file_imports` (with `INSERT OR REPLACE`).
- When a file is deleted (detected by file mtime check from §5.4),
  delete its `importer_path` rows.

**`find_dependents` rewrite:**

Becomes a SQL query:

```sql
SELECT importer_path FROM file_imports
WHERE imported_path = ?1
   OR imported_path LIKE ?2  -- for name-based matching
LIMIT 100;
```

Keep the existing `find_dependents` function signature so callers
don't break. Add a new `find_dependents_cached` that takes the DB
connection and uses the table; have the public function fall back to
the live walk if the DB hasn't been populated yet (cold start).

**Tests:**

- Insert known files into the table; run query; verify result.
- End-to-end: index a small fixture project, call `find_dependents`,
  verify the cached path returns the same set as the live walk.

### 5.4 Incremental indexing (M13)

File: `src-tauri/src/modules/knowledge/mod.rs::knowledge_index`.

**Current:** for every walked file, read entire file → hash →
short-circuit if hash matches the DB row. The early-out happens
*after* the file read.

**New:** check `mtime` first. If the DB has an entry with the same
`mtime`, skip the file entirely (don't even open it).

**Schema migration:**

Add a `mtime INTEGER` column to the `files` table. ALTER TABLE
online (same pattern as Group 02 §5.4).

**Indexer change:**

```rust
let metadata = std::fs::metadata(file)?;
let mtime = metadata.modified()
    .ok()
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);

let row: Option<(String, i64)> = conn
    .query_row("SELECT hash, mtime FROM files WHERE path = ?1",
               params![rel], |r| Ok((r.get(0)?, r.get(1)?)))
    .ok();

if let Some((_, db_mtime)) = &row {
    if *db_mtime == mtime { continue; }
}

// Only now read the file.
let Ok(content) = std::fs::read_to_string(file) else { continue };
let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
if row.as_ref().map(|(h, _)| h.as_str()) == Some(&hash) {
    // mtime changed but content didn't (e.g. touch); just bump mtime.
    conn.execute("UPDATE files SET mtime = ?1 WHERE path = ?2",
                 params![mtime, rel]).ok();
    continue;
}

// Full re-index path.
```

**Deleted file handling:**

After the walk, `DELETE FROM files WHERE last_indexed < ?startTs
AND path NOT IN (?walked_set)`. Implementation note: collect the
walked rel-paths into a set; bulk-delete missing rows.

This is the proper cleanup. The existing `cleanup_old_data` "delete
files not re-indexed in 7 days" rule stays as a backstop but
shouldn't be the primary mechanism.

**Tests:**

- Index a fixture project. Re-index without modifying anything;
  confirm zero file reads (instrument with a counter).
- Modify one file's mtime via `filetime` crate (existing or add as
  dev-dep); re-index; confirm only that file is read.
- Delete a file; re-index; confirm row is removed from DB.

### 5.5 Logging migration

Every new branch logs through the Group 00 logger:

- `log::info` on indexer start/end with file counts.
- `log::warn` on per-file read errors (currently silent).
- `log::error` on HTTP client errors above a 5xx threshold.

## 6. Test plan (cumulative)

- All per-task tests above.
- **Integration:** end-to-end AI chat round-trip with the singleton
  client; existing tests from Group 00 still pass.
- **Performance regression guard:** add a benchmark in
  `tests/bench/` (manual, not in CI) capturing the indexer time on
  a fixed fixture; document expected range so future regressions
  surface.
- **Behavioral baseline replay:** every step identical.

## 7. Code review checklist

- [ ] Exactly one `OnceLock<reqwest::Client>` for non-streaming and
      one for streaming. No new `Client::new()` calls remain in the
      AI module.
- [ ] Streaming path has no overall request timeout; non-streaming
      path has a 120s timeout.
- [ ] SSE buffer is a `Vec<u8>`, drained in place; no `.to_string()`
      in the hot loop.
- [ ] Malformed SSE (no newline, growing buffer) is bounded and
      errors cleanly.
- [ ] Schema migrations are idempotent (`ALTER TABLE ADD COLUMN IF
      NOT EXISTS`-like pattern via try-and-ignore).
- [ ] `find_dependents` cached path returns identical results to
      the live walk on a fixture project (test asserts set equality).
- [ ] mtime-based skip: instrumented test counts file reads = 0 on
      a no-op re-index.
- [ ] Deleted files are removed from the DB on the next index run.
- [ ] Index progress events still fire at the same cadence (every
      20 files, plus final). UI polling frequency is unchanged.
- [ ] No new `unwrap`. No new silent `Err -> Ok`.
- [ ] Behavioral baseline replay passes.

## 8. Rollback

- §5.1 reverts trivially. Performance returns to baseline; nothing
  breaks.
- §5.2 reverts trivially.
- §5.3: the new table can stay in the DB (harmless). Revert the
  `find_dependents_cached` path; the function falls through to the
  live walk.
- §5.4: revert the mtime-skip logic. The schema column remains
  (harmless).

All rollbacks preserve user data.

## 9. Out of scope

- **HTTP/2 pushed events / SSE alternatives.** Current SSE is
  fine; switching transports is a bigger project.
- **Cross-provider streaming abstraction.** The branching on
  provider in `extract_stream_delta` and `is_stream_done` is
  ugly but stable. Refactor in Group 06.
- **Smarter import parsing.** The current line-by-line regex-y
  parser misses many valid imports. Replace with proper AST
  parsing as a separate task; this group only changes the *cache*.
- **Background re-indexing on file save.** The watcher invokes
  full re-index; making it incremental per-file is future work.

## 10. Notes for the implementing agent

- The streaming Client must NOT have a `timeout`. Be paranoid: a
  long Anthropic response can run minutes; capping it would cut
  conversations off.
- `reqwest::Client` is `Clone` and cheap to clone; storing the
  `&'static` reference and cloning on demand also works if the
  borrow lifetime is awkward in `tokio::spawn`.
- The `file_imports` schema is provider-agnostic; do not encode
  language-specific semantics into it. The extractor is the only
  place that knows about JS vs Rust.
- Cancellation behavior: ensure `cancel_rx` is still polled in the
  rewritten loop. The `tokio::select!` block stays.
