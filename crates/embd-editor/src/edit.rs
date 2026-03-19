/// Represents a single atomic edit operation on a buffer.
///
/// These operations are stored in the undo history and can be inverted
/// to undo/redo changes. All offsets are in char indices.
#[derive(Debug, Clone)]
pub enum EditOp {
    /// Characters were inserted at an offset.
    Insert { offset: usize, text: String },
    /// Characters were deleted starting at an offset.
    /// `text` stores what was deleted (needed for undo).
    Delete { offset: usize, text: String },
    /// Characters in a range were replaced.
    Replace {
        offset: usize,
        old_text: String,
        new_text: String,
    },
}

impl EditOp {
    /// The char offset where this edit starts.
    pub fn offset(&self) -> usize {
        match self {
            EditOp::Insert { offset, .. } => *offset,
            EditOp::Delete { offset, .. } => *offset,
            EditOp::Replace { offset, .. } => *offset,
        }
    }

    /// How many characters were added by this edit.
    pub fn chars_inserted(&self) -> usize {
        match self {
            EditOp::Insert { text, .. } => text.chars().count(),
            EditOp::Delete { .. } => 0,
            EditOp::Replace { new_text, .. } => new_text.chars().count(),
        }
    }

    /// How many characters were removed by this edit.
    pub fn chars_removed(&self) -> usize {
        match self {
            EditOp::Insert { .. } => 0,
            EditOp::Delete { text, .. } => text.chars().count(),
            EditOp::Replace { old_text, .. } => old_text.chars().count(),
        }
    }

    /// Net change in document length.
    pub fn delta(&self) -> i64 {
        self.chars_inserted() as i64 - self.chars_removed() as i64
    }
}
