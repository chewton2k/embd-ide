use embd_core::{Error, Result};
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Manages filesystem operations scoped to a project root.
///
/// All path access is validated against the project root to prevent
/// directory traversal and symlink escapes. This is ported from
/// the existing fs_commands.rs security model.
pub struct ProjectFs {
    root: PathBuf,
}

/// A file or directory entry in the tree.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

impl ProjectFs {
    /// Create a new ProjectFs rooted at the given path.
    /// The path must exist and be a directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let canonical = fs::canonicalize(root.as_ref())?;
        if !canonical.is_dir() {
            return Err(Error::Path(format!(
                "Project root is not a directory: {}",
                canonical.display()
            )));
        }
        Ok(Self { root: canonical })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Validate that a path is within the project root.
    /// Returns the canonicalized path.
    pub fn validate(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();
        let canonical = if path.exists() {
            fs::canonicalize(path)?
        } else {
            // Walk up to find the nearest existing ancestor
            self.canonicalize_nonexistent(path)?
        };

        if !canonical.starts_with(&self.root) {
            return Err(Error::AccessDenied(
                "path is outside the project directory".to_string(),
            ));
        }

        Ok(canonical)
    }

    /// Canonicalize a path that may not fully exist yet (for create operations).
    fn canonicalize_nonexistent(&self, path: &Path) -> Result<PathBuf> {
        let mut ancestor = path;
        let mut trailing: Vec<&std::ffi::OsStr> = Vec::new();

        loop {
            if let Some(parent) = ancestor.parent() {
                if let Some(name) = ancestor.file_name() {
                    trailing.push(name);
                } else {
                    return Err(Error::Path("invalid path".to_string()));
                }
                ancestor = parent;
                if ancestor.exists() {
                    break;
                }
            } else {
                return Err(Error::Path("no existing ancestor found".to_string()));
            }
        }

        let mut canonical = fs::canonicalize(ancestor)?;
        for part in trailing.iter().rev() {
            let s = part.to_string_lossy();
            if s == ".." || s == "." {
                return Err(Error::AccessDenied("path traversal not allowed".to_string()));
            }
            canonical.push(part);
        }
        Ok(canonical)
    }

    // ── Directory operations ─────────────────────────────────────────

    /// Read a directory tree, recursively up to max_depth.
    pub fn read_dir_tree(&self, path: impl AsRef<Path>, max_depth: u32) -> Result<Vec<FileEntry>> {
        let path = self.validate(path)?;
        Self::read_dir_recursive(&path, 0, max_depth.min(50))
    }

    fn read_dir_recursive(path: &Path, depth: u32, max_depth: u32) -> Result<Vec<FileEntry>> {
        let entries = fs::read_dir(path)?;
        let mut result: Vec<FileEntry> = Vec::new();

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            if name == ".git" {
                continue;
            }

            let ft = entry.file_type()?;
            if ft.is_symlink() {
                continue;
            }

            let file_path = entry.path();
            let is_dir = ft.is_dir();

            let children = if is_dir && depth < max_depth {
                Some(Self::read_dir_recursive(&file_path, depth + 1, max_depth).unwrap_or_default())
            } else if is_dir {
                Some(Vec::new())
            } else {
                None
            };

            result.push(FileEntry {
                name,
                path: file_path.to_string_lossy().to_string(),
                is_dir,
                children,
            });
        }

