<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { projectRoot, gitBranch } from './stores.ts';

  interface GitFile {
    path: string;       // absolute path
    relPath: string;    // relative to repo root
    status: string;     // A, S, M, D, U
  }

  interface DiffLine {
    kind: string;
    old_num: number | null;
    new_num: number | null;
    text: string;
  }

  interface AheadBehind {
    ahead: number;
    behind: number;
    upstream: string | null;
  }

  interface Warning {
    file: string;
    line: number;
    text: string;
  }

  let stagedFiles = $state<GitFile[]>([]);
  let changedFiles = $state<GitFile[]>([]);
  let selectedFile = $state<GitFile | null>(null);
  let diffLines = $state<DiffLine[]>([]);
  let commitMsg = $state('');
  let aheadBehind = $state<AheadBehind>({ ahead: 0, behind: 0, upstream: null });
  let warnings = $state<Warning[]>([]);
  let commitSummary = $state('');
  let isCommitting = $state(false);
  let isPushing = $state(false);
  let commitError = $state('');
  let commitSuccess = $state('');
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  async function fetchStatus() {
    const root = $projectRoot;
    if (!root) return;

    try {
      const status = await invoke<Record<string, string>>('get_git_status', { path: root });
      const staged: GitFile[] = [];
      const changed: GitFile[] = [];

      for (const [absPath, code] of Object.entries(status)) {
        const relPath = absPath.startsWith(root) ? absPath.slice(root.length + 1) : absPath;
        const file: GitFile = { path: absPath, relPath, status: code };

        if (code === 'A' || code === 'S') {
          staged.push(file);
        } else {
          changed.push(file);
        }
      }

      staged.sort((a, b) => a.relPath.localeCompare(b.relPath));
      changed.sort((a, b) => a.relPath.localeCompare(b.relPath));
      stagedFiles = staged;
      changedFiles = changed;
    } catch {
      // ignore
    }

    try {
      aheadBehind = await invoke<AheadBehind>('git_ahead_behind', { repoPath: root });
    } catch {
      aheadBehind = { ahead: 0, behind: 0, upstream: null };
    }
  }

  async function selectFile(file: GitFile) {
    selectedFile = file;
    const root = $projectRoot;
    if (!root) return;

    const isStaged = file.status === 'A' || file.status === 'S';
    try {
      diffLines = await invoke<DiffLine[]>('git_diff', {
        repoPath: root,
        filePath: file.relPath,
        staged: isStaged,
      });
    } catch {
      diffLines = [];
    }
  }

  async function stageFile(file: GitFile) {
    const root = $projectRoot;
    if (!root) return;
    try {
      await invoke('git_stage', { repoPath: root, paths: [file.relPath] });
      await fetchStatus();
    } catch { /* ignore */ }
  }

  async function unstageFile(file: GitFile) {
    const root = $projectRoot;
    if (!root) return;
    try {
      await invoke('git_unstage', { repoPath: root, paths: [file.relPath] });
      await fetchStatus();
    } catch { /* ignore */ }
  }

  async function stageAll() {
    const root = $projectRoot;
    if (!root) return;
    const paths = changedFiles.map(f => f.relPath);
    if (paths.length === 0) return;
    try {
      await invoke('git_stage', { repoPath: root, paths });
      await fetchStatus();
    } catch { /* ignore */ }
  }

  async function unstageAll() {
    const root = $projectRoot;
    if (!root) return;
    const paths = stagedFiles.map(f => f.relPath);
    if (paths.length === 0) return;
    try {
      await invoke('git_unstage', { repoPath: root, paths });
      await fetchStatus();
    } catch { /* ignore */ }
  }

  async function scanWarnings(): Promise<Warning[]> {
    const root = $projectRoot;
    if (!root) return [];

    const warns: Warning[] = [];
    const debugPatterns = [/console\.log/g, /\bprint\(/g, /\bprintln!/g, /\bdbg!/g];
    const todoPatterns = [/\bTODO\b/g, /\bFIXME\b/g, /\bHACK\b/g];

    for (const file of stagedFiles) {
      try {
        const content = await invoke<string>('read_file_content', { path: file.path });
        const lines = content.split('\n');

        // Check file size
        if (content.length > 1_000_000) {
          warns.push({ file: file.relPath, line: 0, text: 'Large file (>1MB)' });
        }

        lines.forEach((line, i) => {
          for (const pat of debugPatterns) {
            pat.lastIndex = 0;
            if (pat.test(line)) {
              warns.push({ file: file.relPath, line: i + 1, text: `Debug: ${line.trim().slice(0, 60)}` });
            }
          }
          for (const pat of todoPatterns) {
            pat.lastIndex = 0;
            if (pat.test(line)) {
              warns.push({ file: file.relPath, line: i + 1, text: `${line.trim().slice(0, 60)}` });
            }
          }
        });
      } catch {
        // file might be deleted
      }
    }
    return warns;
  }

  function computeSummary() {
    let adds = 0, dels = 0;
    // We'll use a rough count from diff lines if available
    // For a quick summary, just count files
    commitSummary = `This commit changes ${stagedFiles.length} file${stagedFiles.length !== 1 ? 's' : ''}`;
  }

  async function doCommit(andPush = false) {
    if (!commitMsg.trim() || stagedFiles.length === 0) return;
    const root = $projectRoot;
    if (!root) return;

    commitError = '';
    commitSuccess = '';

    // Scan for warnings first
    warnings = await scanWarnings();
    computeSummary();

    isCommitting = true;
    try {
      const hash = await invoke<string>('git_commit', { repoPath: root, message: commitMsg.trim() });
      commitSuccess = `Committed ${hash}`;
      commitMsg = '';
      warnings = [];

      if (andPush) {
        isPushing = true;
        try {
          await invoke<string>('git_push', { repoPath: root });
          commitSuccess += ' and pushed';
        } catch (e) {
          commitError = `Push failed: ${e}`;
        }
        isPushing = false;
      }

      await fetchStatus();
    } catch (e) {
      commitError = `Commit failed: ${e}`;
    }
    isCommitting = false;
  }

  onMount(() => {
    fetchStatus();
    pollInterval = setInterval(fetchStatus, 3000);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
  });

  function statusIcon(status: string): string {
    switch (status) {
      case 'A': return 'A';
      case 'S': return 'M';
      case 'M': return 'M';
      case 'D': return 'D';
      case 'U': return 'U';
      default: return '?';
    }
  }

  function statusColor(status: string): string {
    switch (status) {
      case 'A': return 'var(--success)';
      case 'S': return 'var(--accent)';
      case 'M': return 'var(--warning)';
      case 'D': return 'var(--error)';
      case 'U': return 'var(--success)';
      default: return 'var(--text-muted)';
    }
  }
</script>

<div class="git-panel">
  <!-- Branch Header -->
  <div class="section-header branch-header">
    <div class="branch-info">
      <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
        <path d="M14.7 7.3L8.7 1.3a1 1 0 0 0-1.4 0L5.7 2.9l1.8 1.8A1.2 1.2 0 0 1 9 5.9v4.3a1.2 1.2 0 1 1-1-.1V6.1L6.3 7.8a1.2 1.2 0 1 1-.9-.5l1.8-1.8-1.8-1.8L1.3 7.3a1 1 0 0 0 0 1.4l6 6a1 1 0 0 0 1.4 0l6-6a1 1 0 0 0 0-1.4z"/>
      </svg>
      <span class="branch-name">{$gitBranch ?? 'no branch'}</span>
      {#if aheadBehind.upstream}
        <span class="ahead-behind">
          {#if aheadBehind.ahead > 0}<span class="ahead" title="Commits ahead">↑{aheadBehind.ahead}</span>{/if}
          {#if aheadBehind.behind > 0}<span class="behind" title="Commits behind">↓{aheadBehind.behind}</span>{/if}
        </span>
        <span class="upstream">{aheadBehind.upstream}</span>
      {/if}
    </div>
  </div>

  <div class="scroll-area">
    <!-- Staged Changes -->
    <div class="section">
      <div class="section-header" class:collapsed={stagedFiles.length === 0}>
        <span>Staged Changes ({stagedFiles.length})</span>
        {#if stagedFiles.length > 0}
          <button class="section-action" onclick={unstageAll} title="Unstage All">− all</button>
        {/if}
      </div>
      {#each stagedFiles as file}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="file-row"
          class:selected={selectedFile?.path === file.path}
          onclick={() => selectFile(file)}
        >
          <span class="status-badge" style="color: {statusColor(file.status)}">{statusIcon(file.status)}</span>
          <span class="file-name" title={file.relPath}>{file.relPath}</span>
          <button class="file-action" onclick={(e: MouseEvent) => { e.stopPropagation(); unstageFile(file); }} title="Unstage">−</button>
        </div>
      {/each}
    </div>

    <!-- Changes -->
    <div class="section">
      <div class="section-header">
        <span>Changes ({changedFiles.length})</span>
        {#if changedFiles.length > 0}
          <button class="section-action" onclick={stageAll} title="Stage All">+ all</button>
        {/if}
      </div>
      {#each changedFiles as file}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="file-row"
          class:selected={selectedFile?.path === file.path}
          onclick={() => selectFile(file)}
        >
          <span class="status-badge" style="color: {statusColor(file.status)}">{statusIcon(file.status)}</span>
          <span class="file-name" title={file.relPath}>{file.relPath}</span>
          <button class="file-action" onclick={(e: MouseEvent) => { e.stopPropagation(); stageFile(file); }} title="Stage">+</button>
        </div>
      {/each}
    </div>

    <!-- Diff Preview -->
    {#if selectedFile && diffLines.length > 0}
      <div class="section">
        <div class="section-header">
          <span>Diff: {selectedFile.relPath}</span>
        </div>
        <div class="diff-preview">
          {#each diffLines as line}
            <div class="diff-line {line.kind}">
              <span class="diff-gutter">
                {line.old_num ?? ' '}
              </span>
              <span class="diff-gutter">
                {line.new_num ?? ' '}
              </span>
              <span class="diff-text">{line.text}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Warnings -->
    {#if warnings.length > 0}
      <div class="section">
        <div class="section-header warning-header">
          <span>Warnings ({warnings.length})</span>
        </div>
        {#each warnings as warn}
          <div class="warning-row">
            <span class="warning-icon">!</span>
            <span class="warning-text">{warn.file}{warn.line > 0 ? `:${warn.line}` : ''} — {warn.text}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Commit Section -->
  <div class="commit-section">
    {#if commitSummary}
      <div class="commit-summary">{commitSummary}</div>
    {/if}
    {#if commitError}
      <div class="commit-error">{commitError}</div>
    {/if}
    {#if commitSuccess}
      <div class="commit-success">{commitSuccess}</div>
    {/if}
    <textarea
      class="commit-input"
      placeholder="Commit message..."
      bind:value={commitMsg}
      rows="3"
    ></textarea>
    <div class="commit-buttons">
      <button
        class="commit-btn"
        disabled={!commitMsg.trim() || stagedFiles.length === 0 || isCommitting}
        onclick={() => doCommit(false)}
      >
        {isCommitting ? 'Committing...' : 'Commit'}
      </button>
      <button
        class="commit-btn commit-push-btn"
        disabled={!commitMsg.trim() || stagedFiles.length === 0 || isCommitting || isPushing}
        onclick={() => doCommit(true)}
      >
        {isPushing ? 'Pushing...' : 'Commit & Push'}
      </button>
    </div>
  </div>
</div>

<style>
  .git-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    font-size: 12px;
  }

  .scroll-area {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .section {
    border-bottom: 1px solid var(--border);
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 10px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.3px;
    background: var(--bg-tertiary);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .branch-header {
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    text-transform: none;
    letter-spacing: 0;
    padding: 8px 10px;
  }

  .branch-info {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .branch-name {
    font-weight: 600;
    color: var(--text-primary);
  }

  .ahead-behind {
    display: flex;
    gap: 4px;
    font-size: 11px;
  }

  .ahead { color: var(--success); }
  .behind { color: var(--warning); }

  .upstream {
    color: var(--text-muted);
    font-size: 10px;
  }

  .section-action {
    font-size: 10px;
    color: var(--text-muted);
    padding: 1px 6px;
    border-radius: 3px;
    font-weight: 600;
  }

  .section-action:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .file-row {
    display: flex;
    align-items: center;
    padding: 3px 10px;
    gap: 6px;
    cursor: pointer;
    width: 100%;
    text-align: left;
    color: var(--text-primary);
    background: transparent;
    border: none;
    font-size: 12px;
  }

  .file-row:hover {
    background: var(--bg-surface);
  }

  .file-row.selected {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .status-badge {
    font-weight: 700;
    font-size: 11px;
    width: 14px;
    text-align: center;
    flex-shrink: 0;
    font-family: monospace;
  }

  .file-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }

  .file-action {
    font-size: 14px;
    color: var(--text-muted);
    padding: 0 4px;
    border-radius: 3px;
    opacity: 0;
    flex-shrink: 0;
    font-weight: 700;
    line-height: 1;
  }

  .file-row:hover .file-action {
    opacity: 1;
  }

  .file-action:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  /* Diff preview */
  .diff-preview {
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    font-size: 11px;
    overflow-x: auto;
    max-height: 300px;
    overflow-y: auto;
  }

  .diff-line {
    display: flex;
    white-space: pre;
    line-height: 1.5;
    padding: 0 4px;
  }

  .diff-line.add {
    background: color-mix(in srgb, var(--success) 15%, transparent);
    color: var(--success);
  }

  .diff-line.del {
    background: color-mix(in srgb, var(--error) 15%, transparent);
    color: var(--error);
  }

  .diff-line.ctx {
    color: var(--text-muted);
  }

  .diff-gutter {
    width: 36px;
    text-align: right;
    padding-right: 6px;
    color: var(--text-muted);
    flex-shrink: 0;
    opacity: 0.5;
    user-select: none;
  }

  .diff-text {
    flex: 1;
    min-width: 0;
  }

  /* Warnings */
  .warning-header {
    color: var(--warning);
  }

  .warning-row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 3px 10px;
    font-size: 11px;
    color: var(--warning);
  }

  .warning-icon {
    font-weight: 700;
    flex-shrink: 0;
  }

  .warning-text {
    word-break: break-all;
  }

  /* Commit section */
  .commit-section {
    border-top: 1px solid var(--border);
    padding: 8px 10px;
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .commit-input {
    width: 100%;
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px 8px;
    font-size: 12px;
    font-family: inherit;
    resize: vertical;
    min-height: 40px;
  }

  .commit-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .commit-input::placeholder {
    color: var(--text-muted);
  }

  .commit-buttons {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }

  .commit-btn {
    flex: 1;
    padding: 5px 10px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    background: var(--accent);
    color: var(--bg-primary);
    border: none;
  }

  .commit-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .commit-btn:not(:disabled):hover {
    background: var(--accent-hover);
  }

  .commit-push-btn {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .commit-push-btn:not(:disabled):hover {
    background: var(--border);
  }

  .commit-summary {
    font-size: 11px;
    color: var(--text-muted);
    margin-bottom: 4px;
  }

  .commit-error {
    font-size: 11px;
    color: var(--error);
    margin-bottom: 4px;
  }

  .commit-success {
    font-size: 11px;
    color: var(--success);
    margin-bottom: 4px;
  }
</style>
