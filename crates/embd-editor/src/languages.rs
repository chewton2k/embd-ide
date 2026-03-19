//! Built-in language configurations for syntax highlighting.
//!
//! Each language has a tree-sitter grammar and a highlight query that maps
//! syntax tree nodes to capture names (keyword, string, comment, etc.).
//! The frontend maps these capture names to theme colors.

use crate::syntax::{LanguageConfig, LanguageRegistry};

/// Build a [`LanguageRegistry`] pre-loaded with all built-in language grammars.
pub fn build_default_registry() -> LanguageRegistry {
    let mut reg = LanguageRegistry::new();

    let configs: Vec<Result<LanguageConfig, String>> = vec![
        rust_config(),
        javascript_config(),
        typescript_config(),
        tsx_config(),
        python_config(),
        json_config(),
        css_config(),
        html_config(),
        go_config(),
        c_config(),
    ];

    for result in configs {
        match result {
            Ok(config) => reg.register(config),
            Err(e) => {
                // Highlight query compilation failed — log and skip.
                // This is non-fatal; the language just won't have highlighting.
                eprintln!("Warning: failed to register language: {}", e);
            }
        }
    }

    reg
}

// ── Rust ────────────────────────────────────────────────────────────────

fn rust_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new("Rust", tree_sitter_rust::LANGUAGE.into(), &["rs"])
        .with_highlights(RUST_HIGHLIGHTS)
}

const RUST_HIGHLIGHTS: &str = r#"
"fn" @keyword
"let" @keyword
"if" @keyword
"else" @keyword
"for" @keyword
"while" @keyword
"loop" @keyword
"match" @keyword
"return" @keyword
"break" @keyword
"continue" @keyword
"struct" @keyword
"enum" @keyword
"impl" @keyword
"trait" @keyword
"pub" @keyword
"use" @keyword
"mod" @keyword
"type" @keyword
"const" @keyword
"static" @keyword
"where" @keyword
"as" @keyword
"in" @keyword
"unsafe" @keyword
"async" @keyword
"await" @keyword

; Named keyword nodes
(mutable_specifier) @keyword
(self) @variable.builtin

; Literals
(boolean_literal) @constant.builtin
(integer_literal) @number
(float_literal) @number
(char_literal) @string
(string_literal) @string
(raw_string_literal) @string

; Comments
(line_comment) @comment
(block_comment) @comment

; Types
(type_identifier) @type
(primitive_type) @type.builtin

; Functions
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (field_expression field: (field_identifier) @function.call))
(macro_invocation macro: (identifier) @function.macro)

; Fields
(field_identifier) @property

; Attributes
(attribute_item) @attribute
"#;

// ── JavaScript ──────────────────────────────────────────────────────────

fn javascript_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new(
        "JavaScript",
        tree_sitter_javascript::LANGUAGE.into(),
        &["js", "mjs", "cjs", "jsx"],
    )
    .with_highlights(JAVASCRIPT_HIGHLIGHTS)
}

const JAVASCRIPT_HIGHLIGHTS: &str = r#"
; Keywords
[
  "break" "case" "catch" "class" "const" "continue" "debugger" "default"
  "delete" "do" "else" "export" "extends" "finally" "for" "function"
  "if" "import" "in" "instanceof" "let" "new" "of" "return" "switch"
  "throw" "try" "typeof" "var" "void" "while" "with" "yield"
  "async" "await"
] @keyword

; Literals
(true) @constant.builtin
(false) @constant.builtin
(null) @constant.builtin
(this) @variable.builtin

; Comments
(comment) @comment

; Strings
(string) @string
(template_string) @string
(regex) @string.regex

; Numbers
(number) @number

; Functions
(function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (member_expression property: (property_identifier) @function.call))

; Properties
(property_identifier) @property
"#;

// ── TypeScript ──────────────────────────────────────────────────────────

fn typescript_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new(
        "TypeScript",
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        &["ts", "mts", "cts"],
    )
    .with_highlights(TYPESCRIPT_HIGHLIGHTS)
}

fn tsx_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new(
        "TSX",
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        &["tsx"],
    )
    .with_highlights(TYPESCRIPT_HIGHLIGHTS)
}

const TYPESCRIPT_HIGHLIGHTS: &str = r#"
; Keywords
[
  "break" "case" "catch" "class" "const" "continue" "debugger" "default"
  "delete" "do" "else" "export" "extends" "finally" "for" "function"
  "if" "import" "in" "instanceof" "let" "new" "of" "return" "switch"
  "throw" "try" "typeof" "var" "void" "while" "with" "yield"
  "async" "await" "implements" "interface" "enum" "type" "namespace"
  "declare" "abstract" "readonly" "as" "is" "keyof" "satisfies"
] @keyword

