use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use base64::Engine;

pub type ProjectRootState = Arc<Mutex<Option<PathBuf>>>;

pub fn create_project_root_state() -> ProjectRootState {
    Arc::new(Mutex::new(None))
}

#[tauri::command]
pub fn set_project_root(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<(), String> {
    let canonical = fs::canonicalize(&path).map_err(|e| format!("Invalid path: {}", e))?;
    let mut root = state.lock().map_err(|e| e.to_string())?;
    *root = Some(canonical);
    Ok(())
}

/// Validate that a path is within the current project root.
/// Returns the canonicalized path on success.
fn validate_path(path: &str, state: &tauri::State<'_, ProjectRootState>) -> Result<PathBuf, String> {
    let root = state.lock().map_err(|e| e.to_string())?;
    let root = root.as_ref().ok_or_else(|| "No project is open".to_string())?;

    let p = PathBuf::from(path);
    // For paths that don't exist yet (create_file/create_folder), canonicalize the parent
    let canonical = if p.exists() {
        fs::canonicalize(&p).map_err(|e| format!("Invalid path: {}", e))?
    } else if let Some(parent) = p.parent() {
        if parent.exists() {
            let canon_parent = fs::canonicalize(parent).map_err(|e| format!("Invalid path: {}", e))?;
            canon_parent.join(p.file_name().ok_or("Invalid file name")?)
        } else {
            // Parent doesn't exist either — check that the ultimate ancestor is in root
            PathBuf::from(path)
        }
    } else {
        return Err("Invalid path".to_string());
    };

    if !canonical.starts_with(root) {
        return Err("Access denied: path is outside the project directory".to_string());
    }

    // Deny access to .git internals for write/delete operations
    // (read is ok for branch detection etc.)
    Ok(canonical)
}

/// Validate that a repo_path matches the project root for git commands.
fn validate_repo_path(repo_path: &str, state: &tauri::State<'_, ProjectRootState>) -> Result<PathBuf, String> {
    let root = state.lock().map_err(|e| e.to_string())?;
    let root = root.as_ref().ok_or_else(|| "No project is open".to_string())?;
    let canonical = fs::canonicalize(repo_path).map_err(|e| format!("Invalid repo path: {}", e))?;
    if !canonical.starts_with(root) && !root.starts_with(&canonical) {
        return Err("Access denied: repo path is outside the project directory".to_string());
    }
    Ok(canonical)
}

#[derive(Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

#[tauri::command]
pub fn read_dir_tree(state: tauri::State<'_, ProjectRootState>, path: String, depth: Option<u32>) -> Result<Vec<FileEntry>, String> {
    validate_path(&path, &state)?;
    let max_depth = depth.unwrap_or(1).min(50); // Cap depth to prevent abuse
    read_dir_recursive(&PathBuf::from(path), 0, max_depth)
}

fn read_dir_recursive(path: &PathBuf, current_depth: u32, max_depth: u32) -> Result<Vec<FileEntry>, String> {
    let entries = fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut result: Vec<FileEntry> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip .git — it's huge and never useful to browse
        if file_name == ".git" {
            continue;
        }

        let file_path = entry.path();
        let is_dir = file_path.is_dir();

        let children = if is_dir && current_depth < max_depth {
            Some(read_dir_recursive(&file_path, current_depth + 1, max_depth).unwrap_or_default())
        } else if is_dir {
            Some(Vec::new()) // Indicate it's expandable
        } else {
            None
        };

        result.push(FileEntry {
            name: file_name,
            path: file_path.to_string_lossy().to_string(),
            is_dir,
            children,
        });
    }

    // Sort: directories first, then alphabetical
    result.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(result)
}

#[tauri::command]
pub fn read_file_content(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<String, String> {
    validate_path(&path, &state)?;
    fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e.kind()))
}

