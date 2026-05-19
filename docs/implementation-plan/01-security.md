# Group 01 — Security Hardening

> **Status:** Not started
> **Risk:** Medium — touches auth/key storage and IPC boundary validation
> **Effort:** Medium (2–3 days)
> **Depends on:** Group 00 (logging + tests)

## 1. Goal

Close the highest-impact security gaps surfaced by the audit:
plaintext key storage that defeats the keyring, weak git ref-name
validation that could allow flag injection, lossy OS-string handling
that breaks paths and can be coerced, an over-permissive CSP, and a
hardcoded filesystem scope that both excludes legitimate users and is
broader than necessary.

## 2. Audit references

- **C2** — API keys written to plaintext file even when keyring works.
- **C3** — `keys.json` write is non-atomic.
- **C5** — Git ref-name validation is bypassable (`-`, control chars,
  refname-illegal characters not rejected).
- **C6** — `String::from_utf8_lossy` everywhere on git/fs paths and
  PTY data, silently corrupting non-UTF8 filenames.
- **M9** — CSP `frame-src https://*` is wildcard.
- **M10** — Hardcoded filesystem scope in `tauri.conf.json` /
  `capabilities/default.json` excludes many users (`~/dev`, etc.) and
  is broader than necessary for legitimate users.

## 3. Preservation guarantees

After this group, the following must continue to work identically:

- Setting, reading, and deleting an API key for each provider
  (OpenRouter, OpenAI, Anthropic) on macOS, Windows, and Linux.
- A user with a saved key continues to find that key working after
  upgrading to the new build (migration is non-destructive).
- All current git operations (status, diff, stage, unstage, discard,
  commit, push, pull, fetch, log, list/checkout/delete branch,
  resolve conflict) work for repos with ASCII-only filenames AND for
  repos with non-ASCII filenames (after this group, the latter case
  starts working — currently it silently corrupts).
- Terminal sessions launch with the existing default shell and CWD
  rules.
- The Preview pane continues to display dev servers running on
  localhost (the most common case).
- File-tree open-folder dialog continues to work for projects under
  any subdirectory of `$HOME` that the user could currently access.

## 4. Pre-flight

1. **Capture the current key-storage behavior.** With a test API key:
   - Set via Settings → Models.
   - Confirm key appears via `security find-generic-password -s
     leo-ide -a openrouter -w` (macOS) or platform equivalent.
   - Confirm key also appears in `~/.leo-ide/keys.json`.
   - Document the format of `keys.json` (plain JSON map of provider
     → key string).
2. **Capture git operation baselines on a non-ASCII repo.** Create a
   test repo with a filename like `日本語.txt`, run each git command
   from the IDE, observe what currently happens (status badges
   wrong? operations fail?). Record findings — these are
   regressions-in-disguise that this group fixes.
3. **Capture the current FS scope behavior.** Try to open a project
   under a directory not in the scope list (e.g. `~/foo/bar`); record
   the failure mode.

## 5. Implementation tasks

### 5.1 Keychain-only key storage with controlled fallback (C2 + C3)

File: `src-tauri/src/modules/ai/mod.rs`.

**New behavior:**

- `set_key(provider, key)` first tries the OS keyring. If that
  succeeds, the file fallback is NOT written, and any existing entry
  in the file for that provider is REMOVED. This collapses the key
  to a single source of truth.
- If the keyring write fails (real-world cases: unsigned dev binary
  on macOS, missing libsecret on Linux, locked Credential Manager on
  Windows), then and only then do we fall back to the encrypted file.
- The file is written via temp-file + rename (atomic).
- The file is encrypted at rest using a symmetric key derived from a
  machine-bound source via `keyring::Entry::new("leo-ide",
  "__file_encryption_key__")`. If the keyring can't store *that*
  either, we fall back to a deterministic derivation from
  `dirs::data_local_dir()` path + a build-time constant; this is
  weak but better than plaintext, and we log a warning.
