<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { ask } from '@tauri-apps/plugin-dialog';
  import { projectRoot, gitBranch, activeFilePath, openFiles, reloadFileContent, closeFile, triggerFileTreeRefresh, sharedGitStatus, addFile } from './stores.ts';

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

  interface GitLogCommit {
    hash: string;
    short_hash: string;
    author: string;
    date: string;
    message: string;
  }

  interface GitGraphRow {
    graph: string;
    commit: GitLogCommit | null;
  }

  let stagedFiles = $state<GitFile[]>([]);
  let changedFiles = $state<GitFile[]>([]);
  let conflictFiles = $state<GitFile[]>([]);
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
  let isFetching = $state(false);
  let isPullRunning = $state(false);
  let isPullRebase = $state(false);
  let showHistory = $state(false);
  let graphRows = $state<GitGraphRow[]>([]);
  let historyLoading = $state(false);

  // Branch dropdown state
  interface BranchInfo {
    name: string;
    is_current: boolean;
    is_remote: boolean;
  }
  let showBranchDropdown = $state(false);
  let branchList = $state<BranchInfo[]>([]);
  let branchSearch = $state('');
  let branchLoading = $state(false);
  let branchError = $state('');
  let branchDropdownEl: HTMLDivElement | undefined = $state();
  let branchSearchEl: HTMLInputElement | undefined = $state();

  let filteredBranches = $derived(
    branchSearch
      ? branchList.filter(b => b.name.toLowerCase().includes(branchSearch.toLowerCase()))
      : branchList
  );

  async function toggleBranchDropdown() {
    showBranchDropdown = !showBranchDropdown;
    if (showBranchDropdown) {
      branchSearch = '';
      branchError = '';
      branchLoading = true;
      try {
        branchList = await invoke<BranchInfo[]>('git_list_branches', { repoPath: $projectRoot });
      } catch (e) {
        branchError = String(e);
        branchList = [];
      }
      branchLoading = false;
      // Focus search input after rendering
      requestAnimationFrame(() => branchSearchEl?.focus());
    }
  }

  async function switchBranch(branch: BranchInfo) {
    if (branch.is_current) {
      showBranchDropdown = false;
      return;
    }
    const root = $projectRoot;
    if (!root) return;
    branchError = '';
    try {
      await invoke<string>('git_checkout_branch', {
        repoPath: root,
        branch: branch.name,
        isRemote: branch.is_remote,
      });
      showBranchDropdown = false;
      // Refresh branch name and status
      const newBranch = await invoke<string | null>('get_git_branch', { path: root });
      gitBranch.set(newBranch ?? null);
      await fetchStatusFromBackend();
      if (showHistory) await fetchHistory();
    } catch (e) {
      branchError = String(e);
    }
  }

  async function deleteBranch(e: MouseEvent, branch: BranchInfo) {
    e.stopPropagation();
    const root = $projectRoot;
    if (!root) return;
    const confirmed = await ask(
      `Delete branch "${branch.name}"? This cannot be undone.`,
      { title: 'Delete Branch', kind: 'warning' }
    );
    if (!confirmed) return;
    branchError = '';
    try {
      await invoke<string>('git_delete_branch', {
        repoPath: root,
        branch: branch.name,
        force: false,
      });
      // Refresh branch list
      branchList = await invoke<BranchInfo[]>('git_list_branches', { repoPath: root });
    } catch (e) {
      branchError = String(e);
    }
  }

  function handleBranchKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      showBranchDropdown = false;
    }
  }

  function handleClickOutside(e: MouseEvent) {
    if (showBranchDropdown && branchDropdownEl && !branchDropdownEl.contains(e.target as Node)) {
      showBranchDropdown = false;
    }
  }

  function autoResizeTextarea(e: Event) {
    const textarea = e.target as HTMLTextAreaElement;
    textarea.style.height = 'auto';
    textarea.style.height = textarea.scrollHeight + 'px';
  }

  const GRAPH_COLORS = [
    'var(--git-graph-accent)',
    'var(--success)',
    'var(--warning)',
    '#e06c75',
    '#c678dd',
    '#56b6c2',
    '#d19a66',
    '#e5c07b',
  ];

  function laneColor(col: number): string {
    return GRAPH_COLORS[col % GRAPH_COLORS.length];
  }

  function processGitStatus(status: Record<string, string>) {
    const root = $projectRoot;
    if (!root) return;

    const staged: GitFile[] = [];
    const changed: GitFile[] = [];
    const conflicts: GitFile[] = [];

    for (const [absPath, code] of Object.entries(status)) {
      const relPath = absPath.startsWith(root) ? absPath.slice(root.length + 1) : absPath;
      const file: GitFile = { path: absPath, relPath, status: code };

      if (code === 'C') {
        conflicts.push(file);
      } else if (code === 'A' || code === 'S') {
        staged.push(file);
      } else {
        changed.push(file);
      }
    }

    staged.sort((a, b) => a.relPath.localeCompare(b.relPath));
    changed.sort((a, b) => a.relPath.localeCompare(b.relPath));
    conflicts.sort((a, b) => a.relPath.localeCompare(b.relPath));
    stagedFiles = staged;
    changedFiles = changed;
    conflictFiles = conflicts;
  }

  async function fetchAheadBehind() {
    const root = $projectRoot;
    if (!root) return;
    try {
      aheadBehind = await invoke<AheadBehind>('git_ahead_behind', { repoPath: root });
    } catch {
      aheadBehind = { ahead: 0, behind: 0, upstream: null };
    }
  }

  // For operations that mutate git state (stage, unstage, commit, etc.)
  // we need to force-refresh status from the backend
  async function fetchStatusFromBackend() {
    const root = $projectRoot;
    if (!root) return;
    try {
      const status = await invoke<Record<string, string>>('get_git_status', { path: root });
      sharedGitStatus.set(status);
      processGitStatus(status);
    } catch {
      // ignore
    }
    await fetchAheadBehind();
  }

  async function selectFile(file: GitFile) {
    if (selectedFile?.path === file.path) {
      selectedFile = null;
      diffLines = [];
      return;
    }
    selectedFile = file;
    const root = $projectRoot;
    if (!root) return;

    const isStaged = file.status === 'A' || file.status === 'S';
    try {
      diffLines = await invoke<DiffLine[]>('git_diff', {
        repoPath: root,
        filePath: file.relPath,
        staged: isStaged,
        isUntracked: file.status === 'U',
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
      await fetchStatusFromBackend();
    } catch { /* ignore */ }
  }

  async function unstageFile(file: GitFile) {
    const root = $projectRoot;
    if (!root) return;
    try {
      await invoke('git_unstage', { repoPath: root, paths: [file.relPath] });
      await fetchStatusFromBackend();
    } catch { /* ignore */ }
  }

  async function stageAll() {
    const root = $projectRoot;
    if (!root) return;
    const paths = changedFiles.map(f => f.relPath);
    if (paths.length === 0) return;
    try {
      await invoke('git_stage', { repoPath: root, paths });
      await fetchStatusFromBackend();
    } catch { /* ignore */ }
  }

  async function unstageAll() {
    const root = $projectRoot;
    if (!root) return;
    const paths = stagedFiles.map(f => f.relPath);
    if (paths.length === 0) return;
    try {
      await invoke('git_unstage', { repoPath: root, paths });
      await fetchStatusFromBackend();
    } catch { /* ignore */ }
  }

  async function reloadOpenFiles(discardedFiles: GitFile[]) {
    const currentOpen = $openFiles;
    for (const file of discardedFiles) {
      if (file.status === 'U') {
        // Untracked file was deleted from disk — close its tab
        if (currentOpen.some(f => f.path === file.path)) {
          closeFile(file.path);
        }
        continue;
      }
      // Tracked file — reload content from disk if open
      if (currentOpen.some(f => f.path === file.path)) {
        try {
          const content = await invoke<string>('read_file_content', { path: file.path });
          reloadFileContent(file.path, content);
        } catch { /* file may have been deleted */ }
      }
    }
  }

  async function discardFile(file: GitFile) {
    const root = $projectRoot;
    if (!root) return;
    const confirmed = await ask(
      `Discard changes to "${file.relPath}"? This cannot be undone.`,
      { title: 'Discard Changes', kind: 'warning' }
    );
    if (!confirmed) return;
    try {
      await invoke('git_discard', { repoPath: root, paths: [file.relPath] });
      if (selectedFile?.path === file.path) {
        selectedFile = null;
        diffLines = [];
      }
      await reloadOpenFiles([file]);
      await fetchStatusFromBackend();
      triggerFileTreeRefresh();
    } catch { /* ignore */ }
  }

  async function discardAll() {
    const root = $projectRoot;
    if (!root) return;
    const filesToDiscard = [...changedFiles];
    const paths = filesToDiscard.map(f => f.relPath);
    if (paths.length === 0) return;
    const confirmed = await ask(
      `Discard all changes to ${paths.length} file${paths.length !== 1 ? 's' : ''}? This cannot be undone.`,
      { title: 'Discard All Changes', kind: 'warning' }
    );
    if (!confirmed) return;
    try {
      await invoke('git_discard', { repoPath: root, paths });
      selectedFile = null;
      diffLines = [];
      await reloadOpenFiles(filesToDiscard);
      await fetchStatusFromBackend();
      triggerFileTreeRefresh();
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

  async function doFetch() {
    const root = $projectRoot;
    if (!root) return;
    isFetching = true;
    commitError = '';
    commitSuccess = '';
    try {
      await invoke<string>('git_fetch', { repoPath: root });
      commitSuccess = 'Fetched';
      await fetchAheadBehind();
    } catch (e) {
      commitError = `Fetch failed: ${e}`;
    }
    isFetching = false;
  }

  async function doPull(rebase = false) {
    const root = $projectRoot;
    if (!root || isPullRunning) return;
    isPullRunning = true;
    isPullRebase = rebase;
    commitError = '';
    commitSuccess = '';
    try {
      const cmd = rebase ? 'git_pull_rebase' : 'git_pull';
      await invoke<string>(cmd, { repoPath: root });
      commitSuccess = rebase ? 'Pulled (rebase)' : 'Pulled';
      await fetchStatusFromBackend();
      triggerFileTreeRefresh();
    } catch (e) {
      commitError = `Pull failed: ${e}`;
    } finally {
      isPullRunning = false;
    }
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

      await fetchStatusFromBackend();
    } catch (e) {
      commitError = `Commit failed: ${e}`;
    }
    isCommitting = false;
  }

  async function fetchHistory() {
    const root = $projectRoot;
    if (!root) return;
    historyLoading = true;
    try {
      graphRows = await invoke<GitGraphRow[]>('git_log', { repoPath: root, count: 50 });
    } catch {
      graphRows = [];
    }
    historyLoading = false;
  }

  async function toggleHistory() {
    showHistory = !showHistory;
    if (showHistory && graphRows.length === 0) {
      await fetchHistory();
    }
  }

  function renderGraphSvg(graph: string): { svg: string; width: number } {
    const cellW = 12;
    const cellH = 24;
    const cols = graph.length;
    const w = Math.max(cols * cellW, cellW);
    let paths = '';

    for (let i = 0; i < cols; i++) {
      const ch = graph[i];
      const cx = i * cellW + cellW / 2;
      const cy = cellH / 2;
      const color = laneColor(Math.floor(i / 2));

      if (ch === '*') {
        // Commit node: filled circle + vertical line
        paths += `<line x1="${cx}" y1="0" x2="${cx}" y2="${cy - 4}" stroke="${color}" stroke-width="1.5"/>`;
        paths += `<line x1="${cx}" y1="${cy + 4}" x2="${cx}" y2="${cellH}" stroke="${color}" stroke-width="1.5"/>`;
        paths += `<circle cx="${cx}" cy="${cy}" r="3.5" fill="${color}" stroke="${color}" stroke-width="1"/>`;
      } else if (ch === '|') {
        // Vertical line
        paths += `<line x1="${cx}" y1="0" x2="${cx}" y2="${cellH}" stroke="${color}" stroke-width="1.5"/>`;
      } else if (ch === '/' ) {
        // Diagonal up-left: from bottom-right to top-left
        paths += `<line x1="${cx + cellW / 2}" y1="${cellH}" x2="${cx - cellW / 2}" y2="0" stroke="${color}" stroke-width="1.5"/>`;
      } else if (ch === '\\') {
        // Diagonal down-right: from top-left area to bottom-right area
        paths += `<line x1="${cx - cellW / 2}" y1="0" x2="${cx + cellW / 2}" y2="${cellH}" stroke="${color}" stroke-width="1.5"/>`;
      } else if (ch === '_') {
        // Horizontal connector
        paths += `<line x1="${cx - cellW / 2}" y1="${cellH / 2}" x2="${cx + cellW / 2}" y2="${cellH / 2}" stroke="${color}" stroke-width="1.5"/>`;
      } else if (ch === '.') {
        // Sometimes used as horizontal connector
        paths += `<line x1="${cx - cellW / 2}" y1="${cellH / 2}" x2="${cx + cellW / 2}" y2="${cellH / 2}" stroke="${color}" stroke-width="1.5"/>`;
      }
      // spaces and other chars: nothing drawn
    }

    return {
      svg: `<svg width="${w}" height="${cellH}" viewBox="0 0 ${w} ${cellH}" xmlns="http://www.w3.org/2000/svg">${paths}</svg>`,
      width: w,
    };
  }

  // Subscribe to shared git status from FileTree's poll — no separate polling needed
  $effect(() => {
    const status = $sharedGitStatus;
    processGitStatus(status);
  });

  let aheadBehindInterval: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    fetchAheadBehind();
    // Only poll ahead/behind independently (lightweight, not duplicated)
    aheadBehindInterval = setInterval(fetchAheadBehind, 5000);
    document.addEventListener('mousedown', handleClickOutside);
  });

  onDestroy(() => {
    if (aheadBehindInterval) clearInterval(aheadBehindInterval);
    document.removeEventListener('mousedown', handleClickOutside);
  });

  function statusIcon(status: string): string {
    switch (status) {
      case 'A': return 'A';
      case 'S': return 'M';
      case 'M': return 'M';
      case 'D': return 'D';
      case 'U': return 'U';
      case 'C': return '!';
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
      case 'C': return 'var(--error)';
      default: return 'var(--text-muted)';
    }
  }

  function openConflictFile(file: GitFile) {
    const name = file.relPath.split('/').pop() || file.relPath;
    addFile(file.path, name);
  }
</script>

<div class="git-panel">
  <!-- Branch Header -->
  <div class="branch-header-wrapper" bind:this={branchDropdownEl}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="section-header branch-header" onclick={toggleBranchDropdown} onkeydown={handleBranchKeydown}>
      <div class="branch-info">
        <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
          <path d="M14.7 7.3L8.7 1.3a1 1 0 0 0-1.4 0L5.7 2.9l1.8 1.8A1.2 1.2 0 0 1 9 5.9v4.3a1.2 1.2 0 1 1-1-.1V6.1L6.3 7.8a1.2 1.2 0 1 1-.9-.5l1.8-1.8-1.8-1.8L1.3 7.3a1 1 0 0 0 0 1.4l6 6a1 1 0 0 0 1.4 0l6-6a1 1 0 0 0 0-1.4z"/>
        </svg>
        <span class="branch-name">{$gitBranch ?? 'no branch'}</span>
        <span class="branch-chevron" class:open={showBranchDropdown}>▾</span>
        {#if aheadBehind.upstream}
          <span class="ahead-behind">
            {#if aheadBehind.ahead > 0}<span class="ahead" title="Commits ahead">↑{aheadBehind.ahead}</span>{/if}
            {#if aheadBehind.behind > 0}<span class="behind" title="Commits behind">↓{aheadBehind.behind}</span>{/if}
          </span>
          <span class="upstream">{aheadBehind.upstream}</span>
        {/if}
      </div>
    </div>
    {#if showBranchDropdown}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="branch-dropdown" onkeydown={handleBranchKeydown}>
        <input
          class="branch-search"
          type="text"
          placeholder="Search branches..."
          bind:value={branchSearch}
          bind:this={branchSearchEl}
        />
        {#if branchError}
          <div class="branch-error">{branchError}</div>
        {/if}
        {#if branchLoading}
          <div class="branch-loading">Loading...</div>
        {:else}
          <div class="branch-list">
            {#each filteredBranches as branch}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="branch-item"
                class:current={branch.is_current}
                onclick={() => switchBranch(branch)}
              >
                <span class="branch-item-indicator">{branch.is_current ? '●' : ''}</span>
                <span class="branch-item-name">
                  {#if branch.is_remote}
                    <span class="branch-remote-prefix">{branch.name.split('/')[0]}/</span>{branch.name.split('/').slice(1).join('/')}
                  {:else}
                    {branch.name}
                  {/if}
                </span>
                {#if !branch.is_current && !branch.is_remote}
                  <button class="branch-delete-btn" onclick={(e: MouseEvent) => deleteBranch(e, branch)} title="Delete branch">✕</button>
                {/if}
              </div>
            {/each}
            {#if filteredBranches.length === 0 && !branchLoading}
              <div class="branch-loading">No branches found</div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Git Actions -->
  <div class="git-actions">
    <button class="git-action-btn" disabled={isFetching} onclick={doFetch} title="Fetch from remote">
      {isFetching ? 'Fetching...' : 'Fetch'}
    </button>
    <button class="git-action-btn" disabled={isPullRunning} onclick={() => doPull(false)} title="Pull from remote">
      {isPullRunning && !isPullRebase ? 'Pulling...' : 'Pull'}
    </button>
    <button class="git-action-btn" disabled={isPullRunning} onclick={() => doPull(true)} title="Pull with rebase">
      {isPullRunning && isPullRebase ? 'Rebasing...' : 'Pull Rebase'}
    </button>
  </div>

  <div class="scroll-area">
    <!-- Merge Conflicts -->
    {#if conflictFiles.length > 0}
      <div class="section">
        <div class="section-header conflict-header">
          <span>Merge Conflicts ({conflictFiles.length})</span>
        </div>
        {#each conflictFiles as file}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="file-row" onclick={() => openConflictFile(file)}>
            <span class="status-badge" style="color: {statusColor(file.status)}">{statusIcon(file.status)}</span>
            <span class="file-name" title={file.relPath}>{file.relPath}</span>
            <button class="file-action open-btn" onclick={(e: MouseEvent) => { e.stopPropagation(); openConflictFile(file); }}>Open</button>
          </div>
        {/each}
      </div>
    {/if}

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
          <div class="section-actions">
            <button class="section-action" onclick={discardAll} title="Discard All Changes">✕ all</button>
            <button class="section-action" onclick={stageAll} title="Stage All">+ all</button>
          </div>
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
          <button class="file-action" onclick={(e: MouseEvent) => { e.stopPropagation(); discardFile(file); }} title="Discard Changes">✕</button>
          <button class="file-action" onclick={(e: MouseEvent) => { e.stopPropagation(); stageFile(file); }} title="Stage">+</button>
        </div>
      {/each}
    </div>

    <!-- Diff Preview -->
    {#if selectedFile && diffLines.length > 0}
      <div class="section">
        <div class="section-header">
          <span>Diff: {selectedFile.relPath}</span>
          <button class="section-action" onclick={() => { selectedFile = null; diffLines = []; }} title="Close Diff">✕</button>
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

    <!-- History -->
    <div class="section">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="section-header history-toggle" onclick={toggleHistory}>
        <span class="history-chevron" class:open={showHistory}>▶</span>
        <span>History</span>
        {#if showHistory}
          <button class="section-action" onclick={(e: MouseEvent) => { e.stopPropagation(); fetchHistory(); }} title="Refresh">↻</button>
        {/if}
      </div>
      {#if showHistory}
        {#if historyLoading}
          <div class="history-loading">Loading...</div>
        {:else if graphRows.length === 0}
          <div class="history-loading">No commits yet</div>
        {:else}
          <div class="graph-container">
            {#each graphRows as row}
              {@const rendered = renderGraphSvg(row.graph)}
              <div class="graph-row" class:graph-row-commit={row.commit !== null} title={row.commit ? `${row.commit.hash}\n${row.commit.author}\n${row.commit.date}` : ''}>
                <span class="graph-svg">{@html rendered.svg}</span>
                {#if row.commit}
                  <span class="graph-hash">{row.commit.short_hash}</span>
                  <span class="graph-msg">{row.commit.message}</span>
                  <span class="graph-date">{row.commit.date}</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
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
      rows="1"
      oninput={autoResizeTextarea}
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

  .branch-header-wrapper {
    position: relative;
  }

  .branch-header {
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    text-transform: none;
    letter-spacing: 0;
    padding: 8px 10px;
    cursor: pointer;
  }

  .branch-header:hover {
    background: var(--bg-surface);
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

  .branch-chevron {
    font-size: 10px;
    color: var(--text-muted);
    transition: transform 0.15s;
  }

  .branch-chevron.open {
    transform: rotate(180deg);
  }

  .branch-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-top: none;
    z-index: 100;
    max-height: 280px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .branch-search {
    width: 100%;
    padding: 6px 8px;
    background: var(--bg-primary);
    color: var(--text-primary);
    border: none;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }

  .branch-search::placeholder {
    color: var(--text-muted);
  }

  .branch-list {
    overflow-y: auto;
    flex: 1;
  }

  .branch-item {
    display: flex;
    align-items: center;
    padding: 4px 10px;
    cursor: pointer;
    gap: 6px;
    font-size: 12px;
    color: var(--text-primary);
  }

  .branch-item:hover {
    background: var(--bg-surface);
  }

  .branch-item.current {
    color: var(--accent);
  }

  .branch-item-indicator {
    width: 10px;
    font-size: 8px;
    flex-shrink: 0;
    text-align: center;
    color: var(--accent);
  }

  .branch-item-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .branch-remote-prefix {
    color: var(--text-muted);
  }

  .branch-delete-btn {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 11px;
    color: var(--text-muted);
    padding: 0 4px;
    border-radius: 3px;
    opacity: 0;
    font-weight: 700;
    line-height: 1;
    cursor: pointer;
    background: none;
    border: none;
  }

  .branch-item:hover .branch-delete-btn {
    opacity: 1;
  }

  .branch-delete-btn:hover {
    background: var(--bg-tertiary);
    color: var(--error);
  }

  .branch-loading,
  .branch-error {
    padding: 8px 10px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .branch-error {
    color: var(--error);
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

  .git-actions {
    display: flex;
    gap: 4px;
    padding: 4px 10px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
  }

  .git-action-btn {
    flex: 1;
    padding: 3px 6px;
    border-radius: 3px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    background: var(--bg-surface);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }

  .git-action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .git-action-btn:not(:disabled):hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .conflict-header {
    color: var(--error);
  }

  .open-btn {
    opacity: 0;
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-surface);
    border: 1px solid var(--border);
    cursor: pointer;
  }

  .file-row:hover .open-btn {
    opacity: 1;
  }

  .open-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .section-actions {
    display: flex;
    gap: 4px;
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
    background: color-mix(in srgb, var(--diff-add) 15%, transparent);
    color: var(--diff-add);
  }

  .diff-line.del {
    background: color-mix(in srgb, var(--diff-del) 15%, transparent);
    color: var(--diff-del);
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
    resize: none;
    min-height: 40px;
    max-height: 200px;
    overflow-y: auto;
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
    color: var(--git-notification);
    margin-bottom: 4px;
  }

  /* History */
  .history-toggle {
    cursor: pointer;
    gap: 4px;
    user-select: none;
  }

  .history-toggle:hover {
    background: var(--bg-surface);
  }

  .history-chevron {
    font-size: 8px;
    transition: transform 0.15s;
    display: inline-block;
  }

  .history-chevron.open {
    transform: rotate(90deg);
  }

  .history-loading {
    padding: 8px 10px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .graph-container {
    overflow-x: auto;
  }

  .graph-row {
    display: flex;
    align-items: center;
    height: 24px;
    white-space: nowrap;
    font-size: 11px;
  }

  .graph-row-commit:hover {
    background: var(--bg-surface);
  }

  .graph-svg {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    height: 24px;
  }

  .graph-hash {
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    color: var(--git-graph-accent);
    flex-shrink: 0;
    font-size: 10px;
    margin-left: 4px;
    margin-right: 8px;
  }

  .graph-msg {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-primary);
  }

  .graph-date {
    flex-shrink: 0;
    color: var(--text-muted);
    font-size: 10px;
    padding-left: 8px;
    padding-right: 6px;
  }
</style>
