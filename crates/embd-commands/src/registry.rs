use std::collections::HashMap;
use std::fmt;

/// Unique identifier for a command, typically namespaced like "editor.undo"
/// or "workspace.open_file".
pub type CommandId = &'static str;

/// A registered command with metadata and an executor.
pub struct Command {
    pub id: CommandId,
    pub label: String,
    pub category: Option<String>,
    pub keybinding: Option<String>,
    handler: Box<dyn Fn(&CommandContext) + Send + Sync>,
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("category", &self.category)
            .field("keybinding", &self.keybinding)
            .finish()
    }
}

/// Context passed to command handlers when they execute.
/// This will be extended as the architecture evolves (e.g., with
/// access to the workspace, active buffer, etc.).
pub struct CommandContext {
    /// Optional string argument (e.g., for "workspace.open_file" the path).
    pub argument: Option<String>,
}

impl CommandContext {
    pub fn empty() -> Self {
        Self { argument: None }
    }

    pub fn with_arg(arg: impl Into<String>) -> Self {
        Self {
            argument: Some(arg.into()),
        }
    }
}

/// Central registry for all available commands in the IDE.
///
/// Commands can be invoked by keybinding, command palette, menu, or
/// programmatically. The registry owns command metadata (label, category,
/// keybinding) and the handler function.
pub struct CommandRegistry {
    commands: HashMap<CommandId, Command>,
    /// Commands sorted by label for palette display.
    sorted_ids: Vec<CommandId>,
    dirty: bool,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            sorted_ids: Vec::new(),
            dirty: false,
        }
    }

    /// Register a new command.
    pub fn register<F>(
        &mut self,
        id: CommandId,
        label: impl Into<String>,
        handler: F,
    ) -> &mut Command
    where
        F: Fn(&CommandContext) + Send + Sync + 'static,
    {
        let cmd = Command {
            id,
            label: label.into(),
            category: None,
            keybinding: None,
            handler: Box::new(handler),
        };
        self.commands.insert(id, cmd);
        self.dirty = true;
        self.commands.get_mut(id).unwrap()
    }

    /// Execute a command by ID.
    pub fn execute(&self, id: CommandId, ctx: &CommandContext) -> bool {
        if let Some(cmd) = self.commands.get(id) {
            (cmd.handler)(ctx);
            true
        } else {
            false
        }
    }

    /// Get a command by ID.
    pub fn get(&self, id: CommandId) -> Option<&Command> {
        self.commands.get(id)
    }

    /// Get all registered commands, sorted by label.
    pub fn all_sorted(&mut self) -> &[CommandId] {
        if self.dirty {
            self.sorted_ids = self.commands.keys().copied().collect();
            self.sorted_ids.sort_by(|a, b| {
                let la = &self.commands[a].label;
                let lb = &self.commands[b].label;
                la.cmp(lb)
            });
            self.dirty = false;
        }
        &self.sorted_ids
    }

    /// Find commands matching a query string (for command palette).
    /// Returns matching command IDs sorted by relevance.
    pub fn search(&self, query: &str) -> Vec<CommandId> {
        if query.is_empty() {
            let mut ids: Vec<CommandId> = self.commands.keys().copied().collect();
            ids.sort_by(|a, b| self.commands[a].label.cmp(&self.commands[b].label));
            return ids;
        }

        let query_lower = query.to_lowercase();
        let mut scored: Vec<(CommandId, i32)> = self
            .commands
            .iter()
            .filter_map(|(id, cmd)| {
                let label_lower = cmd.label.to_lowercase();
                let id_lower = id.to_lowercase();

                if label_lower == query_lower || id_lower == query_lower {
                    Some((*id, 100)) // Exact match
                } else if label_lower.starts_with(&query_lower) || id_lower.starts_with(&query_lower) {
                    Some((*id, 80)) // Prefix match
                } else if label_lower.contains(&query_lower) || id_lower.contains(&query_lower) {
                    Some((*id, 50)) // Substring match
                } else if fuzzy_match(&query_lower, &label_lower) {
                    Some((*id, 30)) // Fuzzy match
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        scored.into_iter().map(|(id, _)| id).collect()
    }

    /// Resolve a keybinding to a command ID.
    pub fn find_by_keybinding(&self, keybinding: &str) -> Option<CommandId> {
        self.commands
            .iter()
            .find(|(_, cmd)| cmd.keybinding.as_deref() == Some(keybinding))
            .map(|(id, _)| *id)
    }

    pub fn count(&self) -> usize {
        self.commands.len()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple fuzzy match: all characters of needle appear in haystack in order.
fn fuzzy_match(needle: &str, haystack: &str) -> bool {
    let mut haystack_chars = haystack.chars();
    for needle_char in needle.chars() {
        loop {
            match haystack_chars.next() {
                Some(c) if c == needle_char => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// Builder methods on Command for fluent registration.
impl Command {
    pub fn with_category(mut self, cat: impl Into<String>) -> Self {
        self.category = Some(cat.into());
        self
    }

    pub fn set_category(&mut self, cat: impl Into<String>) -> &mut Self {
        self.category = Some(cat.into());
        self
    }

    pub fn set_keybinding(&mut self, kb: impl Into<String>) -> &mut Self {
        self.keybinding = Some(kb.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_register_and_execute() {
        let mut reg = CommandRegistry::new();
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();

        reg.register("test.hello", "Say Hello", move |_ctx| {
            fired_clone.store(true, Ordering::SeqCst);
        });

        assert_eq!(reg.count(), 1);
        assert!(reg.execute("test.hello", &CommandContext::empty()));
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn test_execute_nonexistent() {
        let reg = CommandRegistry::new();
        assert!(!reg.execute("nope", &CommandContext::empty()));
    }

    #[test]
    fn test_search() {
        let mut reg = CommandRegistry::new();
        reg.register("editor.undo", "Undo", |_| {});
        reg.register("editor.redo", "Redo", |_| {});
        reg.register("workspace.open", "Open File", |_| {});

        let results = reg.search("undo");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "editor.undo");

        let results = reg.search("ed");
        assert_eq!(results.len(), 2); // editor.undo, editor.redo

        let results = reg.search("");
        assert_eq!(results.len(), 3); // all commands
    }

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("opf", "open file"));
        assert!(fuzzy_match("uf", "undo file"));
        assert!(!fuzzy_match("zz", "open file"));
    }

    #[test]
    fn test_keybinding_lookup() {
        let mut reg = CommandRegistry::new();
        reg.register("editor.save", "Save", |_| {})
            .set_keybinding("Cmd+S");

        assert_eq!(reg.find_by_keybinding("Cmd+S"), Some("editor.save"));
        assert_eq!(reg.find_by_keybinding("Cmd+Z"), None);
    }
}
