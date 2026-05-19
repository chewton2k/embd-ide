//! Tree-sitter based symbol extraction.
//!
//! Extracts function, class, and method definitions from source files.
//! Used by the agent's context builder to provide symbol-level retrieval
//! instead of full-file retrieval — cuts tokens 10× for large files.

use serde::Serialize;
use std::path::Path;
use tree_sitter::{Language, Parser, Node};

use crate::modules::fs::ProjectRootState;

// ── Types ──

#[derive(Serialize, Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: String, // "function", "class", "method", "interface", "type"
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
}

// ── Language detection ──

fn language_for_extension(ext: &str) -> Option<Language> {
    match ext {
        "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "ts" | "tsx" | "mts" | "cts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "py" | "pyi" => Some(tree_sitter_python::LANGUAGE.into()),
        _ => None,
    }
}

// ── Extraction ──

/// Extract symbols from source code given a file extension.
pub fn extract_symbols(source: &str, extension: &str) -> Vec<Symbol> {
    let Some(language) = language_for_extension(extension) else {
        return vec![];
    };

    let mut parser = Parser::new();
    parser.set_language(&language).ok();

    let Some(tree) = parser.parse(source, None) else {
        return vec![];
    };

    let mut symbols = Vec::new();
    let root = tree.root_node();
    collect_symbols(root, source, extension, &mut symbols);
    symbols
}

fn collect_symbols(node: Node, source: &str, ext: &str, symbols: &mut Vec<Symbol>) {
    let kind = node.kind();

    let is_symbol = match ext {
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => matches!(
            kind,
            "function_declaration" | "class_declaration" | "method_definition"
            | "arrow_function" | "interface_declaration" | "type_alias_declaration"
            | "export_statement"
        ),
        "rs" => matches!(
            kind,
            "function_item" | "impl_item" | "struct_item" | "enum_item"
            | "trait_item" | "type_item"
        ),
        "py" | "pyi" => matches!(kind, "function_definition" | "class_definition"),
        _ => false,
    };

    if is_symbol {
        if let Some(sym) = extract_single(node, source, ext) {
            symbols.push(sym);
        }
    }

    // Recurse into children (but not into function bodies — we already captured them)
    let dominated = matches!(
        kind,
        "function_declaration" | "function_item" | "function_definition"
        | "method_definition" | "arrow_function"
    );
    if !dominated {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_symbols(child, source, ext, symbols);
        }
    }
}

fn extract_single(node: Node, source: &str, ext: &str) -> Option<Symbol> {
    let kind_str = normalize_kind(node.kind(), ext);
    let name = find_name(node, source, ext)?;
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let body = node.utf8_text(source.as_bytes()).ok()?.to_string();

    // Skip very short symbols (likely noise)
    if body.len() < 10 { return None; }

    Some(Symbol { name, kind: kind_str.to_string(), start_line, end_line, body })
}

fn normalize_kind(kind: &str, _ext: &str) -> &'static str {
    match kind {
        "function_declaration" | "function_item" | "function_definition" | "arrow_function" => "function",
        "class_declaration" | "class_definition" => "class",
        "method_definition" => "method",
        "interface_declaration" => "interface",
        "type_alias_declaration" | "type_item" => "type",
        "impl_item" => "impl",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "trait_item" => "trait",
        "export_statement" => "export",
        _ => "symbol",
    }
}

fn find_name(node: Node, source: &str, ext: &str) -> Option<String> {
    // For export statements, look inside for the actual declaration
    if node.kind() == "export_statement" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(name) = find_name(child, source, ext) {
                return Some(name);
            }
        }
        return None;
    }

    // Look for identifier/name child nodes
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" | "type_identifier" | "property_identifier" => {
                return child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
            }
            "name" => {
                return child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
            }
            _ => {}
        }
    }
    None
}

// ── Tauri commands ──

/// Extract symbols from a file.
#[tauri::command]
pub fn symbols_extract(
    window: tauri::WebviewWindow,
    path: String,
    state: tauri::State<'_, ProjectRootState>,
) -> Result<Vec<Symbol>, String> {
    // Validate path is within project — clone root and drop lock before I/O
    let root = {
        let map = state.blocking_read();
        map.get(window.label())
            .and_then(|opt| opt.as_ref())
            .ok_or("No project is open")?
            .clone()
    };
    let file_path = std::fs::canonicalize(&path).map_err(|e| format!("Invalid path: {e}"))?;
    if !file_path.starts_with(&root) {
        return Err("Access denied: path is outside the project".into());
    }

    let source = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    Ok(extract_symbols(&source, ext))
}

/// Extract a specific symbol's body by name from a file.
#[tauri::command]
pub fn symbols_get_body(
    window: tauri::WebviewWindow,
    path: String,
    symbol_name: String,
    state: tauri::State<'_, ProjectRootState>,
) -> Result<String, String> {
    let symbols = symbols_extract(window, path, state)?;
    symbols
        .into_iter()
        .find(|s| s.name == symbol_name)
        .map(|s| s.body)
        .ok_or_else(|| format!("Symbol '{}' not found", symbol_name))
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_js_function() {
        let source = r#"
function hello(name) {
  return `Hello, ${name}!`;
}

const add = (a, b) => a + b;
"#;
        let symbols = extract_symbols(source, "js");
        assert!(symbols.iter().any(|s| s.name == "hello" && s.kind == "function"));
    }

    #[test]
    fn extract_ts_class_and_interface() {
        let source = r#"
interface User {
  name: string;
  age: number;
}

class UserService {
  getUser(id: string): User {
    return { name: "test", age: 0 };
  }
}
"#;
        let symbols = extract_symbols(source, "ts");
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == "interface"));
        assert!(symbols.iter().any(|s| s.name == "UserService" && s.kind == "class"));
    }

    #[test]
    fn extract_rust_function_and_struct() {
        let source = r#"
pub struct Config {
    pub name: String,
    pub value: i32,
}

pub fn process(config: &Config) -> String {
    format!("{}: {}", config.name, config.value)
}
"#;
        let symbols = extract_symbols(source, "rs");
        assert!(symbols.iter().any(|s| s.name == "Config" && s.kind == "struct"));
        assert!(symbols.iter().any(|s| s.name == "process" && s.kind == "function"));
    }

    #[test]
    fn extract_python_function_and_class() {
        let source = r#"
def greet(name: str) -> str:
    return f"Hello, {name}!"

class Calculator:
    def add(self, a: int, b: int) -> int:
        return a + b
"#;
        let symbols = extract_symbols(source, "py");
        assert!(symbols.iter().any(|s| s.name == "greet" && s.kind == "function"));
        assert!(symbols.iter().any(|s| s.name == "Calculator" && s.kind == "class"));
    }

    #[test]
    fn returns_empty_for_unknown_extension() {
        let symbols = extract_symbols("some content", "xyz");
        assert!(symbols.is_empty());
    }

    #[test]
    fn symbol_has_correct_line_numbers() {
        let source = "line1\nfn foo() {\n  bar()\n}\nline5\n";
        let symbols = extract_symbols(source, "rs");
        let foo = symbols.iter().find(|s| s.name == "foo").unwrap();
        assert_eq!(foo.start_line, 2);
        assert_eq!(foo.end_line, 4);
    }

    #[test]
    fn symbol_body_contains_full_definition() {
        let source = "function add(a, b) {\n  return a + b;\n}\n";
        let symbols = extract_symbols(source, "js");
        let add = symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(add.body.contains("return a + b"));
    }
}
