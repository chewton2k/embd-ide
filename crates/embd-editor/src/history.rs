use crate::cursor::SelectionSet;
use crate::edit::EditOp;
use std::time::{Duration, Instant};

/// A group of edits that should be undone/redone together.
///
/// Sequential small edits (e.g., typing characters quickly) are grouped
/// together so that undo undoes the whole burst rather than one character
/// at a time.
#[derive(Debug, Clone)]
struct EditGroup {
    ops: Vec<EditOp>,
    /// Selections before any of the edits in this group.
    selections_before: SelectionSet,
    /// Selections after all edits in this group.
    selections_after: SelectionSet,
    /// Timestamp of the last edit added to this group.
    last_edit_time: Instant,
}

/// Manages undo/redo history for a buffer.
///
/// Groups consecutive edits within a time window so that fast typing
/// is undone as a block. The grouping interval is configurable.
pub struct History {
    undo_stack: Vec<EditGroup>,
    redo_stack: Vec<EditGroup>,
    /// Maximum time between edits to group them together.
    group_interval: Duration,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            group_interval: Duration::from_millis(300),
        }
    }

    /// Record a new edit operation with the selections before and after.
    pub fn push(&mut self, op: EditOp, selections_before: SelectionSet, selections_after: SelectionSet) {
        let now = Instant::now();

        // Clear redo stack on new edit (standard undo/redo behavior)
        self.redo_stack.clear();

        // Try to group with the last edit if it was recent and compatible
        if let Some(last_group) = self.undo_stack.last_mut() {
            if now.duration_since(last_group.last_edit_time) < self.group_interval
                && Self::should_group(&last_group.ops, &op)
            {
                last_group.ops.push(op);
                last_group.selections_after = selections_after;
                last_group.last_edit_time = now;
                return;
            }
        }

        // Start a new group
        self.undo_stack.push(EditGroup {
            ops: vec![op],
            selections_before,
            selections_after,
            last_edit_time: now,
        });
    }

    /// Pop the most recent edit group for undo.
    /// Returns the operations to invert and the selections to restore.
    pub fn undo(&mut self) -> Option<(Vec<EditOp>, SelectionSet)> {
        let group = self.undo_stack.pop()?;
        let sels = group.selections_before.clone();
        let ops = group.ops.clone();
        self.redo_stack.push(group);
        // Return ops in reverse order for proper undo application
        Some((ops.into_iter().rev().collect(), sels))
    }

    /// Pop the most recent undo for redo.
    pub fn redo(&mut self) -> Option<(Vec<EditOp>, SelectionSet)> {
        let group = self.redo_stack.pop()?;
        let sels = group.selections_after.clone();
        let ops = group.ops.clone();
        self.undo_stack.push(group);
        Some((ops, sels))
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Heuristic for whether two edits should be in the same undo group.
    /// Groups sequential single-character inserts and sequential deletes.
    fn should_group(existing: &[EditOp], new: &EditOp) -> bool {
        let last = match existing.last() {
            Some(op) => op,
            None => return false,
        };

        match (last, new) {
            // Group consecutive single-char inserts that are adjacent
            (
                EditOp::Insert {
                    offset: prev_off,
                    text: prev_text,
                },
                EditOp::Insert {
                    offset: new_off,
                    text: new_text,
                },
            ) => {
                prev_text.chars().count() == 1
                    && new_text.chars().count() == 1
                    && *new_off == *prev_off + prev_text.chars().count()
                    // Break groups on whitespace boundaries (space, newline)
                    && !prev_text.ends_with(|c: char| c.is_whitespace())
            }
            // Group consecutive single-char deletes (backspace) at adjacent positions
            (
                EditOp::Delete {
                    offset: prev_off,
                    text: prev_text,
                },
                EditOp::Delete {
                    offset: new_off,
                    text: new_text,
                },
            ) => {
                prev_text.chars().count() == 1
                    && new_text.chars().count() == 1
                    && (*new_off == prev_off.saturating_sub(1) || *new_off == *prev_off)
            }
            _ => false,
        }
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embd_core::types::Position;

    fn dummy_sels() -> SelectionSet {
        SelectionSet::single_at(Position::zero())
    }

    #[test]
    fn test_basic_undo_redo() {
        let mut hist = History::new();
        hist.push(
            EditOp::Insert {
                offset: 0,
                text: "hello".to_string(),
            },
            dummy_sels(),
            dummy_sels(),
        );

        assert!(hist.can_undo());
        assert!(!hist.can_redo());

        let (ops, _) = hist.undo().unwrap();
        assert_eq!(ops.len(), 1);
        assert!(!hist.can_undo());
        assert!(hist.can_redo());

        let (ops, _) = hist.redo().unwrap();
        assert_eq!(ops.len(), 1);
    }

    #[test]
    fn test_redo_cleared_on_new_edit() {
        let mut hist = History::new();
        hist.push(
            EditOp::Insert {
                offset: 0,
                text: "a".to_string(),
            },
            dummy_sels(),
            dummy_sels(),
        );
        hist.undo();

        // New edit should clear redo
        hist.push(
            EditOp::Insert {
                offset: 0,
                text: "b".to_string(),
            },
            dummy_sels(),
            dummy_sels(),
        );
        assert!(!hist.can_redo());
    }

    #[test]
    fn test_clear() {
        let mut hist = History::new();
        hist.push(
            EditOp::Insert {
                offset: 0,
                text: "a".to_string(),
            },
            dummy_sels(),
            dummy_sels(),
        );
        hist.clear();
        assert!(!hist.can_undo());
        assert!(!hist.can_redo());
    }
}
