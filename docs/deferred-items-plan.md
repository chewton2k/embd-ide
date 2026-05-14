# Deferred Items — Implementation Plan

> **Status:** Planning complete, ready for execution
> **Last updated:** 2026-05-13
> **Prerequisite:** All Groups 00–07 landed and green (64 tests passing)

---

## 1. Execution Tracks

Items are organized into parallel tracks based on file-level dependencies.
Items within a track are sequential; tracks themselves can execute in parallel.

```
Track A (Independent — Rust/AI):
  └── C2/C3: Keychain encryption

Track B (Independent — UI/Config):
  └── M9: CSP tightening + Preview allow-list

Track C (Independent — Rust/FS, soft dependency on M3):
  └── M15: Symlinks in file tree (if M3 lands first, use new RwLock API)

Track D (Sequential — Rust/Git):
  M3: RwLock conversion
  └── C6: Byte-level git parsing

Track E (Sequential — Frontend/Refactor):
  M4: Split App.svelte
  └── M5: Consolidate git stores

Track F (Sequential — Frontend/Perf):
  H4: Memoize parsedMessages
  └── H5: Debounced markdown rendering
```

### Recommended Landing Order

| Priority | Item | Track | Effort | Risk |
|----------|------|-------|--------|------|
| 1 | H4 | F | 2h | Low |
| 2 | H5 | F | 2h | Low |
| 3 | C2/C3 | A | 1-2d | Medium |
| 4 | M3 | D | 4h | Medium |
| 5 | M4 | E | 2-3d | Medium-High |
| 6 | C6 | D | 1-2d | High |
| 7 | M5 | E | 1-2d | Medium |
| 8 | M9 | B | 1d | Medium |
| 9 | M15 | C | 1d | High |

---

## 2. Item Specifications

### H4: Memoize parsedMessages in FloatingChat

**Goal:** During streaming, only re-parse the actively-streaming message (last in array). Messages 0..n-1 are immutable once complete.

**Files touched:**
| File | Change |
|------|--------|
| `src/lib/components/ai/FloatingChat.svelte` | Replace `$derived` parsedMessages with incremental memoization |

**Approach:**
```typescript
let parseCache = new Map<string, { blocks: ChatBlock[]; role: string }>();

const parsedMessages = $derived.by(() => {
  const msgs = $chatMessages;
  const result = [];
  for (let i = 0; i < msgs.length; i++) {
    const msg = msgs[i];
    const fp = `${i}:${msg.role}:${msg.content.length}`;
    const cached = parseCache.get(fp);
    if (cached) { result.push({ ...cached, index: i }); continue; }
    const blocks = msg.role === 'user'
      ? parseUserContent(msg.content)
      : msg.role === 'assistant'
        ? parseAssistantContent(msg.content)
        : [{ kind: 'prose' as const, text: msg.content }];
    const entry = { role: msg.role, blocks };
    parseCache.set(fp, entry);
    result.push({ ...entry, index: i });
  }
  // Prune stale entries
  if (parseCache.size > msgs.length * 2) {
    parseCache = new Map([...parseCache].slice(-msgs.length));
  }
  return result;
});
```

**Fingerprint rationale:** `content.length` changes on every streaming chunk for the active message, triggering re-parse. Completed messages have stable length → cache hit.

**Preservation guarantees:**
- Same `ChatBlock[]` output for same input
- `{#each parsedMessages as m (m.index)}` keying unchanged
- `clearChat` must reset the cache

**Tests:**
- Unit: parse 5 messages, append to last, assert only last re-parsed
- Unit: clearChat resets cache

**Rollback:** Revert to the simple `$derived` map.

---

### H5: Debounced Markdown Rendering

**Goal:** `marked.parse + DOMPurify.sanitize` runs at most once per animation frame during streaming.

**Files touched:**
| File | Change |
|------|--------|
| `src/lib/components/ai/FloatingChat.svelte` | Add render cache + rAF throttle for streaming prose |

