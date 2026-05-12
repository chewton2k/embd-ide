use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::modules::fs::ProjectRootState;

/// Validate that a repo_path is within (or equal to) the project root for git commands.
pub fn validate_repo_path(
    repo_path: &str,
    state: &tauri::State<'_, ProjectRootState>,
) -> Result<PathBuf, String> {
    let root = state.lock().map_err(|e| e.to_string())?;
    let root = root
        .as_ref()
        .ok_or_else(|| "No project is open".to_string())?;
    let canonical = fs::canonicalize(repo_path).map_err(|e| format!("Invalid repo path: {}", e))?;
    if !canonical.starts_with(root) {
        return Err("Access denied: repo path is outside the project directory".to_string());
    }
    Ok(canonical)
}

/// Validate a relative file path used in git commands.
/// Rejects absolute paths, traversal sequences, and NUL bytes.
pub fn validate_git_file_path(file_path: &str) -> Result<(), String> {
    if file_path.is_empty() {
        return Err("Invalid file path: path cannot be empty".to_string());
    }
    if file_path.contains('\0') {
        return Err("Invalid file path: null bytes not allowed".to_string());
    }

    let path = Path::new(file_path);
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err("Invalid file path: traversal not allowed".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("Invalid file path: absolute paths not allowed".to_string());
            }
            Component::Normal(name) if name.eq_ignore_ascii_case(".git") => {
                return Err("Invalid file path: .git paths are not allowed".to_string());
            }
            _ => {}
        }
    }
    Ok(())
}

// ── Serializable types ───────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct DiffLine {
    pub kind: String,
    pub old_num: Option<u32>,
    pub new_num: Option<u32>,
    pub text: String,
}

#[derive(Serialize, Clone)]
pub struct DiffRange {
    pub kind: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Serialize, Clone)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
    pub upstream: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct GitLogCommit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Serialize, Clone)]
pub struct GitGraphRow {
    pub graph: String,
    pub commit: Option<GitLogCommit>,
}

#[derive(Serialize, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

