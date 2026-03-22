use embd_core::types::{LineEnding, Position};
use embd_core::{Error, Result};
use ropey::Rope;
use std::path::PathBuf;

use crate::cursor::SelectionSet;
use crate::edit::EditOp;
use crate::history::History;

/// A text buffer backed by a rope data structure.
///
/// This is the core document model — it owns the text content, tracks
/// modifications, handles undo/redo, and provides efficient line/offset
/// conversion. It has zero UI dependencies.
pub struct Buffer {
    /// Unique identifier within the workspace.
    id: usize,
    /// The text content.
    text: Rope,
    /// File path on disk (None for untitled buffers).
    file: Option<PathBuf>,
    /// Whether the buffer has unsaved modifications.
    modified: bool,
    /// Line ending style (detected on load, applied on save).
    line_ending: LineEnding,
    /// Edit history for undo/redo.
    history: History,
    /// Current cursor/selection state.
    selections: SelectionSet,
    /// Monotonically increasing version counter; bumped on every edit.
    version: u64,
}

impl Buffer {
    /// Create a new empty buffer with the given ID.
    pub fn new(id: usize) -> Self {
        Self {
            id,
            text: Rope::new(),
            file: None,
            modified: false,
            line_ending: LineEnding::default(),
            history: History::new(),
            selections: SelectionSet::single_at(Position::zero()),
            version: 0,
        }
    }

    /// Create a buffer from a string, typically loaded from a file.
    pub fn from_text(id: usize, text: &str) -> Self {
        let line_ending = LineEnding::detect(text);
        // Normalize to LF internally; we re-apply line endings on save.
        let normalized = text.replace("\r\n", "\n");
        Self {
            id,
            text: Rope::from_str(&normalized),
            file: None,
            modified: false,
            line_ending,
            history: History::new(),
            selections: SelectionSet::single_at(Position::zero()),
            version: 0,
        }
    }

