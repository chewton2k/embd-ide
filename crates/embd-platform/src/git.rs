use embd_core::{Error, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git operations scoped to a repository directory.
///
/// Currently wraps the git CLI (matching the existing Tauri implementation).
/// Designed behind a clean interface so the backend can be swapped to
/// gitoxide (gix) in the future without changing callers.
pub struct GitRepo {
    workdir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_num: Option<u32>,
    pub new_num: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineKind {
    Add,
    Del,
    Ctx,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffRange {
    pub kind: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
    pub upstream: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogCommit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphRow {
    pub graph: String,
    pub commit: Option<LogCommit>,
}

impl GitRepo {
    pub fn new(workdir: impl AsRef<Path>) -> Self {
        Self {
            workdir: workdir.as_ref().to_path_buf(),
        }
    }

    /// Run a git command and return stdout on success.
    fn run(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workdir)
            .output()
            .map_err(|e| Error::Git(format!("failed to run git: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(Error::Git(stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run a git command, returning Ok even on non-zero exit (for commands where
    /// failure is informational, like diff on untracked files).
    fn run_allow_failure(&self, args: &[&str]) -> Result<(bool, String)> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workdir)
            .output()
            .map_err(|e| Error::Git(format!("failed to run git: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok((output.status.success(), stdout))
    }

    // ── Status ───────────────────────────────────────────────────────

    /// Get the current branch name, or short hash if detached.
    pub fn branch(&self) -> Result<Option<String>> {
        let git_dir = self.workdir.join(".git");
        if !git_dir.exists() {
            return Ok(None);
        }
        let head = std::fs::read_to_string(git_dir.join("HEAD"))
            .map_err(|e| Error::Git(e.to_string()))?;
        let head = head.trim();
        if let Some(branch) = head.strip_prefix("ref: refs/heads/") {
            Ok(Some(branch.to_string()))
        } else {
            Ok(Some(head[..7.min(head.len())].to_string()))
        }
    }

    /// Get file status map (abs_path -> status code).
    pub fn status(&self) -> Result<HashMap<String, String>> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "-uall", "-z"])
            .current_dir(&self.workdir)
            .output()
            .map_err(|e| Error::Git(e.to_string()))?;

        if !output.status.success() {
            return Ok(HashMap::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut result = HashMap::new();
        let entries: Vec<&str> = stdout.split('\0').collect();
        let mut i = 0;

        while i < entries.len() {
            let entry = entries[i];
            if entry.len() < 4 {
                i += 1;
                continue;
            }
            let ix = entry.as_bytes()[0];
            let wt = entry.as_bytes()[1];
            let file_path = &entry[3..];

            if ix == b'R' || ix == b'C' {
                i += 1; // skip original path
            }

            let status = match (ix, wt) {
                (b'?', b'?') => "U",
                (b'U', b'U') | (b'A', b'A') | (b'D', b'D') | (b'A', b'U') | (b'U', b'A')
                | (b'D', b'U') | (b'U', b'D') => "C",
                (b'A', _) | (b'R', _) => "A",
                (b'M', b' ') | (b'M', b'\0') | (b'D', b' ') | (b'D', b'\0') => "S",
                (_, b'D') => "D",
                (_, b'M') => "M",
                _ => "M",
            };

            let abs = self.workdir.join(file_path);
            result.insert(abs.to_string_lossy().to_string(), status.to_string());
            i += 1;
        }

        Ok(result)
    }

    /// Check ahead/behind relative to upstream.
    pub fn ahead_behind(&self) -> Result<AheadBehind> {
        let upstream = match self.run(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        {
            Ok(s) => s.trim().to_string(),
            Err(_) => return Ok(AheadBehind { ahead: 0, behind: 0, upstream: None }),
        };

        let output = match self.run(&["rev-list", "--count", "--left-right", "HEAD...@{u}"]) {
            Ok(s) => s,
            Err(_) => {
                return Ok(AheadBehind {
                    ahead: 0,
                    behind: 0,
                    upstream: Some(upstream),
                })
            }
        };

        let parts: Vec<&str> = output.trim().split('\t').collect();
        let ahead = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let behind = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

        Ok(AheadBehind {
            ahead,
            behind,
            upstream: Some(upstream),
        })
    }

    // ── Diff ─────────────────────────────────────────────────────────

    pub fn diff(&self, file: &str, staged: bool, is_untracked: bool) -> Result<Vec<DiffLine>> {
        let stdout = if is_untracked {
            let abs = self.workdir.join(file);
            let (_, out) = self.run_allow_failure(&[
                "diff",
                "--no-index",
                "/dev/null",
                &abs.to_string_lossy(),
            ])?;
            out
        } else if staged {
            self.run(&["diff", "--cached", "--", file])?
        } else {
            self.run(&["diff", "--", file])?
        };

        Ok(parse_unified_diff(&stdout))
    }

    pub fn diff_line_ranges(&self, file: &str) -> Result<Vec<DiffRange>> {
        let stdout = match self.run(&["diff", "-U0", "--", file]) {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };

        let mut ranges = Vec::new();
        for line in stdout.lines() {
            if !line.starts_with("@@") {
                continue;
            }
            if let Some(rest) = line.strip_prefix("@@ -") {
                let parts: Vec<&str> = rest.splitn(2, '+').collect();
                if parts.len() != 2 {
                    continue;
                }
                let old_part = parts[0].trim().trim_end_matches(',');
                let new_part = parts[1].split_whitespace().next().unwrap_or("0");
                let (_old_start, old_count) = parse_hunk_range(old_part);
                let (new_start, new_count) = parse_hunk_range(new_part);

                let (kind, start, end) = if old_count == 0 && new_count > 0 {
                    ("add", new_start, new_start + new_count - 1)
                } else if new_count == 0 && old_count > 0 {
                    ("del", new_start.max(1), new_start.max(1))
                } else {
                    ("mod", new_start, new_start + new_count - 1)
                };
                ranges.push(DiffRange {
                    kind: kind.to_string(),
                    start,
                    end,
                });
            }
        }
        Ok(ranges)
    }

    // ── Stage / Commit / Push / Pull ─────────────────────────────────

    pub fn stage(&self, paths: &[&str]) -> Result<()> {
        let mut args = vec!["add", "--"];
        args.extend(paths);
        self.run(&args)?;
        Ok(())
    }

    pub fn unstage(&self, paths: &[&str]) -> Result<()> {
        let mut args = vec!["restore", "--staged", "--"];
        args.extend(paths);
        self.run(&args)?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<String> {
        let stdout = self.run(&["commit", "-m", message])?;
        let hash = stdout
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("unknown")
            .trim_end_matches(']')
            .to_string();
        Ok(hash)
    }

    pub fn push(&self) -> Result<String> {
        match self.run(&["push"]) {
            Ok(s) => Ok(s),
            Err(Error::Git(stderr)) if stderr.contains("no upstream") || stderr.contains("has no upstream branch") => {
                let branch = self.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
                let branch = branch.trim();
                self.run(&["push", "--set-upstream", "origin", branch])
            }
            Err(e) => Err(e),
        }
    }

    pub fn pull(&self) -> Result<String> {
        self.run(&["pull"])
    }

    pub fn pull_rebase(&self) -> Result<String> {
        self.run(&["pull", "--rebase"])
    }

    pub fn fetch(&self) -> Result<String> {
        self.run(&["fetch"])
    }

    // ── Branches ─────────────────────────────────────────────────────

    pub fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        let stdout = self.run(&["branch", "-a", "--no-color"])?;
        let mut branches = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.contains("->") {
                continue;
            }
            let is_current = trimmed.starts_with('*');
            let name = trimmed
                .trim_start_matches("* ")
                .trim_start_matches("remotes/")
                .to_string();
            let is_remote = line.contains("remotes/");
            branches.push(BranchInfo {
                name,
                is_current,
                is_remote,
            });
        }

        branches.sort_by(|a, b| {
            b.is_current
                .cmp(&a.is_current)
                .then(a.is_remote.cmp(&b.is_remote))
                .then(a.name.cmp(&b.name))
        });

        Ok(branches)
    }

    pub fn checkout(&self, branch: &str, is_remote: bool) -> Result<String> {
        if branch.contains("..") || branch.contains(' ') {
            return Err(Error::Git("invalid branch name".to_string()));
        }
        if is_remote {
            self.run(&["checkout", "--track", &format!("remotes/{}", branch)])
        } else {
            self.run(&["checkout", branch])
        }
    }

    pub fn delete_branch(&self, branch: &str, force: bool) -> Result<String> {
        if branch.contains("..") || branch.contains(' ') {
            return Err(Error::Git("invalid branch name".to_string()));
        }
        let current = self.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if branch == current.trim() {
            return Err(Error::Git("cannot delete the current branch".to_string()));
        }
        let flag = if force { "-D" } else { "-d" };
        self.run(&["branch", flag, branch])
    }

    // ── Log ──────────────────────────────────────────────────────────

    pub fn log(&self, count: u32) -> Result<Vec<GraphRow>> {
        let limit = count.min(500).to_string();
        let format_str = format!("--format=format:%H\t%h\t%an\t%ar\t%s");
        let stdout = self.run(&["log", "--graph", &format_str, "-n", &limit])?;

        let mut rows = Vec::new();
        for line in stdout.lines() {
            if let Some(star_pos) = line.find('*') {
                let graph_prefix = &line[..star_pos];
                let after_star = line[star_pos + 1..].trim_start();
                let commit = if !after_star.is_empty() {
                    let parts: Vec<&str> = after_star.splitn(5, '\t').collect();
                    if parts.len() >= 5 {
                        Some(LogCommit {
                            hash: parts[0].to_string(),
                            short_hash: parts[1].to_string(),
                            author: parts[2].to_string(),
                            date: parts[3].to_string(),
                            message: parts[4].to_string(),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                rows.push(GraphRow {
                    graph: format!("{}*", graph_prefix),
                    commit,
                });
            } else {
                rows.push(GraphRow {
                    graph: line.to_string(),
                    commit: None,
                });
            }
        }
        Ok(rows)
    }

    /// Discard changes to files. Tracked files are restored via `git checkout`,
    /// untracked files are moved to the system trash.
    pub fn discard(&self, paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        // Classify files as tracked or untracked using scoped status check
        let mut args = vec!["status", "--porcelain", "-z", "-uall", "--"];
        args.extend(paths);
        let stdout = self.run(&args).unwrap_or_default();

        let path_set: std::collections::HashSet<&str> = paths.iter().copied().collect();
        let mut tracked = Vec::new();
        let mut untracked = Vec::new();

        let entries: Vec<&str> = stdout.split('\0').collect();
        let mut idx = 0;
        while idx < entries.len() {
            let entry = entries[idx];
            if entry.len() < 4 {
                idx += 1;
                continue;
            }
            let ix = entry.as_bytes()[0];
            let wt = entry.as_bytes()[1];
            let file = &entry[3..];

            // Skip rename's extra entry
            if ix == b'R' || ix == b'C' {
                idx += 1;
            }

            if path_set.contains(file) {
                if ix == b'?' && wt == b'?' {
                    untracked.push(file.to_string());
                } else {
                    tracked.push(file.to_string());
                }
            }
            idx += 1;
        }

        // Restore tracked files
        if !tracked.is_empty() {
            let mut checkout_args: Vec<&str> = vec!["checkout", "--"];
            let tracked_refs: Vec<&str> = tracked.iter().map(|s| s.as_str()).collect();
            checkout_args.extend(&tracked_refs);
            self.run(&checkout_args)?;
        }

        // Move untracked files to trash
        for file in &untracked {
            let full_path = self.workdir.join(file);
            if !full_path.exists() {
                continue;
            }
            let canonical = std::fs::canonicalize(&full_path)
                .map_err(|e| Error::Io(e))?;
            if !canonical.starts_with(&self.workdir) {
                return Err(Error::AccessDenied(format!(
                    "'{}' is outside the repository",
                    file
                )));
            }
            trash::delete(&canonical)
                .map_err(|e| Error::Other(format!("Failed to discard '{}': {}", file, e)))?;
        }

        Ok(())
    }

    /// List ignored files/directories (gitignore-aware).
    pub fn ignored(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args([
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
            ])
            .current_dir(&self.workdir)
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let clean = l.trim_end_matches('/');
                self.workdir
                    .join(clean)
                    .to_string_lossy()
                    .to_string()
            })
            .collect())
    }

    pub fn resolve_conflict(&self, file: &str, content: &str, stage: bool) -> Result<()> {
        let abs = self.workdir.join(file);
        if !abs.starts_with(&self.workdir) {
            return Err(Error::AccessDenied("file outside repo".to_string()));
        }
        std::fs::write(&abs, content)?;
        if stage {
            self.run(&["add", "--", file])?;
        }
        Ok(())
    }
}

// ── Diff parsing (shared) ────────────────────────────────────────────

fn parse_unified_diff(diff: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old_line: u32 = 0;
    let mut new_line: u32 = 0;
    let mut in_hunk = false;

    for raw in diff.lines() {
        if raw.starts_with("@@") {
            in_hunk = true;
            if let Some(rest) = raw.strip_prefix("@@ -") {
                let parts: Vec<&str> = rest.splitn(2, '+').collect();
                if parts.len() == 2 {
                    old_line = parts[0]
                        .split(',')
                        .next()
                        .unwrap_or("1")
                        .trim()
                        .parse()
                        .unwrap_or(1);
                    new_line = parts[1]
                        .split(|c: char| c == ',' || c == ' ')
                        .next()
                        .unwrap_or("1")
                        .parse()
                        .unwrap_or(1);
                }
            }
            lines.push(DiffLine {
                kind: DiffLineKind::Ctx,
                old_num: None,
                new_num: None,
                text: raw.to_string(),
            });
        } else if !in_hunk {
            continue;
        } else if let Some(text) = raw.strip_prefix('+') {
            lines.push(DiffLine {
                kind: DiffLineKind::Add,
                old_num: None,
                new_num: Some(new_line),
                text: text.to_string(),
            });
            new_line += 1;
        } else if let Some(text) = raw.strip_prefix('-') {
            lines.push(DiffLine {
                kind: DiffLineKind::Del,
                old_num: Some(old_line),
                new_num: None,
                text: text.to_string(),
            });
            old_line += 1;
        } else {
            let text = raw.strip_prefix(' ').unwrap_or(raw);
            lines.push(DiffLine {
                kind: DiffLineKind::Ctx,
                old_num: Some(old_line),
                new_num: Some(new_line),
                text: text.to_string(),
            });
            old_line += 1;
            new_line += 1;
        }
    }
    lines
}

fn parse_hunk_range(s: &str) -> (u32, u32) {
    let parts: Vec<&str> = s.split(',').collect();
    let start: u32 = parts[0].parse().unwrap_or(0);
    let count: u32 = if parts.len() > 1 {
        parts[1].parse().unwrap_or(1)
    } else {
        1
    };
    (start, count)
}
