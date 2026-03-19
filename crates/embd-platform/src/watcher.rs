use embd_core::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Events emitted by the file watcher.
#[derive(Debug, Clone)]
pub enum FsEvent {
    /// A file or directory was created or modified.
    Changed(PathBuf),
    /// A file or directory was removed (or renamed away).
    Removed(PathBuf),
}

/// Watches a project directory for filesystem changes.
///
/// Uses `notify` with debouncing to batch rapid changes (e.g., save + format)
/// into single events. The watcher runs on a background thread and sends
/// events through a channel.
pub struct FileWatcher {
    /// Receive debounced filesystem events.
    rx: mpsc::Receiver<Vec<FsEvent>>,
    /// Keep the debouncer alive; dropping it stops watching.
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl FileWatcher {
    /// Start watching a directory recursively.
    ///
    /// `debounce_ms` controls how long to wait for additional events before
    /// delivering a batch. 200ms is a good default for IDE use.
    pub fn new(root: &Path, debounce_ms: u64) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let mut debouncer = new_debouncer(
            Duration::from_millis(debounce_ms),
            move |result: std::result::Result<Vec<DebouncedEvent>, notify::Error>| {
                match result {
                    Ok(events) => {
                        let fs_events: Vec<FsEvent> = events
                            .into_iter()
                            .filter_map(|e| {
                                // Skip .git directory changes — too noisy
                                if is_git_internal(&e.path) {
                                    return None;
                                }
                                match e.kind {
                                    DebouncedEventKind::Any => {
                                        if e.path.exists() {
                                            Some(FsEvent::Changed(e.path))
                                        } else {
                                            Some(FsEvent::Removed(e.path))
                                        }
                                    }
                                    DebouncedEventKind::AnyContinuous => {
                                        Some(FsEvent::Changed(e.path))
                                    }
                                    _ => {
                                        // Future variants — treat as generic change
                                        Some(FsEvent::Changed(e.path))
                                    }
                                }
                            })
                            .collect();

                        if !fs_events.is_empty() {
                            let _ = tx.send(fs_events);
                        }
                    }
                    Err(_) => {
                        // Watcher error — silently ignore (logging can be added later)
                    }
                }
            },
        )
        .map_err(|e| embd_core::Error::Other(format!("Failed to create file watcher: {}", e)))?;

        debouncer
            .watcher()
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| embd_core::Error::Other(format!("Failed to watch directory: {}", e)))?;

        Ok(Self {
            rx,
            _debouncer: debouncer,
        })
    }

    /// Try to receive a batch of events without blocking.
    /// Returns `None` if no events are available.
    pub fn try_recv(&self) -> Option<Vec<FsEvent>> {
        self.rx.try_recv().ok()
    }

    /// Block until events arrive or the watcher is dropped.
    pub fn recv(&self) -> Option<Vec<FsEvent>> {
        self.rx.recv().ok()
    }

    /// Receive with a timeout.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Vec<FsEvent>> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Drain all currently pending event batches into a flat list.
    pub fn drain(&self) -> Vec<FsEvent> {
        let mut all = Vec::new();
        while let Ok(batch) = self.rx.try_recv() {
            all.extend(batch);
        }
        all
    }
}

/// Check if a path is inside a .git directory (status files, index, etc.).
fn is_git_internal(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(c, std::path::Component::Normal(name) if name == ".git")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("embd_watcher_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_watcher_detects_create() {
        let tmp = tempdir("create");
        let watcher = FileWatcher::new(&tmp, 100).unwrap();

        // Create a file
        fs::write(tmp.join("new.txt"), "hello").unwrap();

        // Wait for debounced event
        let events = watcher.recv_timeout(Duration::from_secs(2));
        assert!(events.is_some(), "Expected file create event");

        let events = events.unwrap();
        assert!(events.iter().any(|e| matches!(e, FsEvent::Changed(p) if p.file_name().unwrap() == "new.txt")));
    }

    #[test]
    fn test_watcher_detects_modify() {
        let tmp = tempdir("modify");
        let file = tmp.join("existing.txt");
        fs::write(&file, "original").unwrap();

        // Small delay to avoid picking up the initial create
        thread::sleep(Duration::from_millis(200));

        let watcher = FileWatcher::new(&tmp, 100).unwrap();

        // Modify the file
        fs::write(&file, "modified").unwrap();

        let events = watcher.recv_timeout(Duration::from_secs(2));
        assert!(events.is_some(), "Expected file modify event");
    }

    #[test]
    fn test_git_internal_filter() {
        assert!(is_git_internal(Path::new("/project/.git/index")));
        assert!(is_git_internal(Path::new("/project/.git/refs/heads/main")));
        assert!(!is_git_internal(Path::new("/project/src/main.rs")));
        assert!(!is_git_internal(Path::new("/project/.gitignore")));
    }
}