    /// Load a buffer from a file path.
    pub fn from_file(id: usize, path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound(path.display().to_string())
            } else {
                Error::Io(e)
            }
        })?;
        let mut buf = Self::from_text(id, &content);
        buf.file = Some(path.to_path_buf());
        Ok(buf)
    }

    // ── Accessors ────────────────────────────────────────────────────

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn file(&self) -> Option<&PathBuf> {
        self.file.as_ref()
    }

    pub fn set_file(&mut self, path: PathBuf) {
        self.file = Some(path);
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn set_line_ending(&mut self, le: LineEnding) {
        self.line_ending = le;
    }

    pub fn selections(&self) -> &SelectionSet {
        &self.selections
    }

    pub fn selections_mut(&mut self) -> &mut SelectionSet {
        &mut self.selections
    }

    pub fn set_selections(&mut self, sel: SelectionSet) {
        self.selections = sel;
    }

    // ── Text queries ─────────────────────────────────────────────────

    /// Total number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.text.len_lines()
    }

    /// Total number of characters (Unicode scalar values).
    pub fn char_count(&self) -> usize {
        self.text.len_chars()
    }

    /// Byte offset of the start of a line (0-indexed). Clamps to valid range.
    pub fn line_to_byte(&self, line: usize) -> usize {
        let line = line.min(self.text.len_lines());
        self.text.line_to_byte(line)
    }

    /// Total byte length of the internal LF-normalized text.
    pub fn byte_len(&self) -> usize {
        self.text.len_bytes()
    }

    /// Get the text of a single line (0-indexed), without the trailing newline.
    pub fn line(&self, line_idx: usize) -> Result<String> {
        if line_idx >= self.text.len_lines() {
            return Err(Error::Editor(format!(
                "Line {} out of range (buffer has {} lines)",
                line_idx,
                self.text.len_lines()
            )));
        }
        let line = self.text.line(line_idx);
        let s = line.to_string();
        // Strip trailing newline characters
        Ok(s.trim_end_matches(&['\n', '\r'][..]).to_string())
    }

    /// Get the full buffer text as a string (with the buffer's line ending style).
    pub fn text(&self) -> String {
        let raw = self.text.to_string();
        match self.line_ending {
            LineEnding::LF => raw,
            LineEnding::CRLF => raw.replace('\n', "\r\n"),
        }
    }

    /// Get the raw internal rope text (always LF).
    pub fn text_lf(&self) -> String {
        self.text.to_string()
    }

    /// Get a slice of text between two char offsets.
    pub fn slice(&self, start: usize, end: usize) -> Result<String> {
        if start > end || end > self.text.len_chars() {
            return Err(Error::Editor(format!(
                "Slice [{}, {}) out of range (buffer has {} chars)",
                start,
                end,
                self.text.len_chars()
            )));
        }
        Ok(self.text.slice(start..end).to_string())
    }

    // ── Position / Offset conversion ─────────────────────────────────

    /// Convert a (line, col) position to a char offset.
    /// Clamps to valid range rather than erroring.
    pub fn pos_to_offset(&self, pos: Position) -> usize {
        let line = (pos.line as usize).min(self.text.len_lines().saturating_sub(1));
        let line_start = self.text.line_to_char(line);
        let line_len = self.line_char_count(line);
        let col = (pos.col as usize).min(line_len);
        line_start + col
    }

    /// Convert a char offset to a (line, col) position.
    /// Clamps to valid range.
    pub fn offset_to_pos(&self, offset: usize) -> Position {
        let offset = offset.min(self.text.len_chars());
        let line = self.text.char_to_line(offset);
        let line_start = self.text.line_to_char(line);
        let col = offset - line_start;
        Position::new(line as u32, col as u32)
    }

    /// Number of characters in a line (excluding trailing newline).
    fn line_char_count(&self, line_idx: usize) -> usize {
        if line_idx >= self.text.len_lines() {
            return 0;
        }
        let line = self.text.line(line_idx);
        let len = line.len_chars();
        // Subtract trailing newline if present
        if len > 0 {
            let last = line.char(len - 1);
            if last == '\n' {
                if len > 1 && line.char(len - 2) == '\r' {
                    len - 2
                } else {
                    len - 1
                }
            } else {
                len
            }
        } else {
            0
        }
    }

    // ── Edits ────────────────────────────────────────────────────────

    /// Insert text at a char offset. Returns the edit operation for history.
    pub fn insert(&mut self, offset: usize, text: &str) -> Result<()> {
        let offset = offset.min(self.text.len_chars());
        if text.is_empty() {
            return Ok(());
        }

        let old_selections = self.selections.clone();

        // Capture cursor offsets BEFORE editing the rope
        let sel_offsets: Vec<(usize, usize)> = self.selections.selections.iter()
            .map(|s| (self.pos_to_offset(s.anchor), self.pos_to_offset(s.head)))
            .collect();

        let op = EditOp::Insert {
            offset,
            text: text.to_string(),
        };

        self.text.insert(offset, text);
        self.modified = true;
        self.version += 1;

        // Adjust cursor positions using the new rope
        let inserted_len = text.chars().count();
        let new_positions: Vec<_> = sel_offsets.iter().map(|&(a, h)| {
            let new_a = if a >= offset { a + inserted_len } else { a };
            let new_h = if h >= offset { h + inserted_len } else { h };
            (self.offset_to_pos(new_a), self.offset_to_pos(new_h))
        }).collect();
        for (sel, (anchor, head)) in self.selections.selections.iter_mut().zip(new_positions) {
            sel.anchor = anchor;
            sel.head = head;
        }

        self.history.push(op, old_selections, self.selections.clone());
        Ok(())
    }

    /// Delete a range of characters [start, end).
    pub fn delete(&mut self, start: usize, end: usize) -> Result<()> {
        let start = start.min(self.text.len_chars());
        let end = end.min(self.text.len_chars());
        if start >= end {
            return Ok(());
        }

        let old_selections = self.selections.clone();

        // Capture cursor offsets BEFORE editing the rope
        let sel_offsets: Vec<(usize, usize)> = self.selections.selections.iter()
            .map(|s| (self.pos_to_offset(s.anchor), self.pos_to_offset(s.head)))
            .collect();

        let deleted_text = self.text.slice(start..end).to_string();
        let op = EditOp::Delete {
            offset: start,
            text: deleted_text,
        };

        self.text.remove(start..end);
        self.modified = true;
        self.version += 1;

        // Adjust cursor positions using the new rope
        let deleted_len = end - start;
        let new_positions: Vec<_> = sel_offsets.iter().map(|&(a, h)| {
            let new_a = if a >= end { a - deleted_len } else if a > start { start } else { a };
            let new_h = if h >= end { h - deleted_len } else if h > start { start } else { h };
            (self.offset_to_pos(new_a), self.offset_to_pos(new_h))
        }).collect();
        for (sel, (anchor, head)) in self.selections.selections.iter_mut().zip(new_positions) {
            sel.anchor = anchor;
            sel.head = head;
        }

        self.history.push(op, old_selections, self.selections.clone());
        Ok(())
    }

    /// Replace a range [start, end) with new text.
    pub fn replace(&mut self, start: usize, end: usize, new_text: &str) -> Result<()> {
        let start = start.min(self.text.len_chars());
        let end = end.min(self.text.len_chars());

        let old_selections = self.selections.clone();

        // Capture cursor offsets BEFORE editing the rope
        let sel_offsets: Vec<(usize, usize)> = self.selections.selections.iter()
            .map(|s| (self.pos_to_offset(s.anchor), self.pos_to_offset(s.head)))
            .collect();

        let old_text = if start < end {
            self.text.slice(start..end).to_string()
        } else {
            String::new()
        };

        let op = EditOp::Replace {
            offset: start,
            old_text,
            new_text: new_text.to_string(),
        };

        if start < end {
            self.text.remove(start..end);
        }
        if !new_text.is_empty() {
            self.text.insert(start, new_text);
        }
        self.modified = true;
        self.version += 1;

        // Adjust cursor positions using the new rope
        let old_len = end.saturating_sub(start);
        let new_len = new_text.chars().count();
        let new_positions: Vec<_> = sel_offsets.iter().map(|&(a, h)| {
            let new_a = adjust_offset_for_edit(a, start, old_len, new_len);
            let new_h = adjust_offset_for_edit(h, start, old_len, new_len);
            (self.offset_to_pos(new_a), self.offset_to_pos(new_h))
        }).collect();
        for (sel, (anchor, head)) in self.selections.selections.iter_mut().zip(new_positions) {
            sel.anchor = anchor;
            sel.head = head;
        }

        self.history.push(op, old_selections, self.selections.clone());
        Ok(())
    }

    /// Replace the entire buffer content (e.g., after external file reload).
    pub fn set_text(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n");
        self.text = Rope::from_str(&normalized);
        self.line_ending = LineEnding::detect(text);
        self.modified = false;
        self.version += 1;
        self.history.clear();
        self.selections = SelectionSet::single_at(Position::zero());
    }

    // ── Undo/Redo ────────────────────────────────────────────────────

    pub fn undo(&mut self) -> bool {
        if let Some((ops, old_sels)) = self.history.undo() {
            for op in &ops {
                self.apply_inverse(op);
            }
            self.selections = old_sels;
            self.modified = true;
            self.version += 1;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some((ops, new_sels)) = self.history.redo() {
            for op in &ops {
                self.apply_forward(op);
            }
            self.selections = new_sels;
            self.modified = true;
            self.version += 1;
            true
        } else {
            false
        }
    }

    fn apply_forward(&mut self, op: &EditOp) {
        match op {
            EditOp::Insert { offset, text } => {
                self.text.insert(*offset, text);
            }
            EditOp::Delete { offset, text } => {
                let end = *offset + text.chars().count();
                self.text.remove(*offset..end);
            }
            EditOp::Replace {
                offset,
                old_text,
                new_text,
            } => {
                let end = *offset + old_text.chars().count();
                if *offset < end {
                    self.text.remove(*offset..end);
                }
                if !new_text.is_empty() {
                    self.text.insert(*offset, new_text);
                }
            }
        }
    }

    fn apply_inverse(&mut self, op: &EditOp) {
        match op {
            EditOp::Insert { offset, text } => {
                let end = *offset + text.chars().count();
                self.text.remove(*offset..end);
            }
            EditOp::Delete { offset, text } => {
                self.text.insert(*offset, text);
            }
            EditOp::Replace {
                offset,
                old_text,
                new_text,
            } => {
                let end = *offset + new_text.chars().count();
                if *offset < end {
                    self.text.remove(*offset..end);
                }
                if !old_text.is_empty() {
                    self.text.insert(*offset, old_text);
                }
            }
        }
    }
}

