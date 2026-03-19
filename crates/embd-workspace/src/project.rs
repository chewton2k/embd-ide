use embd_core::types::BufferId;
use embd_core::Result;
use embd_editor::syntax::HighlightSpan;
use embd_editor::{Buffer, LanguageRegistry, SyntaxTree};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::settings::Settings;
use crate::tabs::TabManager;

/// The workspace model: one open project with its buffers, tabs, and settings.
///
/// This is the central coordination point that owns all editor state.
/// UI layers (Tauri commands, future GPUI views) interact with the workspace
/// rather than managing buffers/tabs/settings independently.
pub struct Workspace {
    /// The project root directory.
    root: PathBuf,
    /// All open buffers, keyed by BufferId.
    buffers: HashMap<BufferId, Buffer>,
    /// Next buffer ID to assign.
    next_buffer_id: BufferId,
    /// Tab management.
    pub tabs: TabManager,
    /// User settings.
    pub settings: Settings,
    /// Language registry for syntax highlighting.
    language_registry: LanguageRegistry,
    /// Syntax trees for open buffers (one per buffer that has a recognized language).
    syntax_trees: HashMap<BufferId, SyntaxTree>,
}

impl Workspace {
    /// Create a new workspace for a project directory.
    pub fn new(root: PathBuf, settings: Settings) -> Self {
        let max_tabs = settings.session_limits.max_tabs;
        let language_registry = embd_editor::build_default_registry();
        Self {
            root,
            buffers: HashMap::new(),
            next_buffer_id: 1,
            tabs: TabManager::new(max_tabs),
            settings,
            language_registry,
            syntax_trees: HashMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    // ── Buffer management ────────────────────────────────────────────

    /// Open a file into a buffer and tab. Returns the buffer ID.
    /// If the file is already open, activates the existing tab.
    pub fn open_file(&mut self, path: &Path) -> Result<BufferId> {
        let canonical = std::fs::canonicalize(path)
            .map_err(|e| embd_core::Error::Io(e))?;

        // Check if already open
        for (id, buf) in &self.buffers {
            if buf.file().map_or(false, |f| f == &canonical) {
                self.tabs.activate_path(&canonical);
                return Ok(*id);
            }
        }

        // Load and create buffer
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;

        let mut buffer = Buffer::from_file(id, &canonical)?;
        buffer.set_file(canonical.clone());

        let name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());

        // Detect language and create syntax tree for highlighting
        if let Some(ext) = canonical.extension().and_then(|e| e.to_str()) {
            if let Some(config) = self.language_registry.find_by_extension(ext) {
                if let Ok(mut tree) = SyntaxTree::new(config) {
                    tree.parse(buffer.text_lf().as_bytes());
                    self.syntax_trees.insert(id, tree);
                }
            }
        }

        self.buffers.insert(id, buffer);
        self.tabs.open(canonical, name, id);

        Ok(id)
    }

    /// Create a new untitled buffer.
    pub fn new_buffer(&mut self) -> BufferId {
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;

        let buffer = Buffer::new(id);
        self.buffers.insert(id, buffer);
        id
    }

    /// Get a reference to a buffer by ID.
    pub fn buffer(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.get(&id)
    }

    /// Get a mutable reference to a buffer by ID.
    pub fn buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.buffers.get_mut(&id)
    }

    /// Get the buffer for the currently active tab.
    pub fn active_buffer(&self) -> Option<&Buffer> {
        let tab = self.tabs.active_tab()?;
        self.buffers.get(&tab.buffer_id)
    }