; Literals
(true) @constant.builtin
(false) @constant.builtin
(null) @constant.builtin
(this) @variable.builtin

; Comments
(comment) @comment

; Strings
(string) @string
(template_string) @string
(regex) @string.regex

; Numbers
(number) @number

; Types
(type_identifier) @type
(predefined_type) @type.builtin

; Functions
(function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (member_expression property: (property_identifier) @function.call))

; Properties
(property_identifier) @property
"#;

// ── Python ──────────────────────────────────────────────────────────────

fn python_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new(
        "Python",
        tree_sitter_python::LANGUAGE.into(),
        &["py", "pyi", "pyw"],
    )
    .with_highlights(PYTHON_HIGHLIGHTS)
}

const PYTHON_HIGHLIGHTS: &str = r#"
; Keywords
[
  "and" "as" "assert" "async" "await" "break" "class" "continue" "def"
  "del" "elif" "else" "except" "finally" "for" "from" "global" "if"
  "import" "in" "is" "lambda" "nonlocal" "not" "or" "pass" "raise"
  "return" "try" "while" "with" "yield"
] @keyword

; Literals
(true) @constant.builtin
(false) @constant.builtin
(none) @constant.builtin

; Comments
(comment) @comment

; Strings
(string) @string
(concatenated_string) @string

; Numbers
(integer) @number
(float) @number

; Functions
(function_definition name: (identifier) @function)
(call function: (identifier) @function.call)
(call function: (attribute attribute: (identifier) @function.call))

; Decorators
(decorator) @attribute
"#;

// ── JSON ────────────────────────────────────────────────────────────────

fn json_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new("JSON", tree_sitter_json::LANGUAGE.into(), &["json", "jsonc"])
        .with_highlights(JSON_HIGHLIGHTS)
}

const JSON_HIGHLIGHTS: &str = r#"
(string) @string
(number) @number
(true) @constant.builtin
(false) @constant.builtin
(null) @constant.builtin
(pair key: (string) @property)
"#;

// ── CSS ─────────────────────────────────────────────────────────────────

fn css_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new("CSS", tree_sitter_css::LANGUAGE.into(), &["css", "scss"])
        .with_highlights(CSS_HIGHLIGHTS)
}

const CSS_HIGHLIGHTS: &str = r#"
(tag_name) @tag
(class_name) @type
(id_name) @constant
(property_name) @property
(plain_value) @string
(color_value) @number
(integer_value) @number
(float_value) @number
(string_value) @string
(comment) @comment
(at_keyword) @keyword
"#;

// ── HTML ────────────────────────────────────────────────────────────────

fn html_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new(
        "HTML",
        tree_sitter_html::LANGUAGE.into(),
        &["html", "htm", "xhtml"],
    )
    .with_highlights(HTML_HIGHLIGHTS)
}

const HTML_HIGHLIGHTS: &str = r#"
(tag_name) @tag
(attribute_name) @attribute
(attribute_value) @string
(comment) @comment
(doctype) @keyword
"#;

// ── Go ──────────────────────────────────────────────────────────────────

fn go_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new("Go", tree_sitter_go::LANGUAGE.into(), &["go"])
        .with_highlights(GO_HIGHLIGHTS)
}

const GO_HIGHLIGHTS: &str = r#"
; Keywords
[
  "break" "case" "chan" "const" "continue" "default" "defer" "else"
  "fallthrough" "for" "func" "go" "goto" "if" "import" "interface"
  "map" "package" "range" "return" "select" "struct" "switch" "type"
  "var"
] @keyword

; Literals
(true) @constant.builtin
(false) @constant.builtin
(nil) @constant.builtin
(iota) @constant.builtin

; Comments
(comment) @comment

; Strings
(interpreted_string_literal) @string
(raw_string_literal) @string
(rune_literal) @string

; Numbers
(int_literal) @number
(float_literal) @number
(imaginary_literal) @number

; Types
(type_identifier) @type

