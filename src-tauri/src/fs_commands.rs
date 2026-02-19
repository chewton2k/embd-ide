use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use base64::Engine;

#[derive(Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

#[tauri::command]
pub fn read_dir_tree(path: String, depth: Option<u32>) -> Result<Vec<FileEntry>, String> {
    let max_depth = depth.unwrap_or(1);
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
pub fn read_file_content(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path, e))
}

#[tauri::command]
pub fn write_file_content(path: String, content: String) -> Result<(), String> {
    fs::write(&path, &content).map_err(|e| format!("Failed to write {}: {}", path, e))
}

#[tauri::command]
pub fn read_file_binary(path: String) -> Result<String, String> {
    let bytes = fs::read(&path).map_err(|e| format!("Failed to read {}: {}", path, e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
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
pub fn create_folder(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if p.exists() {
        return Err("Folder already exists".to_string());
    }
    fs::create_dir_all(&p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_entries(paths: Vec<String>) -> Result<(), String> {
    for path in paths {
        let p = PathBuf::from(&path);
        if !p.exists() {
            continue;
        }
        if p.is_dir() {
            fs::remove_dir_all(&p).map_err(|e| format!("Failed to delete {}: {}", path, e))?;
        } else {
            fs::remove_file(&p).map_err(|e| format!("Failed to delete {}: {}", path, e))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rename_entry(old_path: String, new_path: String) -> Result<(), String> {
    fs::rename(&old_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))
}

#[tauri::command]
pub fn paste_entries(sources: Vec<String>, dest_dir: String) -> Result<(), String> {
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
pub fn list_all_files(path: String) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&path);
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files);
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "node_modules" || name == "target" || name == ".DS_Store" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out);
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
pub fn get_git_status(path: String) -> Result<HashMap<String, String>, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(&path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(HashMap::new()); // Not a git repo or git error
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut result = HashMap::new();

    for line in stdout.lines() {
        if line.len() < 4 { continue; }
        let index_status = line.as_bytes()[0];
        let wt_status = line.as_bytes()[1];
        let file_path = &line[3..];

        // Handle renames: "R  old -> new"
        let file_path = if let Some(arrow_pos) = file_path.find(" -> ") {
            &file_path[arrow_pos + 4..]
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
    }

    Ok(result)
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
