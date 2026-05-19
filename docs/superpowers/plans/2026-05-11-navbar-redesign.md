# Navbar Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the IDE chrome into a full-width toolbar row (sidebar toggle + tabs + search-in-file + action buttons) above the existing sidebar explorer header, with a breadcrumb path replacing action buttons in the statusbar.

**Architecture:** Three store additions (`showChat`, `showGit`, `triggerSearchInFile`) eliminate prop drilling. A new `Toolbar.svelte` renders Row 1 by composing `<Tabs />` and reading store state. `App.svelte` gains local `sidebarVisible` state, drops its own `showChat`/`showGit` locals, and replaces the statusbar action buttons with a reactive breadcrumb. `Editor.svelte` watches `triggerSearchInFile` to open its search panel.

**Tech Stack:** Svelte 5 (runes), TypeScript, CodeMirror 6 (`openSearchPanel` from `@codemirror/search`)

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/lib/stores.ts` | Modify | Add `showChat`, `showGit`, `triggerSearchInFile` stores + `toggleChatPanel`, `toggleGitPanel` helpers |
| `src/lib/Toolbar.svelte` | Create | Row 1: sidebar toggle, `<Tabs/>`, search-in-file input, action buttons |
| `src/lib/Editor.svelte` | Modify | Watch `triggerSearchInFile`, open CodeMirror search panel on change |
| `src/App.svelte` | Modify | Use Toolbar, sidebar visibility gate, breadcrumb statusbar, updated keyboard handlers |

---

### Task 1: Add stores and panel toggle helpers

**Files:**
- Modify: `src/lib/stores.ts`

- [ ] **Step 1: Add the three new stores and two toggle helpers**

Open `src/lib/stores.ts`. After the `export const showSettings = writable<boolean>(false);` line (line 216), add:

```ts
export const showChat = writable<boolean>(false);
export const showGit = writable<boolean>(false);
export const triggerSearchInFile = writable<number>(0);

export function toggleChatPanel() {
  const next = !get(showChat);
  showChat.set(next);
  if (next) showGit.set(false);
}

export function toggleGitPanel() {
  const next = !get(showGit);
  showGit.set(next);
  if (next) showChat.set(false);
}
```

- [ ] **Step 2: Verify the file compiles**

```bash
cd /Users/charltonshih/Desktop/misc/projects/embd && npm run check 2>&1 | head -30
```

Expected: no new type errors.

- [ ] **Step 3: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: add showChat, showGit, triggerSearchInFile stores with toggle helpers"
```

---

### Task 2: Create Toolbar.svelte

**Files:**
- Create: `src/lib/Toolbar.svelte`

- [ ] **Step 1: Create the file**

Create `src/lib/Toolbar.svelte` with the following content:

