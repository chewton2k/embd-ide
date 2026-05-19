# Group 00 — Foundation: Observability and Test Infrastructure

> **Status:** Not started
> **Risk:** Low — additive, no behavior changes
> **Effort:** Medium (1–2 days)
> **Blocks:** All other groups

## 1. Goal

Make production builds debuggable and make every subsequent change in
this plan testable. Without these foundations, no other fix is
verifiable in the build users actually run.

The chat-save bug that prompted this entire plan was invisible in
production for exactly this reason: every `console.error` is stripped by
`vite.config.ts`, and there are no automated tests, so a regression
silently shipped.

## 2. Audit references

- **C1** — All `console.*` calls stripped in production builds.
- **M8** — No source maps generated in production.
- (Test infrastructure — implicit prerequisite for every other group.)

## 3. Preservation guarantees

This group is **purely additive**. The following must continue to
work identically:

- Every existing Tauri command's signature, behavior, and return type.
- The frontend → backend IPC contract.
- All existing UI flows (open file, edit, save, terminal, AI chat,
  knowledge browser, settings).
- Bundle layout — no new chunks except `vendor-logging` if it makes
  sense; chunk hashes may change but no chunk should grow > 5% from
  baseline.
- Startup time on cold launch (measured below) must not regress
  beyond +50ms p95 on the reference machine.

## 4. Pre-flight

Before any code change:

1. **Capture baseline metrics.** On the reference dev machine, run:

   ```bash
   npm run tauri:build
   open releases/leo-0.1.6.dmg   # install
   # Cold start, measure to first interactive (terminal opens)
   ```

   Record: bundle size per chunk, cold start time, RSS after 60s idle.

2. **Capture behavioral baseline.** Walk through this scripted flow
   and screenshot/record each step:

   1. Launch app
   2. Open a known project (`~/Desktop/misc/projects/leo` itself)
   3. Open a TS file
   4. Type a few characters
   5. Open a terminal
   6. Run `ls`
   7. Open AI chat, send "hello"
   8. Close window

   This becomes the manual smoke test for every group.

3. **Verify clean build.** `cargo check` and `npx svelte-check
   --threshold error` must both pass before this group starts.

## 5. Implementation tasks

### 5.1 Frontend structured logger

Create `src/lib/modules/logging/logger.ts`:

- Public API: `log.info(msg, data?)`, `log.warn(msg, data?)`,
  `log.error(msg, err?, data?)`, `log.debug(msg, data?)`.
- In dev mode (`import.meta.env.DEV`): also call the original
  `console.*` for browser DevTools visibility.
- In all builds: invoke a Tauri command `log_record` with a structured
  payload `{ level, msg, ts, data, err: { message, stack } }`.
- Truncate any single field over 8 KB to avoid runaway log lines.
- Strip known secret keys from `data` (anything matching
  `/api[_-]?key/i`, `/token/i`, `/password/i`, `/secret/i`) by replacing
  the value with `'[redacted]'`.
- Sample debug logs at 1% in production to avoid log flood.
- Add `log.scope('module-name')` returning a sub-logger that prefixes
  every message — used per-module so log lines are filterable.

Update `src/lib/modules/index.ts` to export from `./logging`.

### 5.2 Rust log sink

Create `src-tauri/src/modules/log/mod.rs`:

- Tauri command `log_record(level, msg, ts, data, err)` that appends
  one JSON object per line to `~/.leo-ide/logs/leo.jsonl`.
- Rotate when the active file exceeds 5 MB (rename to `leo-{ts}.jsonl`,
  start a new active file). Keep at most 10 historical files; delete
  older ones in a background sweep at startup.
- Use a `Mutex<BufWriter<File>>` held across calls; flush on a 1s
  interval or on every error-level log.
- On panic from a writer error, fall back to dropping the log
  silently — losing logs is better than crashing the app.
- Never log secret-shaped values; trust the frontend redaction but
  also redact server-side as defense-in-depth.

Wire it into `src-tauri/src/lib.rs::run`:

