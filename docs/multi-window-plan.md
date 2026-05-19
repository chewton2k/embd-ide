# Multi-Window + Native macOS Menu — Implementation Plan

> **Audience:** A coding agent (Claude/Kiro/Cursor) that will read THIS FILE ONLY for context. Everything you need to implement, test, and ship this work is below. Treat this document as the source of truth; don't improvise from intuition.
>
> **Status:** Not started.
> **Risk:** HIGH for the backend state-isolation refactor (Stage A). MED for window lifecycle (Stage C). LOW for menu (Stage B) and frontend integration (Stage D).
> **Effort:** ~2–3 days for an experienced agent doing thorough work.
> **Owner:** Whoever picks it up. Ping the user before merging Stage A — the state-isolation diff has the broadest blast radius.

---

## 0. Operating principles (READ FIRST, NON-NEGOTIABLE)

These are baked into the project's culture from the AI-diff fix series (`docs/implementation-plan/00-foundation.md` for canonical phrasing). Any deviation must be called out explicitly in your reviewer summary.

1. **Behavior preservation is the prime directive.** Single-window behavior must continue to work *identically* in the default-launch case. A user who never opens a second window must not notice this work happened.
2. **Build green at every commit.** `npx vitest run` must pass. `npx svelte-check --threshold error` must pass. `cd src-tauri && cargo check` must pass. `cd src-tauri && cargo test` must pass.
3. **Tests gate every change.** Add unit tests + integration tests for every new state-isolation invariant. Manual smoke tests don't replace automated coverage.
4. **No new dependencies without justification.** Reuse what's in `package.json` / `Cargo.toml`. If you need a new Tauri plugin, justify it in the reviewer summary.
5. **No silent error swallowing.** Every `catch { /* ignore */ }` and Rust `.ok()` chain that drops an error needs a comment explaining why silence is correct.
6. **Use real icons.** UI surfaces use `lucide-svelte` or `@iconify/svelte`. No emoji, no ad-hoc SVG.
7. **Reviewer rounds are mandatory.** End each stage with a `subagent` adversarial review. Address every blocking issue before moving on. The AI-diff series did 1–3 rounds per group; expect similar.
8. **Rollback is a feature.** Each stage is one logical commit. `git revert <stage>` must restore pre-stage behavior cleanly.
9. **Commit only what the user explicitly approves.** Don't `git add` or `git commit` without permission. Edit files freely; let the user decide what to stage.
10. **STOP CONDITIONS** (from `CLAUDE.md` / earlier groups):
    - A preservation guarantee fails → revert the step, escalate as a new finding, do not retry the same approach.
    - The same fix attempt fails twice → step back, diagnose root cause, try a fundamentally different approach.
    - A spec is ambiguous → ask. Do not improvise.
    - The full test suite from prior groups is not green → stop and fix before starting the next stage.

---

## 1. Goal

Transform the application from a single-window app into a Zed-style multi-window IDE where each window has an independent project, plus add a standard native macOS menu bar.

User's stated goals:
- **Multi-window**: each window can have its own project open. Zed-style: project A in window 1, project B in window 2, no state bleeding.
- **Menu**: standard macOS menu (File / Edit / Selection / View / Go / Window / Help) with the conventional items.

---

## 2. Why this is non-trivial: current architecture

Read the following files BEFORE writing any code. Each one tells you why the change is what it is.

### 2.1 Backend (Rust) state is currently global per app, not per window

Open `src-tauri/src/lib.rs`. Every `.manage(...)` call registers a state object that's shared across every window the app spawns:

```rust
.manage(terminal_state)                    // shell::TerminalState
.manage(project_root_state)                // ProjectRootState = Arc<RwLock<Option<PathBuf>>>
.manage(Arc::new(ai::AiState::new()))
.manage(Arc::new(knowledge::KnowledgeState::new()))
.manage(app_log::LogState::new())
.manage(session::AppStateHandle(...))
```

This is the **central problem**. Every fs/git/shell/graph/knowledge/symbols command pulls the project root from a global `Arc<RwLock<Option<PathBuf>>>`. If window 1 sets the project to `/repo-a` and window 2 tries to set it to `/repo-b`, they fight. Window 1's file ops would silently target `/repo-b` after window 2's `set_project_root` lands.

Look at `src-tauri/src/modules/fs/mod.rs`'s `validate_path()`:

```rust
pub fn validate_path(
    path: &str,
    state: &tauri::State<'_, ProjectRootState>,
) -> Result<PathBuf, String> {
    let root = state.blocking_read();
    let root = root
        .as_ref()
        .ok_or_else(|| "No project is open".to_string())?;
    // ... canonicalization + containment check ...
}
```

Every fs command, every git command, every shell command calls `validate_path` (directly or transitively). They all assume one global root. To support multiple windows, every one of those commands needs to be window-aware.

### 2.2 What state needs to be per-window vs. shared

| State | Currently | After this work | Reason |
|-------|-----------|-----------------|--------|
| `ProjectRootState` (current project) | global | **per-window** | Each window owns one project |
| `TerminalState` (terminal pool) | global | **per-window** | Window 1's terminals shouldn't appear in window 2 |
| `KnowledgeState` (vector index) | global | **per-window** *(see §2.4)* | Knowledge is project-scoped; per-window keeps each project's index separate |
| `AiState` (streaming sessions) | global, keyed by session_id | **stays global, partitioned by session_id** | Frontend already passes unique session ids; no race |
| `app_log::LogState` | global | **stays global** | Logs are diagnostic, app-wide is fine |
| `AppStateHandle` (recent projects) | global | **stays global** | Recent projects is a single user-level list (standard IDE convention) |
| Session save/load (per-project files) | per-project | **per-project** *(unchanged)* | Each project's session is keyed by project path, not by window |

### 2.3 Frontend state is already per-window-instance

Each Tauri window is a separate WebKit (or WebView2) process with its own JS context. That means **every Svelte writable** in the frontend is automatically per-window:

- `projectRoot` (`src/lib/modules/git/git.ts`)
- `openFiles`, `activeFilePath` (`src/lib/modules/explorer/files.ts`)
- `pendingEdits`, `aiChangeHistory` (`src/lib/modules/ai/...`)
- `chatMessages`, `agentRunning`, etc.
- `gitState`, `sharedGitStatus`, `sharedGitRemoteStatus`
- `toasts` (`src/lib/modules/ui/toast.ts`)
- All editor-local maps (`stateCache`, `savedContentCache`, `lastHandledVersion` in `Editor.svelte`)

You DO NOT need to change any of these. They're already isolated by JS context.

### 2.4 Knowledge state: the trickiest piece

`src-tauri/src/modules/knowledge/mod.rs` (~1000 lines, read it). It manages a per-project SQLite index for code chunks + embeddings. Currently `KnowledgeState` holds a `HashMap<ProjectPath, Connection>` — already keyed by project path, not by window. **Good news:** if you make project_root per-window, knowledge automatically follows because it keys off the project path the frontend passes in command arguments.

