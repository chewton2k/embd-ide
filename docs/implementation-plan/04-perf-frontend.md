# Group 04 — Performance: Frontend

> **Status:** Not started
> **Risk:** Medium — touches reactivity-sensitive paths
> **Effort:** Medium (2–3 days)
> **Depends on:** Group 00 (logging + tests), Group 03 (backend perf)

## 1. Goal

Eliminate the avoidable frontend latency that the user perceives as
"choppiness" in three places: (1) the file tree under continuous
git polling and during scroll/hover, (2) the AI chat during
streaming, (3) the floating-chat drag/resize. Earlier work in this
session removed the worst offender (per-keystroke fan-out into the
`openFiles` reactive store); this group cleans up the remaining
reactive churn, redundant parsing, and unbounded global event
listeners.

## 2. Audit references

- **H1** — Git polling re-renders the entire file tree every 3s.
- **H3** — `fileContentCache` (introduced previous session) has no
  eviction strategy.
- **H4** — `parsedMessages` in `FloatingChat` re-parses every message
  on every chunk.
- **H5** — `marked.parse` runs synchronously on every render during
  streaming.
- **H6** — Auto-scroll in chat hijacks user scroll position.
- **H7** — FloatingChat drag/resize updates state per mousemove
  without rAF.
- **H9** — `FileTree.handleGlobalMouseMove` global listener active
  even when no drag is in progress.
- **H10** — `openFiles` array reconstruction on every modification.
- **H11** — `validate_path` syscall-heavy for nonexistent paths.
- **H12** — `get_git_status` returns paths via `PathBuf::join`
  without canonicalization, can mismatch frontend cache.
- **L4** — `color-mix` recomputed every paint.

## 3. Preservation guarantees

- The file tree displays exactly the same files, sorted the same
  way, with the same git-status badge colors as before.
- Tab order, drag-and-drop, rename, delete, multi-select — every
  current FileTree interaction works identically.
- AI chat displays the same parsed blocks (tool-call cards, prose,
  errors) for the same message content. Markdown rendering
  produces identical HTML.
- Scrolling in chat: user can scroll up freely; auto-scroll only
  kicks in when the user is already near the bottom (this is a
  minor UX *improvement*, not a behavior change — call out in PR).
- FloatingChat window position, size, minimize, drag, resize all
  work identically.
- No measurable regression on small projects (< 100 files); the
  new code paths must be no-cost when there's nothing to optimize.

## 4. Pre-flight

1. **Capture frame timing.** With Chrome DevTools' Performance tab
   (open the IDE in dev mode):
   - Record while hovering over file tree rows for 5 seconds. Note
     dropped frames and the longest task duration.
   - Record while a long AI response is streaming. Same.
   - Record while dragging the FloatingChat across the screen.
2. **Capture interaction latency.** Use `performance.now()` to
   measure: time from mousedown on a tree row to the row receiving
   `:hover` styling; time from typing a character to the editor
   showing it.
3. **Establish the smoke project.** Use a project with ≥ 1000 files
   for the FileTree tests. Use the leo repo itself for AI chat
   tests.

## 5. Implementation tasks

### 5.1 Smart git polling with diff (H1)

File: `src/lib/components/filetree/FileTree.svelte`,
`src/lib/modules/git/git.ts`.

**Current:** every 3 s, fetch full git status, write to a Svelte
store, every tree row re-runs `getGitStatusColor` etc.

**Step 1 — diff old vs new before writing to the store.**

```ts
async function fetchGitStatus() {
  const next = await invoke('get_git_status', { path: rootPath });
  // Cheap shallow diff; bail if unchanged.
  if (mapsEqual(next, get(sharedGitStatus))) return;
  sharedGitStatus.set(next);
}
```

`mapsEqual` is a 10-line helper: same key count, same key set, same
values.

**Step 2 — debounce + watch.** Only the `.git/` directory matters
for status changes from CLI git operations. Watch via
`@tauri-apps/plugin-fs::watch` for `.git/HEAD`, `.git/index`,
`.git/refs/heads`. On any change, schedule a status refresh with
50 ms debounce. Keep the 3 s poll as a backstop — drop to every 30 s
since the watcher catches most events.