// ── Internal helpers ─────────────────────────────────────────────

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
                        .split([',', ' '])
                        .next()
                        .unwrap_or("1")
                        .parse()
                        .unwrap_or(1);
                }
            }
            lines.push(DiffLine {
                kind: "ctx".to_string(),
                old_num: None,
                new_num: None,
                text: raw.to_string(),
            });
        } else if !in_hunk {
            continue;
        } else if let Some(text) = raw.strip_prefix('+') {
            lines.push(DiffLine {
                kind: "add".to_string(),
                old_num: None,
                new_num: Some(new_line),
                text: text.to_string(),
            });
            new_line += 1;
        } else if let Some(text) = raw.strip_prefix('-') {
            lines.push(DiffLine {
                kind: "del".to_string(),
                old_num: Some(old_line),
                new_num: None,
                text: text.to_string(),
            });
            old_line += 1;
        } else {
            let text = raw.strip_prefix(' ').unwrap_or(raw);
            lines.push(DiffLine {
                kind: "ctx".to_string(),
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

// ── Tauri commands ───────────────────────────────────────────────

#[tauri::command]
pub fn get_git_status(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<HashMap<String, String>, String> {
    validate_repo_path(&path, &state)?;
    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall", "-z"])
        .current_dir(&path)
        .output()
        .map_err(|e| e.to_string())?;

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
        let index_status = entry.as_bytes()[0];
        let wt_status = entry.as_bytes()[1];
        let file_path = &entry[3..];

        let file_path = if index_status == b'R' || index_status == b'C' {
            i += 1;
            file_path
        } else {
            file_path
        };

        let abs_path = PathBuf::from(&path).join(file_path);
        let abs_str = abs_path.to_string_lossy().to_string();

        let status = match (index_status, wt_status) {
            (b'?', b'?') => "U",
            (b'U', b'U')
            | (b'A', b'A')
            | (b'D', b'D')
            | (b'A', b'U')
            | (b'U', b'A')
            | (b'D', b'U')
            | (b'U', b'D') => "C",
            (b'A', _) => "A",
            (b'R', _) => "A",
            (b'M', b' ') | (b'M', b'\0') => "S",
            (b'D', b' ') | (b'D', b'\0') => "S",
            (_, b'D') => "D",
            (_, b'M') => "M",
            _ => "M",
        };

        result.insert(abs_str, status.to_string());
        i += 1;
    }

    Ok(result)
}

#[tauri::command]
pub fn get_git_remote_status(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<HashMap<String, String>, String> {
    validate_repo_path(&path, &state)?;

    let upstream_check = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "@{u}"])
        .current_dir(&path)
        .output()
        .map_err(|e| e.to_string())?;

    if !upstream_check.status.success() {
        return Ok(HashMap::new());
    }

    let output = Command::new("git")
        .args(["diff", "--name-status", "HEAD...@{u}"])
        .current_dir(&path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(HashMap::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut result = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, '\t');
        let status_code = match parts.next() {
            Some(s) => s.trim(),
            None => continue,
        };
        let file_path = match parts.next() {
            Some(p) => p.trim(),
            None => continue,
        };

        let code = if status_code.starts_with('R') {
            "A"
        } else if status_code == "M" {
            "M"
        } else if status_code == "A" {
            "A"
        } else if status_code == "D" {
            "D"
        } else {
            "M"
        };

        let actual_path = if status_code.starts_with('R') {
            file_path.split('\t').next_back().unwrap_or(file_path)
        } else {
            file_path
        };

        let abs_path = PathBuf::from(&path).join(actual_path);
        let abs_str = abs_path.to_string_lossy().to_string();
        result.insert(abs_str, code.to_string());
    }

    Ok(result)
}

#[tauri::command]
pub fn get_git_ignored(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<Vec<String>, String> {
    validate_repo_path(&path, &state)?;
    let output = Command::new("git")
        .args([
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ])
        .current_dir(&path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let root = PathBuf::from(&path);
    let result: Vec<String> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let clean = l.trim_end_matches('/');
            root.join(clean).to_string_lossy().to_string()
        })
        .collect();

    Ok(result)
}

#[tauri::command]
pub fn get_git_branch(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<Option<String>, String> {
    let root = state.lock().map_err(|e| e.to_string())?;
    let root = root
        .as_ref()
        .ok_or_else(|| "No project is open".to_string())?;
    let canonical = fs::canonicalize(&path).map_err(|e| format!("Invalid path: {}", e))?;
    if !canonical.starts_with(root) {
        return Err("Access denied: path is outside the project directory".to_string());
    }

    let mut dir = canonical;
    loop {
        let git_dir = dir.join(".git");
        if git_dir.exists() {
            let head_file = git_dir.join("HEAD");
            if let Ok(content) = fs::read_to_string(&head_file) {
                let content = content.trim();
                if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
                    return Ok(Some(branch.to_string()));
                }
                return Ok(Some(content[..7.min(content.len())].to_string()));
            }
            return Ok(None);
        }
        if dir == *root {
            return Ok(None);
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

#[tauri::command]
pub fn git_diff(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    file_path: String,
    staged: bool,
    is_untracked: Option<bool>,
) -> Result<Vec<DiffLine>, String> {
    validate_repo_path(&repo_path, &state)?;
    validate_git_file_path(&file_path)?;

    let untracked = match is_untracked {
        Some(v) => v,
        None => {
            let status_out = Command::new("git")
                .args(["status", "--porcelain", "--", &file_path])
                .current_dir(&repo_path)
                .output()
                .map_err(|e| e.to_string())?;
            let status_str = String::from_utf8_lossy(&status_out.stdout);
            status_str.lines().any(|l| l.starts_with("??"))
        }
    };

    let output = if untracked {
        let abs = PathBuf::from(&repo_path).join(&file_path);
        Command::new("git")
            .args(["diff", "--no-index", "/dev/null", &abs.to_string_lossy()])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| e.to_string())?
    } else if staged {
        Command::new("git")
            .args(["diff", "--cached", "--", &file_path])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("git")
            .args(["diff", "--", &file_path])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| e.to_string())?
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_unified_diff(&stdout))
}

#[tauri::command]
pub fn git_stage(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    paths: Vec<String>,
) -> Result<(), String> {
    validate_repo_path(&repo_path, &state)?;
    for p in &paths {
        validate_git_file_path(p)?;
    }
    let mut args = vec!["add".to_string(), "--".to_string()];
    args.extend(paths);
    let output = Command::new("git")
        .args(&args)
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn git_unstage(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    paths: Vec<String>,
) -> Result<(), String> {
    validate_repo_path(&repo_path, &state)?;
    for p in &paths {
        validate_git_file_path(p)?;
    }
    let mut args = vec![
        "restore".to_string(),
        "--staged".to_string(),
        "--".to_string(),
    ];
    args.extend(paths);
    let output = Command::new("git")
        .args(&args)
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn git_discard(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    paths: Vec<String>,
) -> Result<(), String> {
    validate_repo_path(&repo_path, &state)?;
    for p in &paths {
        validate_git_file_path(p)?;
    }

    let mut status_args = vec![
        "status".to_string(),
        "--porcelain".to_string(),
        "-z".to_string(),
        "-uall".to_string(),
        "--".to_string(),
    ];
    status_args.extend(paths.iter().cloned());
    let status_output = Command::new("git")
        .args(&status_args)
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&status_output.stdout);
    let mut untracked: Vec<String> = Vec::new();
    let mut tracked: Vec<String> = Vec::new();

    let path_set: std::collections::HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();

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

    if !tracked.is_empty() {
        let mut args = vec!["checkout".to_string(), "--".to_string()];
        args.extend(tracked);
        let output = Command::new("git")
            .args(&args)
            .current_dir(&repo_path)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
    }

    if !untracked.is_empty() {
        let root = state.lock().map_err(|e| e.to_string())?;
        let root = root
            .as_ref()
            .ok_or_else(|| "No project is open".to_string())?;
        for file in &untracked {
            let full_path = PathBuf::from(&repo_path).join(file);
            let canonical = if full_path.exists() {
                fs::canonicalize(&full_path).map_err(|e| format!("Invalid path: {}", e))?
            } else {
                continue;
            };
            if !canonical.starts_with(root) {
                return Err(format!(
                    "Access denied: '{}' is outside the project directory",
                    file
                ));
            }
            trash::delete(&canonical)
                .map_err(|e| format!("Failed to discard '{}': {}", file, e))?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn git_commit(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    message: String,
) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hash = stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("unknown")
        .trim_end_matches(']')
        .to_string();
    Ok(hash)
}

#[tauri::command]
pub fn git_push(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["push"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("no upstream") || stderr.contains("has no upstream branch") {
            let branch_output = Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&repo_path)
                .output()
                .map_err(|e| e.to_string())?;
            let branch_name = String::from_utf8_lossy(&branch_output.stdout)
                .trim()
                .to_string();
            if branch_name.is_empty() {
                return Err(stderr);
            }
            let retry = Command::new("git")
                .args(["push", "--set-upstream", "origin", &branch_name])
                .current_dir(&repo_path)
                .output()
                .map_err(|e| e.to_string())?;
            if !retry.status.success() {
                return Err(String::from_utf8_lossy(&retry.stderr).to_string());
            }
            return Ok(String::from_utf8_lossy(&retry.stderr).to_string());
        }
        return Err(stderr);
    }
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

#[tauri::command]
pub fn git_fetch(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["fetch"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

#[tauri::command]
pub fn git_pull(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["pull"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(format!("{}{}", stdout, stderr))
}

#[tauri::command]
pub fn git_pull_rebase(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["pull", "--rebase"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(format!("{}{}", stdout, stderr))
}

#[tauri::command]
pub fn git_delete_branch(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    branch: String,
    force: bool,
) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    if branch.contains("..") || branch.contains(' ') {
        return Err("Invalid branch name".to_string());
    }
    let head_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    let current_branch = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_string();
    if branch == current_branch {
        return Err("Cannot delete the currently checked-out branch".to_string());
    }
    let flag = if force { "-D" } else { "-d" };
    let output = Command::new("git")
        .args(["branch", flag, &branch])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
pub fn git_ahead_behind(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
) -> Result<AheadBehind, String> {
    validate_repo_path(&repo_path, &state)?;
    let upstream_out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !upstream_out.status.success() {
        return Ok(AheadBehind {
            ahead: 0,
            behind: 0,
            upstream: None,
        });
    }

    let upstream = String::from_utf8_lossy(&upstream_out.stdout)
        .trim()
        .to_string();

    let output = Command::new("git")
        .args(["rev-list", "--count", "--left-right", "HEAD...@{u}"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(AheadBehind {
            ahead: 0,
            behind: 0,
            upstream: Some(upstream),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split('\t').collect();
    let ahead = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let behind = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    Ok(AheadBehind {
        ahead,
        behind,
        upstream: Some(upstream),
    })
}

#[tauri::command]
pub fn git_diff_line_ranges(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    file_path: String,
) -> Result<Vec<DiffRange>, String> {
    validate_repo_path(&repo_path, &state)?;
    validate_git_file_path(&file_path)?;
    let output = Command::new("git")
        .args(["diff", "-U0", "--", &file_path])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
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

            if old_count == 0 && new_count > 0 {
                ranges.push(DiffRange {
                    kind: "add".to_string(),
                    start: new_start,
                    end: new_start + new_count - 1,
                });
            } else if new_count == 0 && old_count > 0 {
                ranges.push(DiffRange {
                    kind: "del".to_string(),
                    start: new_start.max(1),
                    end: new_start.max(1),
                });
            } else {
                ranges.push(DiffRange {
                    kind: "mod".to_string(),
                    start: new_start,
                    end: new_start + new_count - 1,
                });
            }
        }
    }

    Ok(ranges)
}

#[tauri::command]
pub fn git_log(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    count: Option<u32>,
) -> Result<Vec<GitGraphRow>, String> {
    validate_repo_path(&repo_path, &state)?;
    let limit = count.unwrap_or(50).min(500).to_string();
    let format = "%H\x09%h\x09%an\x09%ar\x09%s".to_string();
    let output = Command::new("git")
        .args([
            "log",
            "--graph",
            &format!("--format=format:{}", format),
            "-n",
            &limit,
        ])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut rows = Vec::new();

    for line in stdout.lines() {
        if let Some(star_pos) = line.find('*') {
            let graph_prefix = &line[..star_pos];
            let after_star = &line[star_pos + 1..].trim_start();
            let commit = if !after_star.is_empty() {
                let parts: Vec<&str> = after_star.splitn(5, '\t').collect();
                if parts.len() >= 5 {
                    Some(GitLogCommit {
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
            let graph = format!("{}*", graph_prefix);
            rows.push(GitGraphRow { graph, commit });
        } else {
            rows.push(GitGraphRow {
                graph: line.to_string(),
                commit: None,
            });
        }
    }

    Ok(rows)
}

#[tauri::command]
pub fn git_list_branches(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
) -> Result<Vec<BranchInfo>, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["branch", "-a", "--no-color"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
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

#[tauri::command]
pub fn git_checkout_branch(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    branch: String,
    is_remote: bool,
) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    if branch.contains("..") || branch.contains(' ') {
        return Err("Invalid branch name".to_string());
    }

    let output = if is_remote {
        let local_name = branch.split('/').skip(1).collect::<Vec<&str>>().join("/");
        if local_name.is_empty() {
            return Err("Invalid remote branch name".to_string());
        }
        Command::new("git")
            .args(["checkout", "--track", &format!("remotes/{}", branch)])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("git")
            .args(["checkout", &branch])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| e.to_string())?
    };

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

#[tauri::command]
pub fn git_resolve_conflict(
    state: tauri::State<'_, ProjectRootState>,
    repo_path: String,
    file_path: String,
    content: String,
    stage: bool,
) -> Result<(), String> {
    let canonical_repo = validate_repo_path(&repo_path, &state)?;
    validate_git_file_path(&file_path)?;

    let abs_path = canonical_repo.join(&file_path);

    if !abs_path.starts_with(&canonical_repo) {
        return Err("Access denied: file path is outside the repository".to_string());
    }

    fs::write(&abs_path, &content).map_err(|e| format!("Failed to write file: {}", e))?;

    if stage {
        let output = Command::new("git")
            .args(["add", "--", &file_path])
            .current_dir(&canonical_repo)
            .output()
            .map_err(|e| format!("Failed to run git add: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git add failed: {}", stderr));
        }
    }

    Ok(())
}