```svelte
<script lang="ts">
  import Tabs from './Tabs.svelte';
  import { showTerminal, showSettings, autosaveEnabled, showChat, showGit, gitBranch, triggerSearchInFile, toggleChatPanel, toggleGitPanel } from './stores';

  let { sidebarVisible, onToggleSidebar }: {
    sidebarVisible: boolean;
    onToggleSidebar: () => void;
  } = $props();

  function triggerSearch() {
    triggerSearchInFile.update(n => n + 1);
  }
</script>

<div class="toolbar">
  <button
    class="toolbar-btn sidebar-toggle"
    class:active={sidebarVisible}
    onclick={onToggleSidebar}
    title="Toggle sidebar"
  >
    <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
      <rect x="1" y="1" width="4" height="14" rx="1" opacity={sidebarVisible ? 1 : 0.4}/>
      <rect x="7" y="1" width="8" height="14" rx="1"/>
    </svg>
  </button>

  <div class="tabs-wrapper">
    <Tabs />
  </div>

  <div class="toolbar-right">
    <button class="toolbar-search-btn" onclick={triggerSearch} title="Search in file (Cmd+F)">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="12" height="12">
        <circle cx="7" cy="7" r="4.5"/>
        <path d="M10.5 10.5L14 14"/>
      </svg>
      <span>Search in file</span>
    </button>

    <div class="toolbar-divider"></div>

    <button
      class="toolbar-btn"
      class:active={$showTerminal}
      onclick={() => showTerminal.update(v => !v)}
      title="Toggle terminal (Ctrl+`)"
    >
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="13" height="13">
        <rect x="1" y="2" width="14" height="12" rx="2"/>
        <path d="M4 6.5l3 2-3 2"/>
        <path d="M9 10.5h3"/>
      </svg>
    </button>

    <button
      class="toolbar-btn"
      class:active={$showChat}
      onclick={toggleChatPanel}
      title="Toggle AI chat (Ctrl+L)"
    >
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="13" height="13">
        <path d="M14 5H2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1h2v2l3-2h7a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1z"/>
      </svg>
    </button>

    <button
      class="toolbar-btn"
      class:active={$showGit}
      onclick={toggleGitPanel}
      title="Toggle source control (Ctrl+G)"
    >
      <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
        <path d="M14.7 7.3L8.7 1.3a1 1 0 0 0-1.4 0L5.7 2.9l1.8 1.8A1.2 1.2 0 0 1 9 5.9v4.3a1.2 1.2 0 1 1-1-.1V6.1L6.3 7.8a1.2 1.2 0 1 1-.9-.5l1.8-1.8-1.8-1.8L1.3 7.3a1 1 0 0 0 0 1.4l6 6a1 1 0 0 0 1.4 0l6-6a1 1 0 0 0 0-1.4z"/>
      </svg>
      {#if $gitBranch}
        <span class="branch-label">{$gitBranch}</span>
      {/if}
    </button>

    <button
      class="toolbar-btn autosave-btn"
      class:active={$autosaveEnabled}
      onclick={() => autosaveEnabled.update(v => !v)}
      title="Toggle autosave"
    >
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="13" height="13">
        <path d="M13 5l-5 5-2-2"/>
        <rect x="1" y="1" width="14" height="14" rx="2"/>
      </svg>
    </button>

    <button
      class="toolbar-btn gear-btn"
      class:active={$showSettings}
      onclick={() => showSettings.update(v => !v)}
      title="Settings"
    >
      <svg viewBox="0 0 16 14" fill="currentColor" width="13" height="13">
        <path d="M8 1l1.3.8.8-.5 1 1-.5.8.5 1H12.5v1.4l-.8.5.2 1 .9.5-.3 1.2-1 .1-.3 1 .6.8-.7 1.1-1-.3-.7.8.1 1L8 13l-1.3-.8-.8.5-1-1 .5-.8-.5-1H3.5V8.5l.8-.5-.2-1-.9-.5.3-1.2 1-.1.3-1-.6-.8.7-1.1 1 .3.7-.8L6.5 2 8 1zm0 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z"/>
      </svg>
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    height: var(--density-tabs-height, 36px);
    overflow: hidden;
    flex-shrink: 0;
  }

  .tabs-wrapper {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
  }

  /* Remove the tabs-bar's own bottom border — the toolbar provides it */
  .tabs-wrapper :global(.tabs-bar) {
    border-bottom: none;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 8px;
    flex-shrink: 0;
    height: 100%;
  }

  .toolbar-divider {
    width: 1px;
    height: 16px;
    background: var(--border);
    margin: 0 4px;
    flex-shrink: 0;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 11px;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }

  .toolbar-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .toolbar-btn.active {
    color: var(--accent);
  }

  .sidebar-toggle {
    padding: 4px 8px;
    margin-left: 4px;
    border-right: 1px solid var(--border);
    border-radius: 0;
    height: 100%;
  }

  .toolbar-search-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.1s, color 0.1s;
    margin-right: 6px;
  }

  .toolbar-search-btn:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .branch-label {
    font-size: 11px;
    max-width: 80px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .autosave-btn {
    font-size: 10px;
  }
</style>
```

- [ ] **Step 2: Verify the file compiles**

```bash
cd /Users/charltonshih/Desktop/misc/projects/embd && npm run check 2>&1 | head -30
```

Expected: no errors (Toolbar.svelte is not yet used, so it may produce an "unused" warning — that is fine).

- [ ] **Step 3: Commit**

```bash
git add src/lib/Toolbar.svelte
git commit -m "feat: add Toolbar component with sidebar toggle, tabs, search, and action buttons"
```

---

### Task 3: Wire triggerSearchInFile into Editor.svelte

**Files:**
- Modify: `src/lib/Editor.svelte`

- [ ] **Step 1: Add `triggerSearchInFile` to the stores import**

In `src/lib/Editor.svelte`, find this line (line 35):
```ts
import { updateFileContent, markFileSaved, autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers, projectRoot, openFiles, registerFileRenameCallback } from './stores';
```

Replace it with:
```ts
import { updateFileContent, markFileSaved, autosaveEnabled, autosaveDelay, editorFontSize, editorTabSize, editorWordWrap, editorLineNumbers, projectRoot, openFiles, registerFileRenameCallback, triggerSearchInFile } from './stores';
```

- [ ] **Step 2: Add the $effect that opens the search panel**

In `src/lib/Editor.svelte`, find the last `$effect` block before the closing `</script>` (the one that cleans up stateCache, around line 1032). After it, add:

```ts
  $effect(() => {
    const trigger = $triggerSearchInFile;
    if (trigger === 0) return;
    if (!view) return;
    view.focus();
    openSearchPanel(view);
    requestAnimationFrame(() => {
      view?.dom.querySelectorAll('.cm-panel.cm-search input').forEach((input) => {
        input.setAttribute('autocapitalize', 'off');
        input.setAttribute('autocorrect', 'off');
      });
    });
  });
```

- [ ] **Step 3: Verify**

```bash
cd /Users/charltonshih/Desktop/misc/projects/embd && npm run check 2>&1 | head -30
```

Expected: no new errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/Editor.svelte
git commit -m "feat: open CodeMirror search panel when triggerSearchInFile store increments"
```

---

### Task 4: Update App.svelte

**Files:**
- Modify: `src/App.svelte`

This task has multiple steps. Complete them in order.

#### 4a — Update imports and remove local showChat/showGit state

- [ ] **Step 1: Update the stores import line**

Find (line 17):
```ts
import { openFiles, activeFile, activeFilePath, activeFileModified, addFile, autosaveEnabled, projectRoot, gitBranch, showSettings, showTerminal, currentThemeId, getTheme, uiFontSize, uiDensity, apiKey, sharedGitStatus, nextTab, prevTab } from './lib/stores';
```

Replace with:
```ts
import { openFiles, activeFile, activeFilePath, activeFileModified, addFile, autosaveEnabled, projectRoot, gitBranch, showSettings, showTerminal, currentThemeId, getTheme, uiFontSize, uiDensity, apiKey, sharedGitStatus, nextTab, prevTab, showChat, showGit, toggleChatPanel, toggleGitPanel } from './lib/stores';
```

- [ ] **Step 2: Add the Toolbar import and remove the now-unused Tabs import**

Find:
```ts
import Tabs from './lib/Tabs.svelte';
```
Replace with:
```ts
import Toolbar from './lib/Toolbar.svelte';
```

- [ ] **Step 3: Remove the local showChat and showGit state declarations**

Find and delete these two lines (around line 57-58):
```ts
  let showChat = $state(false);
  let showGit = $state(false);
```

- [ ] **Step 4: Remove the toggleChat and toggleGit functions**

Find and delete:
```ts
  function toggleChat() {
    showChat = !showChat;
    if (showChat) showGit = false;
  }

  function toggleGit() {
    showGit = !showGit;
    if (showGit) showChat = false;
  }
```

- [ ] **Step 5: Add sidebarVisible state and toggleSidebar function**

After the `let terminalHeight = $state(220);` line, add:
```ts
  let sidebarVisible = $state(true);

  function toggleSidebar() {
    sidebarVisible = !sidebarVisible;
  }
```

- [ ] **Step 6: Update handleKeydown to use the new store-based toggle helpers**

Find these two keyboard handler blocks inside `handleKeydown`:
```ts
    if ((e.metaKey || e.ctrlKey) && e.key === 'l') {
      e.preventDefault();
      toggleChat();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'g') {
      e.preventDefault();
      toggleGit();
    }
```

Replace with:
```ts
    if ((e.metaKey || e.ctrlKey) && e.key === 'l') {
      e.preventDefault();
      toggleChatPanel();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'g') {
      e.preventDefault();
      toggleGitPanel();
    }
```

#### 4b — Update the HTML template

- [ ] **Step 7: Add breadcrumb derived computation**

After the `let appVersion = $state('');` line, add:
```ts
  let breadcrumbSegments = $derived.by(() => {
    const path = $activeFilePath;
    const root = $projectRoot;
    if (!path) return [];
    if (root && path.startsWith(root + '/')) {
      const rel = path.slice(root.length + 1);
      return [root.split('/').pop() || root, ...rel.split('/')];
    }
    return [path.split('/').pop() || path];
  });
```

- [ ] **Step 8: Replace the outer layout structure**

Find the HTML block starting with `<div class="ide-layout">` (line 242):
```html
<div class="ide-layout">
  <div class="ide-top">
    <div class="sidebar" style="width: {sidebarWidth}px">
```

Replace with:
```html
<div class="ide-layout">
  <Toolbar {sidebarVisible} onToggleSidebar={toggleSidebar} />
  <div class="ide-top">
    {#if sidebarVisible}
    <div class="sidebar" style="width: {sidebarWidth}px">
```

- [ ] **Step 9: Close the sidebarVisible conditional around the sidebar and its resize handle**

Find the sidebar resize handle line (just after the FileTree closing tag):
```html
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="resize-handle resize-handle-col" onmousedown={startDrag('sidebar')}></div>
```

Replace with:
```html
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="resize-handle resize-handle-col" onmousedown={startDrag('sidebar')}></div>
    {/if}
```

- [ ] **Step 10: Remove `<Tabs />` from the editor-area**

Find this block inside `editor-area`:
```html
      <div class="editor-area" style="flex: 1; min-height: 0;">
        <Tabs />
        <div class="editor-container">
```

Replace with:
```html
      <div class="editor-area" style="flex: 1; min-height: 0;">
        <div class="editor-container">
```

- [ ] **Step 11: Update the chat panel condition**

Find:
```html
    {#if showChat}
```
Replace with:
```html
    {#if $showChat}
```

- [ ] **Step 12: Update the chat panel close button handler**

Find:
```html
          <button onclick={toggleChat}>✕</button>
```
Replace with:
```html
          <button onclick={toggleChatPanel}>✕</button>
```

- [ ] **Step 13: Update the git panel condition**

Find:
```html
    {#if showGit}
```
Replace with:
```html
    {#if $showGit}
```

- [ ] **Step 14: Update the git panel close button handler**

Find:
```html
          <button onclick={toggleGit}>✕</button>
```
Replace with:
```html
          <button onclick={toggleGitPanel}>✕</button>
```

- [ ] **Step 15: Replace the statusbar left content with breadcrumb**

Find the entire statusbar-left div:
```html
    <div class="statusbar-left">
      <button onclick={() => showSettings.update(v => !v)} class="statusbar-btn gear-btn" title="Settings">
        <svg viewBox="0 0 16 14" fill="currentColor" width="13" height="13">
          <path d="M8 1l1.3.8.8-.5 1 1-.5.8.5 1H12.5v1.4l-.8.5.2 1 .9.5-.3 1.2-1 .1-.3 1 .6.8-.7 1.1-1-.3-.7.8.1 1L8 13l-1.3-.8-.8.5-1-1 .5-.8-.5-1H3.5V8.5l.8-.5-.2-1-.9-.5.3-1.2 1-.1.3-1-.6-.8.7-1.1 1 .3.7-.8L6.5 2 8 1zm0 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z"/>
        </svg>
      </button>
      {#if $gitBranch}
        <button class="statusbar-btn statusbar-branch" onclick={toggleGit}>
          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
            <path d="M14.7 7.3L8.7 1.3a1 1 0 0 0-1.4 0L5.7 2.9l1.8 1.8A1.2 1.2 0 0 1 9 5.9v4.3a1.2 1.2 0 1 1-1-.1V6.1L6.3 7.8a1.2 1.2 0 1 1-.9-.5l1.8-1.8-1.8-1.8L1.3 7.3a1 1 0 0 0 0 1.4l6 6a1 1 0 0 0 1.4 0l6-6a1 1 0 0 0 0-1.4z"/>
          </svg>
          {$gitBranch}
        </button>
      {/if}
      <button onclick={toggleTerminal} class="statusbar-btn">
        {$showTerminal ? 'Hide' : 'Show'} Terminal 
      </button>
      <button onclick={toggleChat} class="statusbar-btn">
        | {showChat ? 'Hide' : 'Show'} AI 
      </button>
      <button onclick={toggleGit} class="statusbar-btn">
        | {showGit ? 'Hide' : 'Show'} Git 
      </button>
      <button onclick={() => autosaveEnabled.update(v => !v)} class="statusbar-btn autosave-btn">
        | {$autosaveEnabled ? 'ON Autosave' : 'OFF Autosave'} |
      </button>
    </div>
```

Replace with:
```html
    <div class="statusbar-left">
      {#if breadcrumbSegments.length > 0}
        <div class="breadcrumb">
          {#each breadcrumbSegments as seg, i}
            <span class="breadcrumb-seg">{seg}</span>
            {#if i < breadcrumbSegments.length - 1}
              <span class="breadcrumb-sep">›</span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
```

#### 4c — Update CSS

- [ ] **Step 16: Update the ide-layout grid to add the toolbar row**

Find in the `<style>` block:
```css
  .ide-layout {
    display: grid;
    grid-template-rows: 1fr var(--density-statusbar-height, 24px);
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
```

Replace with:
```css
  .ide-layout {
    display: grid;
    grid-template-rows: var(--density-tabs-height, 36px) 1fr var(--density-statusbar-height, 24px);
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
```

- [ ] **Step 17: Add breadcrumb styles to the App.svelte style block**

At the bottom of the `<style>` block, before the closing `</style>`, add:

```css
  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    min-width: 0;
    overflow: hidden;
  }

  .breadcrumb-seg {
    background: color-mix(in srgb, var(--bg-tertiary) 50%, transparent);
    padding: 1px 7px;
    border-radius: 10px;
    white-space: nowrap;
    font-size: 11px;
    font-weight: 500;
    max-width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--bg-tertiary);
  }

  .breadcrumb-sep {
    opacity: 0.55;
    font-size: 10px;
    flex-shrink: 0;
    color: var(--bg-tertiary);
  }
```

- [ ] **Step 18: Remove now-unused statusbar CSS rules**

Find and delete these CSS rules from the style block (they were for the old statusbar buttons that no longer exist):

```css
  .statusbar-btn {
    color: var(--bg-tertiary);
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .statusbar-btn:hover {
    opacity: 0.8;
  }

  .autosave-btn {
    font-size: 11px;
    opacity: 0.9;
  }

  .gear-btn {
    display: flex;
    align-items: center;
    padding: 0 4px;
  }

  .statusbar-branch {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 500;
    padding-right: 8px;
    border-right: 1px solid color-mix(in srgb, var(--bg-tertiary) 40%, transparent);
  }
```

#### 4d — Verify and commit

- [ ] **Step 19: Type-check the full project**

```bash
cd /Users/charltonshih/Desktop/misc/projects/embd && npm run check 2>&1 | head -40
```

Expected: no errors. If there are errors about `toggleChat`/`toggleGit` not being defined, verify steps 4 and 6 were applied correctly.

- [ ] **Step 20: Run the dev build to confirm no runtime issues**

```bash
cd /Users/charltonshih/Desktop/misc/projects/embd && npm run build 2>&1 | tail -20
```

Expected: build completes successfully.

- [ ] **Step 21: Commit**

```bash
git add src/App.svelte
git commit -m "feat: add toolbar row, sidebar toggle, and breadcrumb statusbar"
```
