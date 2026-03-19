use serde::Serialize;
use std::ops::Range;
use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Language, Parser, Point, Query, QueryCursor, Tree};

/// A span of highlighted text with its capture name (e.g., "keyword", "string").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HighlightSpan {
    /// Byte range in the source text.
    pub byte_range: Range<usize>,
    /// The capture name from the highlight query (e.g., "keyword", "type", "function").
    pub capture_name: String,
}

/// Configuration for a single language: its tree-sitter grammar and highlight query.
pub struct LanguageConfig {
    pub name: String,
    pub language: Language,
    /// Tree-sitter highlight query (S-expression patterns).
    pub highlight_query: Option<Query>,
    /// File extensions this language handles.
    pub extensions: Vec<String>,
}

impl LanguageConfig {
    pub fn new(name: &str, language: Language, extensions: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            language,
            highlight_query: None,
            extensions: extensions.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Set the highlight query from an S-expression string.
    /// These queries define which syntax tree nodes map to which highlight groups.
    pub fn with_highlights(mut self, query_source: &str) -> Result<Self, String> {
        let query = Query::new(&self.language, query_source)
            .map_err(|e| format!("Failed to compile highlight query for {}: {}", self.name, e))?;
        self.highlight_query = Some(query);
        Ok(self)
    }
}

/// Manages the syntax tree for a single buffer.
///
/// Supports incremental re-parsing: when the buffer is edited, call `edit()`
/// with the edit information, then `parse()` with the updated source. The
/// tree-sitter parser will only re-parse the affected region.
pub struct SyntaxTree {
    parser: Parser,
    tree: Option<Tree>,
    language_name: String,
}

impl SyntaxTree {
    /// Create a new syntax tree for the given language.
    pub fn new(config: &LanguageConfig) -> Result<Self, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&config.language)
            .map_err(|e| format!("Failed to set language: {}", e))?;

        Ok(Self {
            parser,
            tree: None,
            language_name: config.name.clone(),
        })
    }

    pub fn language_name(&self) -> &str {
        &self.language_name
    }

    /// Parse (or re-parse) the full source text.
    /// If a previous tree exists, this will be an incremental parse.
    pub fn parse(&mut self, source: &[u8]) -> bool {
        let old_tree = self.tree.as_ref();
        match self.parser.parse(source, old_tree) {
            Some(new_tree) => {
                self.tree = Some(new_tree);
                true
            }
            None => false,
        }
    }

    /// Notify the tree of an edit so the next `parse()` call can be incremental.
    ///
    /// This maps directly to tree-sitter's `Tree::edit()`. The edit info
    /// must be in byte offsets and (row, column) positions.
    pub fn edit(&mut self, edit: &SyntaxEdit) {
        if let Some(tree) = &mut self.tree {
            tree.edit(&InputEdit {
                start_byte: edit.start_byte,
                old_end_byte: edit.old_end_byte,
                new_end_byte: edit.new_end_byte,
                start_position: Point {
                    row: edit.start_row,
                    column: edit.start_col,
                },
                old_end_position: Point {
                    row: edit.old_end_row,
                    column: edit.old_end_col,
                },
                new_end_position: Point {
                    row: edit.new_end_row,
                    column: edit.new_end_col,
                },
            });
        }
    }

    /// Get the root node's S-expression (useful for debugging).
    pub fn sexp(&self) -> Option<String> {
        self.tree.as_ref().map(|t| t.root_node().to_sexp())
    }

    /// Check if the tree has any syntax errors.
    pub fn has_errors(&self) -> bool {
        self.tree
            .as_ref()
            .map_or(false, |t| t.root_node().has_error())
    }

    /// Run a highlight query against the current tree for a byte range.
    /// Returns spans tagged with capture names.
    pub fn highlights(
        &self,
        source: &[u8],
        query: &Query,
        byte_range: Range<usize>,
    ) -> Vec<HighlightSpan> {
        let tree = match &self.tree {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(byte_range);

        let capture_names = query.capture_names();

        // tree-sitter 0.24 uses StreamingIterator (advance + get) instead of Iterator
        let mut matches = cursor.matches(query, tree.root_node(), source);
        let mut spans = Vec::new();

        while let Some(m) = {
            matches.advance();
            matches.get()
        } {
            for capture in m.captures {
                let node = capture.node;
                let name = capture_names
                    .get(capture.index as usize)
                    .copied()
                    .unwrap_or("unknown");

                spans.push(HighlightSpan {
                    byte_range: node.byte_range(),
                    capture_name: name.to_string(),
                });
            }
        }

        // Sort by start position, then by length (longer spans first for nesting)
        spans.sort_by(|a, b| {
            a.byte_range
                .start
                .cmp(&b.byte_range.start)
                .then(b.byte_range.end.cmp(&a.byte_range.end))
        });

        spans
    }

    /// Get the tree-sitter node kind at a byte offset (for debugging/inspection).
    pub fn node_at(&self, byte_offset: usize) -> Option<String> {
        let tree = self.tree.as_ref()?;
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;
        Some(node.kind().to_string())
    }
}

