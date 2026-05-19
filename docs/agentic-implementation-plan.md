# Agentic Features — Implementation Plan

## Overview

This plan breaks the agentic features roadmap into 8 implementation groups, ordered by dependency chain and user impact. Each group is independently shippable and revertable.

---

## Group 0: Cmd+K Inline Edit

**Why first:** Self-contained, no backend changes, highest user-visible ROI. Builds on existing `aiDiffExtension`, `pendingEdits`, and streaming infrastructure.

### Scope
- Selection-based inline edit popover (Cmd+K / Ctrl+K)
- Small input field appears above selection
- User types instruction → streams response → shows inline diff
- Accept/reject controls (same as existing pending edits)

### Files touched
- `src/lib/components/editor/Editor.svelte` — keybinding + popover mount
- `src/lib/modules/ai/inlineEdit.ts` (new) — orchestrates the request
- `src/lib/components/editor/InlineEditPopover.svelte` (new) — UI

### Dependencies
- Existing: `aiDiffExtension.ts`, `pendingEdits.ts`, `ai_chat_stream` backend command
- No new packages, no backend changes

### Tests
- Unit: `inlineEdit.ts` — prompt construction, response parsing
- Integration: popover open/close lifecycle

---

## Group 1: Native Tool-Calling + Streaming Agent

**Why second:** Fixes the brittle regex-based agent loop. Foundation for all subsequent agent features.

### Scope
- Replace markdown regex tool parsing with native JSON `tool_calls` (OpenAI/Anthropic format)
- Unify agent loop to use `ai_chat_stream` (not blocking `ai_chat`)
- Define typed tool schemas: `read_file`, `edit_file`, `run_command`, `search_files`, `list_dir`, `grep`
- Stream agent "thinking" into chat in real-time
- Per-tool permission checks (replaces `DANGEROUS_PATTERNS` regex)

### Files touched
- `src/lib/modules/ai/agentLoop.ts` — full rewrite of the loop
- `src/lib/modules/ai/tools.ts` (new) — tool definitions, schemas, dispatch
- `src/lib/modules/ai/toolPermissions.ts` (new) — permission rules
- `src-tauri/src/modules/ai/mod.rs` — add `tool_calls` support to streaming
- `src/lib/modules/ai/systemPrompts.ts` — remove TOOLS_PROMPT, use native schema

### Dependencies
- Group 0 complete (Cmd+K uses the same streaming path)
- OpenRouter/OpenAI/Anthropic all support `tools` parameter

### Tests
- Unit: tool schema validation, permission checks, tool dispatch
- Unit: streaming tool-call parsing (SSE delta accumulation for tool args)
- Integration: mock agent run with tool calls

---

## Group 2: Plan / Act Split

**Why third:** Biggest trust win. Users see what the agent will do before it does it.

### Scope
- Agent first produces a plan (ordered step list) shown as a checklist in chat
- User can edit/reorder/remove steps before execution
- "Execute plan" button starts the agent loop with the approved steps
- Plan stored as structured data, not just text

### Files touched
- `src/lib/modules/ai/agentLoop.ts` — plan phase before act phase
- `src/lib/modules/ai/agentPlan.ts` (new) — plan store, step types
- `src/lib/components/ai/PlanView.svelte` (new) — checklist UI
- `src/lib/modules/ai/systemPrompts.ts` — plan-mode system prompt

### Dependencies
- Group 1 complete (native tool-calling makes plan steps well-typed)

### Tests
- Unit: plan parsing, step reordering, step removal
- Unit: plan → execution mapping
- Integration: plan approval flow

---

## Group 3: Checkpoints & Undo Agent Run

**Why fourth:** Single biggest trust feature. Users can always go back.

### Scope
- Before any agent edit, snapshot affected files (git stash-like)
- Store checkpoints as hidden git refs (`refs/leo/checkpoints/<session-id>`)
- "Undo agent run" reverts entire session in one click
- Promote `aiChangeHistory` into a first-class session/checkpoint browser
- UI: checkpoint list in chat panel sidebar

