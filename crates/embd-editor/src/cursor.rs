use embd_core::types::Position;
use serde::{Deserialize, Serialize};

/// A single selection in the editor.
///
/// `anchor` is where the selection started (e.g., where the user clicked).
/// `head` is where the cursor currently is (e.g., where shift+arrow moved to).
/// When anchor == head, this is a simple cursor with no selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub anchor: Position,
    pub head: Position,
    /// Preferred column for vertical movement (remembers where the cursor
    /// "wants" to be when moving through short lines).
    pub goal_column: Option<u32>,
}

impl Selection {
    /// Create a cursor at a position (no selection).
    pub fn cursor(pos: Position) -> Self {
        Self {
            anchor: pos,
            head: pos,
            goal_column: None,
        }
    }

    /// Create a selection between two positions.
    pub fn range(anchor: Position, head: Position) -> Self {
        Self {
            anchor,
            head,
            goal_column: None,
        }
    }

    /// True if this selection has no extent (it's just a cursor).
    pub fn is_cursor(&self) -> bool {
        self.anchor == self.head
    }

    /// Get the selection range with start <= end.
    pub fn ordered(&self) -> (Position, Position) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// The leftmost position.
    pub fn start(&self) -> Position {
        self.ordered().0
    }

    /// The rightmost position.
    pub fn end(&self) -> Position {
        self.ordered().1
    }

    /// Collapse the selection to just the head position.
    pub fn collapse(&mut self) {
        self.anchor = self.head;
    }
}

/// A set of selections (supports multi-cursor).
///
/// There is always at least one selection. The primary selection is the one
/// that most operations target (e.g., scrolling to cursor, insert at cursor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionSet {
    /// Index of the primary selection.
    pub primary: usize,
    /// All selections, kept sorted by their start position.
    pub selections: Vec<Selection>,
}

impl SelectionSet {
    /// Create a selection set with a single cursor at a position.
    pub fn single_at(pos: Position) -> Self {
        Self {
            primary: 0,
            selections: vec![Selection::cursor(pos)],
        }
    }

    /// Create a selection set with a single selection.
    pub fn single(sel: Selection) -> Self {
        Self {
            primary: 0,
            selections: vec![sel],
        }
    }

    /// Get the primary selection.
    pub fn primary(&self) -> &Selection {
        &self.selections[self.primary]
    }

    /// Get a mutable reference to the primary selection.
    pub fn primary_mut(&mut self) -> &mut Selection {
        &mut self.selections[self.primary]
    }

    /// Add a new cursor/selection, making it the new primary.
    pub fn add(&mut self, sel: Selection) {
        self.selections.push(sel);
        self.primary = self.selections.len() - 1;
        self.sort_and_merge();
    }

    /// Replace all selections with a single one.
    pub fn set_single(&mut self, sel: Selection) {
        self.selections.clear();
        self.selections.push(sel);
        self.primary = 0;
    }

    /// Number of cursors/selections.
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    /// Shift all cursors that are at or after `offset` by `delta` characters.
    /// Used after insert (positive delta) or delete (negative delta).
    pub fn shift_after(&mut self, offset: usize, delta: i64) {
        for sel in &mut self.selections {
            shift_pos(&mut sel.anchor, offset, delta);
            shift_pos(&mut sel.head, offset, delta);
        }
    }

    /// Sort selections by position and merge overlapping ones.
    fn sort_and_merge(&mut self) {
        if self.selections.len() <= 1 {
            return;
        }
        // Sort by start position
        self.selections.sort_by(|a, b| a.start().cmp(&b.start()));
        // TODO: merge overlapping selections. For now, just sort.
        self.primary = self.primary.min(self.selections.len() - 1);
    }
}

/// Shift a Position based on a char offset and delta.
/// This is a simplified shift that works on a flat offset model.
/// For proper line-aware shifting, the buffer should be consulted.
fn shift_pos(pos: &mut Position, offset: usize, delta: i64) {
    // Convert to a simple linear index for comparison.
    // This is a rough heuristic — precise shifting requires the buffer's
    // line/col mapping. For now, we only shift the column on the same line
    // or mark positions as needing recalculation.
    //
    // In practice, the buffer will re-derive positions from offsets after edits.
    // This function exists so that cursor offsets stay approximately correct
    // between edit and re-derivation.
    let _ = (pos, offset, delta);
    // The actual shifting is done by converting to offset, adjusting, and
    // converting back. But since we don't have the buffer here, we leave this
    // as a no-op stub. The Buffer's insert/delete methods handle cursor
    // adjustment through the full offset->pos pipeline.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_creation() {
        let sel = Selection::cursor(Position::new(5, 10));
        assert!(sel.is_cursor());
        assert_eq!(sel.head, Position::new(5, 10));
    }

    #[test]
    fn test_range_selection() {
        let sel = Selection::range(Position::new(1, 0), Position::new(3, 5));
        assert!(!sel.is_cursor());
        assert_eq!(sel.start(), Position::new(1, 0));
        assert_eq!(sel.end(), Position::new(3, 5));
    }

    #[test]
    fn test_reverse_selection() {
        let sel = Selection::range(Position::new(5, 0), Position::new(1, 0));
        assert_eq!(sel.start(), Position::new(1, 0));
        assert_eq!(sel.end(), Position::new(5, 0));
    }

    #[test]
    fn test_selection_set_single() {
        let set = SelectionSet::single_at(Position::new(0, 0));
        assert_eq!(set.len(), 1);
        assert!(set.primary().is_cursor());
    }

    #[test]
    fn test_selection_set_add() {
        let mut set = SelectionSet::single_at(Position::new(0, 0));
        set.add(Selection::cursor(Position::new(5, 0)));
        assert_eq!(set.len(), 2);
        // Primary should be the newly added one (index may change after sort)
    }

    #[test]
    fn test_collapse() {
        let mut sel = Selection::range(Position::new(1, 0), Position::new(5, 10));
        sel.collapse();
        assert!(sel.is_cursor());
        assert_eq!(sel.anchor, Position::new(5, 10));
    }
}