- File-permissions hardening on Unix stays at `0o600`.
- On startup, `read_keys_file` is called. If the keyring is healthy
  AND a file exists, migrate file entries → keyring, then delete the
  file. Migration is logged via `log::warn` so we know it ran.

**Cipher choice:** use `chacha20poly1305` (already widely used in the
ecosystem; small footprint). Add to `Cargo.toml`:

```toml
chacha20poly1305 = "0.10"
```

Justification (per operating principle on dependencies): no current
crate provides authenticated encryption; rolling our own is worse;
chacha20poly1305 is widely audited. ~30 KB additional binary size.

**API-compatibility:**

- `get_provider_key`, `set_provider_key`, `set_api_key` Tauri
  commands keep their exact signatures.
- The frontend never sees this change.

**Test plan:**

- Round-trip: set key, restart, key reads back.
- Migration: pre-populate `keys.json` with a plaintext entry, mock
  keyring as healthy, restart, confirm keyring contains the key and
  file no longer contains it.
- Encryption: when keyring is mocked unhealthy, confirm the file on
  disk does NOT contain the plaintext key (binary inspection).
- Atomic write: kill the process mid-`fs::write` via a fault
  injection in tests; confirm the file isn't corrupted afterwards.

### 5.2 Git ref-name validation (C5)

File: `src-tauri/src/modules/git/mod.rs`.

**New helper:**

```rust
fn validate_git_ref_name(name: &str) -> Result<(), String>;
```

Rules implemented per `git-check-ref-format(1)`:

- Reject empty.
- Reject any byte < 0x20 (control chars), 0x7F (DEL).
- Reject `\` `:` `?` `*` `[` `~` `^` and space.
- Reject leading `-` (would be parsed as a flag).
- Reject leading or trailing `/`, leading `.`, trailing `.lock`.
- Reject `..`, `@{`, `//`, `/.`.
- Reject `@` (alone).
- Reject NUL.

Defense-in-depth: also call `git check-ref-format --branch <name>`
when a branch is being checked out / deleted; reject if the helper
exits non-zero.

Apply at the entry of:

- `git_delete_branch`
- `git_checkout_branch` (validate `branch` argument; if `is_remote`,
  validate the trailing component after the first `/` — the local
  branch name to be created).

**Test plan:**

- Property-style: a list of known-bad inputs all return `Err`:
  `["", "-x", "..", "foo..bar", "foo space", "ctrl\x01", "foo:bar",
  "foo?", "foo*", "foo[", "foo~", "foo^", "foo\\bar", ".lock",
  "foo.lock", "/foo", "foo/", "foo//bar", "foo/.bar", "@", "foo@{",
  "foo\0bar"]`.
- Positive: `["main", "feature/login", "release-1.0", "fix_bug",
  "dependabot/npm/foo-1.2.3"]` all return `Ok`.

### 5.3 OS-string-aware path handling (C6)

This is the most invasive change. Approach:

**Step A — keep the IPC boundary as `String`** (Tauri's serialization
constrains us). Inside Rust, use `PathBuf`/`OsString` end-to-end and
only convert via `to_string_lossy()` when emitting to the frontend.
This preserves the IPC shape; the lossy conversion happens at one
point, and we annotate it.

**Step B — fix the git output parsers.** `git status --porcelain -z`
emits NUL-separated entries; the file-name bytes after the status
prefix are not necessarily UTF-8. Currently we do
`String::from_utf8_lossy(&output.stdout)` which corrupts these
bytes irrecoverably.

Replace with byte-level parsing:

```rust
let stdout: &[u8] = &output.stdout;
for entry in stdout.split(|&b| b == 0) {
    if entry.len() < 4 { continue; }
    let index_status = entry[0];
    let wt_status = entry[1];
    let file_bytes = &entry[3..];
    // Only stringify here, after we've extracted the key.
    let file_path = String::from_utf8_lossy(file_bytes).into_owned();
    // ...
}
```

The `.into_owned()` keeps a lossy String for IPC, BUT the original
bytes were not corrupted during status-code extraction. This is the
minimum viable fix; a full fix would require returning bytes to the
frontend, which Tauri doesn't trivially support.

Apply this pattern to:

- `get_git_status`
- `get_git_remote_status`
- `git_discard` (the `git status` parsing inside)

For commands that take file paths from the frontend and pass them to
`git`, the path comes in as `String`. If the frontend's path was
obtained via earlier lossy conversion, we have a self-consistent
round trip and operations still work — they just can't represent the
truly-non-UTF8 case. Document this limitation in `docs/limitations.md`.

**Step C — PTY reader.** The existing UTF-8-boundary walk in
`shell::spawn_terminal` is correct and stays. Add a comment
explaining why.

### 5.4 CSP tightening (M9)

File: `src-tauri/tauri.conf.json`.

Current `frame-src`:

```
frame-src 'self' asset: https://asset.localhost http://localhost:* http://127.0.0.1:* https://*
```

The trailing `https://*` is the wildcard. The Preview component's
job is to load *user-specified* dev URLs, which can be HTTPS in the
wild (e.g. ngrok tunnels). We replace the wildcard with a runtime
allow-list:

- Strict default: only `'self' asset: https://asset.localhost
  http://localhost:* http://127.0.0.1:* https://localhost:*
  https://127.0.0.1:*`.
- For non-localhost previews, the user must approve in the URL bar
  (UI work in Group 07). Implementation in this group:
  - Add a `previewAllowList` persisted setting (default: empty).
  - On Preview load attempt, if the URL is not localhost AND not in
    the allow-list, show a confirmation prompt: "Allow leo to
    display content from `<host>`? [Once] [Always] [Cancel]".
  - "Always" appends to the persisted list.

Until the prompt UI is implemented, retain the wildcard so the
preview pane keeps working. Commit the strict CSP only AFTER the
prompt UI lands. Spec it here, ship it via Group 07.

### 5.5 Filesystem scope correction (M10)

File: `src-tauri/capabilities/default.json`.

Current scope is a hardcoded list. Replace with `$HOME/**`. Rationale:

- The actual containment is enforced by `validate_path()` in the
  Rust `fs` module, which canonicalizes and checks `starts_with(root)`.
  The Tauri scope is only the *outer* envelope.
- The current list excludes `~/dev`, `~/work`, `~/`, etc. — many real
  user setups. Users currently can't open these projects.
- `$HOME/**` is the standard pattern for an IDE; VSCode, Zed, Xcode
  do not impose tighter outer envelopes.
- We keep `$APPDATA/**` and `/Volumes/**` for plugin data and
  external drives respectively.
- We keep `$HOME/.leo-ide/**` explicitly because some plugins might
  not consider it "part of $HOME" depending on platform symlinks.

Note: this is a security *trade-off*, not a strict tightening.
Document explicitly in the PR. The mitigation is that
`validate_path` continues to enforce per-project containment, so the
broader Tauri scope is invisible to user code.

**Test plan:**

- Open a project under `~/` directly (e.g. `~/foo/bar`); confirm it
  loads.
- Confirm `validate_path` still rejects accesses outside the project
  root (Rust unit test already exists from Group 00).
- Confirm `$APPDATA` writes still work (knowledge DB, keys file).

### 5.6 Audit `Command::output()` arg construction

For every `Command::new("git").args([...])` call site:

- Verify no user-controlled string is concatenated into a single arg
  in a way that could be flag-confused. Rust's `args(...)` is
  argv-safe (each entry is one argv element), so the only risk is
  the leading-dash case which §5.2 covers.
- Add a regression test: `git_stage(["--exec=evil"])` should be
  rejected by `validate_git_file_path` (it already is, since
  `validate_git_file_path` rejects paths starting with `-` via the
  `Component::ParentDir` / absolute-path checks; verify by test).

### 5.7 Migrate logging in this group

Every new branch, error path, and success path in §5.1–§5.6 logs
through the structured logger from Group 00 at the appropriate level:

- `log::info` for migration events (key migrated to keyring).
- `log::warn` for fallbacks (keyring failed, encrypting to file).
- `log::error` for unexpected errors that surface to the user.

Never log the key value. Never log the encryption key.

## 6. Test plan (cumulative)

In addition to the per-task tests above:

**Frontend tests (Vitest):**

- Settings → Models save/load round-trip via mocked `invoke`.

**Rust unit tests:**

- All ref-name validation cases.
- All path validation cases.
- Encryption round-trip for the key fallback.
- Key migration scenario.

**Manual smoke tests on each platform:**

- macOS: install signed build, verify keys in Keychain only.
- Linux: with libsecret missing, verify graceful fallback to
  encrypted file.
- Windows: verify Credential Manager entry.

**Behavioral baseline replay** (from Group 00 §4.2): every step
identical.

## 7. Code review checklist

- [ ] No code path writes plaintext keys to disk when keyring is
      available.
- [ ] Tests prove the encrypted file is binary-different from
      plaintext for a known input key.
- [ ] No git command accepts a branch/ref name starting with `-` or
      containing forbidden characters. Negative tests cover every
      character class from `git-check-ref-format(1)`.
- [ ] No `String::from_utf8_lossy` on git stdout BEFORE the
      status-code extraction. Audit every `from_utf8_lossy` call site
      and confirm it's at the IPC boundary or unavoidable.
- [ ] Tauri capability scope expansion is documented in the PR
      description with the security trade-off rationale.
- [ ] CSP changes are gated on the prompt-UI being merged. If
      shipping the strict CSP without the UI, the Preview pane must
      gracefully degrade (show "URL not allowed; click to enable").
- [ ] No new `unwrap()` / `expect()` on user-controlled input.
- [ ] All new code paths log at the appropriate level via the Group
      00 logger. No `console.*` calls.
- [ ] Behavioral baseline replay passes (screenshot diff or manual
      sign-off).

## 8. Rollback

The risky changes are §5.1 (key storage) and §5.5 (FS scope).
Rollback strategies:

- **§5.1:** keep a `~/.leo-ide/keys.backup.json` snapshot taken at
  migration time. If users report missing keys after upgrade, a
  release-note one-liner restores: `cp keys.backup.json keys.json
  && relaunch`. The migration code path also has a feature-flag
  env var `LEO_DISABLE_KEY_MIGRATION=1` for emergency disable.
- **§5.5:** revert the capabilities JSON. Users who relied on the
  expanded scope go back to the previous behavior.
- Other changes are pure validation tightenings — revert reverts.

## 9. Out of scope

- **Code-signing automation.** macOS keyring fallback is more reliable
  with signed binaries; setting up signing is a separate ops task.
- **Telemetry of failed keyring writes.** Local logs are enough for
  now (Group 00).
- **Per-provider key rotation UI.** Out of scope; current Settings UI
  supports only set/clear.
- **CSP `script-src` review.** Currently `'self'` only; tight already.
- **Tauri capability split per-window.** Settings window inherits
  defaults; a stricter capability for it would be future hardening.

## 10. Notes for the implementing agent

- `chacha20poly1305` requires a 32-byte key and a 12-byte nonce. Use
  `OsRng` (already a transitive dep via `rand_core`) for nonce
  generation. Store the nonce alongside the ciphertext in the
  fallback file (concatenate, prefix with a 1-byte version tag for
  future migrations).
- The migration in §5.1 must be idempotent: running it twice is a
  no-op.
- When changing `tauri.conf.json` scope, also update
  `capabilities/default.json` in the same commit — they're paired.
- Group 00's logger is the only allowed observability tool. Do not
  introduce `eprintln!` / `dbg!` / `panic!` in production code paths.
- If during implementation you discover a finding not in the audit
  table (Cn/Hn/Mn/Ln), surface it back; do NOT expand scope to fix
  it here.