### Files touched
- `src-tauri/src/modules/git/mod.rs` — `git_create_checkpoint`, `git_restore_checkpoint`, `git_list_checkpoints`
- `src/lib/modules/ai/checkpoints.ts` (new) — frontend checkpoint management
- `src/lib/modules/ai/agentLoop.ts` — create checkpoint before execution
- `src/lib/components/ai/CheckpointBrowser.svelte` (new) — UI
- `src-tauri/capabilities/default.json` — no new permissions needed (uses existing git)

### Dependencies
- Group 1 complete (agent loop refactored)
- Existing git module handles the heavy lifting

### Tests
- Rust unit: checkpoint create/restore/list
- Frontend unit: checkpoint store lifecycle
- Integration: agent run → checkpoint → revert → verify file state

---

## Group 4: Self-Verify (Lint/Typecheck Loop)

**Why fifth:** Quality compounds. Agent fixes its own mistakes.

### Scope
- After any `edit_file` tool call, run project's type-checker/linter
- Config-driven: detect `package.json` scripts, `Cargo.toml`, `tsconfig.json`
- Feed errors back to the model for up to N iterations (default: 3)
- Show verify status in chat ("✓ No errors" or "⚠ 2 errors, fixing...")

### Files touched
- `src/lib/modules/ai/selfVerify.ts` (new) — verify orchestration
- `src/lib/modules/ai/agentLoop.ts` — hook verify after edit tool
- `src-tauri/src/modules/shell/mod.rs` — `run_command_capture` (new command that captures stdout/stderr without a terminal)
- `src/lib/modules/ai/tools.ts` — add `verify` as internal tool

### Dependencies
- Group 1 complete (tool-calling agent)
- Group 3 recommended (checkpoint before verify loop)

### Tests
- Unit: project type detection (package.json → `npm run check`, Cargo.toml → `cargo check`)
- Unit: error parsing from stdout/stderr
- Integration: edit → verify → fix cycle (mocked)

---

## Group 5: Context Quality — Symbol Retrieval + @-Mentions

**Why sixth:** Dramatically improves agent accuracy by reducing token waste.

### Scope
- **@-mentions in chat:** `@file`, `@symbol`, `@folder`, `@terminal`, `@diff`, `@error` pickers
- **Symbol-level retrieval:** Use tree-sitter (via Rust) to extract function/class bodies
- **Live dependency graph as context:** Agent can call `get_neighbors(file)`
- **Repo memory file:** Auto-load `.leo/memory.md` into agent system prompt

### Sub-groups (can be split further):
- 5a: @-mentions UI (frontend only, extends existing `attachedFiles`)
- 5b: Symbol index (Rust tree-sitter, new `knowledge_get_symbols` command)
- 5c: Dependency graph tool (expose existing `graph` module to agent)
- 5d: Repo memory (simple file read on agent init)

### Files touched
- `src/lib/components/ai/MentionPicker.svelte` (new) — @-mention autocomplete
- `src/lib/modules/ai/contextBuilder.ts` — symbol-level retrieval
- `src-tauri/src/modules/knowledge/mod.rs` — tree-sitter symbol extraction
- `src-tauri/Cargo.toml` — add `tree-sitter` + language grammars
- `src/lib/modules/ai/tools.ts` — `get_symbols`, `get_neighbors` tools
- `src/lib/modules/ai/agentLoop.ts` — load repo memory

### Dependencies
- Group 1 complete (tools infrastructure)
- tree-sitter adds ~2MB to binary (acceptable for an IDE)

### Tests
- Rust unit: symbol extraction for JS/TS/Rust/Python
- Frontend unit: @-mention parsing, context injection
- Integration: agent uses symbol context to produce better edits

---

## Group 6: Local Model Provider + MCP

**Why seventh:** Opens the ecosystem. Offline autocomplete, external tool servers.

### Scope
- **Local model provider:** Ollama / LM Studio as first-class provider
- **Model routing:** cheap model for ghost-text, strong model for edits
- **MCP host:** Tauri backend acts as MCP client, users connect external servers

