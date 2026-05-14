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
    /// True when the entry itself is a symbolic link. The frontend
    /// renders a badge so users can see at a glance which entries are
    /// links rather than real files. Symlinks resolving outside the
    /// project root are still shown but cannot be opened —
    /// `validate_path` canonicalizes before any I/O and rejects paths
    /// that escape the root.
    pub is_symlink: bool,
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
    let mut visited = std::collections::HashSet::new();
    read_dir_recursive(&PathBuf::from(path), 0, max_depth, &mut visited)
}

fn read_dir_recursive(
    path: &Path,
    current_depth: u32,
    max_depth: u32,
    visited: &mut std::collections::HashSet<PathBuf>,
) -> Result<Vec<FileEntry>, String> {
    // Cycle detection: track the canonical path of every directory we
    // descend into. A symlinked directory that points back to (or
    // through) an ancestor would otherwise loop forever once
    // recursion follows it. We only insert canonical paths so two
    // routes to the same target (e.g. via different symlink chains)
    // are detected as a cycle.
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(Vec::new());
    }

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
        let is_symlink = ft.is_symlink();

        // For symlinks we need the *target's* metadata to know if the
        // tree should treat this entry as a directory. `entry.file_type`
        // reports the link itself; `fs::metadata` follows it. Dangling
        // symlinks fall back to "treat as file" so they're visible
        // without crashing the walk.
        let is_dir = if is_symlink {
            fs::metadata(&file_path).map(|m| m.is_dir()).unwrap_or(false)
        } else {
            ft.is_dir()
        };

        // Recurse into REAL directories only. Symlinked directories
        // are listed but never expanded — that's the cheapest way to
        // avoid the cycles the visited-set is also guarding against.
        // Users can still navigate symlinked dirs by clicking through
        // (FileTree.svelte fetches their children lazily), where the
        // visited set provides the real safety net.
        let children = if is_dir && !is_symlink && current_depth < max_depth {
            Some(
                read_dir_recursive(&file_path, current_depth + 1, max_depth, visited)
                    .unwrap_or_default(),
            )
        } else if is_dir {
            Some(Vec::new())
        } else {
            None
        };

        result.push(FileEntry {
            name: file_name,
            path: file_path.to_string_lossy().to_string(),
            is_dir,
            is_symlink,
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

    // ── M15: symlinks in the file tree ──────────────────────────

    #[cfg(unix)]
    fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(unix)]
    fn find<'a>(entries: &'a [FileEntry], name: &str) -> Option<&'a FileEntry> {
        entries.iter().find(|e| e.name == name)
    }

    #[cfg(unix)]
    #[test]
    fn read_dir_recursive_marks_file_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.txt");
        std::fs::write(&target, b"hello").unwrap();
        let link = dir.path().join("link.txt");
        symlink(&target, &link).unwrap();

        let mut visited = std::collections::HashSet::new();
        let entries =
            read_dir_recursive(dir.path(), 0, 5, &mut visited).expect("walk");

        let real = find(&entries, "real.txt").expect("real entry present");
        assert!(!real.is_symlink, "regular file must not be flagged as symlink");

        let link_entry = find(&entries, "link.txt").expect("symlink entry present");
        assert!(link_entry.is_symlink, "symlink must be flagged");
        assert!(!link_entry.is_dir, "symlink to file must report is_dir=false");
        assert!(link_entry.children.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn read_dir_recursive_lists_symlinked_directory_without_recursing() {
        let dir = tempfile::tempdir().unwrap();
        // real_dir contains one file
        let real_dir = dir.path().join("real_dir");
        std::fs::create_dir(&real_dir).unwrap();
        std::fs::write(real_dir.join("inner.txt"), b"x").unwrap();
        // link_dir → real_dir
        let link_dir = dir.path().join("link_dir");
        symlink(&real_dir, &link_dir).unwrap();

        let mut visited = std::collections::HashSet::new();
        let entries =
            read_dir_recursive(dir.path(), 0, 5, &mut visited).expect("walk");

        let real = find(&entries, "real_dir").expect("real_dir present");
        assert!(real.is_dir);
        assert!(!real.is_symlink);
        // The real directory's children are populated.
        assert_eq!(real.children.as_ref().map(|c| c.len()), Some(1));

        let linked = find(&entries, "link_dir").expect("link_dir present");
        assert!(linked.is_dir, "symlink to directory should report is_dir=true");
        assert!(linked.is_symlink);
        // Symlinked directory shows up but children is empty: we do
        // not recurse into the link target through the recursive walk.
        // (The frontend's lazy expansion path can fetch them later.)
        assert_eq!(
            linked.children.as_ref().map(|c| c.len()),
            Some(0),
            "symlinked directory must not recurse via the same walk"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_dir_recursive_handles_symlink_cycle_without_infinite_loop() {
        // Construct: parent/{child_dir, loop_link → parent}. A naive
        // walk that follows symlinks would recurse forever; the
        // visited set should stop it on the second hop.
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let child_dir = parent.join("child");
        std::fs::create_dir(&child_dir).unwrap();
        std::fs::write(child_dir.join("leaf.txt"), b"x").unwrap();
        // The cycle: parent/loop -> parent itself.
        let cycle_link = parent.join("loop");
        symlink(&parent, &cycle_link).unwrap();

        let mut visited = std::collections::HashSet::new();
        let entries =
            read_dir_recursive(&parent, 0, 5, &mut visited).expect("walk");

        // The walk completes (no infinite loop) and reports the link.
        let loop_entry = find(&entries, "loop").expect("loop entry present");
        assert!(loop_entry.is_symlink);
        // child still walked and contains its leaf.
        let child = find(&entries, "child").expect("child entry present");
        assert!(child.is_dir);
        assert_eq!(child.children.as_ref().map(|c| c.len()), Some(1));
    }

    #[cfg(unix)]
    #[test]
    fn read_dir_recursive_exposes_dangling_symlink_as_a_file() {
        // Symlink whose target was removed — we must list it (so the
        // user can clean it up) without crashing the walk.
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("dangling");
        symlink(Path::new("/no/such/path/leo-dangling-target"), &link).unwrap();

        let mut visited = std::collections::HashSet::new();
        let entries =
            read_dir_recursive(dir.path(), 0, 5, &mut visited).expect("walk");

        let dangling = find(&entries, "dangling").expect("dangling entry present");
        assert!(dangling.is_symlink);
        // metadata() fails for a dangling symlink → we treat it as a
        // file (is_dir=false). Better than panicking.
        assert!(!dangling.is_dir);
    }
}