But: the current `knowledge_init(project_path: String)` flow assumes one open project at a time. Re-read it. Confirm that:
- Two windows opening *different* projects results in two entries in the `HashMap<ProjectPath, Connection>` — both alive concurrently.
- Two windows opening the *same* project share one Connection (or each opens a separate one without corrupting the SQLite file).

If concurrency on the same SQLite DB is a problem, scope the connection per-window (HashMap<(WindowLabel, ProjectPath), Connection>) and let SQLite's WAL mode handle cross-window reads.

### 2.5 Tauri 2.x APIs you'll use

- **Spawn a window:** `tauri::WebviewWindowBuilder::new(app, label, WebviewUrl::App(...))`. The label is a string identifier (e.g. `"main"`, `"win-2"`). It's used in event addressing and window lookups.
- **Identify the calling window in a command:** the handler can take `window: tauri::WebviewWindow` as a parameter:
  ```rust
  #[tauri::command]
  pub fn read_file_content(
      window: tauri::WebviewWindow,
      path: String,
  ) -> Result<String, String> {
      let label = window.label();
      // ... look up state by label ...
  }
  ```
- **Window lifecycle events:** `app.on_window_event(|window, event| match event { WindowEvent::Destroyed => ..., ... })`. Already in `lib.rs`; you'll extend it.
- **Native menu:** `tauri::menu::{Menu, MenuItem, Submenu, PredefinedMenuItem, MenuItemKind}`. Build the menu in `setup()`, attach it to the app for global mode (macOS) or per-window for Windows/Linux.
- **Emit events to a specific window:** `app.emit_to(EventTarget::WebviewWindow { label }, "event-name", payload)`. Use this for menu items that the frontend should react to (e.g., "Toggle File Tree").
- **Frontend listens with `import { listen } from '@tauri-apps/api/event'`** — listeners are scoped to the current window by default unless you use a webview-label-scoped target.

### 2.6 What an AI agent has historically gotten wrong on this codebase

From the AI-diff fix series (`docs/implementation-plan/*` and the conversation history baked into commit messages), the following classes of bugs have happened repeatedly. Avoid them:

1. **Sticky guard flags.** A boolean that's set "to indicate state X" but never reset, then later code checks "if X" but the flag is stale. The keystroke-revert regression was exactly this. **Use per-key Maps for any "I've handled this" tracking.** Don't reuse a single boolean across multiple identities.
2. **Order of operations in async flows.** Recording history before a write succeeds → phantom entries on failure. Always: do the side effect, *then* commit the metadata that says "we did the side effect."
3. **Tests being gitignored.** `tests/` was in `.gitignore` for the entire AI-diff series. **Verify before committing**: `git check-ignore tests/unit/some/new.test.ts` should exit 1 (not ignored). If it exits 0 (ignored), update `.gitignore` BEFORE writing more tests.
4. **Excessive comments.** Each public function: short JSDoc. Each non-obvious branch: short "why" comment. Don't paraphrase what the code obviously does.
5. **Test failure interpretation.** When a test fails, the **code is wrong, not the test**. Update the code unless the test was asserting an artifact (in which case, prove the test was asserting an artifact and rewrite the test to assert the real invariant).
6. **Rule: if an approach fails twice, STOP and try a fundamentally different approach.** Don't keep tweaking — diagnose first.

---

## 3. Preservation guarantees

Before you change anything, confirm these continue to work after each stage. Any failure stops the stage and reverts. These are the single-window behaviors you cannot regress:

