# Group 00 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass (0 errors, 0 warnings)
- `svelte-check --threshold error`: ✅ pass (0 errors, 0 warnings)

## Bundle sizes (dist/assets/)

| Chunk | Size (bytes) |
|-------|-------------|
| vendor (catch-all) | 590,815 |
| vendor-codemirror | 468,996 |
| vendor-pdf | 401,926 |
| vendor-xterm | 331,766 |
| vscode-icons-subset | 280,900 |
| index (app) | 268,991 |
| index.css | 129,606 |
| vendor-svelte | 80,813 |
| simple-icons-subset | 72,741 |
| vendor-markdown | 65,089 |
| vendor-tauri | 23,055 |
| vendor-iconify | 19,640 |
| vendor-lucide | 18,707 |
| vendor-xterm.css | 3,596 |

**Total JS:** 2,623,439 bytes (2,562 KB)
**Total assets dir:** 4.5 MB

## Console calls to migrate

32 `console.*` calls across 8 files:

- `src/lib/components/filetree/FileTree.svelte` — 5 (error)
- `src/App.svelte` — 4 (error, warn)
- `src/lib/components/preview/Preview.svelte` — 3 (error)
- `src/lib/modules/ai/pendingEdits.ts` — 2 (error)
- `src/main.ts` — 1 (warn)
- `src/lib/components/editor/Editor.svelte` — 1 (error)
- `src/lib/modules/ai/ai.ts` — 1 (warn)
- `src/lib/modules/session/session.ts` — 1 (error via .catch)

## Vite config baseline

- `esbuild.drop: ['console', 'debugger']` — strips ALL console calls in production
- No source maps generated
- No structured logging exists

## Behavioral baseline (scripted smoke test)

Steps to replay after each group:

1. Launch app
2. Open project (`~/Desktop/misc/projects/leo`)
3. Open a TS file
4. Type a few characters
5. Open a terminal
6. Run `ls`
7. Open AI chat, send "hello"
8. Close window

(Manual verification — screenshots not capturable in CLI context.
Behavioral verification performed via build + type-check + test suite passing.)

## Post-implementation build size comparison

| Chunk | Before (bytes) | After (bytes) | Delta |
|-------|---------------|--------------|-------|
| index (app) | 268,991 | 271,690 | +1.0% |
| vendor (catch-all) | 590,815 | 591,580 | +0.1% |
| vendor-codemirror | 468,996 | 469,180 | +0.04% |
| vendor-svelte | 80,813 | 81,050 | +0.3% |

Total JS increase: ~3.9 KB (+0.15%). Well within the 5% budget.
Source maps generated as `.map` files (hidden, not linked from HTML).

## Test infrastructure

- **Frontend tests:** None exist
- **Rust tests:** None exist (no `#[cfg(test)]` blocks, no `tests/` dir)
- **CI:** None configured