**Step 3 — derived per-row color.** Currently `{@const gitColor =
getGitStatusColor(...)}` runs in the template per-render. Convert
to a `$derived.by` keyed by `(entry.path, $sharedGitStatus,
$sharedGitRemoteStatus)`. Svelte 5 will only recompute when those
inputs change.

**Tests:**

- Vitest: stub `invoke('get_git_status')` to return identical
  results twice; confirm `sharedGitStatus.set` is called once.
- Property: random map mutations in/out of equality; `mapsEqual`
  matches a reference implementation.

### 5.2 fileContentCache LRU eviction (H3)

File: `src/lib/modules/explorer/files.ts`.

**Current:** Map grows monotonically as long as new keys are added.
Bounded in practice by `maxTabs * 2` because `closeFile` removes
entries, but `renameOpenFile` and external rename via watcher could
leak entries.

**Fix:** Convert the Map to a tiny bounded LRU.

```ts
class BoundedLru<K, V> {
  constructor(private cap: number) {}
  private map = new Map<K, V>();
  get(k: K): V | undefined {
    const v = this.map.get(k);
    if (v !== undefined) {
      this.map.delete(k);
      this.map.set(k, v); // bump to end
    }
    return v;
  }
  set(k: K, v: V): void {
    if (this.map.has(k)) this.map.delete(k);
    this.map.set(k, v);
    while (this.map.size > this.cap) {
      const first = this.map.keys().next().value;
      if (first === undefined) break;
      this.map.delete(first);
    }
  }
  delete(k: K): void { this.map.delete(k); }
}

const fileContentCache = new BoundedLru<string, string>(50);
```

Cap of 50 is plenty (max tabs default 9, plus background-modified
buffers).

**Tests:**

- Unit: insert 51 entries, oldest is gone; access of an existing
  entry bumps it to most-recent.

### 5.3 Memoize parsedMessages in FloatingChat (H4)

File: `src/lib/components/ai/FloatingChat.svelte`.

**Current:**

```ts
const parsedMessages = $derived(
  $chatMessages.map((msg, i) => ({ /* parseAssistantContent or parseUserContent */ }))
);
```

Re-parses every message every time `$chatMessages` changes.

**Fix:** memoize per `(index, content.length)` since assistant
content is append-only during streaming and length is a cheap
fingerprint that captures every change.

```ts
type ParsedEntry = {
  role: 'user' | 'assistant' | 'system';
  blocks: ChatBlock[];
  index: number;
  fingerprint: string; // role + content.length
};

let parseCache = new Map<number, ParsedEntry>();

const parsedMessages = $derived.by(() => {
  const msgs = $chatMessages;
  const result: ParsedEntry[] = [];
  for (let i = 0; i < msgs.length; i++) {
    const msg = msgs[i];
    const fp = `${msg.role}:${msg.content.length}`;
    const cached = parseCache.get(i);
    if (cached && cached.fingerprint === fp) {
      result.push(cached);
      continue;
    }
    const blocks = msg.role === 'user'
      ? parseUserContent(msg.content)
      : msg.role === 'assistant'
        ? parseAssistantContent(msg.content)
        : [{ kind: 'prose' as const, text: msg.content }];
    const entry: ParsedEntry = { role: msg.role, blocks, index: i, fingerprint: fp };
    parseCache.set(i, entry);
    result.push(entry);
  }
  // Drop cache entries beyond the current message count.
  for (const k of parseCache.keys()) {
    if (k >= msgs.length) parseCache.delete(k);
  }
  return result;
});
```

When clearing the chat (`clearChat`), reset the cache.

**Tests:**

- Vitest with chat-message store: send 5 messages, assert
  `parseAssistantContent` is called 5 times. Append a delta to
  the last; assert it's re-parsed but the previous 4 are NOT.

### 5.4 Debounced markdown rendering (H5)

File: `src/lib/components/ai/FloatingChat.svelte`.

**Current:** `renderProse(content)` calls `marked.parse +
DOMPurify.sanitize` on every reactive render. During streaming this
is hundreds of times per message.