| # | Behavior | Verification |
|---|----------|--------------|
| P1 | Launch app, open a folder via the file tree → editor shows files | Manual smoke test |
| P2 | Edit + save a file → disk state matches editor | Existing `tests/unit/explorer/files.test.ts` + manual |
| P3 | Open a terminal → `ls` shows files in the project | Manual smoke test |
| P4 | AI chat: send a message, get a response | Manual; existing AI tests pass |
| P5 | AI chat: accept an edit, the file updates AND the user can still type freely | Existing `tests/unit/ai/pendingEdits.test.ts` + new manual scenario from this plan |
| P6 | File watcher: external `echo` to an open file → editor reloads | Manual |
| P7 | Recent projects list survives app restart | Manual |
| P8 | Cmd+Z undoes user typing (note: NOT AI-applied edits — that's intentional, see versionGate work) | Manual |
| P9 | Knowledge indexing for one project completes successfully | Manual |
| P10 | All 483+ tests in `tests/unit/` continue to pass | `npx vitest run` |

---

## 4. Pre-flight (before touching code)

1. **Read every file listed in §6.** Don't skim. Note where state is read and written.
2. **Capture single-window baseline metrics:**
   ```bash
   # Cold-start time, RSS after 30s idle, terminal-spawn latency
   ```
   Record numbers. Multi-window must not regress single-window beyond +10% on these.
3. **Run the full test suite. It must be green.**
   ```bash
   npx vitest run
   npx svelte-check --threshold error
   cd src-tauri && cargo check && cargo test
   ```
4. **Verify your test files won't be gitignored.** Run `git check-ignore tests/unit/some/path.test.ts` for any test you plan to add. If gitignored, fix `.gitignore` first.
5. **Read `docs/implementation-plan/00-foundation.md`** for the testing infrastructure conventions used elsewhere.
6. **Read the AI-diff fix series notes** in commit messages on `src/lib/modules/ai/pendingEdits.ts` for the pattern of incremental-fix + reviewer-round + summary that this codebase expects.

---

## 5. Files you MUST read before implementing

Sorted by criticality. Read these in order. Each entry says *why* and *what to extract* from the file.

### Backend (Rust)

| File | Why | What to extract |
|------|-----|-----------------|
| `src-tauri/src/lib.rs` | Entry point. Every state object is registered here. | The set of `.manage(...)` calls, command list, `setup()` body, `on_window_event` handler. |
| `src-tauri/src/modules/fs/mod.rs` | `ProjectRootState` lives here; `validate_path()` is called by ~30 commands. | `ProjectRootState` type, `set_project_root`, `validate_path` signature, every `tauri::State<'_, ProjectRootState>` parameter. |
| `src-tauri/src/modules/shell/mod.rs` | Terminal pool — second-most-shared state. | `TerminalState`, `spawn_terminal`, `kill_terminal`, kill_all behavior on window destroy. |
| `src-tauri/src/modules/git/mod.rs` | Every git command takes the project root. | List of git commands and their state usage. |
| `src-tauri/src/modules/knowledge/mod.rs` | The HashMap<ProjectPath, Connection> pattern. | `KnowledgeState` shape, `knowledge_init`, project-keyed lookup. Confirm thread safety. |
| `src-tauri/src/modules/ai/mod.rs` | Streaming sessions; cancellation tokens. | `AiState` shape, session_id partitioning. |
| `src-tauri/src/modules/session/mod.rs` | Recent projects + per-project session save/load. | `AppState`, `RecentProject`, `SessionData`, `save_session`, `load_state_from_disk`. |
| `src-tauri/src/modules/symbols/mod.rs` | Tree-sitter symbol extraction. | `validate_path` usage, any state. |
| `src-tauri/src/modules/graph/mod.rs` | File graph analyzer. | `validate_path` usage, any state. |
| `src-tauri/tauri.conf.json` | Window config. | Currently single-window declaration. |
| `src-tauri/Cargo.toml` | Tauri version pin (`2.10.0`). | Confirm 2.10+ APIs you plan to use are available. |
| `src-tauri/capabilities/default.json` | Allowed Tauri commands per window. | Confirm new commands are allowed; new windows may need separate capability files. |

### Frontend (Svelte/TS)

| File | Why | What to extract |
|------|-----|-----------------|
| `src/main.ts` | App entrypoint. | Where the root component is mounted. |
| `src/App.svelte` | Root component. Hosts the editor, file tree, AI panel, terminal, etc. | Tab management, project-open flow, where `set_project_root` is called. |
| `src/lib/modules/git/git.ts` | `projectRoot` writable. | The store and its setters. |
| `src/lib/modules/explorer/files.ts` | `openFiles`, `activeFilePath`, `fileContentCache`. | Already per-window; confirm. |
| `src/lib/components/filetree/FileTree.svelte` | `openFolderByPath` invokes `set_project_root`. | The flow that initializes the project. |
| `src/lib/modules/session/session.ts` | Save-session helper. | How the frontend pushes session data on close. |
| `src/lib/modules/session/persisted.ts` | localStorage helpers. | Per-window persisted state. |
| `src/lib/components/Toast.svelte` | Toast UI. | How toasts render — this is where menu-driven errors will surface. |
| `src/lib/modules/ui/toast.ts` | `showToast`. | API for surfacing menu/window failures. |

### Tests

| File | Why |
|------|-----|
| `tests/setup.ts` | The vitest setup; mocks for `@tauri-apps/api/core` and `@tauri-apps/plugin-dialog`. Pattern to follow for new mocks. |
| `tests/mocks/tauri.ts` | `mockInvoke`, `getInvokeCalls`, `expectInvoked` — the established mocking idioms. |
| `tests/unit/ai/pendingEdits.test.ts` | The most-tested module. Read its setup pattern, especially `beforeEach` resets, store cleanup, mock-invoke expectations. Mirror this for backend-call mocking in your new tests. |
| `tests/unit/editor/versionGate.test.ts` | Recent example of the "extract pure helper, test it heavily, wire into a Svelte component" pattern. Use this as a template. |

### Docs

| File | Why |
|------|-----|
| `CLAUDE.md` | Project-level prompt. Stop conditions and rules. |
| `docs/implementation-plan/README.md` | Operating principles. |
| `docs/implementation-plan/00-foundation.md` | Tone and structure of an implementation plan in this codebase. |

---

## 6. Implementation stages

Each stage is one commit. Each ends with a reviewer round. Don't interleave — finish one stage's tests + review before starting the next.

```
Stage A — Backend per-window state isolation              [HIGH RISK]
   ↓
Stage B — Native macOS menu                              [LOW-MED RISK]
   ↓
Stage C — Window spawn + lifecycle commands              [MED RISK]
   ↓
Stage D — Frontend integration (menu wiring, keybindings) [LOW RISK]
```

**Why this order?** Stage A is foundational — every later stage assumes state is per-window. If you do menu/spawn first, you'll hit invariant violations the moment a second window opens. Stage A first means the rest is purely additive.

---

## 7. Stage A — Backend per-window state isolation

**Risk:** HIGH. Every fs/git/shell/graph/knowledge/symbols command's signature changes. The blast radius is the entire backend.

**Goal:** Replace global `Arc<RwLock<Option<PathBuf>>>` and `TerminalState` with per-window-keyed state. Every command identifies its caller via `tauri::WebviewWindow` and operates on that window's state slice.

### 7.1 New state types

In `src-tauri/src/modules/fs/mod.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-window project root. Each Tauri window has its own entry,
/// keyed by `WebviewWindow::label()`. The outer RwLock guards the
/// map (rare writes — only on window create/destroy or set_project_root);
/// inner Option holds the per-window root.
pub type ProjectRootState = Arc<RwLock<HashMap<String, Option<PathBuf>>>>;

pub fn create_project_root_state() -> ProjectRootState {
    Arc::new(RwLock::new(HashMap::new()))
}
```

Similar treatment for `TerminalState` in `src-tauri/src/modules/shell/mod.rs`:

```rust
pub type TerminalState = Arc<Mutex<HashMap<String, TerminalManager>>>;
//                                       ^^^^^^ window label
```

### 7.2 New `validate_path` signature

```rust
pub fn validate_path(
    path: &str,
    window_label: &str,
    state: &tauri::State<'_, ProjectRootState>,
) -> Result<PathBuf, String> {
    let map = state.blocking_read();
    let root = map.get(window_label)
        .and_then(|opt| opt.as_ref())
        .ok_or_else(|| "No project is open in this window".to_string())?;
    // ... rest unchanged ...
}
```

Every command that called `validate_path(&path, &state)` now needs the window:

```rust
#[tauri::command]
pub fn read_file_content(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<String, String> {
    validate_path(&path, window.label(), &state)?;
    // ... rest unchanged ...
}
```

### 7.3 Migration scope (the boring but critical part)

You MUST update every command that currently takes `state: tauri::State<'_, ProjectRootState>`. Use `git grep -n 'ProjectRootState\|validate_path' src-tauri/src/` to find them. As of writing, this includes (non-exhaustive — verify with grep):

- `fs::set_project_root`, `read_file_content`, `write_file_content`, `read_file_binary`, `read_dir_tree`, `create_file`, `create_folder`, `delete_entries`, `rename_entry`, `move_entries`, `import_external_files`, `paste_entries`, `duplicate_entry`, `reveal_in_file_manager`, `list_all_files`
- `git::*` — every command (~25)
- `shell::run_command_capture` (terminal-spawn commands need the window for the per-window terminal map separately)
- `graph::analyze_file_graph`
- `knowledge::knowledge_init`, `knowledge_get_context`, etc. (where they call `validate_path`)
- `symbols::symbols_extract`, `symbols::symbols_get_body`

For each: add `window: tauri::WebviewWindow` parameter and pass `window.label()` to `validate_path`.

### 7.4 Window lifecycle: create + destroy hooks

In `src-tauri/src/lib.rs`'s setup, register the main window's label in the state map. On window destroy, remove its entries.

```rust
.setup(|app| {
    // ... existing setup ...
    {
        let state: tauri::State<ProjectRootState> = app.state();
        let mut map = state.blocking_write();
        map.insert("main".to_string(), None);
    }
    Ok(())
})
.on_window_event(|window, event| {
    if let tauri::WindowEvent::Destroyed = event {
        let label = window.label().to_string();
        // Drop project root entry
        if let Some(state) = window.try_state::<ProjectRootState>() {
            if let Ok(mut map) = state.blocking_write() {
                map.remove(&label);
            }
        }
        // Drop terminal state for this window
        if let Some(state) = window.try_state::<TerminalState>() {
            if let Ok(mut map) = state.lock() {
                if let Some(mut manager) = map.remove(&label) {
                    manager.kill_all();
                }
            }
        }
        // Drop knowledge connections for this window? See §2.4.
    }
})
```

### 7.5 Tests for Stage A

Create `src-tauri/tests/multi_window_state.rs` (Rust integration test). The test framework is already wired via `tempfile` + tokio in `Cargo.toml`'s dev-deps.

**REQUIRED test cases:**

| # | Test | What it asserts |
|---|------|-----------------|
| A1 | `per-window project root: set in window_a, read in window_b returns None` | Window isolation for project root |
| A2 | `set_project_root in window_a does not change window_b's root` | Mutation isolation |
| A3 | `validate_path scoped to caller's window` | Window A's path can't be validated against window B's root |
| A4 | `window destroy removes the entry from project_root_state` | Cleanup on close |
| A5 | `window destroy kills terminals for that window only` | Terminal isolation |
| A6 | `concurrent fs ops from two windows on different roots don't deadlock` | RwLock fairness |
| A7 | `re-using a window label after destroy works` | (Tauri-internal but document expected behavior) |

**Frontend unit tests** — add `tests/unit/multiwindow/state.test.ts`:

| # | Test | What it asserts |
|---|------|-----------------|
| AF1 | `invoke('set_project_root') in this window does not affect another window's mocked state` | Frontend believes its own window |
| AF2 | `error message from "no project open" is window-scoped` | Error text correctness |

**Run the full pre-existing suite (483+ tests).** Stage A must not break any of them. If a test breaks, the breakage is in Stage A's code, not the test (rule from §0.5).

### 7.6 Stage A reviewer checklist

Dispatch a `subagent` with this prompt template (mirroring the AI-diff series style):

```
Adversarial review of Stage A: backend per-window state isolation.

Verify:
1. Every command that previously took ProjectRootState now also takes
   WebviewWindow and passes window.label() to validate_path.
2. The state-isolation tests (A1–A7) cover the key invariants.
3. No global state read remains in any fs/git/shell/graph/knowledge/symbols
   command (grep for `state.blocking_read()` and `state.blocking_write()`
   to confirm — every read/write should go through the per-window key).
4. Window destroy cleanup is exhaustive — no leaked terminals, no leaked
   project root entries.
5. The mainline test suite (483+ tests) still passes.
6. Behavior preservation guarantees P1–P10 hold under single-window use.

Look for:
- Lock ordering: project_root_state vs terminal_state. Do any commands
  hold both? If so, document a consistent acquisition order.
- Rapid window create/destroy doesn't leak.
- Any code path that reads `app.state::<ProjectRootState>()` from outside
  a command handler (e.g. plugin init) and assumes a single window.

Output: APPROVE / APPROVE-WITH-CHANGES / REQUEST-CHANGES.
```

Address every blocking issue. Re-run reviewer once more if changes were substantial. Stage A is done when the second-pass reviewer says APPROVE.

---

## 8. Stage B — Native macOS menu

**Risk:** LOW-MED. Additive; doesn't change existing behavior.

**Goal:** Define a standard macOS menu with the items the user proposed. Items dispatch to either Tauri commands (no frontend change) or events the frontend listens for.

### 8.1 Menu structure (final, no deviation without user approval)

```
leo                      File                Edit             Selection         View                  Go                   Window           Help
─────                    ────                ────             ─────────         ────                  ──                   ──────           ────
About leo                New Window  ⇧⌘N     Undo       ⌘Z    Select All  ⌘A   Toggle File Tree      Go to File   ⌘P      Minimize  ⌘M     Documentation
Settings…       ⌘,       New File    ⌘N      Redo       ⇧⌘Z                    Toggle AI Panel       Go to Line   ⌃G      Zoom             Report Issue
Hide leo        ⌘H       Open Folder…⌘O      Cut        ⌘X                     Toggle Terminal       Go to Symbol ⇧⌘O     Bring All Front
Hide Others    ⌥⌘H       Open Recent ▶       Copy       ⌘C                     Toggle Sidebar                                              ─────
Quit leo        ⌘Q       ─────               Paste      ⌘V                     Toggle Fullscreen     Back         ⌃-      leo (current)
                         Save        ⌘S      Find       ⌘F                     Reload       ⌘R       Forward      ⌃⇧-
                         Save All    ⌥⌘S     Replace   ⌥⌘F                     Toggle DevTools⌥⌘I*
                         Close Tab   ⌘W      ─────
                         Close Window⇧⌘W     Undo last AI edit ⌃⌘Z
                         Revert File         Toggle Comment ⌘/
                                             Indent      ⌘]
                                             Outdent     ⌘[
```

\* Toggle DevTools shown only in `cfg!(debug_assertions)` builds.

### 8.2 Implementation pattern

In `src-tauri/src/modules/menu/mod.rs` (new file):

```rust
use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Emitter, Manager, Runtime,
};

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let app_submenu = SubmenuBuilder::new(app, "leo")
        .item(&PredefinedMenuItem::about(app, Some("About leo"), None)?)
        .separator()
        .item(&MenuItemBuilder::with_id("settings", "Settings…")
            .accelerator("CmdOrCtrl+,").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("Hide leo"))?)
        .item(&PredefinedMenuItem::hide_others(app, Some("Hide Others"))?)
        .item(&PredefinedMenuItem::quit(app, Some("Quit leo"))?)
        .build()?;

    let file_submenu = SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::with_id("new_window", "New Window")
            .accelerator("CmdOrCtrl+Shift+N").build(app)?)
        // ... etc.
        .build()?;

    // ... Edit, Selection, View, Go, Window, Help submenus ...

    MenuBuilder::new(app)
        .item(&app_submenu)
        .item(&file_submenu)
        .item(&edit_submenu)
        // ... etc.
        .build()
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
    match event_id {
        "settings" => {
            let _ = app.emit("menu:open-settings", ());
        }
        "new_window" => {
            // Calls the command from Stage C.
            let _ = crate::modules::window_mgr::open_new_window_impl(app, None);
        }
        "open_folder" => {
            let _ = app.emit("menu:open-folder", ());
        }
        // ... etc.
        _ => {}
    }
}
```

In `lib.rs` `setup()`:

```rust
let menu = menu::build_menu(app.handle())?;
app.set_menu(menu)?;
app.on_menu_event(|app, event| {
    menu::handle_menu_event(app, event.id().as_ref());
});
```

### 8.3 Frontend listeners

Each menu item that emits an event needs a frontend listener. Add them in `src/App.svelte`'s `onMount`:

```ts
import { listen } from '@tauri-apps/api/event';
const unlisteners: Array<() => void> = [];

onMount(async () => {
    unlisteners.push(await listen('menu:open-settings', () => { showSettings = true; }));
    unlisteners.push(await listen('menu:open-folder', () => { triggerOpenFolderDialog(); }));
    unlisteners.push(await listen('menu:toggle-file-tree', () => { fileTreeVisible = !fileTreeVisible; }));
    unlisteners.push(await listen('menu:toggle-ai-panel', () => { aiPanelVisible = !aiPanelVisible; }));
    // ... etc.
});

onDestroy(() => unlisteners.forEach(u => u()));
```

For items that map to existing keyboard shortcuts (Find, Replace, Toggle Comment, Indent/Outdent), the menu just emits the event; the existing CodeMirror keymap handler runs the same code.

### 8.4 Open Recent submenu

This is dynamic: rebuild it whenever `recent_projects` changes. Tauri 2.x menus support submenu replacement. In `setup()`:

```rust
// After menu is built, listen for AppStateHandle updates and rebuild
// the Open Recent submenu. The simplest path: emit a `recents-changed`
// event from session::save_session, listen in Rust, rebuild the menu.
```

Or simpler: rebuild the entire menu on each `save_session` call. Cost is negligible (menus are lightweight).

### 8.5 macOS-specific concerns

- The OS menu bar is a single global bar. Items always target the focused window. `app.on_menu_event` gives you the AppHandle; if you need the active window: `app.get_focused_window()`. For most items (toggle file tree, etc.), emit an event globally — the frontend listener in the focused window picks it up.
- Predefined items (Hide, Quit, Minimize, Zoom, Bring All to Front) MUST use `PredefinedMenuItem` so macOS handles them natively. Don't reimplement.
- Submenu order matters: leo (app) submenu must be FIRST. Help submenu must be LAST.

### 8.6 Tests for Stage B

This is harder to unit-test (menu state is in the OS). Add what's tractable:

| # | Test | Where |
|---|------|-------|
| B1 | `build_menu produces a Menu with all expected submenus` | Rust unit test in `menu/mod.rs` (use `cargo test --lib`) |
| B2 | `handle_menu_event for "open_folder" emits the right event name` | Rust unit test with a mock AppHandle |
| B3 | `frontend receives menu:open-folder event and triggers the dialog` | TS test mocking `listen()` |
| B4 | `Open Recent submenu reflects current recent_projects` | Manual smoke test (document the steps) |
| B5 | `All keyboard shortcuts in menu match the existing CodeMirror keymap (no double-bind conflict)` | Manual checklist + automated grep against `Editor.svelte` keymap |

Manual smoke tests (REQUIRED, document the result):
- Open every menu item in turn. Each must do something (or be visibly disabled with a reason).
- Cmd+W closes a tab; if no tab, closes the window. Don't double-fire.
- Cmd+Q quits the app cleanly (not via Tauri-default; via PredefinedMenuItem).

### 8.7 Stage B reviewer checklist

```
Adversarial review of Stage B: native macOS menu.

Verify:
1. Menu structure matches the spec in §8.1 exactly.
2. Predefined items (Hide, Quit, Minimize, Zoom, etc.) use PredefinedMenuItem,
   not custom reimplementations.
3. Every custom MenuItem has either: (a) a Tauri command target, or
   (b) an emitted event that has a corresponding frontend listener.
4. Open Recent rebuilds when recent_projects changes.
5. Toggle DevTools is gated on cfg!(debug_assertions).
6. No keyboard shortcut conflicts: check the menu's accelerators against
   src/lib/components/editor/Editor.svelte's keymap. If a CM keymap and
   menu accelerator both bind the same combo, the menu wins (fires the event)
   and the CM keymap may not run. Decide which surface owns each combo.
7. Manual: open every menu item; each does the expected thing.

Output: APPROVE / APPROVE-WITH-CHANGES / REQUEST-CHANGES.
```

---

## 9. Stage C — Window spawn + lifecycle commands

**Risk:** MED. New Tauri commands; new IPC; new event flow.

**Goal:** Add the Rust commands needed to spawn additional windows, plus the frontend keybindings/menu wiring to invoke them.

### 9.1 New module: `src-tauri/src/modules/window_mgr/mod.rs`

```rust
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

/// Atomic counter for unique window labels. Tauri labels must be
/// stable identifiers, not random — we use "win-{n}" so they're
/// debuggable. Persisting the counter across restarts isn't necessary;
/// each fresh launch starts at 2 (main is always "main").
static NEXT_WINDOW_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(2);

pub fn open_new_window_impl<R: Runtime>(
    app: &AppHandle<R>,
    initial_project: Option<String>,
) -> Result<String, String> {
    let id = NEXT_WINDOW_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let label = format!("win-{}", id);

    // Pre-register the per-window state slot so the very first command
    // from the new window finds an entry.
    {
        let project_state: tauri::State<crate::modules::fs::ProjectRootState> = app.state();
        let mut map = project_state.blocking_write();
        map.insert(label.clone(), None);
    }

    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("leo")
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    let window = builder.build()
        .map_err(|e| format!("failed to spawn window: {e}"))?;

    if let Some(project) = initial_project {
        // Wait for the frontend to be ready, then push the project path.
        // The frontend listens for `init:project` on its own window.
        let label_clone = label.clone();
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            // Small delay so the frontend has registered its listener.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = app_clone.emit_to(
                tauri::EventTarget::WebviewWindow { label: label_clone },
                "init:project",
                project,
            );
        });
    }

    Ok(label)
}

#[tauri::command]
pub fn open_new_window(app: AppHandle, initial_project: Option<String>) -> Result<String, String> {
    open_new_window_impl(&app, initial_project)
}

#[tauri::command]
pub fn open_folder_in_new_window(app: AppHandle, path: String) -> Result<String, String> {
    open_new_window_impl(&app, Some(path))
}

#[tauri::command]
pub fn close_focused_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e| format!("failed to close window: {e}"))
}
```

Register these in `lib.rs`'s `invoke_handler!`.

### 9.2 Frontend bindings

`src/lib/modules/window/window.ts` (new):

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export async function openNewWindow(initialProject?: string): Promise<string> {
    return invoke<string>('open_new_window', { initialProject });
}

export async function openFolderInNewWindow(path: string): Promise<string> {
    return invoke<string>('open_folder_in_new_window', { path });
}

export async function closeWindow(): Promise<void> {
    return invoke<void>('close_focused_window');
}
```

In `App.svelte`'s `onMount`, listen for `init:project`:

```ts
unlisteners.push(await listen<string>('init:project', (event) => {
    openFolderByPath(event.payload);  // existing function in FileTree.svelte
}));
```

If the project store is in `git/git.ts` and the file-tree owns folder-loading, expose a global signal store for "open this project" and have FileTree listen to it.

### 9.3 Cmd+Shift+N keybinding

In `src/lib/modules/shortcuts/shortcuts.ts` (existing — read it), register:

```ts
{ key: 'Mod-Shift-n', run: () => { openNewWindow(); return true; } }
```

Match the existing shortcut module's conventions.

### 9.4 Menu wiring

The Stage B menu's `new_window` event already calls `open_new_window_impl` directly in Rust. No frontend handler needed for that one. Confirm.

### 9.5 "Open in New Window" in recent projects

In `src/App.svelte`'s welcome screen, the recent projects list. Add a button or right-click context menu: "Open in New Window" calls `openFolderInNewWindow(project.path)`.

### 9.6 Tests for Stage C

| # | Test | Where |
|---|------|-------|
| C1 | `open_new_window creates a window label "win-{n}" and registers the state slot` | Rust integration test |
| C2 | `open_folder_in_new_window emits init:project to the new window after a delay` | Rust integration test (with a frontend mock listener) |
| C3 | `closing a window destroys its state cleanly (combined with Stage A's A4-A5)` | Rust integration test |
| C4 | `closing the last window: macOS keeps app alive (NSApp behavior)` | Manual on macOS |
| C5 | `closing the last window on Linux/Windows: app quits` | Manual or platform-conditional test |
| C6 | `frontend openNewWindow() returns the new window label` | TS test mocking invoke |
| C7 | `Cmd+Shift+N keybinding opens a new window` | Manual; integration test of the shortcut module if possible |
| C8 | `Open Folder dialog → Cmd+click "OK" → opens in new window` | Manual |

### 9.7 Stage C reviewer checklist

```
Adversarial review of Stage C: window spawn + lifecycle.

Verify:
1. open_new_window registers the per-window state slot BEFORE building
   the window (race: if the new window's frontend invokes a command
   before the slot is registered, the command's validate_path will say
   "no project open" — which is correct, but verify the new window's
   frontend handles this gracefully).
2. NEXT_WINDOW_ID is a monotonic counter; no label collisions on rapid
   spawn/close.
3. Window-destroy cleanup runs even on force-quit (Cmd+Q from menu).
4. init:project event is scoped to the target window only (uses
   EventTarget::WebviewWindow, not a global emit).
5. No keystroke shortcut conflicts (Cmd+Shift+N might be a CM-default
   binding; verify it overrides cleanly).
6. Stage A's tests still pass.

Output: APPROVE / APPROVE-WITH-CHANGES / REQUEST-CHANGES.
```

---

## 10. Stage D — Frontend integration polish

**Risk:** LOW. UI polish only.

### 10.1 Tasks

1. **"New Window" button in toolbar** (optional — only if visual is desired). Lucide icon: `MonitorPlus` or `SquarePlus`.
2. **Recent projects: Cmd+click → new window.** Wire the click handler to check `event.metaKey || event.ctrlKey`.
3. **Welcome screen: "Open Folder in New Window" button** as a secondary option.
4. **Window title shows project name.** Currently `title: "leo"`. Update the Tauri title via `WebviewWindow.setTitle(projectName)` whenever the project changes.
5. **Tab indicator:** when the focus is on a non-main window, a small dot or window number in the title bar would help users identify. Optional; check with the user before implementing.
6. **Toast on cross-window error:** if one window's error somehow affects another (it shouldn't — Stage A enforces isolation), show a clear toast.

### 10.2 Tests for Stage D

| # | Test | Where |
|---|------|-------|
| D1 | `Cmd+click on a recent project opens it in a new window, regular click reuses current` | TS test or manual |
| D2 | `Window title updates when project changes` | TS test or manual |

### 10.3 Reviewer

Light touch — usually one round suffices.

---

## 11. Comprehensive testing strategy

This is the section the user emphasized. Be thorough.

### 11.1 Layered testing

```
┌─────────────────────────────────────────────────────────┐
│  Manual smoke tests (REQUIRED — see §11.5)              │
├─────────────────────────────────────────────────────────┤
│  Frontend integration tests (vitest + @testing-library) │
├─────────────────────────────────────────────────────────┤
│  Frontend unit tests (vitest, pure helpers)             │
├─────────────────────────────────────────────────────────┤
│  Backend integration tests (cargo test, multiple        │
│    windows simulated via tauri::test)                   │
├─────────────────────────────────────────────────────────┤
│  Backend unit tests (cargo test, pure functions)        │
└─────────────────────────────────────────────────────────┘
```

Every layer must be exercised. Skipping the manual smoke tests has bitten this codebase before — see the Group 1 chat-save bug that shipped because no manual smoke tested the failure mode.

### 11.2 Backend integration tests

`src-tauri/tests/multi_window_isolation.rs` (new file). Use `tauri::test::mock_app` for window setup:

```rust
use tauri::{test::mock_app, Manager};

#[tokio::test]
async fn project_root_isolated_per_window() {
    let app = mock_app();
    // Manually register the per-window state map.
    let state = leo::modules::fs::create_project_root_state();
    app.manage(state.clone());

    // Simulate two windows.
    {
        let mut map = state.write().await;
        map.insert("main".to_string(), None);
        map.insert("win-2".to_string(), None);
    }

    // Set window "main" root.
    leo::modules::fs::set_project_root_for_label(&app, "main", "/tmp/repo-a").await.unwrap();

    // win-2 root unchanged.
    let map = state.read().await;
    assert_eq!(map.get("main"), Some(&Some(PathBuf::from("/tmp/repo-a"))));
    assert_eq!(map.get("win-2"), Some(&None));
}
```

(You'll need to expose a `set_project_root_for_label` helper that's testable; the regular `set_project_root` command takes `tauri::WebviewWindow` which is hard to construct in tests.)

### 11.3 Required test list (minimum for sign-off)

#### Stage A — Backend isolation
| # | Test | File |
|---|------|------|
| A1 | per-window project root: set in window_a, read in window_b returns None | `multi_window_isolation.rs` |
| A2 | set_project_root in window_a does not change window_b's root | `multi_window_isolation.rs` |
| A3 | validate_path scoped to caller's window | `multi_window_isolation.rs` |
| A4 | window destroy removes the entry from project_root_state | `multi_window_isolation.rs` |
| A5 | window destroy kills terminals for that window only | `multi_window_isolation.rs` |
| A6 | concurrent fs ops from two windows on different roots don't deadlock | `multi_window_isolation.rs` |
| A7 | re-using a window label after destroy works | `multi_window_isolation.rs` |
| A8 | path traversal rejected per-window (existing security guard still works) | `multi_window_isolation.rs` |
| A9 | when a window has no project, fs commands return "no project open" with the window label included | `multi_window_isolation.rs` |
| A10 | knowledge state for project A in window 1 is independent of project A in window 2 | `multi_window_isolation.rs` |

#### Stage B — Menu
| # | Test | File |
|---|------|------|
| B1 | build_menu produces a Menu with all expected submenus | `src-tauri/src/modules/menu/mod.rs` (`#[cfg(test)] mod tests`) |
| B2 | handle_menu_event for "open_folder" emits the right event name | same |
| B3 | frontend menu listener wires up correctly | `tests/unit/menu/listeners.test.ts` (new) |
| B4 | Open Recent submenu rebuilds on session save | manual |
| B5 | accelerator collision check against Editor.svelte keymap | `tests/unit/menu/keymap-collisions.test.ts` (new — extract menu accelerators and editor keymap, assert no overlap or document intentional ones) |

#### Stage C — Window spawn
| # | Test | File |
|---|------|------|
| C1 | open_new_window creates a unique label "win-{n}" | `src-tauri/tests/window_spawn.rs` |
| C2 | open_folder_in_new_window emits init:project after delay | `src-tauri/tests/window_spawn.rs` |
| C3 | closing window destroys its state cleanly | covered by A4/A5 |
| C4 | NEXT_WINDOW_ID monotonic across rapid spawn | `src-tauri/tests/window_spawn.rs` |
| C5 | closing the last window: macOS app stays alive | manual |
| C6 | openNewWindow frontend wrapper returns the label | `tests/unit/window/window.test.ts` |
| C7 | Cmd+Shift+N opens a new window | manual + shortcut module test |

#### Stage D — Frontend polish
| # | Test | File |
|---|------|------|
| D1 | Cmd+click on recent project opens new window | `tests/unit/window/recents.test.ts` |
| D2 | Window title updates on project change | manual or `tests/unit/window/title.test.ts` |

### 11.4 Cross-stage regression tests

Re-run the **full pre-existing suite** (483+ tests) after each stage. If any pre-existing test breaks, you've regressed something. The break is in your code, not the test.

### 11.5 Manual smoke tests (REQUIRED before merge)

Every one of these must pass. Document each result in your reviewer summary.

#### M1. Single-window baseline (regression check)
1. Launch app fresh.
2. Open `~/Desktop/misc/projects/leo`.
3. Open `src/App.svelte`. Edit a comment. Save (Cmd+S).
4. Open a terminal. Run `ls`.
5. Open AI chat. Send "hello". Verify response.
6. Accept an AI edit (test with a small synthetic edit). Verify file updates AND that you can type freely afterward.
7. Quit.

**All P1–P10 preservation guarantees must hold.**

#### M2. Two-window independence
1. Launch.
2. Open project A in window 1.
3. Cmd+Shift+N → window 2.
4. Open project B in window 2.
5. Verify in window 1: file tree still shows project A. AI chat history is project A's.
6. Verify in window 2: file tree shows project B. AI chat history is project B's (or empty if first time).
7. Edit a file in window 1. Save. The disk change is in project A only.
8. Edit a file in window 2. Save. Project B only.
9. Open a terminal in window 1: pwd is project A. Same in window 2 → project B.
10. Run a command in window 1's terminal. The output appears only in window 1.

#### M3. Window close + reopen
1. With both windows from M2 open: close window 2.
2. Window 1's state intact (still project A, files still open, terminal alive).
3. Cmd+Shift+N → fresh window 3 (label "win-3" or higher).
4. Verify window 3 is empty (welcome screen).

#### M4. Last-window-close behavior
1. Single window open with project A.
2. Close it.
3. macOS: app stays alive (still in dock with light dot under icon).
4. macOS: Cmd+N or click dock → new window opens.
5. Quit (Cmd+Q): app fully exits.

#### M5. Menu functionality
1. Open every menu item from §8.1. Each has the expected effect.
2. Open Recent submenu populates with the last few projects.
3. Each shortcut works while a window is focused.

#### M6. Heavy-load stress
1. Open 5 windows, each with a different project. (Or the same project — both should work.)
2. Type rapidly in each. No cross-window content bleeding.
3. Run AI chat in each. Sessions stay separate.
4. Close all windows in random order. App quits cleanly.

#### M7. Edge cases
1. Open two windows showing the same project. Edit a file in window 1, save. The file watcher in window 2 reloads the editor (existing behavior). No corruption.
2. Open the same file in both windows' editors. Edit in window 1 + save. Window 2 reloads. Edit in window 2 + save. Verify last-write-wins (or whatever the contract is — document if unclear).
3. Knowledge re-indexing in window 1 doesn't lock up window 2.

#### M8. AI-diff regression check (mandatory)
This is the regression that most recently bit the codebase.
1. Open project A in window 1. AI chat: ask for an edit.
2. Accept the edit. Verify the file updates.
3. Type into the file. Verify the typing is NOT reverted.
4. Open project A in window 2 simultaneously. AI chat in window 2: ask for a different edit on a different file.
5. Accept in window 2. Verify window 1's editor is unaffected.

### 11.6 What "thorough testing" means here

The user explicitly asked for thoroughness. Don't ship Stage A with only tests A1 and A2. Cover every isolation invariant. If you find a case that's hard to reach with a unit test, write a manual smoke test for it AND document the limitation.

**Definition of done for testing:**
- Every backend command that takes `WebviewWindow` has a test covering: (a) per-window scope, (b) the "no project open" error path, (c) cleanup on window destroy.
- Every menu item has either a unit test or a documented manual smoke test.
- The reviewer's adversarial questions about race conditions, lock ordering, and label collisions all have a corresponding test.
- The test suite count grows by at least +30 tests (rough guide).

---

## 12. Reviewer round process (mandatory)

After every stage, dispatch a `subagent` with the stage's reviewer prompt. The conventions, mirroring the AI-diff fix series:

1. The review is **adversarial**. The reviewer's job is to find bugs you missed. Don't take it personally.
2. **Round 1** verdict is one of:
   - APPROVE → proceed to next stage.
   - APPROVE-WITH-CHANGES → fix the listed concrete issues, run the round again.
   - REQUEST-CHANGES → fix everything blocking, run the round again.
3. **Round 2 (if needed)** uses a similar prompt template but emphasizes "verify the round-1 fixes and look for regressions introduced by them."
4. After the second-pass APPROVE, the stage is done.

If you find yourself on round 3+ for the same stage, **stop**. The architectural choice is wrong. Step back, re-read the relevant audit notes in §2, and try a fundamentally different approach.

---

## 13. Rollback

Each stage is one logical commit. Rollback procedure:

| Stage | Rollback | Risk if rolled back |
|-------|----------|---------------------|
| A | `git revert <stage-a-sha>` | All later stages stop working; but single-window behavior returns to pre-work state |
| B | `git revert <stage-b-sha>` | Menu disappears; nothing else breaks |
| C | `git revert <stage-c-sha>` | Multi-window UX gone; single window via main label still works |
| D | `git revert <stage-d-sha>` | UI polish gone; functional multi-window remains |

**Critical:** Do NOT commit Stage A and Stage C together. Stage A is reversible alone; Stage C depends on A. If A is rolled back, C must be rolled back first.

---

## 14. Specific gotchas drawn from prior work on this codebase

These are landmines you'll step on if you don't read them now.

### 14.1 The keystroke-revert regression class

`src/lib/components/editor/Editor.svelte` has a `$effect` watching `$openFiles`. It used to silently revert user typing because of a sticky `version === 0` guard. The fix introduced `lastHandledVersion: Map<string, number>` per-path tracking. **If you touch Editor.svelte's $effects, do not regress this.** The per-window aspect: `lastHandledVersion` is a Map inside the component, so each window's editor instance has its own. Confirm this stays true.

### 14.2 The pendingEdits HMR rename callback

`src/lib/modules/ai/pendingEdits.ts` registers a `fileRenameCallback` at module load with a `import.meta.hot.dispose` guard. **Each window's JS context registers its own.** The dispose guard handles HMR; it doesn't handle window destroy (each window has its own JS lifetime, GC'd on close).

### 14.3 The aiChangeHistory store

`src/lib/modules/ai/aiHistory.ts` is a Svelte writable. **Each window has its own.** "Undo last AI edit" should pop from the current window's store, not a cross-window global. Match this convention for any cross-window features.

### 14.4 Test gitignore

`tests/` is gitignored as of the AI-diff series fix. After your work, run:
```bash
for f in $(find tests -type f -name '*.test.ts'); do git check-ignore -q "$f" && echo "STILL IGNORED: $f"; done
```
Output should be empty.

### 14.5 Capabilities

`src-tauri/capabilities/default.json` declares which Tauri commands each window can invoke. Currently scoped to "main". When you spawn additional windows with different labels, you need either:
- Add the new labels to the existing capability's `windows` array.
- OR use `windows: ["*"]` if the capability should apply to all windows (read the Tauri 2 docs on capabilities to confirm wildcard syntax).

If a new window can't invoke commands, this is the cause.

### 14.6 Tauri title bar style

`tauri.conf.json` uses `"titleBarStyle": "Overlay"` and `"hiddenTitle": true`. The new window builder code must match these for visual consistency.

### 14.7 macOS dock badge / window count

When multiple windows are open, macOS auto-shows them in the Window menu. Tauri's Window submenu (PredefinedMenuItem-based) handles this. If you implement the Window submenu manually, you'll need to populate it dynamically.

### 14.8 Permission prompts

If your new commands trigger any permission-gated APIs (file dialog, etc.), confirm `capabilities/default.json` has the right `tauri:default` and plugin permissions.

### 14.9 The agent edit-revert latent bug (carried from AI-diff series)

`src/lib/modules/ai/agentLoop.ts` lines 153-157 and 313-318 record `recordAiChange(path, ..., '', newContent)` with empty `before` content. If `revertAiChange` were ever exposed in the UI, calling it would write empty string to the file (data loss). This is documented as a known issue but not fixed. **If you wire the menu "Undo last AI edit" item, route it through the agent path and it will hit this bug.** Either: (a) fix the agent path's recordAiChange to use the actual original content (requires API reshape on `tools.ts edit_file`'s `onEdit` callback signature), or (b) skip undo for agent-applied edits and only handle chat-accept edits.

---

## 15. Definition of done

Before announcing completion:

- [ ] All four stages committed in order.
- [ ] Each stage has a passing reviewer round (round 2 if round 1 had issues).
- [ ] The 483+ pre-existing tests still pass.
- [ ] The new tests (~30+) all pass.
- [ ] `npx svelte-check --threshold error` is clean.
- [ ] `cd src-tauri && cargo check && cargo test` is clean.
- [ ] Manual smoke tests M1–M8 documented as PASS.
- [ ] `.gitignore` does not silently exclude any of your new test files.
- [ ] `CLAUDE.md` (if you modified it) reflects the new conventions.
- [ ] Final summary written in the same style as `docs/baselines/g0X-completion.md` files in this directory: pre-flight, tasks completed, test counts, behavioral diff, reviewer sign-off, new findings.

---

## 16. If the user asks you to deviate

Common requests and how to handle:

| Request | How to handle |
|---------|---------------|
| "Skip Stage A, just add the menu" | Refuse; Stage B alone doesn't do what the user asked for. Multi-window is the headline feature. |
| "Use a different menu item set" | Confirm the spec change with the user, then update §8.1 in this doc and proceed. |
| "Don't do per-window terminal isolation, share the pool" | Proceed at user's discretion but document the implication: closing window 1 would kill window 2's terminals. |
| "Skip the manual smoke tests" | Refuse. Manual tests cover what unit tests miss in this codebase. |
| "Just commit it without a reviewer round" | Refuse. The codebase's culture is reviewer-driven; skipping has caused all known regressions. |

---

## 17. Quick reference: command and event names you'll add

For grep-ability later. Keep these consistent across stages.

**New Rust commands:**
- `open_new_window(initial_project: Option<String>) -> Result<String, String>`
- `open_folder_in_new_window(path: String) -> Result<String, String>`
- `close_focused_window() -> Result<(), String>`
- (Possibly) `set_window_title(title: String)` if you implement §10.1.4.

**New events emitted by the Rust menu handler (frontend listens):**
- `menu:open-settings`
- `menu:open-folder`
- `menu:open-recent` (with project path payload)
- `menu:save`
- `menu:save-all`
- `menu:close-tab`
- `menu:revert-file`
- `menu:undo-last-ai-edit`
- `menu:toggle-comment`
- `menu:indent`
- `menu:outdent`
- `menu:toggle-file-tree`
- `menu:toggle-ai-panel`
- `menu:toggle-terminal`
- `menu:toggle-sidebar`
- `menu:toggle-fullscreen`
- `menu:toggle-devtools` (debug builds only)
- `menu:reload`
- `menu:go-to-file`
- `menu:go-to-line`
- `menu:go-to-symbol`
- `menu:back`
- `menu:forward`
- `menu:documentation`
- `menu:report-issue`

**Events emitted to a specific new window:**
- `init:project` (payload: project path; emitted after `open_folder_in_new_window`)

---

## 18. Final notes for the implementing agent

- This work touches the most-shared code in the app. Take your time. Read every file in §5 before writing one line of new code.
- The test suite is your friend. If you don't know whether something works, write a test.
- The reviewer rounds are not theater. They've caught critical bugs in every prior group. Take their findings seriously.
- When in doubt about scope, **ask the user**. Don't improvise on architectural choices.
- The goal is a feature that works as well in production as the rest of the IDE. Anything less means another regression-fix series later.

Good luck. Read this whole document once before starting, then use it as a reference while working.