#[tauri::command]
pub fn write_file_content(state: tauri::State<'_, ProjectRootState>, path: String, content: String) -> Result<(), String> {
    validate_path(&path, &state)?;
    fs::write(&path, &content).map_err(|e| format!("Failed to write file: {}", e.kind()))
}

#[tauri::command]
pub fn read_file_binary(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<String, String> {
    validate_path(&path, &state)?;
    let bytes = fs::read(&path).map_err(|e| format!("Failed to read file: {}", e.kind()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

#[tauri::command]
pub fn create_file(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<(), String> {
    validate_path(&path, &state)?;
    let p = PathBuf::from(&path);
    if p.exists() {
        return Err("File already exists".to_string());
    }
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&p, "").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_folder(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<(), String> {
    validate_path(&path, &state)?;
    let p = PathBuf::from(&path);
    if p.exists() {
        return Err("Folder already exists".to_string());
    }
    fs::create_dir_all(&p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_entries(state: tauri::State<'_, ProjectRootState>, paths: Vec<String>) -> Result<(), String> {
    for path in &paths {
        validate_path(path, &state)?;
    }
    for path in paths {
        let p = PathBuf::from(&path);
        if !p.exists() {
            continue;
        }
        trash::delete(&p).map_err(|e| format!("Failed to move to trash: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn rename_entry(state: tauri::State<'_, ProjectRootState>, old_path: String, new_path: String) -> Result<(), String> {
    validate_path(&old_path, &state)?;
    validate_path(&new_path, &state)?;
    fs::rename(&old_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))
}

#[tauri::command]
pub fn move_entries(state: tauri::State<'_, ProjectRootState>, sources: Vec<String>, dest_dir: String) -> Result<(), String> {
    for src in &sources {
        validate_path(src, &state)?;
    }
    validate_path(&dest_dir, &state)?;

    let dest = fs::canonicalize(&dest_dir).map_err(|e| format!("Invalid destination: {}", e))?;
    if !dest.is_dir() {
        return Err("Destination is not a directory".into());
    }

    for src in &sources {
        let src_path = fs::canonicalize(src).map_err(|e| format!("Invalid source: {}", e))?;
        // Prevent moving a folder into itself or its descendants
        if dest.starts_with(&src_path) {
            return Err(format!("Cannot move '{}' into itself or a subdirectory", src));
        }
        let file_name = src_path.file_name().ok_or("Invalid source file name")?;
        let dst_path = dest.join(file_name);
        // Skip if already in the same location
        if src_path == dst_path {
            continue;
        }
        fs::rename(&src_path, &dst_path).map_err(|e| format!("Failed to move '{}': {}", src, e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn import_external_files(state: tauri::State<'_, ProjectRootState>, sources: Vec<String>, dest_dir: String) -> Result<(), String> {
    // Only validate destination is within project root (sources are from outside)
    validate_path(&dest_dir, &state)?;
    let dest = PathBuf::from(&dest_dir);
    if !dest.is_dir() {
        return Err("Destination is not a directory".to_string());
    }
    for src in sources {
        let src_path = PathBuf::from(&src);
        if !src_path.exists() {
            return Err(format!("Source does not exist: {}", src));
        }
        let file_name = src_path
            .file_name()
            .ok_or_else(|| format!("Invalid source path: {}", src))?;
        let mut target = dest.join(file_name);
        // Avoid overwriting — add " copy" suffix if needed
        if target.exists() {
            let stem = target.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let ext = target.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            let mut i = 1;
            loop {
                if i > 10_000 {
                    return Err("Too many copies exist".to_string());
                }
                let name = if i == 1 {
                    format!("{} copy{}", stem, ext)
                } else {
                    format!("{} copy {}{}", stem, i, ext)
                };
                target = dest.join(&name);
                if !target.exists() { break; }
                i += 1;
            }
        }
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &target)
                .map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        } else {
            fs::copy(&src_path, &target)
                .map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn paste_entries(state: tauri::State<'_, ProjectRootState>, sources: Vec<String>, dest_dir: String) -> Result<(), String> {
    for src in &sources {
        validate_path(src, &state)?;
    }
    validate_path(&dest_dir, &state)?;
    let dest = PathBuf::from(&dest_dir);
    if !dest.is_dir() {
        return Err("Destination is not a directory".to_string());
    }
    for src in sources {
        let src_path = PathBuf::from(&src);
        let file_name = src_path
            .file_name()
            .ok_or_else(|| format!("Invalid source path: {}", src))?;
        let mut target = dest.join(file_name);
        // Avoid overwriting — add " copy" suffix if needed
        if target.exists() {
            let stem = target.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let ext = target.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            let mut i = 1;
            loop {
                if i > 10_000 {
                    return Err("Too many copies exist".to_string());
                }
                let name = if i == 1 {
                    format!("{} copy{}", stem, ext)
                } else {
                    format!("{} copy {}{}", stem, i, ext)
                };
                target = dest.join(&name);
                if !target.exists() { break; }
                i += 1;
            }
        }
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &target)
                .map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        } else {
            fs::copy(&src_path, &target)
                .map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn duplicate_entry(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<(), String> {
    validate_path(&path, &state)?;
    let src_path = PathBuf::from(&path);
    if !src_path.exists() {
        return Err("Path does not exist".to_string());
    }
    let parent = src_path.parent().ok_or("No parent directory")?;
    let stem = src_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = src_path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let is_dir = src_path.is_dir();

    let mut target;
    let mut i = 1;
    loop {
        if i > 10_000 {
            return Err("Too many copies exist".to_string());
        }
        let name = if i == 1 {
            if is_dir {
                format!("{} copy", stem)
            } else {
                format!("{} copy{}", stem, ext)
            }
        } else if is_dir {
            format!("{} copy {}", stem, i)
        } else {
            format!("{} copy {}{}", stem, i, ext)
        };
        target = parent.join(&name);
        if !target.exists() { break; }
        i += 1;
    }

    if is_dir {
        copy_dir_recursive(&src_path, &target)?;
    } else {
        fs::copy(&src_path, &target).map_err(|e| format!("Failed to duplicate: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn reveal_in_file_manager(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        // Try xdg-open on the parent directory
        let parent = PathBuf::from(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path);
        Command::new("xdg-open")
            .arg(&parent)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_all_files(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<Vec<String>, String> {
    validate_path(&path, &state)?;
    let root = PathBuf::from(&path);
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files, 0);
    Ok(files)
}

const MAX_COLLECT_DEPTH: u32 = 100;
const MAX_COLLECT_FILES: usize = 100_000;

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>, depth: u32) {
    if depth > MAX_COLLECT_DEPTH || out.len() >= MAX_COLLECT_FILES {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_COLLECT_FILES { return; }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "node_modules" || name == "target" || name == ".DS_Store" {
            continue;
        }
        // Skip symlinks to prevent cycles
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() {
                continue;
            }
        }
        if path.is_dir() {
            collect_files(root, &path, out, depth + 1);
        } else {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
}

/// Returns a map of relative_path -> status_code for all changed files.
/// Status codes: "M" = modified (unstaged), "A" = staged new, "S" = staged modified,
/// "D" = deleted, "?" = untracked, "R" = renamed
#[tauri::command]
pub fn get_git_status(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<HashMap<String, String>, String> {
    validate_repo_path(&path, &state)?;
    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall", "-z"])
        .current_dir(&path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(HashMap::new()); // Not a git repo or git error
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut result = HashMap::new();

    // With -z flag, entries are NUL-separated and paths are not quoted
    let entries: Vec<&str> = stdout.split('\0').collect();
    let mut i = 0;
    while i < entries.len() {
        let entry = entries[i];
        if entry.len() < 4 { i += 1; continue; }
        let index_status = entry.as_bytes()[0];
        let wt_status = entry.as_bytes()[1];
        let file_path = &entry[3..];

        // For renames/copies, -z puts the original path as the next NUL-separated entry
        let file_path = if index_status == b'R' || index_status == b'C' {
            i += 1; // skip the original path
            file_path
        } else {
            file_path
        };

        let abs_path = PathBuf::from(&path).join(file_path);
        let abs_str = abs_path.to_string_lossy().to_string();

        // Determine status — staged takes priority display, but show modified if only worktree changed
        let status = match (index_status, wt_status) {
            (b'?', b'?') => "U",  // untracked
            (b'A', _) => "A",     // staged new file
            (b'R', _) => "A",     // renamed (treat as staged)
            (b'M', b' ') | (b'M', b'\0') => "S", // staged modified only
            (b'D', _) | (_, b'D') => "D", // deleted
            (_, b'M') => "M",     // modified in worktree (includes staged + further modified)
            _ => "M",
        };

        result.insert(abs_str, status.to_string());
        i += 1;
    }

    Ok(result)
}

#[tauri::command]
pub fn get_git_ignored(state: tauri::State<'_, ProjectRootState>, path: String) -> Result<Vec<String>, String> {
    validate_repo_path(&path, &state)?;
    // List all files, then use git check-ignore to filter
    let output = Command::new("git")
        .args(["ls-files", "--others", "--ignored", "--exclude-standard", "--directory"])
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

// ── Git panel commands ───────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct DiffLine {
    pub kind: String, // "add", "del", "ctx"
    pub old_num: Option<u32>,
    pub new_num: Option<u32>,
    pub text: String,
}

#[tauri::command]
pub fn git_diff(state: tauri::State<'_, ProjectRootState>, repo_path: String, file_path: String, staged: bool) -> Result<Vec<DiffLine>, String> {
    validate_repo_path(&repo_path, &state)?;
    // Ensure file_path is relative and has no traversal
    if file_path.contains("..") {
        return Err("Invalid file path".to_string());
    }
    // Check if file is untracked
    let status_out = Command::new("git")
        .args(["status", "--porcelain", "--", &file_path])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    let status_str = String::from_utf8_lossy(&status_out.stdout);
    let is_untracked = status_str.lines().any(|l| l.starts_with("??"));

    let output = if is_untracked {
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

fn parse_unified_diff(diff: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old_line: u32 = 0;
    let mut new_line: u32 = 0;
    let mut in_hunk = false;

    for raw in diff.lines() {
        if raw.starts_with("@@") {
            // Parse hunk header: @@ -old,count +new,count @@
            in_hunk = true;
            if let Some(rest) = raw.strip_prefix("@@ -") {
                let parts: Vec<&str> = rest.splitn(2, '+').collect();
                if parts.len() == 2 {
                    old_line = parts[0].split(',').next().unwrap_or("1").trim().parse().unwrap_or(1);
                    new_line = parts[1].split(|c: char| c == ',' || c == ' ').next().unwrap_or("1").parse().unwrap_or(1);
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
            // Context line (starts with ' ' or is empty)
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

#[tauri::command]
pub fn git_stage(state: tauri::State<'_, ProjectRootState>, repo_path: String, paths: Vec<String>) -> Result<(), String> {
    validate_repo_path(&repo_path, &state)?;
    for p in &paths {
        if p.contains("..") { return Err("Invalid file path".to_string()); }
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
pub fn git_unstage(state: tauri::State<'_, ProjectRootState>, repo_path: String, paths: Vec<String>) -> Result<(), String> {
    validate_repo_path(&repo_path, &state)?;
    for p in &paths {
        if p.contains("..") { return Err("Invalid file path".to_string()); }
    }
    let mut args = vec!["restore".to_string(), "--staged".to_string(), "--".to_string()];
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
pub fn git_discard(state: tauri::State<'_, ProjectRootState>, repo_path: String, paths: Vec<String>) -> Result<(), String> {
    validate_repo_path(&repo_path, &state)?;
    for p in &paths {
        if p.contains("..") { return Err("Invalid file path".to_string()); }
    }

    // Separate tracked (modified/deleted) from untracked files
    let status_output = Command::new("git")
        .args(["status", "--porcelain", "-z", "-uall"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&status_output.stdout);
    let mut untracked: Vec<String> = Vec::new();
    let mut tracked: Vec<String> = Vec::new();

    let entries: Vec<&str> = stdout.split('\0').collect();
    let mut idx = 0;
    while idx < entries.len() {
        let entry = entries[idx];
        if entry.len() < 4 { idx += 1; continue; }
        let ix = entry.as_bytes()[0];
        let wt = entry.as_bytes()[1];
        let file = &entry[3..];

        // Skip rename's extra entry
        if ix == b'R' || ix == b'C' {
            idx += 1;
        }

        if paths.contains(&file.to_string()) {
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

    // Remove untracked files
    if !untracked.is_empty() {
        for file in &untracked {
            let full_path = PathBuf::from(&repo_path).join(file);
            if full_path.is_dir() {
                fs::remove_dir_all(&full_path).map_err(|e| e.to_string())?;
            } else {
                fs::remove_file(&full_path).map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn git_commit(state: tauri::State<'_, ProjectRootState>, repo_path: String, message: String) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    // Extract commit hash from output
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
pub fn git_push(state: tauri::State<'_, ProjectRootState>, repo_path: String) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    let output = Command::new("git")
        .args(["push"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(stderr);
    }
    Ok(String::from_utf8_lossy(&output.stderr).to_string()) // git push outputs to stderr
}

#[derive(Serialize, Clone)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
    pub upstream: Option<String>,
}

#[tauri::command]
pub fn git_ahead_behind(state: tauri::State<'_, ProjectRootState>, repo_path: String) -> Result<AheadBehind, String> {
    validate_repo_path(&repo_path, &state)?;
    // Get upstream name
    let upstream_out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !upstream_out.status.success() {
        return Ok(AheadBehind { ahead: 0, behind: 0, upstream: None });
    }

    let upstream = String::from_utf8_lossy(&upstream_out.stdout).trim().to_string();

    let output = Command::new("git")
        .args(["rev-list", "--count", "--left-right", "HEAD...@{u}"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(AheadBehind { ahead: 0, behind: 0, upstream: Some(upstream) });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split('\t').collect();
    let ahead = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let behind = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    Ok(AheadBehind { ahead, behind, upstream: Some(upstream) })
}

#[derive(Serialize, Clone)]
pub struct DiffRange {
    pub kind: String, // "add", "del", "mod"
    pub start: u32,
    pub end: u32,
}

#[tauri::command]
pub fn git_diff_line_ranges(state: tauri::State<'_, ProjectRootState>, repo_path: String, file_path: String) -> Result<Vec<DiffRange>, String> {
    validate_repo_path(&repo_path, &state)?;
    if file_path.contains("..") {
        return Err("Invalid file path".to_string());
    }
    let output = Command::new("git")
        .args(["diff", "-U0", "--", &file_path])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ranges = Vec::new();

    for line in stdout.lines() {
        if !line.starts_with("@@") { continue; }
        // Parse @@ -old[,count] +new[,count] @@
        if let Some(rest) = line.strip_prefix("@@ -") {
            let parts: Vec<&str> = rest.splitn(2, '+').collect();
            if parts.len() != 2 { continue; }

            let old_part = parts[0].trim().trim_end_matches(',');
            let new_part = parts[1].split_whitespace().next().unwrap_or("0");

            let (_old_start, old_count) = parse_hunk_range(old_part);
            let (new_start, new_count) = parse_hunk_range(new_part);

            if old_count == 0 && new_count > 0 {
                // Pure addition
                ranges.push(DiffRange {
                    kind: "add".to_string(),
                    start: new_start,
                    end: new_start + new_count - 1,
                });
            } else if new_count == 0 && old_count > 0 {
                // Pure deletion — mark the line after deletion point
                ranges.push(DiffRange {
                    kind: "del".to_string(),
                    start: new_start.max(1),
                    end: new_start.max(1),
                });
            } else {
                // Modification
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

#[tauri::command]
pub fn git_log(state: tauri::State<'_, ProjectRootState>, repo_path: String, count: Option<u32>) -> Result<Vec<GitGraphRow>, String> {
    validate_repo_path(&repo_path, &state)?;
    let limit = count.unwrap_or(50).min(500).to_string();
    // Use a unique separator unlikely to appear in commit messages
    let format = format!("%H\x09%h\x09%an\x09%ar\x09%s");
    let output = Command::new("git")
        .args(["log", "--graph", &format!("--format=format:{}", format), "--all", "-n", &limit])
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
            // Commit line: graph chars before *, commit data after "* "
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
            // Reconstruct graph: prefix + "*" + rest of graph chars after the data
            let mut graph = format!("{}*", graph_prefix);
            // Pad with spaces from the line structure after the commit node
            // We need the graph chars that come after * on the same line
            // In git's output, after "* <commit data>", there are no trailing graph chars
            // But for merge lines like "|\  ", the graph is the whole line
            graph = graph.to_string();
            rows.push(GitGraphRow { graph, commit });
        } else {
            // Graph-only line (connector line between commits)
            rows.push(GitGraphRow {
                graph: line.to_string(),
                commit: None,
            });
        }
    }

    Ok(rows)
}

fn parse_hunk_range(s: &str) -> (u32, u32) {
    let parts: Vec<&str> = s.split(',').collect();
    let start: u32 = parts[0].parse().unwrap_or(0);
    let count: u32 = if parts.len() > 1 { parts[1].parse().unwrap_or(1) } else { 1 };
    (start, count)
}

#[derive(Serialize, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

#[tauri::command]
pub fn git_list_branches(state: tauri::State<'_, ProjectRootState>, repo_path: String) -> Result<Vec<BranchInfo>, String> {
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
        if trimmed.is_empty() { continue; }
        // Skip HEAD pointer lines like "remotes/origin/HEAD -> origin/main"
        if trimmed.contains("->") { continue; }

        let is_current = trimmed.starts_with('*');
        let name = trimmed.trim_start_matches("* ").trim_start_matches("remotes/").to_string();
        let is_remote = line.contains("remotes/");

        branches.push(BranchInfo { name, is_current, is_remote });
    }

    // Sort: current first, then local, then remote
    branches.sort_by(|a, b| {
        b.is_current.cmp(&a.is_current)
            .then(a.is_remote.cmp(&b.is_remote))
            .then(a.name.cmp(&b.name))
    });

    Ok(branches)
}

#[tauri::command]
pub fn git_checkout_branch(state: tauri::State<'_, ProjectRootState>, repo_path: String, branch: String, is_remote: bool) -> Result<String, String> {
    validate_repo_path(&repo_path, &state)?;
    // Sanitize branch name
    if branch.contains("..") || branch.contains(' ') {
        return Err("Invalid branch name".to_string());
    }

    let output = if is_remote {
        // For remote branches like "origin/feature", track as local "feature"
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
pub fn get_git_branch(path: String) -> Result<Option<String>, String> {
    let mut dir = PathBuf::from(&path);
    // Walk up to find .git directory
    loop {
        let git_dir = dir.join(".git");
        if git_dir.exists() {
            let head_file = git_dir.join("HEAD");
            if let Ok(content) = fs::read_to_string(&head_file) {
                let content = content.trim();
                if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
                    return Ok(Some(branch.to_string()));
                }
                // Detached HEAD - show short hash
                return Ok(Some(content[..7.min(content.len())].to_string()));
            }
            return Ok(None);
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}