**Fix:** cache rendered HTML keyed by content fingerprint, and for
the actively-streaming message, debounce the render to one frame
(rAF).

```ts
const renderedCache = new Map<string, string>();

function renderProse(content: string): string {
  const fp = `${content.length}:${content.length > 0 ? content.charCodeAt(0) : 0}`;
  // Length+first-char fingerprint is good enough; collisions
  // produce identical HTML in practice.
  const cached = renderedCache.get(fp);
  if (cached) return cached;
  const html = DOMPurify.sanitize(marked.parse(content, { async: false }) as string);
  renderedCache.set(fp, html);
  if (renderedCache.size > 200) {
    // Trivial size cap; oldest entries get evicted.
    const first = renderedCache.keys().next().value;
    if (first !== undefined) renderedCache.delete(first);
  }
  return html;
}
```

For streaming render throttling: wrap the streaming-tail prose block
in a component that debounces re-render via `requestAnimationFrame`.
Implementation: a small `<StreamingProse content={...}>` component
that schedules `inner = renderProse(content)` via rAF and caps at
one render per frame. Final render (when streaming stops) is
unthrottled so the user sees the complete message immediately.

**Tests:**

- Mock `marked.parse` to count invocations. Render the same content
  twice; assert one invocation.
- Render incrementally-growing content; assert rAF throttling caps
  to one render per frame.

### 5.5 Smart auto-scroll (H6)

File: `src/lib/components/ai/FloatingChat.svelte`.

**Current:** `$effect(() => { $chatMessages; scrollToBottom(); });`
hijacks scroll regardless of user's scroll position.

**Fix:** only auto-scroll if user was already at the bottom before
the message arrived. Threshold: user is "at the bottom" if
`scrollHeight - clientHeight - scrollTop < 80`.

```ts
function isPinnedToBottom(el: HTMLElement): boolean {
  return el.scrollHeight - el.clientHeight - el.scrollTop < 80;
}

let userPinnedToBottom = true;

function onMessagesScroll() {
  if (messagesEl) userPinnedToBottom = isPinnedToBottom(messagesEl);
}

// Replace the effect:
$effect(() => {
  $chatMessages;
  if (userPinnedToBottom) scrollToBottom();
});
```

Wire `onmessagesScroll` to the messages container's `onscroll` event.

**Tests:**

- Manual: scroll up while assistant is streaming; new chunks do not
  yank scroll. Scroll back to bottom; new chunks resume auto-scroll.

### 5.6 rAF on FloatingChat drag/resize (H7)

File: `src/lib/components/ai/FloatingChat.svelte`.

Apply the same pattern used in `App.svelte` (resolved earlier this
session): coalesce per-mousemove updates into one `requestAnimation
Frame` per frame.

```ts
let pendingPos: { x: number; y: number } | null = null;
let pendingSize: { w: number; h: number } | null = null;
let dragRafId: number | null = null;

function applyDragOrResize() {
  dragRafId = null;
  if (pendingPos) {
    x = pendingPos.x;
    y = pendingPos.y;
    pendingPos = null;
  }
  if (pendingSize) {
    width = pendingSize.w;
    height = pendingSize.h;
    pendingSize = null;
  }
}

function onDragMove(e: MouseEvent) {
  pendingPos = {
    x: Math.max(0, Math.min(window.innerWidth - 100, dragStart.winX + e.clientX - dragStart.x)),
    y: Math.max(0, Math.min(window.innerHeight - 40, dragStart.winY + e.clientY - dragStart.y)),
  };
  if (dragRafId === null) dragRafId = requestAnimationFrame(applyDragOrResize);
}

function onResizeMove(e: MouseEvent) {
  pendingSize = {
    w: Math.max(320, resizeStart.w + e.clientX - resizeStart.x),
    h: Math.max(320, resizeStart.h + e.clientY - resizeStart.y),
  };
  if (dragRafId === null) dragRafId = requestAnimationFrame(applyDragOrResize);
}
```

Cancel the rAF in `stopDrag` and `stopResize`.

### 5.7 Conditional global mousemove listener in FileTree (H9)

