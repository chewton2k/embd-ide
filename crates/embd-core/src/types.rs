use serde::{Deserialize, Serialize};

/// Unique identifier for a buffer in the editor.
pub type BufferId = usize;

/// A position in a text document (0-indexed line and column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub col: u32,
}

impl Position {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }

    pub fn zero() -> Self {
        Self { line: 0, col: 0 }
    }
}

/// A range in a text document, defined by start (inclusive) and end (exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns a range with start <= end (canonical ordering).
    pub fn ordered(&self) -> Self {
        if self.start <= self.end {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }
}

/// Line ending style detected in or applied to a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEnding {
    LF,
    CRLF,
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::LF => "\n",
            LineEnding::CRLF => "\r\n",
        }
    }

    /// Detect the dominant line ending in a string.
    pub fn detect(text: &str) -> Self {
        let crlf_count = text.matches("\r\n").count();
        let lf_count = text.matches('\n').count().saturating_sub(crlf_count);
        if crlf_count > lf_count {
            LineEnding::CRLF
        } else {
            LineEnding::LF
        }
    }
}

impl Default for LineEnding {
    fn default() -> Self {
        if cfg!(windows) {
            LineEnding::CRLF
        } else {
            LineEnding::LF
        }
    }
}

/// Git file status codes — mirrors the existing convention from fs_commands.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GitStatus {
    Modified,
    Added,
    Staged,
    Deleted,
    Untracked,
    Conflict,
    Renamed,
}

impl GitStatus {
    pub fn code(&self) -> &'static str {
        match self {
            GitStatus::Modified => "M",
            GitStatus::Added => "A",
            GitStatus::Staged => "S",
            GitStatus::Deleted => "D",
            GitStatus::Untracked => "U",
            GitStatus::Conflict => "C",
            GitStatus::Renamed => "R",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "M" => Some(GitStatus::Modified),
            "A" => Some(GitStatus::Added),
            "S" => Some(GitStatus::Staged),
            "D" => Some(GitStatus::Deleted),
            "U" => Some(GitStatus::Untracked),
            "C" => Some(GitStatus::Conflict),
            "R" => Some(GitStatus::Renamed),
            _ => None,
        }
    }
}
