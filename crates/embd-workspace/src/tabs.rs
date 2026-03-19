use embd_core::types::BufferId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// An open file tab in the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub path: PathBuf,
    pub name: String,
    pub buffer_id: BufferId,
    pub pinned: bool,
    pub modified: bool,
}

/// Manages the set of open tabs.
///
/// This is the Rust equivalent of the `openFiles` store + tab management
/// functions from stores.ts. Enforces limits, handles pin semantics,
/// and provides navigation (next/prev tab).
pub struct TabManager {
    tabs: Vec<Tab>,
    active: Option<usize>,
    max_tabs: usize,
}

impl TabManager {
    pub fn new(max_tabs: usize) -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            max_tabs: max_tabs.max(1),
        }
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active.and_then(|i| self.tabs.get(i))
    }

    pub fn active_path(&self) -> Option<&PathBuf> {
        self.active_tab().map(|t| &t.path)
    }

    /// Open a file in a tab. If already open, just activate it.
    /// Returns the buffer_id for the tab.
    pub fn open(&mut self, path: PathBuf, name: String, buffer_id: BufferId) -> BufferId {
        // If already open, activate it
        if let Some(idx) = self.find_by_path(&path) {
            self.active = Some(idx);
            return self.tabs[idx].buffer_id;
        }

        // Add new tab
        self.tabs.push(Tab {
            path,
            name,
            buffer_id,
            pinned: false,
            modified: false,
        });

        self.active = Some(self.tabs.len() - 1);

        // Evict oldest non-pinned, non-modified tab if over limit
        self.enforce_limit();

        buffer_id
    }

    /// Close a tab by path. Pinned tabs are not closed.
    /// Returns true if a tab was actually closed.
    pub fn close(&mut self, path: &PathBuf) -> bool {
        let idx = match self.find_by_path(path) {
            Some(i) => i,
            None => return false,
        };

        if self.tabs[idx].pinned {
            return false;
        }

        self.tabs.remove(idx);

        // Adjust active index
        if self.tabs.is_empty() {
            self.active = None;
        } else if let Some(active) = self.active {
            if active == idx {
                // Activate the previous tab, or the first one
                self.active = Some(if idx > 0 { idx - 1 } else { 0 }.min(self.tabs.len() - 1));
            } else if active > idx {
                self.active = Some(active - 1);
            }
        }

        true
    }

    /// Close all non-pinned tabs.
    pub fn close_all_unpinned(&mut self) {
        self.tabs.retain(|t| t.pinned);
        if self.tabs.is_empty() {
            self.active = None;
        } else {
            self.active = Some(self.tabs.len() - 1);
        }
    }

    /// Toggle the pinned state of a tab.
    pub fn toggle_pin(&mut self, path: &PathBuf) {
        if let Some(idx) = self.find_by_path(path) {
            self.tabs[idx].pinned = !self.tabs[idx].pinned;
        }
    }

    /// Mark a tab as modified or saved.
    pub fn set_modified(&mut self, path: &PathBuf, modified: bool) {
        if let Some(idx) = self.find_by_path(path) {
            self.tabs[idx].modified = modified;
        }
    }

    /// Update a tab's path (after rename).
    pub fn rename_tab(&mut self, old_path: &PathBuf, new_path: PathBuf, new_name: String) {
        if let Some(idx) = self.find_by_path(old_path) {
            self.tabs[idx].path = new_path;
            self.tabs[idx].name = new_name;
        }
    }

    /// Navigate to the next tab (wrapping).
    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let current = self.active.unwrap_or(0);
        self.active = Some((current + 1) % self.tabs.len());
    }

    /// Navigate to the previous tab (wrapping).
    pub fn prev_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let current = self.active.unwrap_or(0);
        self.active = Some(if current == 0 {
            self.tabs.len() - 1
        } else {
            current - 1
        });
    }

    /// Activate a specific tab by index.
    pub fn activate(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = Some(index);
        }
    }

    /// Activate a tab by path.
    pub fn activate_path(&mut self, path: &PathBuf) {
        if let Some(idx) = self.find_by_path(path) {
            self.active = Some(idx);
        }
    }

    fn find_by_path(&self, path: &PathBuf) -> Option<usize> {
        self.tabs.iter().position(|t| &t.path == path)
    }

    fn enforce_limit(&mut self) {
        while self.tabs.len() > self.max_tabs {
            // Find the oldest non-pinned, non-modified tab that isn't active
            let active_path = self.active_path().cloned();
            let victim = self
                .tabs
                .iter()
                .position(|t| !t.pinned && !t.modified && Some(&t.path) != active_path.as_ref());

            if let Some(idx) = victim {
                self.tabs.remove(idx);
                // Adjust active after removal
                if let Some(active) = self.active {
                    if active > idx {
                        self.active = Some(active - 1);
                    }
                }
            } else {
                break; // Can't evict anything, allow over-limit
            }
        }
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new(9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn test_open_and_activate() {
        let mut tm = TabManager::new(5);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.open(p("/b.rs"), "b.rs".into(), 1);

        assert_eq!(tm.tabs().len(), 2);
        assert_eq!(tm.active_path(), Some(&p("/b.rs")));
    }

    #[test]
    fn test_open_existing_activates() {
        let mut tm = TabManager::new(5);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.open(p("/b.rs"), "b.rs".into(), 1);
        tm.open(p("/a.rs"), "a.rs".into(), 0);

        assert_eq!(tm.tabs().len(), 2); // No duplicate
        assert_eq!(tm.active_path(), Some(&p("/a.rs"))); // Re-activated
    }

    #[test]
    fn test_close() {
        let mut tm = TabManager::new(5);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.open(p("/b.rs"), "b.rs".into(), 1);

        assert!(tm.close(&p("/b.rs")));
        assert_eq!(tm.tabs().len(), 1);
        assert_eq!(tm.active_path(), Some(&p("/a.rs")));
    }

    #[test]
    fn test_pinned_not_closed() {
        let mut tm = TabManager::new(5);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.toggle_pin(&p("/a.rs"));
        assert!(!tm.close(&p("/a.rs")));
        assert_eq!(tm.tabs().len(), 1);
    }

    #[test]
    fn test_tab_limit_eviction() {
        let mut tm = TabManager::new(3);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.open(p("/b.rs"), "b.rs".into(), 1);
        tm.open(p("/c.rs"), "c.rs".into(), 2);
        tm.open(p("/d.rs"), "d.rs".into(), 3);

        assert_eq!(tm.tabs().len(), 3); // One was evicted
        assert!(tm.find_by_path(&p("/a.rs")).is_none()); // Oldest evicted
    }

    #[test]
    fn test_pinned_survives_eviction() {
        let mut tm = TabManager::new(2);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.toggle_pin(&p("/a.rs"));
        tm.open(p("/b.rs"), "b.rs".into(), 1);
        tm.open(p("/c.rs"), "c.rs".into(), 2);

        // /a.rs is pinned so /b.rs should be evicted
        assert!(tm.find_by_path(&p("/a.rs")).is_some());
        assert!(tm.find_by_path(&p("/c.rs")).is_some());
    }

    #[test]
    fn test_next_prev() {
        let mut tm = TabManager::new(5);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.open(p("/b.rs"), "b.rs".into(), 1);
        tm.open(p("/c.rs"), "c.rs".into(), 2);

        assert_eq!(tm.active_index(), Some(2));
        tm.next_tab();
        assert_eq!(tm.active_index(), Some(0)); // Wraps around
        tm.prev_tab();
        assert_eq!(tm.active_index(), Some(2));
    }

    #[test]
    fn test_close_all_unpinned() {
        let mut tm = TabManager::new(5);
        tm.open(p("/a.rs"), "a.rs".into(), 0);
        tm.toggle_pin(&p("/a.rs"));
        tm.open(p("/b.rs"), "b.rs".into(), 1);
        tm.open(p("/c.rs"), "c.rs".into(), 2);

        tm.close_all_unpinned();
        assert_eq!(tm.tabs().len(), 1);
        assert_eq!(tm.tabs()[0].path, p("/a.rs"));
    }

    #[test]
    fn test_rename() {
        let mut tm = TabManager::new(5);
        tm.open(p("/old.rs"), "old.rs".into(), 0);
        tm.rename_tab(&p("/old.rs"), p("/new.rs"), "new.rs".into());
        assert_eq!(tm.tabs()[0].path, p("/new.rs"));
        assert_eq!(tm.tabs()[0].name, "new.rs");
    }
}
