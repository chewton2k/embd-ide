# Agentic Feature Roadmap

Ideas for evolving **leo-IDE** into a first-class agentic coding environment while staying a great plain code editor for users who don't want AI.

## Current state (what's already built)

- Streaming chat (`sendStreamingMessage`) with OpenRouter / OpenAI / Anthropic providers
- Floating draggable chat window (`FloatingChat.svelte`) + panel chat (`ChatPanel.svelte`)
- Ghost-text inline autocomplete (`ghostText.ts`)
- Structured edit parsing (```` ```edit:<file>:<start>-<end> ````), pending-edit store, inline approve/reject
- AI diff extension + diff toolbar in the editor
- Agent loop with `read_file`, `search`, `edit`, `run` tools and dangerous-command detection
- Attach files for context, @-style file injection
- Knowledge indexer (`contextBuilder.ts` → `knowledge_get_context`)
- Conversation persistence (SQLite via `knowledge_save_conversation`)
- AI change history (`recordAiChange`)
- Agents / Knowledge / Models settings sections
- File dependency graph (`FileDiagram.svelte` + Rust `graph` module)
- CodeMirror 6 editor, xterm.js terminals, git panel, merge editor, preview panel

The bones are in place. The roadmap below is about hardening the agent loop, raising context quality, and making it all optional for the non-AI audience.

---

## 2. Tighten the agent loop

`agentLoop.ts` uses regex to parse tool calls against one non-streaming response per step. That works but leaves a lot on the table.

- **Structured tool calls instead of markdown regex.** Use OpenAI/Anthropic/OpenRouter native tool-calling (JSON `tool_calls`), so you stop misparsing triple-backticks inside model output and get reliable `read_file`, `edit`, `run`, `search`, `grep`, `list_dir`, `get_diagnostics`, `apply_patch` tools. The `DANGEROUS_PATTERNS` checks become per-tool permission rules rather than regex on a string.
- **Stream tool-using agent turns.** The agent currently uses blocking `invoke('ai_chat', ...)` while chat uses `ai_chat_stream`. Unifying them gives a live view of the agent "thinking" and cheap cancellation mid-step.
- **Plan / Act split.** Add a "plan" stage where the model proposes an ordered step list shown as a checklist in chat. The user can edit/reorder steps before the agent executes. Biggest trust win — users see what it's about to do, not just what it did.
- **Checkpoints & rollback.** Before applying an edit, snapshot the file (or write a scratch commit to a hidden `refs/leo/checkpoints/*` ref). "Undo agent run" reverts an entire session in one click. `recordAiChange` already exists — promote it into a first-class session/checkpoint browser.
- **Self-verify step.** After any `edit`, automatically run the project's type-checker / linter (config-driven: `npm run check`, `cargo check`, `tsc --noEmit`) and feed errors back to the model until clean or N iterations. Dramatically raises perceived agent competence.
- **Subagents** — long tasks decomposed into parallel isolated agents (research + implement + review). Existing agent store + a "runs" list covers the UI.

## 3. Context quality

Most agent magic actually comes from context, not the model.

- **Symbol-level retrieval, not just file-level.** `buildProjectContext` returns files + exports. A tree-sitter or LSP-backed index that returns *just the function body* the user is asking about cuts tokens 10× and improves quality. Add a Rust-side `tree-sitter` index the knowledge store can query.
- **LSP integration** on the Rust side. Even a minimal JSON-RPC client to `rust-analyzer`, `typescript-language-server`, `pyright` gives diagnostics, go-to-definition, references, hover — exposed to the user (as a real IDE) *and* to the agent (as tools). Single biggest feature that elevates it from "editor + chatbot" to "IDE."
- **Live dependency graph as context.** `FileDiagram.svelte` / `graph` Rust module already exist. Let the agent call `get_neighbors(file)` to pull files a change actually touches.
- **Repo memory file.** Persist a `docs/superpowers/specs/*`-style memory (that dir already exists) and auto-load into every agent's system prompt — like `CLAUDE.md` / `.cursorrules`. The agent can maintain it ("update memory with what you learned").
- **@-mentions in chat.** `@file`, `@symbol`, `@folder`, `@terminal`, `@diff`, `@error` pickers that inject structured context. `attachedFiles` is already there — extend to symbols/ranges.

## 4. In-editor agentic features (serves both audiences)

- **Inline `Cmd+K` ("edit this").** Select code, press a key, describe the change in a small popover, preview diff inline, accept/reject. The single feature most agentic-IDE users reach for.
- **Next-edit prediction.** After an edit lands, predict the likely next edit (often in a different file) and show as a ghost suggestion. `ghostText.ts` already does single-line; extend to multi-location.
- **Inline "explain this" / "write a test for this"** as code actions on selection — transient popover, no chat roundtrip.
- **Smart rename / refactor** — LSP first, AI fallback (rename a concept across files where symbol names differ).
- **Auto-fix on save** (opt-in) — run linters + a small AI pass to fix trivial errors.
- **Commit message generator** in the git panel: one button, uses `git diff --staged`.

## 5. Trust, safety, observability

- **Permission scopes per agent.** Each agent config (`AgentsSection.svelte` already exists) declares its capabilities: `read`, `edit`, `run:safe`, `run:any`, `network`. Enforced by the tool dispatcher.
- **Sandboxed `run` tool.** Current `run` pipes into the first open terminal and sleeps 2s — fragile and bypasses output capture. A dedicated backend `run_sandboxed(command, cwd, timeout)` that streams stdout/stderr/exit back to the agent is much cleaner.
- **Session viewer.** Timeline per agent run: prompt → plan → each tool call (args + output) → final diff. Replay, fork, or share a run.
- **Cost + token meter** per conversation and per project.
- **Redaction rules.** Pattern-based scrubbers for env files, keys, secrets before anything is sent upstream.

## 6. Model flexibility

- **Local-model provider** via Ollama or LM Studio as a first-class provider (currently `openrouter | openai | anthropic`; add `local`). Big draw for the editor-first crowd who still want offline autocomplete. Ghost-text is a perfect fit for a small local model.
- **Model routing policy** — cheap/fast model for autocomplete & classification, strong model for planning & edits. Configurable per-project.
- **MCP (Model Context Protocol) support.** Tauri backend acts as MCP host; users plug in servers they already run (GitHub, Postgres, Linear, Jira, Figma) and every agent inherits those tools. Future-proofs the agent system.

## 7. Smaller refinements with high ROI

- **Diff UI polish.** `aiDiffExtension` + `DiffToolbar` are solid — add per-hunk accept/reject (not just per-edit) and a keyboard loop (`n` next hunk, `y` accept, `r` reject).
- **"Ask about this error."** Click a diagnostic or terminal error → chat pre-filled with the error, stack trace, and surrounding code.
- **Knowledge graph in UI**, not just settings. Sidebar panel showing what the agent currently thinks is in scope, with a "this is wrong, ignore these" action. Already flagged in `todo.md`.
- **Background indexing status** in the status bar. Users need to know "agent is cold" vs "agent has indexed 342 files."
- **Agent presets** packaging (model + system prompt + tool permissions + memory): "Refactor agent," "Test writer," "Doc writer," "Security reviewer." `AgentsSection.svelte` looks ready for this.
- **Conflict-aware edits.** If a file changed after an edit was proposed, re-anchor by content match rather than line number. `originalCode` is already in `EditProposal` — use it as source of truth instead of line ranges.

---

## Suggested implementation order

1. **LSP integration + `Cmd+K` inline edit** — both audiences feel the upgrade immediately.
2. **Native tool-calling + streaming agent + plan mode** — fixes the brittle regex agent, gives users visibility.
3. **Checkpoints / undo agent run** — single biggest trust feature.
4. **Self-verify (lint/typecheck loop)** — quality compounds once this is in.
5. **Local model provider + MCP** — opens the ecosystem story.
6. **Permission scopes, cost meter, session viewer** — production polish.

---

## References to existing code

| Feature area           | File(s)                                                          |
| ---------------------- | ---------------------------------------------------------------- |
| Agent loop             | `src/lib/modules/ai/agentLoop.ts`                                |
| Edit parsing           | `src/lib/modules/ai/editParser.ts`                               |
| Command parsing        | `src/lib/modules/ai/commandParser.ts`                            |
| Context builder        | `src/lib/modules/ai/contextBuilder.ts`                           |
| System prompts         | `src/lib/modules/ai/systemPrompts.ts`                            |
| Chat / streaming       | `src/lib/modules/stores/ai.ts`                                   |
| Pending edits          | `src/lib/modules/stores/pendingEdits.ts`                         |
| AI change history      | `src/lib/modules/stores/aiHistory.ts`                            |
| Ghost text             | `src/lib/modules/editor/ghostText.ts`                            |
| Diff extension         | `src/lib/modules/editor/aiDiffExtension.ts`                      |
| Chat UI                | `src/lib/components/ai/ChatPanel.svelte`, `FloatingChat.svelte`  |
| File graph             | `src/lib/components/diagram/FileDiagram.svelte`                  |
| Settings               | `src/lib/settings/sections/{Agents,Knowledge,Models}Section.svelte` |
| Rust backend modules   | `src-tauri/src/modules/{ai,knowledge,graph,fs,git,shell,session}` |