/// Adjust a char offset after an edit at `edit_start` that removed `old_len`
/// chars and inserted `new_len` chars.
fn adjust_offset_for_edit(off: usize, edit_start: usize, old_len: usize, new_len: usize) -> usize {
    let edit_end = edit_start + old_len;
    if off >= edit_end {
        // After the edited range: shift by the size difference
        off - old_len + new_len
    } else if off > edit_start {
        // Within the edited range: move to end of replacement
        edit_start + new_len
    } else {
        // Before the edit: unchanged
        off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buf = Buffer::new(0);
        assert_eq!(buf.char_count(), 0);
        assert_eq!(buf.line_count(), 1); // ropey counts empty text as 1 line
        assert!(!buf.is_modified());
    }

    #[test]
    fn test_from_text() {
        let buf = Buffer::from_text(0, "hello\nworld\n");
        assert_eq!(buf.line_count(), 3); // "hello", "world", ""
        assert_eq!(buf.line(0).unwrap(), "hello");
        assert_eq!(buf.line(1).unwrap(), "world");
    }

    #[test]
    fn test_crlf_normalization() {
        let buf = Buffer::from_text(0, "hello\r\nworld\r\n");
        assert_eq!(buf.line_ending(), LineEnding::CRLF);
        assert_eq!(buf.text_lf(), "hello\nworld\n");
        // text() should re-apply CRLF
        assert_eq!(buf.text(), "hello\r\nworld\r\n");
    }

    #[test]
    fn test_insert() {
        let mut buf = Buffer::from_text(0, "hello world");
        buf.insert(5, ",").unwrap();
        assert_eq!(buf.text_lf(), "hello, world");
        assert!(buf.is_modified());
    }

    #[test]
    fn test_delete() {
        let mut buf = Buffer::from_text(0, "hello world");
        buf.delete(5, 6).unwrap();
        assert_eq!(buf.text_lf(), "helloworld");
    }

    #[test]
    fn test_replace() {
        let mut buf = Buffer::from_text(0, "hello world");
        buf.replace(0, 5, "goodbye").unwrap();
        assert_eq!(buf.text_lf(), "goodbye world");
    }

    #[test]
    fn test_undo_redo() {
        let mut buf = Buffer::from_text(0, "hello");
        buf.insert(5, " world").unwrap();
        assert_eq!(buf.text_lf(), "hello world");

        assert!(buf.undo());
        assert_eq!(buf.text_lf(), "hello");

        assert!(buf.redo());
        assert_eq!(buf.text_lf(), "hello world");

        // Nothing more to redo
        assert!(!buf.redo());
    }

    #[test]
    fn test_multiple_undo() {
        let mut buf = Buffer::from_text(0, "abc");
        // These consecutive single-char inserts will be grouped by the
        // history's time-based grouping (all within 300ms).
        buf.insert(3, "d").unwrap();
        buf.insert(4, "e").unwrap();
        buf.insert(5, "f").unwrap();
        assert_eq!(buf.text_lf(), "abcdef");

        // Single undo undoes the whole group
        buf.undo();
        assert_eq!(buf.text_lf(), "abc");
        assert!(!buf.undo());
    }

    #[test]
    fn test_separate_undo_groups() {
        let mut buf = Buffer::from_text(0, "abc");
        // Inserting a space creates a group break (whitespace boundary)
        buf.insert(3, " ").unwrap();
        buf.insert(4, "d").unwrap();
        assert_eq!(buf.text_lf(), "abc d");

        buf.undo();
        assert_eq!(buf.text_lf(), "abc ");
        buf.undo();
        assert_eq!(buf.text_lf(), "abc");
    }

    #[test]
    fn test_pos_offset_roundtrip() {
        let buf = Buffer::from_text(0, "hello\nworld\nfoo");
        let pos = Position::new(1, 3); // "wor|ld"
        let offset = buf.pos_to_offset(pos);
        let back = buf.offset_to_pos(offset);
        assert_eq!(back, pos);
    }

    #[test]
    fn test_pos_to_offset() {
        let buf = Buffer::from_text(0, "ab\ncd\nef");
        assert_eq!(buf.pos_to_offset(Position::new(0, 0)), 0);
        assert_eq!(buf.pos_to_offset(Position::new(0, 2)), 2);
        assert_eq!(buf.pos_to_offset(Position::new(1, 0)), 3);
        assert_eq!(buf.pos_to_offset(Position::new(1, 1)), 4);
        assert_eq!(buf.pos_to_offset(Position::new(2, 2)), 8);
    }

    #[test]
    fn test_offset_to_pos() {
        let buf = Buffer::from_text(0, "ab\ncd\nef");
        assert_eq!(buf.offset_to_pos(0), Position::new(0, 0));
        assert_eq!(buf.offset_to_pos(3), Position::new(1, 0));
        assert_eq!(buf.offset_to_pos(8), Position::new(2, 2));
    }

    #[test]
    fn test_clamping() {
        let buf = Buffer::from_text(0, "abc");
        // Out of range positions should clamp
        assert_eq!(buf.pos_to_offset(Position::new(100, 100)), 3);
        assert_eq!(buf.offset_to_pos(1000), Position::new(0, 3));
    }

    #[test]
    fn test_set_text_resets() {
        let mut buf = Buffer::from_text(0, "hello");
        buf.insert(5, " world").unwrap();
        assert!(buf.is_modified());

        buf.set_text("new content");
        assert!(!buf.is_modified());
        assert_eq!(buf.text_lf(), "new content");
        // History should be cleared
        assert!(!buf.undo());
    }

    #[test]
    fn test_slice() {
        let buf = Buffer::from_text(0, "hello world");
        assert_eq!(buf.slice(0, 5).unwrap(), "hello");
        assert_eq!(buf.slice(6, 11).unwrap(), "world");
        assert!(buf.slice(5, 100).is_err());
    }

    #[test]
    fn test_empty_operations() {
        let mut buf = Buffer::from_text(0, "hello");
        // Inserting empty string should be no-op
        buf.insert(0, "").unwrap();
        assert!(!buf.is_modified());
        // Deleting empty range should be no-op
        buf.delete(3, 3).unwrap();
        assert!(!buf.is_modified());
    }

    #[test]
    fn test_version_increments() {
        let mut buf = Buffer::from_text(0, "hello");
        assert_eq!(buf.version(), 0);
        buf.insert(5, "!").unwrap();
        assert_eq!(buf.version(), 1);
        buf.delete(5, 6).unwrap();
        assert_eq!(buf.version(), 2);
        buf.undo();
        assert_eq!(buf.version(), 3);
    }
}