**Approach:**
```typescript
const renderCache = new Map<string, string>();

function renderProse(content: string, isStreaming: boolean): string {
  if (!content) return '';
  const cached = renderCache.get(content);
  if (cached) return cached;
  const html = DOMPurify.sanitize(marked.parse(content, { async: false }) as string);
  renderCache.set(content, html);
  // Cap cache size
  if (renderCache.size > 200) {
    const first = renderCache.keys().next().value;
    if (first !== undefined) renderCache.delete(first);
  }
  return html;
}
```

During streaming, the template uses `content` directly (the cache misses on every chunk since content grows). The key insight: we cache the *final* rendered HTML so completed messages never re-render.

**Preservation guarantees:**
- Final rendered HTML is identical to current behavior
- DOMPurify always runs (no security regression)
- Non-streaming messages render immediately from cache

**Depends on:** H4 (memoization ensures only the last message triggers render)

**Tests:**
- Unit: same content renders once (cache hit)
- Manual: streaming doesn't stutter

**Rollback:** Remove cache, revert to direct `marked.parse` calls.

---

### C2/C3: Keychain-Only Storage with Encrypted Fallback

**Goal:** API keys stored in OS keyring only. File fallback is encrypted, not plaintext.

**Files touched:**
| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `chacha20poly1305 = "0.10"`, `rand = "0.8"` |
| `src-tauri/src/modules/ai/mod.rs` | Rewrite `get_key`/`set_key` internals |

**Approach:**

```rust
// Key hierarchy:
// 1. Try OS keyring (primary)
// 2. If keyring fails: encrypt to ~/.leo-ide/keys.enc using a
//    machine-bound key stored in keyring entry "__file_key__"
// 3. If keyring can't store __file_key__ either: derive from
//    machine-specific data (dirs::data_local_dir path + build constant)

fn get_key(provider: &str) -> Result<Option<String>, String> {
    // 1. Try keyring
    if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
        if let Ok(pw) = entry.get_password() {
            if !pw.is_empty() { return Ok(Some(pw)); }
        }
    }
    // 2. Try encrypted file
    read_encrypted_key(provider)
}

fn set_key(provider: &str, key: &str) -> Result<(), String> {
    // Try keyring first
    if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
        if key.is_empty() {
            entry.delete_credential().ok();
        } else if entry.set_password(key).is_ok() {
            // Success — remove from file if present
            remove_from_encrypted_file(provider);
            return Ok(());
        }
    }
    // Keyring failed — encrypt to file
    write_encrypted_key(provider, key)
}
```

**Migration on startup:**
```rust
fn migrate_plaintext_keys() {
    let path = keys_file_path(); // ~/.leo-ide/keys.json
    if !path.exists() { return; }
    let Ok(bytes) = std::fs::read(&path) else { return; };
    let Ok(map): Result<HashMap<String, String>, _> = serde_json::from_slice(&bytes) else { return; };
    for (provider, key) in &map {
        set_key(provider, key).ok();
    }
    // Rename old file as backup
    let backup = path.with_extension("json.bak");
    std::fs::rename(&path, &backup).ok();
    log::info!("Migrated {} keys from plaintext to secure storage", map.len());
}
```

**Preservation guarantees:**
- `get_provider_key`, `set_provider_key`, `set_api_key` Tauri command signatures unchanged
- Existing keys readable after migration
- Frontend sees no change

**Security properties:**
- Plaintext keys.json eliminated after migration
- Encrypted file uses ChaCha20-Poly1305 with random 12-byte nonce per write
- File format: `[1-byte version tag][12-byte nonce][ciphertext+tag]` (version=0x01)
- File permissions 0o600 on Unix
- Key never logged (existing redaction in Group 00 logger)
- Migration uses atomic write (temp file + rename) to prevent data loss on crash
- Tier-3 fallback (machine-derived key when keyring completely unavailable) provides obfuscation only — documented as known limitation. Acceptable because: (a) if attacker has disk access, they can also read the binary to extract the derivation constant, (b) the primary protection is OS-level FDE + file permissions, (c) this fallback only activates on systems without any keyring support (rare)

