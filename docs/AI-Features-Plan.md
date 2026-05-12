# AI Features — Implementation Plan

## Overview

Transform leo-IDE from a lightweight code editor into an agentic AI-powered IDE with persistent memory, inline code editing with diff review, a floating chat window, and autonomous multi-step execution.

### Design Principles

- Stay lightweight (~20MB target, SQLite adds ~2MB)
- All AI features are opt-in and non-blocking
- User always has final say (approve/reject) before code changes
- Works with multiple providers (OpenRouter, OpenAI, Anthropic)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Frontend (Svelte 5)                      │
├──────────┬──────────┬───────────────┬───────────────────────┤
│ Floating │ Editor   │ File Context  │ Terminal Agent         │
│ Chat     │ Diff     │ Picker        │ Integration           │
│ Window   │ Overlay  │               │                       │
├──────────┴──────────┴───────────────┴───────────────────────┤
│                    AI Service Layer (TS)                      │
│  (streaming, tool calls, conversation management)            │
├─────────────────────────────────────────────────────────────┤
│                      Tauri IPC                               │
├─────────────────────────────────────────────────────────────┤
│                    Backend (Rust)                             │
├──────────┬──────────┬───────────────┬───────────────────────┤
│ AI Chat  │ Knowledge│ File Indexer  │ Agent Executor        │
│ (stream) │ Store    │               │ (apply edits, run     │
│          │ (SQLite) │               │  terminal cmds)       │
└──────────┴──────────┴───────────────┴───────────────────────┘
```

### New Rust Dependencies

| Crate | Purpose | Size Impact |
|-------|---------|-------------|
| `rusqlite = "0.31"` | Knowledge store (SQLite) | ~1-2MB |
| `sha2 = "0.10"` | File content hashing | minimal |
| `tokio-util = "0.7"` | CancellationToken for streaming | minimal |
| `eventsource-stream = "0.2"` | SSE parsing for streaming | minimal |

No new frontend dependencies needed (marked, dompurify already installed).

---

## Phase 1: Foundation — Streaming + Floating Chat

**Timeline: 2-3 weeks**

### Task 1: Streaming AI Responses

Replace the blocking `ai_chat` command with real-time token streaming.

**Backend (Rust):**
- Add `ai_chat_stream` Tauri command that spawns a tokio task
- Use `reqwest` with streaming response body
- Parse SSE for OpenAI-compatible APIs, Anthropic streaming format
- Emit `ai-stream-chunk` events: `{ session_id: String, delta: String, done: bool }`
- Add `ai_chat_cancel` command using `tokio_util::sync::CancellationToken`

**Frontend (Svelte):**
- Listen to `ai-stream-chunk` events, append deltas to current message
- Show typing indicator while streaming
- "Stop" button to cancel mid-stream

**Files to create/modify:**
- `src-tauri/src/modules/ai/stream.rs` (new)
- `src-tauri/src/modules/ai/mod.rs` (add stream command)
- `src/lib/modules/stores/ai.ts` (streaming state)

---

### Task 2: Multi-turn Conversation

Send full conversation history instead of single prompt.

**Implementation:**
- Modify `AiRequest` struct to accept `messages: Vec<{role, content}>`
- Frontend maintains conversation state in store
- System prompt as first message, file context injected as user message
- Token estimation: truncate oldest messages when approaching context limit (~128k tokens)

**Files to modify:**
- `src-tauri/src/modules/ai/mod.rs`
- `src/lib/modules/stores/ai.ts`

---

### Task 3: Floating Draggable Chat Window

Replace the fixed sidebar chat with a floating window.

**Implementation:**
- `FloatingChat.svelte` — `position: fixed`, high z-index, rendered at App.svelte level
- Title bar with drag handler (mousedown → track delta → update top/left)
- Resize handle on bottom-right corner (min 300x400, max viewport)
- Minimize (collapse to title bar), maximize (fill viewport), close buttons
- Persist position/size in localStorage
- Toggle via Cmd+Shift+L and toolbar button
- Remove old ChatPanel sidebar integration

**Files to create/modify:**
- `src/lib/components/ai/FloatingChat.svelte` (new)
- `src/lib/components/ai/ChatMessages.svelte` (extract from ChatPanel)
- `src/lib/components/ai/ChatInput.svelte` (extract from ChatPanel)
- `src/App.svelte` (render FloatingChat, remove sidebar)
- `src/lib/modules/stores/ai.ts` (window state)

---

### Task 4: File Context Attachment

Let users attach specific files to chat context.

**Implementation:**
- "+" button in chat input opens a file picker (fuzzy search over project files)
- `@filename` syntax in input with autocomplete dropdown
- Attached files shown as removable chips above input
- On send: read file contents via `invoke('read_file_content')`, prepend to messages
- Token count estimate displayed (chars / 4 approximation)
- Limit: warn at >50k tokens of context

**Files to create/modify:**
- `src/lib/components/ai/FileContextPicker.svelte` (new)
- `src/lib/components/ai/ChatInput.svelte`
- `src/lib/modules/stores/ai.ts` (attached files state)

---

## Phase 2: Agentic Code Editing

**Timeline: 3-4 weeks**

### Task 5: Structured Edit Parsing

Parse AI responses containing code edit proposals.

**Response format the AI will use:**
```
```edit:src/lib/utils.ts:10-25
// original code here (for matching)
---
// new code here
```​
```

**Implementation:**
- `parseAiEdits(response: string): EditProposal[]`
- ```ts
  interface EditProposal {
    id: string;
    filePath: string;
    startLine: number;
    endLine: number;
    originalCode: string;
    newCode: string;
    description?: string;
    status: 'pending' | 'approved' | 'rejected';
  }
  ```
- Update system prompt to instruct AI to use this format
- Handle both single-file and multi-file responses
- Fallback: if AI returns plain code block, treat as replacement for current selection

**Files to create:**
- `src/lib/modules/ai/editParser.ts` (new)
- `src/lib/modules/ai/systemPrompts.ts` (new)

---

### Task 6: Inline Diff Decorations in CodeMirror

Show proposed changes as colored inline diffs.

**Implementation:**
- CM6 StateField `aiDiffField` holding pending `EditProposal[]`
- Decorations:
  - Deleted lines: red background + strikethrough via `Decoration.mark`
  - Added lines: green background, inserted via `Decoration.widget` (block widget between lines)
  - Gutter: AI icon (sparkle ✦) on lines with pending changes
- Diff regions are read-only (filter transactions that touch them)
- Support multiple simultaneous pending edits in different regions

**Files to create/modify:**
- `src/lib/modules/editor/aiDiffExtension.ts` (new)
- `src/lib/components/editor/Editor.svelte` (load extension)

---

### Task 7: Approve/Reject/Re-prompt Controls

Interactive buttons on each diff block.

**Implementation:**
- Widget decoration at end of each diff block:
  - ✓ Approve — apply edit via CM6 transaction, remove decoration
  - ✗ Reject — remove decoration, no change
  - ↻ Re-prompt — open chat with edit context, ask AI to revise
- Floating toolbar when multiple edits pending: "Accept All (3)" / "Reject All"
- Each approved edit is a single CM6 undo group (Cmd+Z reverts it)
- Keyboard shortcuts: Cmd+Enter (approve focused), Cmd+Backspace (reject focused)

**Files to create/modify:**
- `src/lib/modules/editor/aiDiffExtension.ts`
- `src/lib/components/editor/DiffToolbar.svelte` (new)
- `src/lib/modules/stores/ai.ts` (pending edits state)

---

### Task 8: Multi-file Edit Orchestration

Handle AI responses that edit multiple files.

**Implementation:**
- When AI proposes edits to files not currently open, open them in background tabs
- Summary panel in chat: "AI wants to edit 3 files" with:
  - File list with change counts (+5 / -3 lines)
  - Click to navigate to that file's diffs
  - Next/Prev buttons to cycle through files with pending changes
- Batch approve/reject all changes across files
- Store: `pendingEdits: Map<filePath, EditProposal[]>`

**Files to create/modify:**
- `src/lib/components/ai/MultiFileEditPanel.svelte` (new)
- `src/lib/modules/stores/ai.ts`

---

## Phase 3: Knowledge Graph Brain

**Timeline: 2-3 weeks**

### Task 9: SQLite Knowledge Store

Persistent local database for project knowledge.

**Schema:**
```sql
CREATE TABLE files (
  path TEXT PRIMARY KEY,
  hash TEXT NOT NULL,
  language TEXT,
  size INTEGER,
  last_indexed INTEGER,
  summary TEXT,
  exports TEXT  -- JSON array of exported symbols
);

CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  title TEXT,
  created_at INTEGER,
  updated_at INTEGER,
  messages TEXT  -- JSON array of {role, content, timestamp}
);

CREATE TABLE patterns (
  id TEXT PRIMARY KEY,
  description TEXT,
  file_paths TEXT,  -- JSON array
  created_at INTEGER
);

CREATE TABLE project_meta (
  key TEXT PRIMARY KEY,
  value TEXT
);
```

**Implementation:**
- DB location: `~/.leo-ide/knowledge/{project_path_hash}.db`
- Tauri commands: `knowledge_init`, `knowledge_query`, `knowledge_upsert_file`, `knowledge_get_context`
- Auto-create on project open, run migrations on schema changes
- Add `rusqlite` to Cargo.toml with `bundled` feature

**Files to create:**
- `src-tauri/src/modules/knowledge/mod.rs` (new)
- `src-tauri/src/modules/knowledge/schema.rs` (new)
- `src-tauri/src/modules/knowledge/queries.rs` (new)

---

### Task 10: Codebase Indexing

Index project files for AI context retrieval.

**Implementation:**
- On project open: walk file tree (respect .gitignore), hash each file with SHA-256
- Store: path, language (from extension), size, content hash, last modified
- Background tokio task with progress events: `indexing-progress { done: u32, total: u32 }`
- On file save: re-hash, update if changed
- Per-file summary extraction (lightweight, no full AST):
  - JS/TS: regex for `export`, `function`, `class`, `interface`
  - Rust: regex for `pub fn`, `pub struct`, `pub enum`, `mod`
  - Python: regex for `def`, `class`, top-level assignments
- Project-level metadata: main language, framework (detect package.json scripts, Cargo.toml), directory structure summary
- Skip: node_modules, .git, dist, build, binary files

**Files to create/modify:**
- `src-tauri/src/modules/knowledge/indexer.rs` (new)
- `src-tauri/src/modules/knowledge/summarizer.rs` (new)

---

### Task 11: Context-aware AI Prompts

Auto-enrich prompts with relevant project context.

**Implementation:**
- Before sending a message, query knowledge store for relevant files:
  1. Files explicitly mentioned by name in user's message
  2. Files imported by the currently open file
  3. Files in the same directory as the active file
  4. Files matching keywords in the user's question
- Inject "Project Context" section into system prompt:
  ```
  ## Project Context
  Language: TypeScript (Svelte + Tauri)
  Structure: src/lib/components/, src/lib/modules/, src-tauri/src/
  
  ### Relevant Files:
  - src/lib/modules/stores/ai.ts: exports aiMessages, streamingState, pendingEdits
  - src/lib/components/ai/FloatingChat.svelte: floating chat window component
  ```
- Limit injected context to ~2000 tokens
- Show indicator in chat: "Context: auth.ts, stores.ts, +1 more"

**Files to create/modify:**
- `src/lib/modules/ai/contextBuilder.ts` (new)
- `src-tauri/src/modules/knowledge/queries.rs`

---

### Task 12: Conversation Persistence & Recall

Save and browse past conversations.

**Implementation:**
- Auto-save conversation to SQLite when ≥2 messages (debounced 2s after last message)
- Title = first user message (truncated to 60 chars)
- "History" button in chat header → dropdown/panel with past conversations
- Click to load and continue a conversation
- Include brief summary of last 3 conversations in system prompt for continuity
- `/clear` command to start fresh conversation
- `/forget` command to delete all conversation history for project

**Files to create/modify:**
- `src/lib/components/ai/ChatHistory.svelte` (new)
- `src/lib/modules/stores/ai.ts`
- `src-tauri/src/modules/knowledge/queries.rs`

---

## Phase 4: Agent Execution & Terminal Integration

**Timeline: 2-3 weeks**

### Task 13: Terminal Command Execution

Let the AI propose and execute terminal commands.

**Response format:**
```
```run
npm install express
```​
```

**Implementation:**
- Parse `CommandProposal = { command: string, description?: string }`
- Display in chat with "▶ Run" / "Skip" buttons
- On Run: write command to active terminal via `invoke('write_terminal', { id, data: command + '\n' })`
- Capture terminal output (listen to `terminal-output-{id}` events for 5s after execution)
- Feed output back to AI for follow-up reasoning
- Safety: dangerous command detection (rm -rf, git push --force, drop table, etc.) → always require explicit confirmation with warning badge

**Files to create/modify:**
- `src/lib/modules/ai/commandParser.ts` (new)
- `src/lib/components/ai/CommandProposal.svelte` (new)
- `src/lib/modules/ai/dangerousCommands.ts` (new — pattern list)

---

### Task 14: AI Undo History & Change Tracking

Separate undo stack for AI-made changes.

**Implementation:**
- Store: `aiChangeHistory: { id, timestamp, description, filePath, beforeContent, afterContent }[]`
- Push to stack when an AI edit is approved
- "AI Changes" panel (toolbar button, sparkle icon):
  - List of recent AI modifications with timestamps
  - Each entry: file name, description, "Revert" button
  - Revert: restore file to `beforeContent`, remove from stack
- Integrate with CM6: AI edits wrapped in `EditorState.changeFilter` annotation so they form a single undo group
- Limit: last 50 changes per session, persist to localStorage

**Files to create/modify:**
- `src/lib/components/ai/AiChangesPanel.svelte` (new)
- `src/lib/modules/stores/ai.ts`
- `src/lib/modules/editor/aiDiffExtension.ts`

---

### Task 15: Agent Loop (Autonomous Multi-step)

AI executes multiple steps autonomously.

**Implementation:**
- "Agent Mode" toggle in chat header (off by default)
- Tool-use protocol — AI can call:
  - `read_file(path)` → returns file content
  - `edit_file(path, edits)` → proposes inline diff
  - `run_command(cmd)` → executes in terminal
  - `search_files(query)` → searches codebase
- Agent loop:
  1. Send user request + tools available
  2. AI responds with tool call or final answer
  3. Execute tool call, feed result back
  4. Repeat until AI sends final answer or max steps reached
- Max 10 steps per run (configurable)
- Progress shown in chat: "Step 3/10: Reading auth.ts..."
- Pause/Stop button to interrupt
- Edits still go through approve/reject unless "Auto-approve" is checked
- Auto-approve has a confirmation dialog: "Agent will apply changes without review. Continue?"

**Files to create/modify:**
- `src/lib/modules/ai/agentLoop.ts` (new)
- `src/lib/modules/ai/tools.ts` (new — tool definitions)
- `src/lib/components/ai/AgentProgress.svelte` (new)
- `src-tauri/src/modules/ai/mod.rs` (tool execution commands)

---

## Phase 5: Polish

**Timeline: 1-2 weeks**

### Task 16: Markdown Rendering in Chat

Render AI responses with proper formatting.

**Implementation:**
- Use existing `marked` to parse markdown
- Sanitize with existing `dompurify`
- Syntax highlight code blocks (reuse Lezer parsers from CodeMirror for JS/TS/Rust/Python)
- "Copy" button on code blocks (top-right corner)
- Render: inline code, bold, italic, headers, lists, links, tables
- Code blocks with `edit:` prefix get an "Apply" button instead of just Copy

**Files to create/modify:**
- `src/lib/components/ai/MessageRenderer.svelte` (new)
- `src/lib/components/ai/CodeBlock.svelte` (new)

---

### Task 17: Inline Ghost Text Completions

AI-suggested completions as grey text after cursor.

**Implementation:**
- Trigger: 500ms after last keystroke (debounced), only in code files
- Context sent to AI: current line, 20 lines above/below, file language
- Use fast/cheap model (configurable in settings, default: small model via OpenRouter)
- Display: CM6 `Decoration.widget` (inline, grey text after cursor position)
- Accept: Tab key applies the suggestion
- Dismiss: Escape, or any other keystroke
- Settings: enable/disable, trigger delay (300-2000ms), model selection
- Don't trigger: in comments, strings, or when autocomplete menu is open

**Files to create/modify:**
- `src/lib/modules/editor/ghostText.ts` (new)
- `src/lib/settings/sections/AiSection.svelte` (new or extend GeneralSection)

---

## Dependency Graph

```
Phase 1:
  Task 1 (Streaming) → Task 2 (Multi-turn)
  Task 3 (Floating Chat) → Task 4 (File Context)