- Register the command in `tauri::generate_handler!`.
- Initialize the log state in `setup()`.

### 5.3 Adjust `vite.config.ts`

Change:

```ts
esbuild: { drop: ['console', 'debugger'] }
```

to:

```ts
esbuild: { drop: ['debugger'] },
```

…and replace every existing `console.error`/`console.warn` call across
the frontend with `log.error` / `log.warn` (sub-task 5.4). The
remaining `console.*` calls become noise in DevTools during dev — fine
because in production we now route through the logger.

Add source maps:

```ts
build: {
  sourcemap: 'hidden',
  // ...rest unchanged
}
```

`'hidden'` writes the `.map` files but does not link them from the
bundle, so production HTML doesn't reference them. Stack traces
captured by the logger include line/col that can be resolved manually
later. We don't ship the maps; they stay on the build machine.

### 5.4 Migrate existing console calls

Audit every `console.error` / `console.warn` / `console.log` in `src/`:

```bash
grep -rE "console\.(log|warn|error)" src/ --include="*.ts" --include="*.svelte"
```

Files known to contain calls (from the audit):

- `src/App.svelte` (4 calls)
- `src/main.ts` (1)
- `src/lib/components/filetree/FileTree.svelte` (5+)
- `src/lib/components/editor/Editor.svelte` (1)
- `src/lib/components/preview/Preview.svelte` (3)
- `src/lib/modules/ai/pendingEdits.ts` (2)
- `src/lib/modules/ai/ai.ts` (1)
- `src/lib/modules/session/session.ts` (1, via `.catch(console.error)`)

For each, replace with `log.error` / `log.warn`, scoping the logger by
module:

```ts
import { log } from '$lib/modules/logging';
const slog = log.scope('FileTree');
// ...
slog.error('Undo move failed', e);
```

Do not delete legitimate dev-side `console.log` debug aids if any
exist; they're harmless in dev and stripped by the existing config
even after this change (we kept `drop: ['debugger']` only — wait, we
also stripped console. Re-check: with our change, `console.log` will
NOT be stripped in production. Confirm we want that. Decision:
**migrate all `console.*` to the logger** so production builds have a
single observability path).

### 5.5 Frontend test framework — Vitest

Add Vitest with the Svelte plugin:

```json
"devDependencies": {
  "vitest": "^2.x",
  "@testing-library/svelte": "^5.x",
  "jsdom": "^25.x"
}
```

Pin minor versions per the operating principle on dependencies.

Create `vitest.config.ts`:

- `test.environment = 'jsdom'`.
- `test.setupFiles = ['./tests/setup.ts']`.
- `test.coverage.provider = 'v8'`.
- `resolve.alias['$lib'] = './src/lib'` to mirror the existing import
  pattern.

Create `tests/setup.ts`:

- Mock `@tauri-apps/api/core::invoke` with a settable handler. Tests
  register handlers per command name; unregistered calls throw so
  tests don't silently pass on mock-out errors.
- Mock `@tauri-apps/api/event::listen` with an event-bus harness.
- Reset all mocks between tests.

Create `tests/mocks/tauri.ts` exposing:

```ts
export function mockInvoke(cmd: string, handler: (args: unknown) => unknown): void;
export function expectInvoked(cmd: string, args?: unknown): void;
export function resetInvokeMocks(): void;
```

### 5.6 Frontend baseline tests

Cover the existing critical helpers — these are the contracts we must
not break in any subsequent group:

`tests/unit/explorer/files.test.ts`:

- `addFile` adds a new tab with empty content / pinned=false.
- `addFile` activates an already-open file rather than duplicating.
- `addFile` evicts oldest unpinned/unmodified when over `maxTabs`.
- `closeFile` keeps pinned files.
- `closeFile` removes the content-cache entry (regression guard for
  the cache introduced in the previous session).
- `updateFileContent` flips `modified` only on the first edit.
- `getFileContent` returns the cached value after `updateFileContent`.
- `reloadFileContent` updates both store content and cache and bumps
  version.