        // Sort: directories first, then case-insensitive alphabetical
        result.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Ok(result)
    }

    // ── File operations ──────────────────────────────────────────────

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<String> {
        let path = self.validate(path)?;
        Ok(fs::read_to_string(&path)?)
    }

    pub fn write_file(&self, path: impl AsRef<Path>, content: &str) -> Result<()> {
        let path = self.validate(path)?;
        Ok(fs::write(&path, content)?)
    }

    pub fn read_file_binary(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        let path = self.validate(path)?;
        Ok(fs::read(&path)?)
    }

    pub fn create_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = self.validate(path)?;
        if path.exists() {
            return Err(Error::AlreadyExists(path.display().to_string()));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, "")?;
        Ok(())
    }

    pub fn create_folder(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = self.validate(path)?;
        if path.exists() {
            return Err(Error::AlreadyExists(path.display().to_string()));
        }
        fs::create_dir_all(&path)?;
        Ok(())
    }

    pub fn delete_entries(&self, paths: &[impl AsRef<Path>]) -> Result<()> {
        for path in paths {
            let path = self.validate(path)?;
            if !path.exists() {
                continue;
            }
            trash::delete(&path).map_err(|e| Error::Other(format!("Failed to trash: {}", e)))?;
        }
        Ok(())
    }

    pub fn rename(&self, old: impl AsRef<Path>, new: impl AsRef<Path>) -> Result<()> {
        let old = self.validate(old)?;
        let new = self.validate(new)?;
        fs::rename(&old, &new)?;
        Ok(())
    }

    pub fn move_entries(&self, sources: &[impl AsRef<Path>], dest_dir: impl AsRef<Path>) -> Result<()> {
        let dest = self.validate(&dest_dir)?;
        if !dest.is_dir() {
            return Err(Error::Path("destination is not a directory".to_string()));
        }

        for src in sources {
            let src_path = self.validate(src)?;
            if dest.starts_with(&src_path) {
                return Err(Error::Path(format!(
                    "cannot move '{}' into itself",
                    src_path.display()
                )));
            }
            let file_name = src_path
                .file_name()
                .ok_or_else(|| Error::Path("invalid source file name".to_string()))?;
            let dst_path = dest.join(file_name);
            if src_path == dst_path {
                continue;
            }
            fs::rename(&src_path, &dst_path)?;
        }
        Ok(())
    }

    /// Copy files/folders into a destination directory (paste operation).
    /// Automatically appends " copy" suffix if a name conflict exists.
    pub fn paste_entries(&self, sources: &[impl AsRef<Path>], dest_dir: impl AsRef<Path>) -> Result<()> {
        let dest = self.validate(&dest_dir)?;
        if !dest.is_dir() {
            return Err(Error::Path("destination is not a directory".to_string()));
        }

        for src in sources {
            let src_path = self.validate(src)?;
            let file_name = src_path
                .file_name()
                .ok_or_else(|| Error::Path("invalid source file name".to_string()))?;
            let mut target = dest.join(file_name);

            if target.exists() {
                let stem = target
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let ext = target
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                let mut i = 1;
                loop {
                    let name = if i == 1 {
                        format!("{stem} copy{ext}")
                    } else {
                        format!("{stem} copy {i}{ext}")
                    };
                    target = dest.join(&name);
                    if !target.exists() {
                        break;
                    }
                    i += 1;
                    if i > 10_000 {
                        return Err(Error::Other("too many copies exist".to_string()));
                    }
                }
            }

            if src_path.is_dir() {
                Self::copy_dir_recursive(&src_path, &target)?;
            } else {
                fs::copy(&src_path, &target)?;
            }
        }
        Ok(())
    }

    /// Duplicate a file or folder, appending " copy" to the stem.
    /// Returns the path of the new entry.
    pub fn duplicate(&self, path: impl AsRef<Path>) -> Result<String> {
        let src = self.validate(path)?;
        let parent = src.parent().ok_or_else(|| Error::Path("no parent".to_string()))?;
        let stem = src
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let ext = src.extension().map(|e| e.to_string_lossy().to_string());

        let new_name = if let Some(ext) = &ext {
            format!("{stem} copy.{ext}")
        } else {
            format!("{stem} copy")
        };
        let mut dest = parent.join(&new_name);

        // If already exists, append a number
        let mut counter = 2;
        while dest.exists() {
            let numbered = if let Some(ext) = &ext {
                format!("{stem} copy {counter}.{ext}")
            } else {
                format!("{stem} copy {counter}")
            };
            dest = parent.join(&numbered);
            counter += 1;
        }

        if src.is_dir() {
            Self::copy_dir_recursive(&src, &dest)?;
        } else {
            fs::copy(&src, &dest)?;
        }

        Ok(dest.to_string_lossy().to_string())
    }

    fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
        fs::create_dir_all(dest)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let target = dest.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                Self::copy_dir_recursive(&entry.path(), &target)?;
            } else {
                fs::copy(entry.path(), &target)?;
            }
        }
        Ok(())
    }

    /// List all files recursively (for file finder), up to a limit.
    pub fn list_all_files(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();
        Self::collect_files(&self.root, &self.root, &mut files, 0);
        Ok(files)
    }

    fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>, depth: u32) {
        const MAX_DEPTH: u32 = 100;
        const MAX_FILES: usize = 100_000;

        if depth > MAX_DEPTH || out.len() >= MAX_FILES {
            return;
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            if out.len() >= MAX_FILES {
                return;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | ".DS_Store"
            ) {
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
                Self::collect_files(root, &path, out, depth + 1);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }

    pub fn reveal_in_file_manager(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = self.validate(path)?;
        let path_str = path.to_string_lossy().to_string();

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .args(["-R", &path_str])
                .spawn()?;
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .args(["/select,", &path_str])
                .spawn()?;
        }
        #[cfg(target_os = "linux")]
        {
            let parent = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or(path_str);
            std::process::Command::new("xdg-open")
                .arg(&parent)
                .spawn()?;
        }
        Ok(())
    }
}

