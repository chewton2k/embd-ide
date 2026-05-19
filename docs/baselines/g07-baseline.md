# Group 07 — Pre-flight Baseline

Captured: 2026-05-13

## Build verification

- `cargo check`: ✅ pass
- `svelte-check`: ✅ pass
- Full test suite: ✅ 57 tests (7 Rust + 50 frontend)

## alert() usage (L2)

One `alert()` call in App.svelte line 59:
```ts
alert(`Project folder no longer exists:\n${project.path}`);
```
Jarring native dialog against dark IDE chrome.

## Default model IDs (M12)

Rust `default_model()`:
- openai: `gpt-5-mini` — may not exist as a real model ID
- anthropic: `claude-sonnet-4-6` — may not exist as a real model ID
- openrouter: `openrouter/auto` — correct (meta-model)

Frontend `aiModel` default: `openrouter/auto` — correct.

Frontend MODELS lists reference `gpt-5-mini`, `claude-sonnet-4-6` which may be
hypothetical IDs. Need to verify against actual API model lists.

## Scope decisions

- §5.1 (Toast system): Implement
- §5.2 (Linux file manager D-Bus): Skip — requires zbus crate, Linux-only
- §5.3 (L4 color-mix): Already in G04 scope
- §5.4 (todo.md cleanup): Skip — documentation task
- §5.5 (Recent project bound): Skip — documentation only
- §5.6 (Settings window split): Defer — optimization, not correctness
- §5.7 (Vendor chunk audit): Defer — optimization
- §5.8 (Default model IDs): Implement
- §5.9 (Preview allow-list): Defer — requires full UI component
