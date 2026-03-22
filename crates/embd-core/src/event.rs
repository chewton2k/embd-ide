use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A type-erased event that carries its payload as Any.
pub type EventPayload = Box<dyn Any + Send>;

/// Callback signature for event handlers.
type Handler = Box<dyn Fn(&dyn Any) + Send + Sync>;

/// Simple publish-subscribe event bus.
///
/// Used for decoupled communication between subsystems (e.g., file watcher
/// notifies workspace of changes, editor notifies workspace of buffer saves).
///
/// Events are identified by string keys. Handlers receive a type-erased payload
/// and downcast to the expected type.
pub struct EventBus {
    handlers: Mutex<HashMap<String, Vec<Option<Arc<Handler>>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
        }
    }

    /// Subscribe to events with the given key.
    /// Returns a subscription ID that can be used for unsubscription.
    pub fn subscribe<F>(&self, event: &str, handler: F) -> SubscriptionId
    where
        F: Fn(&dyn Any) + Send + Sync + 'static,
    {
        let handler = Arc::new(Box::new(handler) as Handler);
        let mut map = self.handlers.lock().unwrap();
        let handlers = map.entry(event.to_string()).or_default();
        let id = SubscriptionId {
            event: event.to_string(),
            index: handlers.len(),
        };
        handlers.push(Some(handler));
        id
    }

    /// Remove a previously registered handler.
    pub fn unsubscribe(&self, id: &SubscriptionId) {
        if let Ok(mut map) = self.handlers.lock() {
            if let Some(handlers) = map.get_mut(&id.event) {
                if id.index < handlers.len() {
                    handlers[id.index] = None;
                }
            }
        }
    }

    /// Emit an event, calling all registered handlers synchronously.
    pub fn emit(&self, event: &str, payload: &dyn Any) {
        let handlers = {
            let map = self.handlers.lock().unwrap();
            map.get(event).cloned().unwrap_or_default()
        };
        for handler in handlers.iter().flatten() {
            handler(payload);
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Identifies a subscription for potential future unsubscribe support.
#[derive(Debug, Clone)]
pub struct SubscriptionId {
    pub event: String,
    pub index: usize,
}

/// Well-known event keys used across subsystems.
pub mod events {
    /// Emitted when a buffer is saved to disk. Payload: PathBuf.
    pub const BUFFER_SAVED: &str = "buffer.saved";
    /// Emitted when a buffer's content changes. Payload: BufferId (usize).
    pub const BUFFER_CHANGED: &str = "buffer.changed";
    /// Emitted when a file changes on disk externally. Payload: PathBuf.
    pub const FILE_CHANGED_EXTERNAL: &str = "file.changed.external";
    /// Emitted when the active file changes. Payload: Option<PathBuf>.
    pub const ACTIVE_FILE_CHANGED: &str = "active_file.changed";
    /// Emitted when git status should be refreshed. Payload: ().
    pub const GIT_REFRESH: &str = "git.refresh";
    /// Emitted when the file tree should be refreshed. Payload: ().
    pub const FILE_TREE_REFRESH: &str = "file_tree.refresh";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_emit_and_subscribe() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        bus.subscribe("test.event", move |payload| {
            if let Some(val) = payload.downcast_ref::<u32>() {
                count_clone.fetch_add(*val as usize, Ordering::SeqCst);
            }
        });

        bus.emit("test.event", &42u32);
        assert_eq!(count.load(Ordering::SeqCst), 42);

        bus.emit("test.event", &8u32);
        assert_eq!(count.load(Ordering::SeqCst), 50);
    }

    #[test]
    fn test_no_handlers_is_fine() {
        let bus = EventBus::new();
        bus.emit("nonexistent", &()); // should not panic
    }
}
