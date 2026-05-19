# Group 06 — Code Quality & Refactoring

> **Status:** Not started
> **Risk:** High — refactors compound merge-conflict surface
> **Effort:** Large (3+ days)
> **Depends on:** Groups 00–05 (all functional fixes must land first)

## 1. Goal

Behavior-neutral cleanup of the issues that don't change what the
app does but make it harder to maintain. Reduce silent error
swallowing, split the overweight `App.svelte`, consolidate
fragmented git stores, surface the symlink-skip behavior in the file
tree, remove dead code, and update stale documentation.

This group is intentionally last among functional groups because
refactors and feature work conflict badly when interleaved.

## 2. Audit references

- **M1** — 35+ silently-swallowed errors across the frontend.
- **M4** — `App.svelte` is 1000+ lines, owns most of the layout
  logic.
- **M5** — Eight separate Svelte stores for git status that all
  update together.
- **M15** — `read_dir_recursive` skips symlinks silently; users
  can't see legitimate symlinks.
- **L1** — `ChatPanel.svelte` is dead code (mounted nowhere).
- **L5** — `CLAUDE.md` is out of date (refers to `src/lib/stores.ts`
  which has been split).
- **L9** — `lucide-svelte` and `@iconify/svelte` are both used;
  consolidate.

## 3. Preservation guarantees

This group changes ZERO user-visible behavior. The contract is:

- Every smoke-test step from Group 00's behavioral baseline
  produces an identical UI state.
- Every keyboard shortcut still works.
- Every Tauri command keeps its signature.
- Bundle size does not increase by more than 2%.
- Test suite (Group 00 baseline + per-group tests from 01–05)
  passes 100%.

If any preservation guarantee fails, the corresponding refactor is
backed out, and the finding is escalated to a follow-up task.

## 4. Pre-flight

1. Confirm the entire test suite from Groups 00–05 is green at
   HEAD. This is the gate.
2. Capture a baseline bundle size report:
   `npm run build && du -h dist/assets/*.js | sort -h`. Compare
   after each major refactor in this group.
3. Capture the App.svelte line count: `wc -l src/App.svelte`.
   Target: < 500 lines after this group.

## 5. Implementation tasks

### 5.1 Migrate silent catches to logger (M1)

Files: every file in `grep -rE "catch\s*\{\s*\}" src/`.

**Process:**

For each catch block:

1. Read the surrounding context.
2. Decide one of three outcomes:
   - **Legitimate silence:** the catch's purpose is to let an
     optional feature fail gracefully (e.g. focus call on a
     window that doesn't exist yet, keyring read on a system
     without a keyring). Add a one-line comment explaining why
     and leave silent. NO logger call.
   - **Should warn:** unexpected but recoverable. Replace with
     `slog.warn('description', e)` using a module-scoped logger.
   - **Should error:** unexpected and we want to know. Replace
     with `slog.error('description', e)`.