    /// Get a mutable reference to the active buffer.
    pub fn active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        let buffer_id = self.tabs.active_tab()?.buffer_id;
        self.buffers.get_mut(&buffer_id)
    }

    /// Save a buffer to disk.
    pub fn save_buffer(&mut self, id: BufferId) -> Result<()> {
        let buffer = self.buffers.get_mut(&id).ok_or_else(|| {
            embd_core::Error::NotFound(format!("buffer {} not found", id))
        })?;

        let path = buffer.file().cloned().ok_or_else(|| {
            embd_core::Error::Editor("buffer has no file path".to_string())
        })?;

        let content = buffer.text();
        std::fs::write(&path, &content)?;
        buffer.mark_saved();

        // Update tab state
        self.tabs.set_modified(&path, false);

        Ok(())
    }

    /// Close a file (remove buffer and tab).
    pub fn close_file(&mut self, path: &Path) -> bool {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => path.to_path_buf(),
        };

        // Find the buffer ID for this path
        let buffer_id = self.buffers.iter()
            .find(|(_, buf)| buf.file().map_or(false, |f| f == &canonical))
            .map(|(id, _)| *id);

        if let Some(id) = buffer_id {
            if self.tabs.close(&canonical) {
                self.buffers.remove(&id);
                self.syntax_trees.remove(&id);
                return true;
            }
        }
        false
    }

    /// Reload a buffer from disk (e.g., after external changes).
    pub fn reload_buffer(&mut self, id: BufferId) -> Result<()> {
        let buffer = self.buffers.get_mut(&id).ok_or_else(|| {
            embd_core::Error::NotFound(format!("buffer {} not found", id))
        })?;

        let path = buffer.file().cloned().ok_or_else(|| {
            embd_core::Error::Editor("buffer has no file path".to_string())
        })?;

        let content = std::fs::read_to_string(&path)?;
        buffer.set_text(&content);
        self.tabs.set_modified(&path, false);

        Ok(())
    }

    /// Get all buffer IDs that have unsaved modifications.
    pub fn modified_buffers(&self) -> Vec<BufferId> {
        self.buffers
            .iter()
            .filter(|(_, buf)| buf.is_modified())
            .map(|(id, _)| *id)
            .collect()
    }

    // ── Syntax highlighting ───────────────────────────────────────────

    /// Re-parse the syntax tree for a buffer after an edit.
    /// Call this after modifying buffer content to keep highlights up to date.
    pub fn reparse_syntax(&mut self, buffer_id: BufferId) {
        if let Some(buffer) = self.buffers.get(&buffer_id) {
            if let Some(tree) = self.syntax_trees.get_mut(&buffer_id) {
                let source = buffer.text_lf();
                tree.parse(source.as_bytes());
            }
        }
    }

    /// Get highlight spans for a line range of a buffer.
    /// Returns empty vec if the buffer has no syntax tree or unrecognized language.
    pub fn highlights(
        &self,
        buffer_id: BufferId,
        start_line: usize,
        end_line: usize,
    ) -> Vec<HighlightSpan> {
        let buffer = match self.buffers.get(&buffer_id) {
            Some(b) => b,
            None => return Vec::new(),
        };

        let tree = match self.syntax_trees.get(&buffer_id) {
            Some(t) => t,
            None => return Vec::new(),
        };

        // Find the highlight query for this buffer's language
        let ext = buffer
            .file()
            .and_then(|f| f.extension())
            .and_then(|e| e.to_str());

        let query = ext
            .and_then(|e| self.language_registry.find_by_extension(e))
            .and_then(|c| c.highlight_query.as_ref());

        let query = match query {
            Some(q) => q,
            None => return Vec::new(),
        };

        // Convert line range to byte range
        let start_byte = buffer.line_to_byte(start_line);
        let end_byte = buffer.line_to_byte(end_line);
        let source = buffer.text_lf();

        tree.highlights(source.as_bytes(), query, start_byte..end_byte)
    }

    /// Get highlight spans for a byte range.
    pub fn highlights_byte_range(
        &self,
        buffer_id: BufferId,
        byte_range: Range<usize>,
    ) -> Vec<HighlightSpan> {
        let buffer = match self.buffers.get(&buffer_id) {
            Some(b) => b,
            None => return Vec::new(),
        };

        let tree = match self.syntax_trees.get(&buffer_id) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let ext = buffer
            .file()
            .and_then(|f| f.extension())
            .and_then(|e| e.to_str());

        let query = ext
            .and_then(|e| self.language_registry.find_by_extension(e))
            .and_then(|c| c.highlight_query.as_ref());

        let query = match query {
            Some(q) => q,
            None => return Vec::new(),
        };

        let source = buffer.text_lf();
        tree.highlights(source.as_bytes(), query, byte_range)
    }

    /// Check whether a buffer has syntax highlighting available.
    pub fn has_syntax(&self, buffer_id: BufferId) -> bool {
        self.syntax_trees.contains_key(&buffer_id)
    }

    /// Get the detected language name for a buffer.
    pub fn language_name(&self, buffer_id: BufferId) -> Option<&str> {
        self.syntax_trees
            .get(&buffer_id)
            .map(|t| t.language_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tempdir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "embd_ws_test_{}_{}", std::process::id(), n
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_open_file() {
        let tmp = tempdir();
        fs::write(tmp.join("test.rs"), "fn main() {}").unwrap();

        let mut ws = Workspace::new(tmp.clone(), Settings::default());
        let id = ws.open_file(&tmp.join("test.rs")).unwrap();

        let buf = ws.buffer(id).unwrap();
        assert_eq!(buf.text_lf(), "fn main() {}");
        assert!(!buf.is_modified());
        assert_eq!(ws.tabs.tabs().len(), 1);
    }

    #[test]
    fn test_open_same_file_twice() {
        let tmp = tempdir();
        fs::write(tmp.join("test.rs"), "code").unwrap();

        let mut ws = Workspace::new(tmp.clone(), Settings::default());
        let id1 = ws.open_file(&tmp.join("test.rs")).unwrap();
        let id2 = ws.open_file(&tmp.join("test.rs")).unwrap();

        assert_eq!(id1, id2);
        assert_eq!(ws.tabs.tabs().len(), 1);
    }

    #[test]
    fn test_save_buffer() {
        let tmp = tempdir();
        fs::write(tmp.join("test.rs"), "original").unwrap();

        let mut ws = Workspace::new(tmp.clone(), Settings::default());
        let id = ws.open_file(&tmp.join("test.rs")).unwrap();

        ws.buffer_mut(id).unwrap().insert(8, " modified").unwrap();
        assert!(ws.buffer(id).unwrap().is_modified());

        ws.save_buffer(id).unwrap();
        assert!(!ws.buffer(id).unwrap().is_modified());
        assert_eq!(fs::read_to_string(tmp.join("test.rs")).unwrap(), "original modified");
    }

    #[test]
    fn test_close_file() {
        let tmp = tempdir();
        fs::write(tmp.join("test.rs"), "code").unwrap();

        let mut ws = Workspace::new(tmp.clone(), Settings::default());
        ws.open_file(&tmp.join("test.rs")).unwrap();

        assert!(ws.close_file(&tmp.join("test.rs")));
        assert_eq!(ws.tabs.tabs().len(), 0);
    }

    #[test]
    fn test_modified_buffers() {
        let tmp = tempdir();
        fs::write(tmp.join("a.rs"), "a").unwrap();
        fs::write(tmp.join("b.rs"), "b").unwrap();

        let mut ws = Workspace::new(tmp.clone(), Settings::default());
        let a = ws.open_file(&tmp.join("a.rs")).unwrap();
        let _b = ws.open_file(&tmp.join("b.rs")).unwrap();

        ws.buffer_mut(a).unwrap().insert(1, "!").unwrap();

        let modified = ws.modified_buffers();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0], a);
    }
}