Phase 2 (depends on Phase 1):
  Task 5 (Edit Parsing) → Task 6 (Inline Diffs) → Task 7 (Approve/Reject) → Task 8 (Multi-file)

Phase 3 (can start in parallel with Phase 2):
  Task 9 (SQLite) → Task 10 (Indexing) → Task 11 (Context-aware) → Task 12 (Persistence)

Phase 4 (depends on Phase 2 + 3):
  Task 13 (Terminal Exec) → Task 14 (Undo History) → Task 15 (Agent Loop)

Phase 5 (independent):
  Task 16 (Markdown) — can start after Phase 1
  Task 17 (Ghost Text) — can start after Task 1
```

---

## Summary

| Phase | Focus | Tasks | Timeline |
|-------|-------|-------|----------|
| 1 | Foundation | Streaming, multi-turn, floating chat, file context | 2-3 weeks |
| 2 | Code Editing | Edit parsing, inline diffs, approve/reject, multi-file | 3-4 weeks |
| 3 | Knowledge Brain | SQLite, indexing, context-aware prompts, history | 2-3 weeks |
| 4 | Agent Execution | Terminal commands, undo history, agent loop | 2-3 weeks |
| 5 | Polish | Markdown rendering, ghost text completions | 1-2 weeks |

**Total estimated timeline: 10-15 weeks**

**Binary size impact: +2-3MB (mostly SQLite), staying within ~20MB target.**
