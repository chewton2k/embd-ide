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
    // Sync command path: see fs::set_project_root for why blocking_read
    // is safe here (Tauri sync commands run off the tokio worker pool).
    let root = state.blocking_read();
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
    if file_path.starts_with('-') {
        return Err("Invalid file path: cannot start with '-' (flag injection)".to_string());
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

/// Validate a git ref name per git-check-ref-format(1).
/// Rejects names that could be misinterpreted as flags or contain illegal characters.
pub fn validate_git_ref_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Invalid ref name: cannot be empty".to_string());
    }
    if name.starts_with('-') {
        return Err("Invalid ref name: cannot start with '-'".to_string());
    }
    if name.starts_with('.') {
        return Err("Invalid ref name: cannot start with '.'".to_string());
    }
    if name.ends_with('/') || name.starts_with('/') {
        return Err("Invalid ref name: cannot start or end with '/'".to_string());
    }
    if name.ends_with(".lock") {
        return Err("Invalid ref name: cannot end with '.lock'".to_string());
    }
    if name == "@" {
        return Err("Invalid ref name: '@' alone is not allowed".to_string());
    }
    if name.contains("..") || name.contains("@{") || name.contains("//") || name.contains("/.") {
        return Err("Invalid ref name: contains forbidden sequence".to_string());
    }
    for byte in name.bytes() {
        match byte {
            0..=0x1F | 0x7F => return Err("Invalid ref name: control characters not allowed".to_string()),
            b' ' | b'\\' | b':' | b'?' | b'*' | b'[' | b'~' | b'^' => {
                return Err(format!("Invalid ref name: character '{}' not allowed", byte as char));
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

/// One row from `git status --porcelain -z` output. The full stdout is
/// a sequence of NUL-separated entries; rename ('R') and copy ('C')
/// entries are followed by an additional entry containing the source
/// path. Parsing as raw bytes (not `String::from_utf8_lossy` over the
/// whole buffer) preserves non-UTF-8 path bytes per-entry rather than
/// smearing replacement characters across the whole buffer.
pub(crate) struct PorcelainEntry {
    pub index_status: u8,
    pub wt_status: u8,
    pub file: String,
    /// Source path for renames/copies. `None` for everything else.
    #[allow(dead_code)] // present for future consumers; not used yet
    pub orig: Option<String>,
}

/// Parse byte output from `git status --porcelain -z`.
///
/// The `-z` flag produces NUL-separated entries with no quoting, so we
/// can treat the buffer as `&[u8]` and only convert per-entry. Each
/// entry has the layout `XY <path>` where X is the index status, Y is
/// the working-tree status, and one space follows. Rename and copy
/// entries are followed by an extra NUL-separated entry containing the
/// source path.
pub(crate) fn parse_status_porcelain_z(stdout: &[u8]) -> Vec<PorcelainEntry> {
    let entries: Vec<&[u8]> = stdout.split(|&b| b == 0).collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < entries.len() {
        let entry = entries[i];
        if entry.len() < 4 {
            i += 1;
            continue;
        }
        let index_status = entry[0];
        let wt_status = entry[1];
        // entry[2] is a space separator; the path is everything after.
        let file_bytes = &entry[3..];
        let file = String::from_utf8_lossy(file_bytes).into_owned();

        let orig = if (index_status == b'R' || index_status == b'C')
            && i + 1 < entries.len()
            && !entries[i + 1].is_empty()
        {
            i += 1;
            Some(String::from_utf8_lossy(entries[i]).into_owned())
        } else {
            None
        };

        result.push(PorcelainEntry {
            index_status,
            wt_status,
            file,
            orig,
        });
        i += 1;
    }
    result
}

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

    let mut result = HashMap::new();
    for entry in parse_status_porcelain_z(&output.stdout) {
        let abs_path = PathBuf::from(&path).join(&entry.file);
        let abs_str = abs_path.to_string_lossy().to_string();

        let status = match (entry.index_status, entry.wt_status) {
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

    // Parse line-by-line over the raw byte buffer rather than running
    // `String::from_utf8_lossy` over the whole output up-front. This
    // confines any UTF-8 replacement characters to a single line if a
    // path happens to contain non-UTF-8 bytes, instead of smearing
    // them across the buffer.
    let mut result = HashMap::new();

    for line_bytes in output.stdout.split(|&b| b == b'\n') {
        if line_bytes.is_empty() {
            continue;
        }
        let line_cow = String::from_utf8_lossy(line_bytes);
        let line = line_cow.trim();
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
    let root = state.blocking_read();
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
    let mut untracked: Vec<String> = Vec::new();
    let mut tracked: Vec<String> = Vec::new();

    let path_set: std::collections::HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();

    for entry in parse_status_porcelain_z(&status_output.stdout) {
        if path_set.contains(entry.file.as_str()) {
            if entry.index_status == b'?' && entry.wt_status == b'?' {
                untracked.push(entry.file);
            } else {
                tracked.push(entry.file);
            }
        }
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
        let root = state.blocking_read();
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
    validate_git_ref_name(&branch)?;
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
    if is_remote {
        // For remote branches like "origin/feature", validate the local name portion
        let local_name = branch.split('/').skip(1).collect::<Vec<&str>>().join("/");
        if local_name.is_empty() {
            return Err("Invalid remote branch name".to_string());
        }
        validate_git_ref_name(&local_name)?;
    } else {
        validate_git_ref_name(&branch)?;
    }

    let output = if is_remote {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_git_ref_name_rejects_bad_inputs() {
        let bad = vec![
            "", "-x", "..", "foo..bar", "foo bar", "\x01ctrl", "foo:bar",
            "foo?", "foo*", "foo[", "foo~", "foo^", "foo\\bar", ".lock",
            "foo.lock", "/foo", "foo/", "foo//bar", "foo/.bar", "@", "foo@{",
            "foo\0bar",
        ];
        for input in bad {
            assert!(
                validate_git_ref_name(input).is_err(),
                "Expected rejection for: {:?}",
                input
            );
        }
    }

    #[test]
    fn test_validate_git_ref_name_accepts_good_inputs() {
        let good = vec![
            "main",
            "feature/login",
            "release-1.0",
            "fix_bug",
            "dependabot/npm/foo-1.2.3",
        ];
        for input in good {
            assert!(
                validate_git_ref_name(input).is_ok(),
                "Expected acceptance for: {:?}",
                input
            );
        }
    }

    #[test]
    fn test_validate_git_file_path_rejects_leading_dash() {
        assert!(validate_git_file_path("--exec=evil").is_err());
        assert!(validate_git_file_path("-flag").is_err());
    }

    #[test]
    fn test_validate_git_file_path_rejects_traversal() {
        assert!(validate_git_file_path("../etc/passwd").is_err());
        assert!(validate_git_file_path("foo/../../bar").is_err());
    }

    #[test]
    fn test_validate_git_file_path_rejects_absolute() {
        assert!(validate_git_file_path("/etc/passwd").is_err());
    }

    #[test]
    fn test_validate_git_file_path_rejects_git_dir() {
        assert!(validate_git_file_path(".git/config").is_err());
    }

    #[test]
    fn test_validate_git_file_path_accepts_valid() {
        assert!(validate_git_file_path("src/main.rs").is_ok());
        assert!(validate_git_file_path("README.md").is_ok());
        assert!(validate_git_file_path("path/to/file.txt").is_ok());
    }

    // ── Byte-level porcelain-z parser tests (C6) ────────────────

    fn build_z(parts: &[&[u8]]) -> Vec<u8> {
        // Helper: join byte slices with NUL separators, ending with a
        // trailing NUL just like real `git status -z` output.
        let mut out = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            out.extend_from_slice(p);
            if i + 1 < parts.len() {
                out.push(0);
            }
        }
        out.push(0);
        out
    }

    #[test]
    fn test_parse_porcelain_z_ascii_paths() {
        // Two simple modifications, one untracked file.
        let raw = build_z(&[
            b" M src/main.rs",
            b"M  Cargo.toml",
            b"?? notes.txt",
        ]);
        let entries = parse_status_porcelain_z(&raw);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].file, "src/main.rs");
        assert_eq!(entries[0].index_status, b' ');
        assert_eq!(entries[0].wt_status, b'M');
        assert!(entries[0].orig.is_none());

        assert_eq!(entries[1].file, "Cargo.toml");
        assert_eq!(entries[1].index_status, b'M');
        assert_eq!(entries[1].wt_status, b' ');

        assert_eq!(entries[2].file, "notes.txt");
        assert_eq!(entries[2].index_status, b'?');
        assert_eq!(entries[2].wt_status, b'?');
    }

    #[test]
    fn test_parse_porcelain_z_handles_utf8_cjk_paths() {
        // Mandarin filename — valid UTF-8, should round-trip cleanly.
        let path = "src/笔记.md";
        let raw = build_z(&[format!(" M {path}").as_bytes()]);
        let entries = parse_status_porcelain_z(&raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, path);
    }

    #[test]
    fn test_parse_porcelain_z_handles_rename_pair() {
        // R<status> NEW \0 OLD \0 — ensure we consume both entries
        // and remember the source path under `orig`.
        let raw = build_z(&[b"R  src/new.rs", b"src/old.rs", b" M README.md"]);
        let entries = parse_status_porcelain_z(&raw);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file, "src/new.rs");
        assert_eq!(entries[0].orig.as_deref(), Some("src/old.rs"));
        // Critical: the next entry is the unrelated README modification,
        // NOT the consumed source path.
        assert_eq!(entries[1].file, "README.md");
        assert!(entries[1].orig.is_none());
    }

    #[test]
    fn test_parse_porcelain_z_handles_copy_pair() {
        let raw = build_z(&[b"C  copy.rs", b"original.rs"]);
        let entries = parse_status_porcelain_z(&raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "copy.rs");
        assert_eq!(entries[0].orig.as_deref(), Some("original.rs"));
    }

    #[test]
    fn test_parse_porcelain_z_skips_short_entries() {
        // Real `-z` output includes a trailing NUL, which produces an
        // empty trailing entry. Anything < 4 bytes is skipped.
        let raw = build_z(&[b" M ok.rs", b"", b"x"]);
        let entries = parse_status_porcelain_z(&raw);
        // Only the valid entry survives.
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "ok.rs");
    }

    #[test]
    fn test_parse_porcelain_z_empty_input() {
        let entries = parse_status_porcelain_z(b"");
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_porcelain_z_preserves_non_utf8_paths_lossily() {
        // Construct an entry whose path contains invalid UTF-8 (a lone
        // 0xFF byte). The parser must NOT crash; the path becomes a
        // lossy string but the rest of the parse still works.
        let mut raw = Vec::new();
        raw.extend_from_slice(b" M ok\xFFbad.rs");
        raw.push(0);
        raw.extend_from_slice(b" M next.rs");
        raw.push(0);
        let entries = parse_status_porcelain_z(&raw);
        assert_eq!(entries.len(), 2);
        // The next entry is unaffected — this is the win over the
        // previous "lossy the whole buffer first" approach where a
        // malformed path could shift offsets.
        assert_eq!(entries[1].file, "next.rs");
    }

    #[test]
    fn test_parse_porcelain_z_truncated_rename_does_not_consume_trailing_nul() {
        // Edge case: a rename entry with no source path, where the
        // buffer simply ends with the trailing NUL that real git output
        // always emits. The parser must not treat the empty trailing
        // slice as a valid source path; orig stays None.
        let raw = build_z(&[b"R  src/new.rs"]);
        let entries = parse_status_porcelain_z(&raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "src/new.rs");
        assert!(entries[0].orig.is_none(),
            "truncated rename must not produce an empty-string source");
    }
}