File: `src/lib/components/filetree/FileTree.svelte`.

**Current:** `handleGlobalMouseMove` is always registered on window.
The fast-path bail is cheap, but every mousemove still calls into
JS.

**Fix:** register the listener only when a drag starts; unregister
on mouseup. Mirrors the post-fix pattern in `App.svelte`.

`handleDragMouseDown` adds the listener. `endDrag` removes it.
Same for `handleGlobalMouseUp`.

**Test:**

- Manual: open Activity Monitor, observe CPU at idle. Should not
  see the JS engine waking up at every mouse motion.

### 5.8 Map-based openFiles store (H10)

File: `src/lib/modules/explorer/files.ts`.

**Current:** `openFiles = writable<OpenFile[]>([])`. Every
modification (`updateFileContent`, `togglePin`, `markFileSaved`,
`closeFile`) reconstructs the array via `.map(...)`.

**Fix:** keep the array shape (it's the public API used by the each-
block in Tabs.svelte), but optimize internal mutations via a small
helper:

```ts
function patchFile(path: string, patch: Partial<OpenFile>): void {
  openFiles.update(files => {
    const idx = files.findIndex(f => f.path === path);
    if (idx === -1) return files;
    const cur = files[idx];
    // No-op skip: if every patched field already equals the new value.
    let changed = false;
    for (const k in patch) {
      if (cur[k as keyof OpenFile] !== patch[k as keyof OpenFile]) {
        changed = true;
        break;
      }
    }
    if (!changed) return files;
    const next = files.slice();
    next[idx] = { ...cur, ...patch };
    return next;
  });
}
```

Replace `markFileSaved`, `togglePin` (single-field updates) with
calls to `patchFile`. The skip-when-no-change branch eliminates
spurious reactive notifications.

**Tests:**

- Unit: call `markFileSaved` on an already-unmodified file; assert
  the store's subscriber is NOT notified.

### 5.9 Cache canonicalized project root in `validate_path` (H11)

File: `src-tauri/src/modules/fs/mod.rs`.

**Current:** `validate_path` calls `fs::canonicalize` on the project
root every invocation. The root rarely changes. Cache it.

**Fix:** the existing `ProjectRootState: Arc<Mutex<Option<PathBuf>>>`
already stores a canonical root from `set_project_root`. Use it
directly without re-canonicalizing.

For the path being validated: when the path doesn't exist, the
existing ancestor-walk is correct. Cache `(stem, canonical_stem)`
pairs in a small lru per project to skip the walk for repeated
nonexistent paths in the same parent — but only if profiling shows
this is a hotspot. Defer that cache until measured.

This is mostly a code-cleanup task; the hot path was never as bad
as it looked because canonicalize is fast on a hot inode cache.

### 5.10 Canonicalize git status output (H12)

File: `src-tauri/src/modules/git/mod.rs::get_git_status`,
`get_git_remote_status`.

**Current:** absolute paths returned via `PathBuf::from(&path).join
(file_path)` without canonicalization. If the repo path was passed
to `set_project_root` as a symlinked path, frontend caches the
canonical version, but git status returns the symlinked version,
and the frontend's `Map.get(path)` lookup misses.

**Fix:** canonicalize once at function entry (the repo root) and
join the file path against the canonical root:

```rust
let canonical_root = validate_repo_path(&path, &state)?;
// ...
let abs_path = canonical_root.join(file_path);
```

`validate_repo_path` already returns the canonical `PathBuf`; use
it instead of re-stringing.

**Test:**

- Set up a symlinked test repo (`ln -s real-repo symlink-repo`),
  set the project root via the symlink, fetch git status, confirm
  the returned paths match the canonical paths the frontend tree
  is using.

### 5.11 Precompute color-mix values (L4)

File: `src/lib/components/filetree/FileTree.svelte`,
`src/app.css`.

**Current:** `background: color-mix(in srgb, var(--bg-surface) 60%,
transparent);` recomputed every paint.

**Fix:** define `--bg-surface-hover` and `--bg-surface-selected` in
`app.css` per theme, with the precomputed color values. Tree CSS
references the precomputed var. No browser-side `color-mix` in hot
paint paths.