/// Edit information for incremental parsing.
/// All values are byte offsets and row/column positions in the source text.
#[derive(Debug, Clone)]
pub struct SyntaxEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_row: usize,
    pub start_col: usize,
    pub old_end_row: usize,
    pub old_end_col: usize,
    pub new_end_row: usize,
    pub new_end_col: usize,
}

/// Registry of available languages.
/// Maps file extensions to language configurations.
pub struct LanguageRegistry {
    configs: Vec<LanguageConfig>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
        }
    }

    /// Register a language configuration.
    pub fn register(&mut self, config: LanguageConfig) {
        self.configs.push(config);
    }

    /// Find a language by file extension (e.g., "rs", "js", "py").
    pub fn find_by_extension(&self, ext: &str) -> Option<&LanguageConfig> {
        let ext_lower = ext.to_lowercase();
        self.configs
            .iter()
            .find(|c| c.extensions.iter().any(|e| e == &ext_lower))
    }

    /// Find a language by name.
    pub fn find_by_name(&self, name: &str) -> Option<&LanguageConfig> {
        let name_lower = name.to_lowercase();
        self.configs
            .iter()
            .find(|c| c.name.to_lowercase() == name_lower)
    }

    /// Get all registered language names.
    pub fn language_names(&self) -> Vec<&str> {
        self.configs.iter().map(|c| c.name.as_str()).collect()
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: build a SyntaxEdit from buffer state before and after an edit.
/// This bridges the gap between ropey's char-based edits and tree-sitter's
/// byte-based edit tracking.
pub fn build_syntax_edit(
    source_before: &str,
    edit_start_char: usize,
    chars_removed: usize,
    new_text: &str,
) -> SyntaxEdit {
    // Convert char offset to byte offset in the old text
    let start_byte = char_to_byte_offset(source_before, edit_start_char);
    let old_end_byte = char_to_byte_offset(source_before, edit_start_char + chars_removed);

    // Compute new end byte
    let new_end_byte = start_byte + new_text.len();

    // Compute row/col positions
    let (start_row, start_col) = byte_to_row_col(source_before, start_byte);
    let (old_end_row, old_end_col) = byte_to_row_col(source_before, old_end_byte);

    // For new end position, we need to compute it from the new text
    let (new_end_row, new_end_col) = if new_text.is_empty() {
        (start_row, start_col)
    } else {
        let newlines_in_new = new_text.matches('\n').count();
        if newlines_in_new == 0 {
            (start_row, start_col + new_text.len())
        } else {
            let last_newline = new_text.rfind('\n').unwrap();
            (
                start_row + newlines_in_new,
                new_text.len() - last_newline - 1,
            )
        }
    };

    SyntaxEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_row,
        start_col,
        old_end_row,
        old_end_col,
        new_end_row,
        new_end_col,
    }
}

/// Convert a char offset to a byte offset in a UTF-8 string.
fn char_to_byte_offset(s: &str, char_offset: usize) -> usize {
    s.char_indices()
        .nth(char_offset)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or(s.len())
}