3. Document the decision in the diff. The PR description
   summarizes the breakdown (e.g. "8 legitimate, 19 warn, 8
   error").

**Enumerated targets** (from the audit):

| File                                       | Count | Initial assessment |
|--------------------------------------------|-------|--------------------|
| `src/lib/components/git/GitPanel.svelte`   | 7     | Mostly warn        |
| `src/App.svelte`                           | 4     | Mix                |
| `src/lib/components/shell/Terminal.svelte` | 4     | Legit (xterm fit)  |
| `src/lib/settings/sections/ModelsSection.svelte` | 3 | Warn         |
| `src/lib/settings/sections/AboutSection.svelte`  | 3 | Legit (version) |
| `src/lib/components/preview/Preview.svelte`      | 3 | Mix             |
| `src/lib/modules/ai/ai.ts`                       | 3 | Mostly warn      |
| `src/lib/components/filetree/FileTree.svelte`    | 2 | Warn             |
| `src/lib/components/ai/FloatingChat.svelte`      | 2 | Warn             |
| `src/lib/components/ai/ChatPanel.svelte`         | 1 | (file may be deleted; see §5.5) |
| `src/lib/modules/settings/settings.ts`           | 1 | Legit (parse fall-through) |
| `src/lib/modules/settingsSync.ts`                | 1 | Legit              |
| `src/lib/modules/editor/ghostText.ts`            | 1 | Legit (completion failure) |

**Tests:**

- Vitest: for the 'should warn / should error' cases, mock
  `invoke` to throw, assert the logger was called with the right
  scope and level.

### 5.2 Split App.svelte (M4)

File: `src/App.svelte` and new module files.

**Current structure:** single 1000-line component with:

- Toolbar imports + recent project flow (~80 lines)
- Drag-resize logic (~50 lines)
- Settings window opener (~30 lines)
- Recent projects + open-folder helpers (~60 lines)
- Knowledge subscription (~30 lines)
- Session save-on-close (~40 lines)
- Mode-change effects (terminal panel/tab) (~50 lines)
- Breadcrumb logic (~70 lines)
- Keydown shortcut dispatch (~50 lines)
- Render template (~400 lines)

**Target extraction:**

1. **`src/lib/modules/ui/panelResize.ts`** — drag handler. Export
   `usePanelResize(target: 'sidebar' | 'chat' | 'git', getWidth, setWidth)`
   returning `{ start, stop }`. Tested in isolation.

2. **`src/lib/modules/ui/windows.ts`** — Settings window opener,
   knowledge graph window opener, etc. The factory functions move
   here.

3. **`src/lib/modules/ui/breadcrumb.ts`** — pure breadcrumb
   computation `breadcrumbSegmentsFor(path: string, root: string)`.
   Easy to unit test.

4. **`src/lib/modules/session/lifecycle.ts`** — save-on-close
   subscriber. Export `installSessionLifecycle()` returning a
   teardown function.

5. **`src/lib/modules/ai/lifecycle.ts`** — knowledge-init and
   conversation save-on-close. Same pattern.

6. **`src/lib/components/RecentProjects.svelte`** — the welcome-
   screen recent-project list. Move out of App.svelte.

After extraction, App.svelte should be the layout shell and
component composition only. Target: < 500 lines.

**Approach:**

- Extract one piece at a time.
- After each extraction, run `npm test` and the smoke-flow.
- Commit after each piece passes.
- Do NOT extract anything that would change a behavior. If the
  extraction would force a behavior change, leave it inline and
  document why.

**Tests:**

- Each new module gets unit tests for its pure logic.
- The smoke-flow remains identical.

### 5.3 Consolidate git status stores (M5)

File: `src/lib/modules/git/git.ts`,
`src/lib/components/filetree/FileTree.svelte`,
`src/lib/components/git/GitPanel.svelte`.

**Current eight stores:**

- `sharedGitStatus` (Record<path, code>) — flat absolute paths
- `gitFileStatus` (Map<path, code>) — derived flat for files
- `gitFolderStatus` (Map<path, code>) — derived for dirs
- `sharedGitRemoteStatus` — same shape, remote
- `gitRemoteFileStatus`, `gitRemoteFolderStatus`
- `gitIgnoredPaths` (Set), `gitIgnoredPrefixes` (string[])

**Refactor target:**

```ts
export interface GitState {
  status: { files: Map<string, GitCode>; folders: Map<string, GitCode> };
  remote: { files: Map<string, GitCode>; folders: Map<string, GitCode> };
  ignored: { paths: Set<string>; prefixes: string[] };
  branch: string | null;
}

export const gitState = writable<GitState>(initialGitState());
```

One store, one update per poll. Components subscribe and access
the slice they care about. Reactive churn drops to one notification
per poll cycle.

**Compatibility shim:** keep the existing exported stores as
`derived` selectors over `gitState` so consumers don't need to be
rewritten in the same commit. Migrate consumers in a follow-up.

```ts
export const sharedGitStatus = derived(gitState, $g => /* flatten */);
// etc.
```

Once consumers are migrated, delete the shims.

**Tests:**

- Vitest: drive `gitState.set(...)`; assert each derived store
  emits the expected slice.

### 5.4 Symlinks in file tree (M15)

File: `src-tauri/src/modules/fs/mod.rs`, `FileTree.svelte`.

**Current:** symlinks silently skipped in `read_dir_recursive`.

**Fix:** include symlinks in the listing, with a flag:

```rust
#[derive(Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,    // NEW
    pub children: Option<Vec<FileEntry>>,
}
```

For symlinked directories, do NOT recurse into them by default
(prevent cycles). The frontend renders them with a small overlay
icon (lucide-svelte's `Link2` or similar) and on click reveals the
target. Following symlinks could be a future setting.

**Cycle prevention:** track inode set during recursive walk; if
inode seen, skip.

**Tests:**

- Rust unit: temp dir with `a/`, `b -> a`, recursive read returns
  both with `b.is_symlink = true` and no children loaded for `b`.
- Vitest: render a tree with a symlink entry; UI shows the Link2
  badge.

### 5.5 Remove ChatPanel.svelte (L1)

File: `src/lib/components/ai/ChatPanel.svelte` — DELETE.

The audit confirmed it's mounted nowhere. The cache fallback fix
applied to it during the previous session is moot.

**Verification before delete:**

```bash
grep -r "ChatPanel" src/ --include="*.svelte" --include="*.ts"
```

If the only matches are within the file itself, delete is safe.
Document the result in the PR.

**Tests:**

- Run the full suite. Bundle size decreases. Behavioral baseline
  unchanged.

### 5.6 Update CLAUDE.md (L5)

File: `/Users/.../leo/CLAUDE.md`.

Sync to the actual current architecture:

- `src/lib/stores.ts` no longer exists; state is in
  `src/lib/modules/<domain>/`.
- `Editor.svelte` is at `src/lib/components/editor/Editor.svelte`.
- `Terminal.svelte` is at `src/lib/components/shell/Terminal.svelte`.
- Update the "Adding a new Tauri command" section to reflect that
  modules are now in `src-tauri/src/modules/<name>/mod.rs`.
- Add a section pointing at this implementation plan directory.

**Tests:** none (documentation).

### 5.7 Consolidate icon libraries (L9)

Files: any using both `lucide-svelte` and `@iconify/svelte`.

**Decision matrix:**

- `lucide-svelte` is the UI-action icon library (buttons, tab
  controls, chat icons). Per-icon imports; tree-shakeable.
- `@iconify/svelte` is the file-type icon library (file tree
  icons), powered by the vscode-icons collection.

**Refactor:**

- Audit every `import` from each. If a UI-action icon is being
  rendered via Iconify, switch to lucide. If a file-type icon is
  being rendered via lucide, switch to Iconify. The libraries do
  different jobs; this is just consistency.

**Bundle impact:**

- Lucide tree-shakes well; we already pay for what we import.
- Iconify pays a runtime fee but the offline subset is preloaded.
- No expected bundle change either direction.

**Tests:**

- Visual diff: every icon-rendering surface looks identical.

## 6. Test plan (cumulative)

Per-task tests above plus:

- **Smoke flow replay** after each major step.
- **App.svelte line count** `wc -l src/App.svelte` < 500.
- **Bundle size diff** ≤ +2% from baseline.
- **All tests from Groups 00–05** continue to pass.

## 7. Code review checklist

- [ ] Every silent `catch {}` either has an explanatory comment OR
      logs through the Group 00 logger.
- [ ] No catch decision reduces existing user feedback. (E.g. a
      catch that previously triggered a retry must still trigger
      a retry — only the silence is being changed.)
- [ ] Extracted modules from App.svelte have unit tests.
- [ ] App.svelte template is unchanged in semantics — same
      components in same containers in same order.
- [ ] Git state consolidation: every consumer either uses the new
      `gitState` directly or a documented derived shim. No
      consumer references both old and new API in the same file.
- [ ] Symlink rendering does not break the existing keyboard nav,
      drag-and-drop, or file open behavior.
- [ ] ChatPanel.svelte deletion is verified by `grep`.
- [ ] CLAUDE.md is accurate to the new module layout.
- [ ] Icon consolidation does not introduce a new icon library.
      Same two libraries, just consistent application.
- [ ] Bundle size diff is documented in the PR.
- [ ] No new `console.*` calls.
- [ ] Behavioral baseline replay passes.

## 8. Rollback

Every refactor is independently revertable.

- §5.1: revert; existing silences return.
- §5.2: revert specific extractions; the inline code returns. The
  new modules can stay as no-op exports without callers.
- §5.3: revert; the eight stores return.
- §5.4: revert; symlink-skip returns.
- §5.5: revert; ChatPanel.svelte returns to the tree.
- §5.7: revert; mixed icon usage returns.

This group's changes do not touch user data; rollback is purely
code.

## 9. Out of scope

- **State management library swap.** Svelte's writables work fine.
  No XState, no Zustand-equivalent. Refactor in-place only.
- **Theming system rewrite.** The current CSS-vars approach is
  adequate; a tokens-based design system is a separate project.
- **Accessibility audit.** Aria attributes are mostly already in
  place from `bits-ui`. A full a11y review is its own group.
- **Test coverage thresholds.** Setting CI-enforced coverage
  numbers can come after the test suite settles.

## 10. Notes for the implementing agent

- Each subtask in §5.1–§5.7 is an independent commit. Don't
  batch them.
- The App.svelte split (§5.2) is the most error-prone. Take it
  slowly. Run the smoke flow after every extraction. If a
  refactor produces ANY behavior change, back it out.
- Don't optimize during refactor. The goal here is structural
  cleanup, not perf. Any perf improvement that emerges must be
  incidental and documented.
- Consult the audit's L1, M1, M4, M5, M15, L5, L9 sections for the
  exact list of issues.
- After all changes, the test suite must be 100% green. If a test
  has to be modified to pass, that's a behavior change — revert
  the offending refactor.