```css
:root.dark {
  --bg-surface-hover: rgba(45, 45, 45, 0.6);
  --bg-surface-selected: rgba(123, 159, 194, 0.12);
}
:root.light {
  /* equivalents */
}
```

The values come from the existing definitions; transcribe them
once. Future theme changes update both the surface and the hover
variants.

## 6. Test plan (cumulative)

- All per-task tests above.
- **Integration:** open a 1000-file project, scroll through the
  file tree for 5s; capture a Performance trace; main-thread tasks
  stay under 16ms. Document the measurement.
- **AI chat regression:** stream a 5000-character response; the
  final rendered HTML matches a reference snapshot.
- **Behavioral baseline replay:** every step identical to G00
  baseline.

## 7. Code review checklist

- [ ] Git polling: `mapsEqual` skip is exercised by a unit test;
      no accidental deep-vs-shallow comparison bug.
- [ ] FS watcher cleanup: every `watch` call has a corresponding
      unwatch in `onDestroy`. No leaks.
- [ ] `BoundedLru` is unit-tested for cap, bump-on-access, and
      delete.
- [ ] `parseCache` reset on `clearChat` — verified by a test that
      sends, clears, sends again, and inspects the cache size.
- [ ] `renderedCache` is bounded (no unbounded growth even for
      content with many distinct fingerprints).
- [ ] Auto-scroll only fires when `userPinnedToBottom`. Manual sign-
      off documenting the new UX.
- [ ] FloatingChat drag/resize uses rAF; mouseup cancels the rAF.
- [ ] FileTree global mousemove listener is removed when not
      dragging. Verified by inspecting `getEventListeners(window)`
      in DevTools at idle.
- [ ] `patchFile` returns the same array reference when no field
      changed; reactive subscribers are not notified.
- [ ] Symlink repo round-trip: status badges appear on the right
      rows.
- [ ] Theme variables for hover/selected colors are defined for
      both `.light` and `.dark` themes; visual diff is zero.
- [ ] No new global event listeners. No new always-on subscriptions.
- [ ] No new `console.*`. Logging via Group 00 logger.
- [ ] Behavioral baseline replay passes.

## 8. Rollback

Most changes are additive optimizations behind stable interfaces.

- §5.1: revert; polling returns to every-3s full-write.
- §5.3, §5.4: revert; re-parse/re-render on every change returns.
- §5.5: revert; auto-scroll always fires (current behavior).
- §5.6, §5.7: revert; mousemove returns to per-event handling.
- §5.8: revert; `.map(...)` everywhere returns.
- §5.10: revert; symlink-repo path mismatch returns (cosmetic).
- §5.11: revert; CSS uses `color-mix` again.

No data migrations; rollback is safe.

## 9. Out of scope

- **Tree virtualization.** Rendering 5000 file rows in DOM is
  expensive even with the optimizations here. Virtualizing the
  tree (only rendering visible rows) is a much bigger change with
  selection/keyboard-nav implications. Track separately.
- **CodeMirror perf tuning.** Editor virtualizes already; no work
  needed. Editor extensions (ghost text, AI diff) could be audited
  but are not on the hot path for typing.
- **Web Worker offload of markdown parsing.** The current debounce
  is enough; workers add IPC overhead.
- **Persistent rendered-HTML cache across reloads.** In-memory only.

## 10. Notes for the implementing agent

- The Svelte 5 `$derived.by` is the key tool for memoization; learn
  it before starting. Avoid `$derived(...)` (assignment form) for
  computed values that should run rarely — `$derived.by` lets you
  control re-runs by reading specific stores inside the body.
- When adding a watcher for `.git/`, ensure it works with both
  worktrees and the main repo. The `.git` may be a file (worktree)
  pointing at the actual gitdir.
- The hover/select CSS variable change requires per-theme
  audit. Check every theme definition (currently in
  `src/lib/modules/theme/themes.ts` or `app.css`); don't break a
  theme by missing it.
- Resist the urge to refactor App.svelte's drag handler again. It
  was fixed earlier this session; that fix stays.