/// Convert a byte offset to (row, col) where col is also in bytes.
fn byte_to_row_col(s: &str, byte_offset: usize) -> (usize, usize) {
    let prefix = &s[..byte_offset.min(s.len())];
    let row = prefix.matches('\n').count();
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = byte_offset - last_newline;
    (row, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_registry() {
        let mut reg = LanguageRegistry::new();

        // Use tree-sitter-rust for testing
        let rust_lang =
            LanguageConfig::new("Rust", tree_sitter_rust::LANGUAGE.into(), &["rs"]);
        reg.register(rust_lang);

        assert!(reg.find_by_extension("rs").is_some());
        assert!(reg.find_by_extension("py").is_none());
        assert!(reg.find_by_name("Rust").is_some());
        assert_eq!(reg.language_names(), vec!["Rust"]);
    }

    #[test]
    fn test_parse_rust() {
        let config =
            LanguageConfig::new("Rust", tree_sitter_rust::LANGUAGE.into(), &["rs"]);
        let mut syntax = SyntaxTree::new(&config).unwrap();

        let source = b"fn main() { let x = 42; }";
        assert!(syntax.parse(source));
        assert!(!syntax.has_errors());

        // Check that we can find nodes
        let sexp = syntax.sexp().unwrap();
        assert!(sexp.contains("function_item"));
    }

    #[test]
    fn test_incremental_parse() {
        let config =
            LanguageConfig::new("Rust", tree_sitter_rust::LANGUAGE.into(), &["rs"]);
        let mut syntax = SyntaxTree::new(&config).unwrap();

        let source_v1 = "fn main() { let x = 1; }";
        syntax.parse(source_v1.as_bytes());

        // Edit: change "1" to "100" at byte position 20
        let edit = build_syntax_edit(source_v1, 20, 1, "100");
        syntax.edit(&edit);

        let source_v2 = "fn main() { let x = 100; }";
        assert!(syntax.parse(source_v2.as_bytes()));
        assert!(!syntax.has_errors());
    }

    #[test]
    fn test_highlight_query() {
        let config = LanguageConfig::new("Rust", tree_sitter_rust::LANGUAGE.into(), &["rs"])
            .with_highlights(
                r#"
                (function_item name: (identifier) @function)
                "fn" @keyword
                "let" @keyword
                (integer_literal) @number
                "#,
            )
            .unwrap();

        let mut syntax = SyntaxTree::new(&config).unwrap();
        let source = b"fn main() { let x = 42; }";
        syntax.parse(source);

        let query = config.highlight_query.as_ref().unwrap();
        let spans = syntax.highlights(source, query, 0..source.len());

        // Should find: "fn" as keyword, "main" as function, "let" as keyword, "42" as number
        let keywords: Vec<_> = spans
            .iter()
            .filter(|s| s.capture_name == "keyword")
            .collect();
        assert!(keywords.len() >= 2, "Expected at least 2 keywords (fn, let)");

        let functions: Vec<_> = spans
            .iter()
            .filter(|s| s.capture_name == "function")
            .collect();
        assert_eq!(functions.len(), 1);
        assert_eq!(
            std::str::from_utf8(&source[functions[0].byte_range.clone()]).unwrap(),
            "main"
        );

        let numbers: Vec<_> = spans
            .iter()
            .filter(|s| s.capture_name == "number")
            .collect();
        assert_eq!(numbers.len(), 1);
        assert_eq!(
            std::str::from_utf8(&source[numbers[0].byte_range.clone()]).unwrap(),
            "42"
        );
    }

    #[test]
    fn test_node_at() {
        let config =
            LanguageConfig::new("Rust", tree_sitter_rust::LANGUAGE.into(), &["rs"]);
        let mut syntax = SyntaxTree::new(&config).unwrap();

        let source = b"fn main() {}";
        syntax.parse(source);

        // "fn" is at byte 0
        assert_eq!(syntax.node_at(0).unwrap(), "fn");
        // "main" is at byte 3
        assert_eq!(syntax.node_at(3).unwrap(), "identifier");
    }

    #[test]
    fn test_syntax_edit_helper() {
        let source = "hello world";
        // Replace "world" (chars 6..11) with "rust" (4 chars)
        let edit = build_syntax_edit(source, 6, 5, "rust");
        assert_eq!(edit.start_byte, 6);
        assert_eq!(edit.old_end_byte, 11);
        assert_eq!(edit.new_end_byte, 10); // 6 + 4
        assert_eq!(edit.start_row, 0);
        assert_eq!(edit.start_col, 6);
    }

    #[test]
    fn test_multiline_edit() {
        let source = "line1\nline2\nline3";
        // Replace "line2" (chars 6..11) with "new\nstuff"
        let edit = build_syntax_edit(source, 6, 5, "new\nstuff");
        assert_eq!(edit.start_row, 1);
        assert_eq!(edit.start_col, 0);
        assert_eq!(edit.new_end_row, 2); // "new\nstuff" adds a newline
        assert_eq!(edit.new_end_col, 5); // "stuff" is 5 bytes after the newline
    }

    #[test]
    fn test_char_to_byte_offset() {
        // ASCII
        assert_eq!(char_to_byte_offset("hello", 3), 3);
        // Multi-byte (Japanese)
        let s = "aあb";
        assert_eq!(char_to_byte_offset(s, 0), 0); // 'a'
        assert_eq!(char_to_byte_offset(s, 1), 1); // 'あ' starts at byte 1
        assert_eq!(char_to_byte_offset(s, 2), 4); // 'b' starts at byte 4 (あ is 3 bytes)
    }
}