/// Validate a relative file path for git commands.
/// Rejects absolute paths, traversal, NUL bytes, and .git access.
pub fn validate_git_file_path(file_path: &str) -> Result<()> {
    if file_path.is_empty() {
        return Err(Error::Path("path cannot be empty".to_string()));
    }
    if file_path.contains('\0') {
        return Err(Error::Path("null bytes not allowed".to_string()));
    }

    let path = Path::new(file_path);
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(Error::AccessDenied("path traversal not allowed".to_string()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(Error::AccessDenied("absolute paths not allowed".to_string()));
            }
            Component::Normal(name) if name.eq_ignore_ascii_case(".git") => {
                return Err(Error::AccessDenied(".git paths not allowed".to_string()));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Check if a path component is a sensitive directory that should not be imported from.
pub fn is_sensitive_path(path: &Path) -> bool {
    const BLOCKED: &[&str] = &[".ssh", ".gnupg", ".aws"];
    path.components().any(|c| {
        matches!(c, Component::Normal(name) if BLOCKED.iter().any(|b| name == std::ffi::OsStr::new(b)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as stdfs;

    #[test]
    fn test_validate_git_file_path() {
        assert!(validate_git_file_path("src/main.rs").is_ok());
        assert!(validate_git_file_path("../escape").is_err());
        assert!(validate_git_file_path("/absolute").is_err());
        assert!(validate_git_file_path(".git/config").is_err());
        assert!(validate_git_file_path("").is_err());
    }

    #[test]
    fn test_sensitive_path() {
        assert!(is_sensitive_path(Path::new("/home/user/.ssh/id_rsa")));
        assert!(is_sensitive_path(Path::new("/home/user/.gnupg/key")));
        assert!(!is_sensitive_path(Path::new("/home/user/projects/code.rs")));
    }

    #[test]
    fn test_project_fs_create_and_read() {
        let tmp = tempdir();
        let pfs = ProjectFs::new(&tmp).unwrap();

        let file = tmp.join("test.txt");
        pfs.write_file(&file, "hello").unwrap();
        assert_eq!(pfs.read_file(&file).unwrap(), "hello");
    }

    #[test]
    fn test_project_fs_rejects_outside() {
        let tmp = tempdir();
        let pfs = ProjectFs::new(&tmp).unwrap();

        // Try to access parent directory
        let outside = tmp.parent().unwrap().join("other.txt");
        assert!(pfs.validate(&outside).is_err());
    }

    #[test]
    fn test_dir_tree() {
        let tmp = tempdir();
        let pfs = ProjectFs::new(&tmp).unwrap();

        stdfs::create_dir(tmp.join("subdir")).unwrap();
        stdfs::write(tmp.join("file.txt"), "content").unwrap();
        stdfs::write(tmp.join("subdir/nested.txt"), "nested").unwrap();

        let tree = pfs.read_dir_tree(&tmp, 2).unwrap();
        assert!(tree.iter().any(|e| e.name == "subdir" && e.is_dir));
        assert!(tree.iter().any(|e| e.name == "file.txt" && !e.is_dir));
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("embd_test_{}", std::process::id()));
        let _ = stdfs::remove_dir_all(&dir);
        stdfs::create_dir_all(&dir).unwrap();
        dir
    }
}