; Functions
(function_declaration name: (identifier) @function)
(method_declaration name: (field_identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (selector_expression field: (field_identifier) @function.call))

; Fields
(field_identifier) @property
"#;

// ── C ───────────────────────────────────────────────────────────────────

fn c_config() -> Result<LanguageConfig, String> {
    LanguageConfig::new("C", tree_sitter_c::LANGUAGE.into(), &["c", "h"])
        .with_highlights(C_HIGHLIGHTS)
}

const C_HIGHLIGHTS: &str = r#"
; Keywords
[
  "break" "case" "continue" "default" "do" "else" "enum" "extern" "for"
  "goto" "if" "return" "sizeof" "static" "struct" "switch" "typedef"
  "union" "volatile" "while" "inline"
] @keyword

; Comments
(comment) @comment

; Strings
(string_literal) @string
(char_literal) @string
(system_lib_string) @string

; Numbers
(number_literal) @number

; Types
(type_identifier) @type
(primitive_type) @type.builtin
(sized_type_specifier) @type.builtin

; Functions
(function_definition declarator: (function_declarator declarator: (identifier) @function))
(call_expression function: (identifier) @function.call)
(call_expression function: (field_expression field: (field_identifier) @function.call))

; Fields
(field_identifier) @property

; Preprocessor
(preproc_include) @keyword
(preproc_def) @keyword
(preproc_ifdef) @keyword
(preproc_if) @keyword
(preproc_else) @keyword
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_default_registry() {
        let reg = build_default_registry();
        let names = reg.language_names();

        assert!(names.contains(&"Rust"));
        assert!(names.contains(&"JavaScript"));
        assert!(names.contains(&"TypeScript"));
        assert!(names.contains(&"TSX"));
        assert!(names.contains(&"Python"));
        assert!(names.contains(&"JSON"));
        assert!(names.contains(&"CSS"));
        assert!(names.contains(&"HTML"));
        assert!(names.contains(&"Go"));
        assert!(names.contains(&"C"));
    }

    #[test]
    fn test_extension_lookup() {
        let reg = build_default_registry();

        assert_eq!(reg.find_by_extension("rs").unwrap().name, "Rust");
        assert_eq!(reg.find_by_extension("js").unwrap().name, "JavaScript");
        assert_eq!(reg.find_by_extension("ts").unwrap().name, "TypeScript");
        assert_eq!(reg.find_by_extension("tsx").unwrap().name, "TSX");
        assert_eq!(reg.find_by_extension("py").unwrap().name, "Python");
        assert_eq!(reg.find_by_extension("json").unwrap().name, "JSON");
        assert_eq!(reg.find_by_extension("css").unwrap().name, "CSS");
        assert_eq!(reg.find_by_extension("html").unwrap().name, "HTML");
        assert_eq!(reg.find_by_extension("go").unwrap().name, "Go");
        assert_eq!(reg.find_by_extension("c").unwrap().name, "C");
    }

    #[test]
    fn test_all_languages_have_highlight_queries() {
        let reg = build_default_registry();
        for name in reg.language_names() {
            let config = reg.find_by_name(name).unwrap();
            assert!(
                config.highlight_query.is_some(),
                "Language {} is missing highlight query",
                name
            );
        }
    }

    #[test]
    fn test_rust_highlights_parse_correctly() {
        use crate::syntax::SyntaxTree;

        let reg = build_default_registry();
        let config = reg.find_by_extension("rs").unwrap();
        let mut tree = SyntaxTree::new(config).unwrap();

        let source = b"fn main() { let x = 42; }";
        tree.parse(source);

        let query = config.highlight_query.as_ref().unwrap();
        let spans = tree.highlights(source, query, 0..source.len());

        let keywords: Vec<_> = spans.iter().filter(|s| s.capture_name == "keyword").collect();
        assert!(keywords.len() >= 2, "Expected fn and let as keywords");

        let numbers: Vec<_> = spans.iter().filter(|s| s.capture_name == "number").collect();
        assert_eq!(numbers.len(), 1);
    }

    #[test]
    fn test_javascript_highlights() {
        use crate::syntax::SyntaxTree;

        let reg = build_default_registry();
        let config = reg.find_by_extension("js").unwrap();
        let mut tree = SyntaxTree::new(config).unwrap();

        let source = b"function hello() { return 42; }";
        tree.parse(source);

        let query = config.highlight_query.as_ref().unwrap();
        let spans = tree.highlights(source, query, 0..source.len());

        let keywords: Vec<_> = spans.iter().filter(|s| s.capture_name == "keyword").collect();
        assert!(!keywords.is_empty(), "Expected at least one keyword");
    }

    #[test]
    fn test_python_highlights() {
        use crate::syntax::SyntaxTree;

        let reg = build_default_registry();
        let config = reg.find_by_extension("py").unwrap();
        let mut tree = SyntaxTree::new(config).unwrap();

        let source = b"def hello():\n    return 42";
        tree.parse(source);

        let query = config.highlight_query.as_ref().unwrap();
        let spans = tree.highlights(source, query, 0..source.len());

        let keywords: Vec<_> = spans.iter().filter(|s| s.capture_name == "keyword").collect();
        assert!(keywords.len() >= 2, "Expected def and return as keywords");
    }
}
