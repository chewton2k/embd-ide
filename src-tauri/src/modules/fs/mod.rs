use base64::Engine;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

/// The project root path is read by every command that performs path
/// validation (every fs/git/shell/graph/knowledge command). Using
/// `tokio::sync::RwLock` lets many concurrent commands hold a shared
/// read lock at once and only serializes writers (currently just
/// `set_project_root` and the knowledge-init bootstrap path).
///
/// We expose the lock as `RwLock<Option<PathBuf>>` rather than baking the
/// type behind a façade because every consumer site already deals with
/// the `Option` explicitly.
pub type ProjectRootState = Arc<RwLock<Option<PathBuf>>>;

const MAX_TEXT_FILE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB
const MAX_BINARY_FILE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

pub fn create_project_root_state() -> ProjectRootState {
    Arc::new(RwLock::new(None))
}

#[tauri::command]
pub fn set_project_root(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<String, String> {
    let canonical = fs::canonicalize(&path).map_err(|e| format!("Invalid path: {}", e))?;
    let canonical_str = canonical.to_string_lossy().to_string();
    // `blocking_write` is safe here: this is a synchronous Tauri command,
    // and Tauri runs sync commands on its own worker thread (not on a
    // tokio runtime worker), so we cannot starve the async runtime.
    let mut root = state.blocking_write();
    *root = Some(canonical);
    Ok(canonical_str)
}

/// Validate that a path is within the current project root.
/// Returns the canonicalized path on success.
pub fn validate_path(
    path: &str,
    state: &tauri::State<'_, ProjectRootState>,
) -> Result<PathBuf, String> {
    let root = state.blocking_read();
    let root = root
        .as_ref()
        .ok_or_else(|| "No project is open".to_string())?;

    let p = PathBuf::from(path);
    let canonical = if p.exists() {
        fs::canonicalize(&p).map_err(|e| format!("Invalid path: {}", e))?
    } else {
        let mut ancestor = p.as_path();
        let mut trailing_parts: Vec<&std::ffi::OsStr> = Vec::new();
        loop {
            if let Some(parent) = ancestor.parent() {
                if let Some(name) = ancestor.file_name() {
                    trailing_parts.push(name);
                } else {
                    return Err("Invalid path".to_string());
                }
                ancestor = parent;
                if ancestor.exists() {
                    break;
                }
            } else {
                return Err("Invalid path: no existing ancestor found".to_string());
            }
        }
        let mut canonical =
            fs::canonicalize(ancestor).map_err(|e| format!("Invalid path: {}", e))?;
        for part in trailing_parts.iter().rev() {
            let s = part.to_string_lossy();
            if s == ".." || s == "." {
                return Err("Invalid path: traversal not allowed".to_string());
            }
            canonical.push(part);
        }
        canonical
    };

    if !canonical.starts_with(root) {
        return Err("Access denied: path is outside the project directory".to_string());
    }

    Ok(canonical)
}

// ── Copy-naming helper (used by paste, import, duplicate) ────────

/// Generate a unique copy name in `dest_dir` for a file/folder with the given stem and extension.
/// For directories, pass an empty string for `ext`.
fn next_copy_name(dest_dir: &Path, stem: &str, ext: &str, is_dir: bool) -> Result<PathBuf, String> {
    let mut i = 1u32;
    loop {
        if i > 10_000 {
            return Err("Too many copies exist".to_string());
        }
        let name = match (i, is_dir) {
            (1, true) => format!("{} copy", stem),
            (1, false) => format!("{} copy{}", stem, ext),
            (_, true) => format!("{} copy {}", stem, i),
            (_, false) => format!("{} copy {}{}", stem, i, ext),
        };
        let target = dest_dir.join(&name);
        if !target.exists() {
            return Ok(target);
        }
        i += 1;
    }
}

// ── Serializable types ───────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

// ── File system commands ─────────────────────────────────────────

#[tauri::command]
pub fn read_dir_tree(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
    depth: Option<u32>,
) -> Result<Vec<FileEntry>, String> {
    validate_path(&path, &state)?;
    let max_depth = depth.unwrap_or(1).min(50);
    read_dir_recursive(&PathBuf::from(path), 0, max_depth)
}

fn read_dir_recursive(
    path: &PathBuf,
    current_depth: u32,
    max_depth: u32,
) -> Result<Vec<FileEntry>, String> {
    let entries = fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut result: Vec<FileEntry> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_name = entry.file_name().to_string_lossy().to_string();

        if file_name == ".git" {
            continue;
        }

        let file_path = entry.path();
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        if ft.is_symlink() {
            continue;
        }
        let is_dir = ft.is_dir();

        let children = if is_dir && current_depth < max_depth {
            Some(read_dir_recursive(&file_path, current_depth + 1, max_depth).unwrap_or_default())
        } else if is_dir {
            Some(Vec::new())
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

    result.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(result)
}

#[tauri::command]
pub fn read_file_content(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<String, String> {
    validate_path(&path, &state)?;
    let meta = fs::metadata(&path).map_err(|e| format!("Failed to read file: {}", e.kind()))?;
    if meta.len() > MAX_TEXT_FILE_BYTES {
        return Err(format!("FILE_TOO_LARGE: {} bytes; limit {}", meta.len(), MAX_TEXT_FILE_BYTES));
    }
    fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e.kind()))
}

#[tauri::command]
pub fn write_file_content(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
    content: String,
) -> Result<(), String> {
    validate_path(&path, &state)?;
    if content.len() as u64 > MAX_TEXT_FILE_BYTES {
        return Err(format!("CONTENT_TOO_LARGE: {} bytes; limit {}", content.len(), MAX_TEXT_FILE_BYTES));
    }
    fs::write(&path, &content).map_err(|e| format!("Failed to write file: {}", e.kind()))
}

#[tauri::command]
pub fn read_file_binary(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<String, String> {
    validate_path(&path, &state)?;
    let meta = fs::metadata(&path).map_err(|e| format!("Failed to read file: {}", e.kind()))?;
    if meta.len() > MAX_BINARY_FILE_BYTES {
        return Err(format!("FILE_TOO_LARGE: {} bytes; limit {}", meta.len(), MAX_BINARY_FILE_BYTES));
    }
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
pub fn create_folder(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<(), String> {
    validate_path(&path, &state)?;
    let p = PathBuf::from(&path);
    if p.exists() {
        return Err("Folder already exists".to_string());
    }
    fs::create_dir_all(&p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_entries(
    state: tauri::State<'_, ProjectRootState>,
    paths: Vec<String>,
) -> Result<(), String> {
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
pub fn rename_entry(
    state: tauri::State<'_, ProjectRootState>,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    validate_path(&old_path, &state)?;
    validate_path(&new_path, &state)?;
    fs::rename(&old_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))
}

#[tauri::command]
pub fn move_entries(
    state: tauri::State<'_, ProjectRootState>,
    sources: Vec<String>,
    dest_dir: String,
) -> Result<(), String> {
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
        if dest.starts_with(&src_path) {
            return Err(format!(
                "Cannot move '{}' into itself or a subdirectory",
                src
            ));
        }
        let file_name = src_path.file_name().ok_or("Invalid source file name")?;
        let dst_path = dest.join(file_name);
        if src_path == dst_path {
            continue;
        }
        fs::rename(&src_path, &dst_path).map_err(|e| format!("Failed to move '{}': {}", src, e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn import_external_files(
    state: tauri::State<'_, ProjectRootState>,
    sources: Vec<String>,
    dest_dir: String,
) -> Result<(), String> {
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
        let canonical_src =
            fs::canonicalize(&src_path).map_err(|e| format!("Invalid source: {}", e))?;
        let blocked = [".ssh", ".gnupg", ".aws"];
        let is_sensitive = canonical_src.components().any(|c| {
            matches!(
                c,
                std::path::Component::Normal(name) if blocked.iter().any(|b| name == std::ffi::OsStr::new(b))
            )
        });
        if is_sensitive {
            return Err(format!("Cannot import from sensitive directory: {}", src));
        }
        let file_name = src_path
            .file_name()
            .ok_or_else(|| format!("Invalid source path: {}", src))?;
        let mut target = dest.join(file_name);
        if target.exists() {
            let stem = target.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let ext = target.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            target = next_copy_name(&dest, &stem, &ext, src_path.is_dir())?;
        }
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &target)
                .map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        } else {
            fs::copy(&src_path, &target).map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn paste_entries(
    state: tauri::State<'_, ProjectRootState>,
    sources: Vec<String>,
    dest_dir: String,
) -> Result<(), String> {
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
        if target.exists() {
            let stem = target.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let ext = target.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            target = next_copy_name(&dest, &stem, &ext, src_path.is_dir())?;
        }
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &target)
                .map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        } else {
            fs::copy(&src_path, &target).map_err(|e| format!("Failed to copy {}: {}", src, e))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn duplicate_entry(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<(), String> {
    validate_path(&path, &state)?;
    let src_path = PathBuf::from(&path);
    if !src_path.exists() {
        return Err("Path does not exist".to_string());
    }
    let parent = src_path.parent().ok_or("No parent directory")?;
    let stem = src_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let is_dir = src_path.is_dir();
    let ext = if is_dir {
        String::new()
    } else {
        src_path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default()
    };

    let target = next_copy_name(parent, &stem, &ext, is_dir)?;

    if is_dir {
        copy_dir_recursive(&src_path, &target)?;
    } else {
        fs::copy(&src_path, &target).map_err(|e| format!("Failed to duplicate: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn reveal_in_file_manager(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<(), String> {
    let canonical = validate_path(&path, &state)?;
    let safe_path = canonical.to_string_lossy().to_string();
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &safe_path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &safe_path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = canonical
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

// ── Directory copy helper ────────────────────────────────────────

const MAX_COPY_DEPTH: u32 = 50;

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    copy_dir_recursive_inner(src, dst, 0)
}

fn copy_dir_recursive_inner(src: &Path, dst: &Path, depth: u32) -> Result<(), String> {
    if depth > MAX_COPY_DEPTH {
        return Err("Maximum directory depth exceeded during copy".to_string());
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if let Ok(meta) = fs::symlink_metadata(&src_path) {
            if meta.file_type().is_symlink() {
                continue;
            }
        }
        if src_path.is_dir() {
            copy_dir_recursive_inner(&src_path, &dst_path, depth + 1)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ── File listing ─────────────────────────────────────────────────

#[tauri::command]
pub fn list_all_files(
    state: tauri::State<'_, ProjectRootState>,
    path: String,
) -> Result<Vec<String>, String> {
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
        if out.len() >= MAX_COLLECT_FILES {
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "node_modules" || name == "target" || name == ".DS_Store" {
            continue;
        }
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            collect_files(root, &path, out, depth + 1);
        } else {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_project_root_state_starts_empty() {
        let state = create_project_root_state();
        // Synchronous read on a tokio RwLock is safe outside any tokio
        // runtime context, which is how Tauri sync commands run.
        let guard = state.blocking_read();
        assert!(guard.is_none());
    }

    #[test]
    fn blocking_write_then_blocking_read_round_trip() {
        let state = create_project_root_state();
        {
            let mut w = state.blocking_write();
            *w = Some(PathBuf::from("/tmp/example"));
        }
        let r = state.blocking_read();
        assert_eq!(r.as_ref().map(|p| p.as_path()), Some(Path::new("/tmp/example")));
    }

    #[test]
    fn many_concurrent_blocking_readers_do_not_deadlock() {
        // Confirms the conversion to a reader-writer lock actually buys
        // us reader concurrency: many threads taking blocking_read() at
        // once must all proceed without serialization.
        let state = create_project_root_state();
        {
            let mut w = state.blocking_write();
            *w = Some(PathBuf::from("/tmp/example"));
        }
        let mut handles = vec![];
        for _ in 0..16 {
            let s = state.clone();
            handles.push(std::thread::spawn(move || {
                let g = s.blocking_read();
                assert!(g.is_some());
            }));
        }
        for h in handles {
            h.join().expect("reader thread panicked");
        }
    }

    #[tokio::test]
    async fn async_read_write_round_trip() {
        // Exercises the async path used by knowledge_init and
        // validate_knowledge_root.
        let state = create_project_root_state();
        {
            let mut w = state.write().await;
            *w = Some(PathBuf::from("/tmp/from-async"));
        }
        let r = state.read().await;
        assert_eq!(
            r.as_ref().map(|p| p.as_path()),
            Some(Path::new("/tmp/from-async"))
        );
    }
}