### Sub-groups:
- 6a: Local provider (add `local` to provider enum, configurable base URL)
- 6b: Model routing (per-task model selection in settings)
- 6c: MCP client (JSON-RPC over stdio/SSE, tool discovery, proxy to agent)

### Files touched
- `src-tauri/src/modules/ai/mod.rs` — local provider endpoint routing
- `src/lib/modules/ai/ai.ts` — provider types, routing logic
- `src/lib/settings/sections/ModelsSection.svelte` — local model config UI
- `src-tauri/src/modules/mcp/mod.rs` (new) — MCP client implementation
- `src/lib/modules/ai/tools.ts` — MCP tool registration

### Dependencies
- Group 1 complete (tool infrastructure for MCP tools)
- No hard dependency on Groups 2-5

### Tests
- Rust unit: local provider request building, MCP handshake
- Frontend unit: model routing logic
- Integration: MCP tool discovery + invocation (mocked server)

---

## Group 7: Permission Scopes, Cost Meter, Session Viewer

**Why last:** Production polish. Everything else works first.

### Scope
- **Permission scopes:** Each agent config declares capabilities (`read`, `edit`, `run:safe`, `run:any`, `network`)
- **Sandboxed run:** Dedicated `run_sandboxed(command, cwd, timeout)` backend command
- **Cost/token meter:** Track tokens per conversation and per project
- **Session viewer:** Timeline per agent run (prompt → plan → tool calls → diff)
- **Redaction rules:** Pattern-based scrubbers for secrets before upstream send

### Files touched
- `src/lib/modules/ai/toolPermissions.ts` — full permission system
- `src-tauri/src/modules/shell/mod.rs` — `run_sandboxed` command
- `src/lib/modules/ai/costTracker.ts` (new) — token/cost accounting
- `src/lib/components/ai/SessionViewer.svelte` (new) — timeline UI
- `src/lib/modules/ai/redaction.ts` (new) — secret scrubbing
- `src/lib/settings/sections/AgentsSection.svelte` — permission config UI

### Dependencies
- Groups 1-4 complete (agent loop, tools, checkpoints)

### Tests
- Unit: permission enforcement, redaction patterns, cost calculation
- Unit: session serialization/deserialization
- Integration: agent blocked by permission → user prompted

---

## Dependency Graph

```
Group 0 (Cmd+K)
    │
    ▼
Group 1 (Native Tool-Calling + Streaming Agent)
    │
    ├──────────────────┐
    ▼                  ▼
Group 2 (Plan/Act)   Group 6a (Local Provider)
    │                  │
    ▼                  ▼
Group 3 (Checkpoints) Group 6b (Model Routing)
    │                  │
    ▼                  ▼
Group 4 (Self-Verify) Group 6c (MCP)
    │
    ▼
Group 5 (Context Quality)
    │
    ▼
Group 7 (Permissions, Cost, Sessions)
```

---

## Estimated Effort

| Group | Effort | Risk | New Packages |
|-------|--------|------|--------------|
| 0 | 1 session | Low | None |
| 1 | 2 sessions | Medium (API format differences) | None |
| 2 | 1 session | Low | None |
| 3 | 1 session | Medium (git edge cases) | None |
| 4 | 1 session | Low | None |
| 5 | 2-3 sessions | Medium (tree-sitter integration) | `tree-sitter`, grammars |
| 6 | 2-3 sessions | High (MCP spec complexity) | None (JSON-RPC is manual) |
| 7 | 2 sessions | Low | None |

---

## Preservation Guarantees (per group)

Every group must maintain:
1. ✅ Existing chat streaming works unchanged
2. ✅ Ghost-text autocomplete unaffected
3. ✅ File save/load cycle intact
4. ✅ Git operations unchanged
5. ✅ Settings persist across windows
6. ✅ All existing tests pass (112 frontend + 42 Rust)
7. ✅ Bundle size increase < 500KB per group (excluding tree-sitter in G5)
8. ✅ No new required user configuration (features are opt-in)

---

## Ready to Start

Confirm which group to begin with, or say "start Group 0" to proceed.