**Tests:**
- Rust unit: round-trip encrypt/decrypt
- Rust unit: migration from plaintext file (atomic write verified)
- Rust unit: keyring failure falls back to encrypted file
- Rust unit: corrupted encrypted file returns error (doesn't crash)
- Manual: verify key persists across app restart

**Rollback:**
- `keys.json.bak` preserved for emergency restore
- Env var `LEO_DISABLE_KEY_MIGRATION=1` skips migration

---

### M3: Convert ProjectRootState to tokio::sync::RwLock

**Goal:** Eliminate potential thread blocking when async commands lock the project root state.

**Files touched:**
| File | Functions affected |
|------|-------------------|
| `src-tauri/src/modules/fs/mod.rs` | `create_project_root_state`, `set_project_root`, `validate_path` + all 15 commands |
| `src-tauri/src/modules/git/mod.rs` | `validate_repo_path` + all 20 commands |
| `src-tauri/src/modules/knowledge/mod.rs` | `validate_knowledge_root` |
| `src-tauri/src/modules/shell/mod.rs` | `spawn_terminal` (reads project root for cwd validation) |
| `src-tauri/src/modules/graph/mod.rs` | `analyze_file_graph` (reads project root) |
| `src-tauri/src/lib.rs` | Type of managed state |

**Approach:** Use `blocking_read()` / `blocking_write()` for sync commands, `.read().await` / `.write().await` for async commands. This avoids converting all 35+ commands to async.

```rust
pub type ProjectRootState = Arc<tokio::sync::RwLock<Option<PathBuf>>>;

// For sync commands (most git commands):
pub fn validate_repo_path(
    repo_path: &str,
    state: &tauri::State<'_, ProjectRootState>,
) -> Result<PathBuf, String> {
    let root = state.blocking_read();
    // ...
}

// For async commands (knowledge_init, etc.):
pub async fn validate_path_async(
    path: &str,
    state: &tauri::State<'_, ProjectRootState>,
) -> Result<PathBuf, String> {
    let root = state.read().await;
    // ...
}
```

**Migration strategy:**
1. Change type alias
2. `cargo check` — fix every compile error (mechanical: `.lock()` → `.blocking_read()` or `.blocking_write()`)
3. For `set_project_root`: use `.blocking_write()` (it's a sync command)
4. Run tests

**Preservation guarantees:**
- All command signatures unchanged at the IPC boundary
- Same validation behavior
- No deadlock risk: `blocking_read` is safe from sync context; `blocking_write` is only called in `set_project_root` (infrequent)

**Tests:**
- Existing 7 Rust tests must pass
- Manual: open project, switch project, verify no hangs

**Rollback:** Revert type alias back to `std::sync::Mutex`.

---

### M4: Split App.svelte

**Goal:** Reduce App.svelte from ~1000 lines to <500 by extracting pure-logic modules.

**Files touched:**
| New file | Extracted from |
|----------|---------------|
| `src/lib/modules/layout/panelResize.ts` | Sidebar/chat/git drag handlers (~50 lines) |
| `src/lib/modules/layout/breadcrumb.ts` | `breadcrumbSegments` derivation (~30 lines) |
| `src/lib/modules/ui/windows.ts` (exists) | `openSettingsWindow` (~40 lines) |

**Extraction order (safest first):**
1. `breadcrumbSegmentsFor(path, root)` — pure function, zero side effects
2. Panel resize handlers — stateful but self-contained
3. `openSettingsWindow` — already partially in windowChrome.ts

**Preservation guarantees:**
- Every keyboard shortcut still works
- Layout dimensions unchanged
- Session save-on-close timing unchanged
- No new reactive subscriptions

**Approach:** Extract one piece per commit. Run `npm test` + smoke flow after each.

**Tests:**
- Unit: `breadcrumbSegmentsFor('/Users/x/project/src/lib/foo.ts', '/Users/x/project')` returns expected segments
- Existing tests must remain green

**Rollback:** Revert individual extraction commits.

---

### C6: Byte-Level Git Parsing

**Goal:** Parse `git status --porcelain -z` output as raw bytes, not lossy UTF-8 strings.

**Files touched:**
| File | Functions |
|------|-----------|
| `src-tauri/src/modules/git/mod.rs` | `get_git_status`, `get_git_remote_status`, `git_discard` (internal parsing) |

**Depends on:** M3 (same functions are modified)

**Approach:**
```rust
// Before:
let stdout = String::from_utf8_lossy(&output.stdout);
for line in stdout.lines() { ... }

// After:
let stdout: &[u8] = &output.stdout;
// -z flag uses NUL separators
for entry in stdout.split(|&b| b == 0) {
    if entry.len() < 4 { continue; }
    let index_status = entry[0];
    let wt_status = entry[1];
    // entry[2] is space
    let file_bytes = &entry[3..];
    let file_path = String::from_utf8_lossy(file_bytes).into_owned();
    // ...
}
```

**Key edge cases:**
- Rename entries (`R` status): the *next* NUL-separated entry is the source path
- Copy entries (`C` status): same pattern as rename
- Untracked entries: only working-tree status, no index status

**Preservation guarantees:**
- Return type `HashMap<String, String>` unchanged (IPC contract)
- ASCII filenames produce identical results
- Non-ASCII filenames now display correctly (previously corrupted)

**Tests:**
- Rust unit: parse a known `git status -z` byte sequence with ASCII paths
- Rust unit: parse with non-ASCII paths (UTF-8 CJK characters)
- Rust unit: parse with rename entry (two consecutive paths)
- Existing git tests must pass

**Rollback:** Revert to `String::from_utf8_lossy` on full output.

---

### M5: Consolidate Git Stores

**Goal:** Replace 4 separate git stores with a single atomic update.

**Files touched:**
| File | Change |
|------|--------|
| `src/lib/modules/git/git.ts` | New `gitState` store + derived shims |
| `src/lib/components/filetree/FileTree.svelte` | Write to `gitState` instead of individual stores |
| `src/lib/components/git/GitPanel.svelte` | Read from derived shims (no change if shims match old API) |
| `src/App.svelte` | Read from derived shim |

**Depends on:** M4 (to avoid merge conflicts)

**Approach (backward-compatible shims):**
```typescript
// Phase 1: Add unified store + shims
export const gitState = writable<{
  status: Record<string, string>;
  remoteStatus: Record<string, string>;
}>({ status: {}, remoteStatus: {} });

// Backward-compat shims (consumers don't need to change yet)
export const sharedGitStatus = derived(gitState, $g => $g.status);
export const sharedGitRemoteStatus = derived(gitState, $g => $g.remoteStatus);

// Phase 2: Migrate writers (FileTree)
// Phase 3: Migrate readers to use gitState directly
// Phase 4: Remove shims
```

**Preservation guarantees:**
- Same reactive behavior (derived stores fire on same triggers)
- `recordsEqual` optimization preserved (check before writing to gitState)
- `projectRoot` and `gitBranch` remain separate (used by non-git modules)

**Tests:**
- Unit: write to gitState, assert derived shims emit correct values
- Existing tests must pass

**Rollback:** Revert; individual stores return.

---

### M9: CSP Tightening + Preview Allow-List

**Goal:** Remove `https://*` from frame-src; gate non-localhost previews on user approval.

**Files touched:**
| File | Change |
|------|--------|
| `src-tauri/tauri.conf.json` | Tighten `frame-src` |
| `src/lib/components/preview/Preview.svelte` | Add allow-list check before loading URL |
| New: `src/lib/modules/preview/allowList.ts` | Persisted allow-list store |

**Approach:**
```typescript
// allowList.ts
import { persistedString } from '../session/persisted';
const raw = persistedString('leo-preview-allowlist', '[]');
export const previewAllowList = derived(raw, $r => {
  try { return JSON.parse($r) as string[]; } catch { return []; }
});
export function addToAllowList(host: string) { ... }
export function isAllowed(url: string): boolean { ... }
```

**Preview.svelte change:**
```typescript
function loadUrl(target: string) {
  const url = new URL(target);
  if (url.hostname === 'localhost' || url.hostname === '127.0.0.1') {
    // Always allowed
    iframeSrc = target;
  } else if (isAllowed(target)) {
    iframeSrc = target;
  } else {
    showAllowPrompt = true;
    pendingUrl = target;
  }
}
```

**CSP change (only after UI ships):**
```json
"frame-src": "'self' asset: https://asset.localhost http://localhost:* http://127.0.0.1:* https://localhost:* https://127.0.0.1:*"
```
Also add `object-src 'none'` to prevent plugin-based CSP bypasses.

**Preservation guarantees:**
- Localhost previews work identically
- Previously-allowed external URLs continue working (persisted list)
- "Open in popup" fallback still available for blocked URLs

**Tests:**
- Unit: `isAllowed` with localhost → true
- Unit: `isAllowed` with unlisted host → false
- Unit: `addToAllowList` persists and subsequent `isAllowed` → true
- Manual: load external preview, see prompt, approve, verify loads

**Rollback:** Revert CSP change + remove allow-list check. Atomic.

---

### M15: Symlinks in File Tree

**Goal:** Show symlinks in the file tree instead of silently skipping them.

**Files touched:**
| File | Change |
|------|--------|
| `src-tauri/src/modules/fs/mod.rs` | `read_dir_recursive`: include symlinks, add `is_symlink` field, cycle detection |
| `src/lib/components/filetree/FileTree.svelte` | Render symlink badge (lucide `Link2` icon) |

**Approach:**
```rust
// Cycle detection via canonical path set
fn read_dir_recursive(path: &Path, depth: u32, visited: &mut HashSet<PathBuf>) -> Vec<FileEntry> {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return vec![]; // Cycle detected
    }
    // ...
    for entry in entries {
        let ft = entry.file_type()?;
        let is_symlink = ft.is_symlink();
        let is_dir = if is_symlink {
            // Check if symlink target is a directory
            entry.path().is_dir()
        } else {
            ft.is_dir()
        };
        // For symlinked directories: include but don't recurse (prevent cycles)
        let children = if is_dir && !is_symlink && depth > 0 {
            Some(read_dir_recursive(&entry.path(), depth - 1, visited))
        } else {
            None
        };
        // ...
    }
}
```

**Frontend rendering:**
```svelte
{#if entry.is_symlink}
  <span class="symlink-badge" title="Symbolic link"><Link2 size={10} /></span>
{/if}
```

**Preservation guarantees:**
- Non-symlink files/dirs render identically
- Symlinked directories show but don't expand (prevents infinite recursion)
- `validate_path` still rejects paths outside project root (symlinks resolving outside are shown but not openable)

**Security consideration:**
- A symlink pointing to `/etc/passwd` would be visible in the tree but `read_file_content` would reject it via `validate_path` (canonicalizes and checks `starts_with(root)`)

**Tests:**
- Rust unit: temp dir with symlink, `read_dir_recursive` returns entry with `is_symlink: true`
- Rust unit: symlink cycle (a→b→a) doesn't infinite-loop
- Rust unit: symlinked directory has `children: None`
- Manual: create symlink in project, verify it appears with badge

**Rollback:** Revert; symlinks silently skipped again.

---

## 3. Risk Matrix

| Item | Behavioral Risk | Security Risk | Merge Conflict Risk |
|------|----------------|---------------|---------------------|
| H4 | None | None | None |
| H5 | None | Low (DOMPurify must always run) | None |
| C2/C3 | Medium (key migration) | High (encryption correctness) | None |
| M3 | Low (mechanical) | None | Medium (touches 35+ functions) |
| M4 | Medium (reactive context) | None | High (App.svelte is hot file) |
| C6 | High (git parsing) | None | Medium (git/mod.rs) |
| M5 | Medium (reactive churn) | None | Medium (multiple consumers) |
| M9 | Medium (breaks external previews) | Positive (tightens CSP) | Low |
| M15 | High (cycles, security) | Medium (path traversal) | Low |

---

## 4. Testing Strategy

Each item must:
1. Pass all existing 64 tests before and after
2. Add item-specific tests (listed per item above)
3. Pass `cargo check --quiet` and `npx svelte-check --threshold error`
4. Be reviewed by a code-review sub-agent with the item's checklist

**Smoke flow (replay after each item):**
1. Launch app → Open project → Open file → Type → Terminal → `ls` → AI chat → Close

---

## 5. Emergency Rollback

Every item is one focused commit. Rollback = `git revert <hash>`.

For C2/C3 specifically:
- `~/.leo-ide/keys.json.bak` preserved during migration
- `LEO_DISABLE_KEY_MIGRATION=1` env var skips migration on startup
- If encrypted file is corrupted, user can restore backup: `cp keys.json.bak keys.json`