- `closeAllUnpinned` purges cache for closed files only.
- `renameOpenFile` migrates the cache key.

`tests/unit/ai/editParser.test.ts`:

- Round-trip a known assistant message through `parseAiEdits` and
  assert each edit's path/startLine/endLine.
- Empty input returns `{ edits: [], displayText: '' }` (or whatever
  the existing contract is — read it first, codify it).

`tests/unit/ai/chatRenderer.test.ts`:

- `parseUserContent` handles plain prose, file mentions, and
  attachments.
- `parseAssistantContent` handles tool-call/tool-result blocks.

`tests/unit/knowledge/knowledge.test.ts`:

- `shortProjectName('/Users/x/Projects/leo')` returns
  `'Projects/leo'`.
- `shortProjectName('(unknown)')` returns `'(unknown project)'`.
- `formatBytes(0)` returns `'0 B'`.
- `formatRelativeTime(0)` returns `'never'`.
- `formatRelativeTime(now - 30)` returns `'just now'`.

`tests/unit/git/mergeUtils.test.ts`:

- `parseConflicts` on a known three-way diff returns the expected
  hunks.
- `hasConflictMarkers` detects standard `<<<<<<<` / `=======` /
  `>>>>>>>` patterns.

These are the "behavior contract" suite. Subsequent groups MUST keep
this suite passing.

### 5.7 Rust test framework

Cargo's built-in test runner is already available. Set up:

- `src-tauri/tests/` directory for integration tests.
- `src-tauri/src/modules/*/mod.rs` get inline `#[cfg(test)] mod tests`
  blocks for unit tests of pure functions.

Add a test fixture helper:

`src-tauri/tests/common/mod.rs`:

```rust
pub fn make_temp_project() -> (tempfile::TempDir, std::path::PathBuf) {
    // Creates a real on-disk temp project root, returns the TempDir
    // (for cleanup) and its canonicalized path.
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `Cargo.toml`.

### 5.8 Rust baseline tests

`src-tauri/src/modules/fs/mod.rs` — tests for:

- `validate_path` rejects absolute paths outside the project root.
- `validate_path` rejects paths with `..` components.
- `validate_path` accepts a path that doesn't exist yet but whose
  parent does (for create scenarios).
- `next_copy_name` returns `'foo copy'`, `'foo copy 2'`, etc.

`src-tauri/src/modules/git/mod.rs`:

- `validate_git_file_path` rejects empty, NUL-containing, absolute,
  `..`-traversal, and `.git/`-prefixed paths.
- `parse_unified_diff` on a known diff produces the expected
  `DiffLine` sequence.

`src-tauri/src/modules/knowledge/mod.rs`:

- `db_hash_of('/Users/x/p')` is deterministic, 16 hex chars.
- `validate_knowledge_root` accepts the active root and rejects an
  unrelated path.
- `validate_knowledge_root` accepts a path whose canonicalize fails
  but matches the active root by string (regression guard for the
  bug fixed in the previous session).

`src-tauri/src/modules/ai/mod.rs`:

- `default_model` returns the right model per provider.
- `extract_stream_delta` parses Anthropic vs OpenAI shapes.
- `is_stream_done` recognizes `message_stop` for Anthropic and
  `finish_reason: "stop"` for OpenAI.

### 5.9 `scripts/test.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
echo "→ Running Rust tests..."
(cd src-tauri && cargo test --quiet)
echo "→ Running frontend tests..."
npx vitest run
echo "→ Type-checking..."
npx svelte-check --threshold error
echo "→ Rust check..."
(cd src-tauri && cargo check --quiet)
echo "✓ All checks passed."
```

`chmod +x scripts/test.sh` and add a `package.json` script
`"test": "bash scripts/test.sh"`.

### 5.10 Document the testing setup

Create `docs/testing.md`:

- Project test layout.
- How to mock `invoke` in frontend tests.
- How to use the temp-project fixture in Rust tests.
- Conventions: one assertion per test name, AAA pattern, no shared
  mutable state across tests.
- How to run a single test (`vitest run --t name` /
  `cargo test name`).

## 6. Test plan

The work in this group **is** the test plan for every other group.
The validation here is meta:

1. **Logger smoke test.** In dev:

   - Call `log.info('hello')`, confirm both DevTools console and
     `~/.leo-ide/logs/leo.jsonl` show the entry.
   - Call `log.error('oops', new Error('bad'))`, confirm the JSON
     object includes `err.message` and `err.stack`.
   - Call `log.info('key', { apiKey: 'abc123' })`, confirm the file
     entry has `apiKey: '[redacted]'`.

2. **Logger production test.** Build with `npm run tauri:build`,
   install, run the same calls (via a temporary debug button),
   confirm the file is written.

3. **Log rotation test.** Lower the rotation threshold to 100 bytes
   temporarily, generate 50 log lines, confirm rotation creates new
   files and old files are pruned.

4. **Test runner smoke test.** Run `npm test`. Every baseline test
   passes. Then deliberately break one (e.g. flip an assertion),
   confirm CI-mode exit code is non-zero.

5. **Bundle size guard.** After this group: confirm total bundle
   size is within +5% of baseline. If not, investigate.

6. **Behavioral baseline replay.** Walk the smoke flow from §4.2.
   Every step works identically. No new console errors in DevTools.

## 7. Code review checklist

The review sub-agent must verify:

- [ ] No production code path can call into the logger and crash the
      app. Every logger call is wrapped to swallow its own errors.
- [ ] Secret-redaction tested with at least: `apiKey`, `api_key`,
      `API_KEY`, `token`, `Token`, `password`, `secret`. Negative test:
      `apikeyValid` (a non-secret name containing `apikey`) is NOT
      redacted (acceptable false positive — call out if so).
- [ ] No `console.*` call remains in production code paths
      (`grep -rE "console\.(log|warn|error)" src/` returns only test
      files or the dev branch of the logger itself).
- [ ] Source maps are generated but NOT linked from production HTML.
- [ ] `vite.config.ts` change does not enlarge the main bundle. Compare
      build output sizes before/after.
- [ ] Rust log writer holds its mutex for the minimum duration; no
      other code path can deadlock on it.
- [ ] Log file path uses the same `~/.leo-ide/` convention as keys
      and knowledge DBs — consistent on-disk layout.
- [ ] Test infra mocks `invoke` correctly: tests for unmocked
      commands fail loudly.
- [ ] Every baseline test maps to an audit-finding-or-existing-feature.
      No tests for hypothetical behavior.
- [ ] The behavioral-baseline screenshots are stored somewhere
      retrievable (commit them, or attach to the PR).

## 8. Rollback

This group is purely additive. Rollback steps:

1. Revert the commits.
2. Delete `~/.leo-ide/logs/` if you want to fully undo (optional —
   leaving the dir is harmless).
3. The `package.json` and `Cargo.toml` dependency additions revert
   automatically with the commit revert.

No data migration. No user-visible state changes.

## 9. Out of scope

- **Telemetry / phone-home.** This group writes logs locally only.
  Any external transmission of logs is a separate decision with
  product/privacy implications.
- **Crash reporting integration** (e.g. Sentry). Out of scope; the
  jsonl log is enough for manual debugging.
- **Live log viewer in the IDE.** Useful, but a polish task — file
  it for later under Group 7 if desired.
- **Migrating existing `catch { /* ignore */ }` blocks.** Group 06
  owns that cleanup. Group 00 only introduces the logger.
- **Per-environment log levels.** A constant level (`info` in prod,
  `debug` in dev) is fine for now. Configurable levels can come later.

## 10. Notes for the implementing agent

- The chat-save retry-and-warn change in `src/lib/modules/ai/ai.ts`
  uses `console.warn`. Migrate it as part of 5.4 — confirm the
  retry behavior still works after the migration.
- The `App.svelte` knowledge-init `console.warn` introduced last
  session likewise needs migration.
- When testing the logger redaction, be paranoid. Production logs
  must never contain secrets. If unsure whether a field is sensitive,
  treat it as sensitive.
