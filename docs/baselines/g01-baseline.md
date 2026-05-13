# Group 01 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass
- `svelte-check --threshold error`: ✅ pass
- `npx vitest run`: ✅ 48 tests passing

## Current git ref-name validation

In `git_delete_branch` and `git_checkout_branch`:
- Only rejects `..` and space characters
- Does NOT reject: leading `-`, control chars, `\`, `:`, `?`, `*`, `[`, `~`, `^`, NUL, `.lock` suffix, `@{`, `//`, `/.`

## Current filesystem scope

Hardcoded list in `capabilities/default.json`:
- `$APPDATA/**`
- `$HOME/.leo-ide/**`
- `$HOME/Desktop/**`, `$HOME/Documents/**`, `$HOME/Projects/**`
- `$HOME/Developer/**`, `$HOME/repos/**`, `$HOME/src/**`, `$HOME/code/**`
- `/Volumes/**`

Users with projects in `~/dev`, `~/work`, `~/`, etc. cannot open them.

## Current Command::output() arg construction

All git commands use `Command::new("git").args([...])` with separate argv elements.
The only risk is leading-dash injection, which `validate_git_file_path` partially covers
(rejects absolute paths and `..` but does NOT explicitly reject leading `-`).

## Scope decisions for this group

- §5.1 (Keychain-only storage): Implement
- §5.2 (Git ref-name validation): Implement
- §5.3 (OS-string path handling): Implement (byte-level git status parsing)
- §5.4 (CSP tightening): Deferred to Group 07 per spec (needs prompt UI first)
- §5.5 (Filesystem scope): Implement
- §5.6 (Audit arg construction): Implement (add leading-dash rejection + test)
- §5.7 (Logging migration): Implement
