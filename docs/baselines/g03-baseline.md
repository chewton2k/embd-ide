# Group 03 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass
- `svelte-check`: ✅ pass
- Full test suite: ✅ 55 tests (7 Rust + 48 frontend)

## reqwest::Client usage (C7)

`reqwest::Client::new()` called in 3 places:
- `stream_response()` — creates new client per streaming request
- `call_blocking()` — creates new client per blocking request (2 branches)

Each call defeats connection pooling and TCP keepalive.

## SSE buffer pattern (H8)

In `stream_response()`:
```rust
let mut buffer = String::new();
buffer.push_str(&String::from_utf8_lossy(&chunk));
// ...
buffer = buffer[line_end + 1..].to_string(); // realloc every line
```
Quadratic allocation: creates a new String for every line extracted.

## Indexing (M13)

`knowledge_index` reads every file, computes SHA-256, then checks if hash matches.
The file read happens BEFORE the hash check — no mtime short-circuit.
On a 5000-file project, every re-index reads all 5000 files even if none changed.

## H2 (persisted dependent graph)

Deferring H2 to keep this group focused. The `find_dependents` live walk is
functional; the SQL cache is a performance optimization for large projects.
