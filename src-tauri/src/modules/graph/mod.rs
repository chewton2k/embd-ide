use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ── Types ──

#[derive(Serialize, Clone)]
pub struct FileGraph {
    pub target: FileNode,
    pub imports: Vec<ImportNode>,
    pub dependents: Vec<DependentNode>,
    pub exports: Vec<ExportNode>,
    pub endpoints: Vec<EndpointNode>,
    pub schemas: Vec<SchemaNode>,
    pub external_deps: Vec<String>,
    pub calls: Vec<CallNode>,
}

#[derive(Serialize, Clone)]
pub struct FileNode {
    pub path: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct ImportNode {
    pub path: String,
    pub name: String,
    pub symbols: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct DependentNode {
    pub path: String,
    pub name: String,
    pub symbols: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct ExportNode {
    pub name: String,
    pub kind: String,
    pub signature: String,
    pub params: Vec<String>,
    pub return_type: String,
}

#[derive(Serialize, Clone)]
pub struct EndpointNode {
    pub method: String,
    pub route: String,
    pub handler: String,
    pub params: Vec<String>,
    pub middleware: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct SchemaNode {
    pub name: String,
    pub kind: String,
    pub fields: Vec<SchemaField>,
    pub source: String, // "type", "mongoose", "prisma", "sequelize", "typeorm", "drizzle", "sqlalchemy", "diesel"
}

#[derive(Serialize, Clone)]
pub struct SchemaField {
    pub name: String,
    pub field_type: String,
    pub optional: bool,
    pub constraints: Vec<String>, // e.g. "unique", "required", "default: 0", "ref: User"
}

/// Represents a function/method call relationship within the file.
#[derive(Serialize, Clone)]
pub struct CallNode {
    pub caller: String,
    pub callee: String,
    pub is_async: bool,
}

// ── Parser ──

fn detect_language(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript",
        Some("ts" | "tsx" | "mts" | "cts") => "typescript",
        Some("svelte") => "svelte",
        Some("rs") => "rust",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        _ => "unknown",
    }
}

fn parse_js_ts(content: &str, file_dir: &Path, project_root: &Path) -> (Vec<ImportNode>, Vec<ExportNode>, Vec<EndpointNode>, Vec<SchemaNode>, Vec<String>, Vec<CallNode>) {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut endpoints = Vec::new();
    let mut schemas = Vec::new();
    let mut external = Vec::new();
    let mut calls = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut current_function: Option<String> = None;
    let mut in_interface = false;
    let mut current_schema: Option<SchemaNode> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Track current function scope for call detection
        if trimmed.contains("function ") || trimmed.contains("=> {") || trimmed.contains("async ") {
            if let Some(name) = extract_function_name(trimmed) {
                current_function = Some(name);
            }
        }

        // Detect function calls within current function
        if let Some(ref caller) = current_function {
            for callee in extract_calls(trimmed) {
                if callee != *caller {
                    let is_async = trimmed.contains("await ");
                    calls.push(CallNode { caller: caller.clone(), callee, is_async });
                }
            }
        }

        // Imports
        if trimmed.starts_with("import ") || trimmed.contains("require(") {
            if let Some(path_str) = extract_module_path(trimmed) {
                let symbols = extract_import_symbols(trimmed);
                if path_str.starts_with('.') {
                    let resolved = resolve_relative_path(&path_str, file_dir, project_root);
                    let name = Path::new(&resolved).file_name().unwrap_or_default().to_string_lossy().to_string();
                    imports.push(ImportNode { path: resolved, name, symbols });
                } else {
                    let pkg = path_str.split('/').next().unwrap_or(&path_str);
                    if !external.contains(&pkg.to_string()) {
                        external.push(pkg.to_string());
                    }
                }
            }
        }

        // Exports with full signatures
        if trimmed.starts_with("export ") {
            if let Some(exp) = parse_export_detailed(trimmed, &lines, i) {
                exports.push(exp);
            }
        }

        // API endpoints with handler and middleware
        if let Some(ep) = parse_endpoint_detailed(trimmed) {
            endpoints.push(ep);
        }

        // Schema/interface/type with fields
        if (trimmed.contains("interface ") || trimmed.contains("type ") && trimmed.contains("=")) && trimmed.contains('{') {
            let kind = if trimmed.contains("interface") { "interface" } else { "type" };
            let name = extract_schema_name(trimmed, kind);
            if !name.is_empty() {
                in_interface = true;
                current_schema = Some(SchemaNode { name, kind: kind.into(), fields: vec![], source: String::new() });
            }
        } else if in_interface {
            if trimmed == "}" || trimmed.starts_with("};") {
                in_interface = false;
                if let Some(schema) = current_schema.take() {
                    schemas.push(schema);
                }
            } else if let Some(ref mut schema) = current_schema {
                if let Some(field) = parse_schema_field(trimmed) {
                    schema.fields.push(field);
                }
            }
        }
    }

    // Deduplicate calls
    calls.sort_by(|a, b| (&a.caller, &a.callee).cmp(&(&b.caller, &b.callee)));
    calls.dedup_by(|a, b| a.caller == b.caller && a.callee == b.callee);

    // ── DB Schema Detection ──
    parse_db_schemas_js(&lines, &mut schemas);

    (imports, exports, endpoints, schemas, external, calls)
}

/// Detect database schema definitions: Mongoose, Sequelize, Drizzle, TypeORM, Prisma-like patterns.
fn parse_db_schemas_js(lines: &[&str], schemas: &mut Vec<SchemaNode>) {
    let content = lines.join("\n");
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // ── Mongoose: new Schema({...}) or mongoose.Schema({...})
        if trimmed.contains("new Schema(") || trimmed.contains("mongoose.Schema(") || trimmed.contains("new mongoose.Schema(") {
            // Try to find the model name from preceding line or same line
            let name = find_mongoose_model_name(lines, i);
            if !name.is_empty() {
                let fields = extract_object_fields(lines, i);
                schemas.push(SchemaNode {
                    name,
                    kind: "model".into(),
                    fields,
                    source: "mongoose".into(),
                });
            }
        }

        // ── Sequelize: define('ModelName', {...}) or Model.init({...})
        if trimmed.contains(".define(") || trimmed.contains(".init(") {
            if let Some(name) = extract_sequelize_name(trimmed) {
                let fields = extract_object_fields(lines, i);
                schemas.push(SchemaNode {
                    name,
                    kind: "model".into(),
                    fields,
                    source: "sequelize".into(),
                });
            }
        }

        // ── Drizzle: pgTable('name', {...}) / mysqlTable / sqliteTable
        if trimmed.contains("pgTable(") || trimmed.contains("mysqlTable(") || trimmed.contains("sqliteTable(") {
            if let Some(name) = extract_drizzle_name(trimmed) {
                let fields = extract_drizzle_fields(lines, i);
                schemas.push(SchemaNode {
                    name,
                    kind: "table".into(),
                    fields,
                    source: "drizzle".into(),
                });
            }
        }

        // ── TypeORM: @Entity() decorator followed by class
        if trimmed.starts_with("@Entity(") {
            // Next non-empty line should be the class
            if let Some(class_line) = lines.get(i + 1) {
                let ct = class_line.trim();
                if ct.starts_with("class ") || ct.starts_with("export class ") {
                    let name = ct.split("class ").nth(1).unwrap_or("")
                        .split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string();
                    if !name.is_empty() {
                        let fields = extract_typeorm_fields(lines, i + 1);
                        schemas.push(SchemaNode {
                            name,
                            kind: "entity".into(),
                            fields,
                            source: "typeorm".into(),
                        });
                    }
                }
            }
        }

        // ── Prisma-style model (in .prisma files or embedded)
        if trimmed.starts_with("model ") && trimmed.ends_with('{') {
            let name = trimmed.strip_prefix("model ").unwrap_or("").trim().trim_end_matches('{').trim().to_string();
            if !name.is_empty() {
                let fields = extract_prisma_fields(lines, i);
                schemas.push(SchemaNode {
                    name,
                    kind: "model".into(),
                    fields,
                    source: "prisma".into(),
                });
            }
        }

        // ── Raw SQL: CREATE TABLE
        if trimmed.to_uppercase().contains("CREATE TABLE") {
            if let Some(name) = extract_sql_table_name(trimmed) {
                let fields = extract_sql_fields(lines, i);
                schemas.push(SchemaNode {
                    name,
                    kind: "table".into(),
                    fields,
                    source: "sql".into(),
                });
            }
        }

        i += 1;
    }

    // Deduplicate by name (prefer DB schemas over plain types)
    let _ = content; // suppress unused warning
}

fn find_mongoose_model_name(lines: &[&str], idx: usize) -> String {
    // Check same line: const User = new Schema(...)
    let line = lines[idx].trim();
    if let Some(pos) = line.find('=') {
        let before = line[..pos].trim();
        let name = before.split_whitespace().last().unwrap_or("").to_string();
        if !name.is_empty() && name != "const" && name != "let" && name != "var" {
            return name.trim_end_matches("Schema").to_string();
        }
    }
    // Check line before: const UserSchema = ...
    if idx > 0 {
        let prev = lines[idx - 1].trim();
        if let Some(pos) = prev.find('=') {
            let before = prev[..pos].trim();
            let name = before.split_whitespace().last().unwrap_or("").to_string();
            return name.trim_end_matches("Schema").to_string();
        }
    }
    // Check for mongoose.model('Name', ...)
    for line in lines.iter() {
        if line.contains("mongoose.model(") || line.contains(".model(") {
            if let Some(start) = line.find("model(") {
                let after = &line[start + 6..];
                let quote = after.chars().next().unwrap_or(' ');
                if quote == '\'' || quote == '"' {
                    if let Some(end) = after[1..].find(quote) {
                        return after[1..1 + end].to_string();
                    }
                }
            }
        }
    }
    String::new()
}

fn extract_object_fields(lines: &[&str], start: usize) -> Vec<SchemaField> {
    let mut fields = Vec::new();
    let mut brace_depth = 0;
    let mut started = false;

    for line in &lines[start..] {
        for c in line.chars() {
            if c == '{' { brace_depth += 1; started = true; }
            if c == '}' { brace_depth -= 1; }
        }
        if started && brace_depth == 1 {
            let trimmed = line.trim().trim_end_matches(',');
            if let Some(colon_pos) = trimmed.find(':') {
                let name = trimmed[..colon_pos].trim().trim_matches(|c| c == '\'' || c == '"').to_string();
                if !name.is_empty() && !name.contains(' ') && name != "{" {
                    let value = trimmed[colon_pos + 1..].trim().to_string();
                    let (field_type, constraints) = parse_mongoose_field_value(&value);
                    let optional = constraints.iter().any(|c| c.contains("required: false")) || !constraints.iter().any(|c| c.contains("required"));
                    fields.push(SchemaField { name, field_type, optional, constraints });
                }
            }
        }
        if started && brace_depth <= 0 { break; }
    }
    fields
}

fn parse_mongoose_field_value(value: &str) -> (String, Vec<String>) {
    let mut constraints = Vec::new();
    let trimmed = value.trim();

    // Simple type: String, Number, Boolean, Date, ObjectId
    if trimmed == "String" || trimmed == "Number" || trimmed == "Boolean" || trimmed == "Date" {
        return (trimmed.to_string(), constraints);
    }

    // Object form: { type: String, required: true, unique: true, default: ... }
    if trimmed.starts_with('{') {
        let inner = trimmed.trim_start_matches('{').trim_end_matches('}');
        let mut field_type = String::new();
        for part in inner.split(',') {
            let part = part.trim();
            if part.starts_with("type:") {
                field_type = part.trim_start_matches("type:").trim().to_string();
            } else if part.starts_with("required:") && part.contains("true") {
                constraints.push("required".into());
            } else if part.starts_with("unique:") && part.contains("true") {
                constraints.push("unique".into());
            } else if part.starts_with("default:") {
                constraints.push(part.trim().to_string());
            } else if part.starts_with("ref:") {
                constraints.push(part.trim().to_string());
            }
        }
        return (field_type, constraints);
    }

    // Array: [String] or [{ type: ... }]
    if trimmed.starts_with('[') {
        return (format!("Array<{}>", trimmed.trim_matches(|c| c == '[' || c == ']')), constraints);
    }

    (trimmed.to_string(), constraints)
}

fn extract_sequelize_name(line: &str) -> Option<String> {
    // .define('ModelName', ...) or .define("ModelName", ...)
    if let Some(idx) = line.find(".define(") {
        let after = &line[idx + 8..];
        let quote = after.chars().next()?;
        if quote == '\'' || quote == '"' {
            let end = after[1..].find(quote)?;
            return Some(after[1..1 + end].to_string());
        }
    }
    // Model.init({...}, ...) — get model name from before .init
    if let Some(idx) = line.find(".init(") {
        let before = line[..idx].trim();
        let name = before.split_whitespace().last()?.to_string();
        if !name.is_empty() { return Some(name); }
    }
    None
}

fn extract_drizzle_name(line: &str) -> Option<String> {
    // pgTable('name', {...})
    let patterns = ["pgTable(", "mysqlTable(", "sqliteTable("];
    for pat in patterns {
        if let Some(idx) = line.find(pat) {
            let after = &line[idx + pat.len()..];
            let quote = after.chars().next()?;
            if quote == '\'' || quote == '"' {
                let end = after[1..].find(quote)?;
                return Some(after[1..1 + end].to_string());
            }
        }
    }
    None
}

fn extract_drizzle_fields(lines: &[&str], start: usize) -> Vec<SchemaField> {
    let mut fields = Vec::new();
    let mut brace_depth = 0;
    let mut started = false;

    for line in &lines[start..] {
        for c in line.chars() {
            if c == '{' { brace_depth += 1; started = true; }
            if c == '}' { brace_depth -= 1; }
        }
        if started && brace_depth >= 1 {
            let trimmed = line.trim();
            // Pattern: fieldName: type('column_name').constraints()
            if let Some(colon_pos) = trimmed.find(':') {
                let name = trimmed[..colon_pos].trim().to_string();
                if !name.is_empty() && !name.contains(' ') && !name.starts_with('{') && !name.starts_with('/') {
                    let value = trimmed[colon_pos + 1..].trim().trim_end_matches(',');
                    let field_type = value.split('(').next().unwrap_or("").trim().to_string();
                    let mut constraints = Vec::new();
                    if value.contains(".notNull()") { constraints.push("NOT NULL".into()); }
                    if value.contains(".unique()") { constraints.push("UNIQUE".into()); }
                    if value.contains(".primaryKey()") { constraints.push("PRIMARY KEY".into()); }
                    if value.contains(".default(") { constraints.push("HAS DEFAULT".into()); }
                    if value.contains(".references(") { constraints.push("FOREIGN KEY".into()); }
                    fields.push(SchemaField { name, field_type, optional: !value.contains(".notNull()"), constraints });
                }
            }
        }
        if started && brace_depth <= 0 { break; }
    }
    fields
}

fn extract_typeorm_fields(lines: &[&str], class_start: usize) -> Vec<SchemaField> {
    let mut fields = Vec::new();
    let mut brace_depth = 0;
    let mut started = false;
    let mut pending_column = false;
    let mut constraints = Vec::new();

    for line in &lines[class_start..] {
        let trimmed = line.trim();
        for c in line.chars() {
            if c == '{' { brace_depth += 1; started = true; }
            if c == '}' { brace_depth -= 1; }
        }

        if trimmed.starts_with("@Column(") || trimmed.starts_with("@PrimaryGeneratedColumn(") || trimmed.starts_with("@PrimaryColumn(") {
            pending_column = true;
            constraints = Vec::new();
            if trimmed.contains("PrimaryGeneratedColumn") { constraints.push("PRIMARY KEY".into()); constraints.push("AUTO INCREMENT".into()); }
            if trimmed.contains("PrimaryColumn") { constraints.push("PRIMARY KEY".into()); }
            if trimmed.contains("unique: true") { constraints.push("UNIQUE".into()); }
            if trimmed.contains("nullable: true") { constraints.push("NULLABLE".into()); }
        } else if trimmed.starts_with("@ManyToOne(") || trimmed.starts_with("@OneToMany(") || trimmed.starts_with("@ManyToMany(") {
            pending_column = true;
            constraints = vec!["RELATION".into()];
        } else if pending_column && !trimmed.starts_with('@') && trimmed.contains(':') {
            let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
            if parts.len() == 2 {
                let name = parts[0].trim().to_string();
                let field_type = parts[1].trim().trim_end_matches(';').to_string();
                let optional = field_type.contains('?') || constraints.iter().any(|c| c == "NULLABLE");
                fields.push(SchemaField { name, field_type, optional, constraints: constraints.clone() });
            }
            pending_column = false;
        }

        if started && brace_depth <= 0 { break; }
    }
    fields
}

fn extract_prisma_fields(lines: &[&str], start: usize) -> Vec<SchemaField> {
    let mut fields = Vec::new();
    for line in &lines[start + 1..] {
        let trimmed = line.trim();
        if trimmed == "}" { break; }
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("@@") { continue; }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let field_type = parts[1].to_string();
            let optional = field_type.ends_with('?');
            let mut constraints = Vec::new();
            let rest = parts[2..].join(" ");
            if rest.contains("@id") { constraints.push("PRIMARY KEY".into()); }
            if rest.contains("@unique") { constraints.push("UNIQUE".into()); }
            if rest.contains("@default") { constraints.push("HAS DEFAULT".into()); }
            if rest.contains("@relation") { constraints.push("RELATION".into()); }
            if rest.contains("@map") { constraints.push("MAPPED".into()); }
            fields.push(SchemaField { name, field_type, optional, constraints });
        }
    }
    fields
}

fn extract_sql_table_name(line: &str) -> Option<String> {
    let upper = line.to_uppercase();
    let idx = upper.find("CREATE TABLE")?;
    let after = &line[idx + 12..].trim_start();
    let after = after.strip_prefix("IF NOT EXISTS").unwrap_or(after).trim();
    let name = after.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.').next()?.trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']').to_string();
    if name.is_empty() { None } else { Some(name) }
}

fn extract_sql_fields(lines: &[&str], start: usize) -> Vec<SchemaField> {
    let mut fields = Vec::new();
    let mut paren_depth = 0;
    let mut started = false;

    for line in &lines[start..] {
        for c in line.chars() {
            if c == '(' { paren_depth += 1; started = true; }
            if c == ')' { paren_depth -= 1; }
        }
        if started && paren_depth >= 1 {
            let trimmed = line.trim().trim_end_matches(',');
            if trimmed.is_empty() || trimmed.starts_with("--") || trimmed.starts_with("(") { continue; }
            // Skip constraints lines
            let upper = trimmed.to_uppercase();
            if upper.starts_with("PRIMARY KEY") || upper.starts_with("FOREIGN KEY") || upper.starts_with("CONSTRAINT") || upper.starts_with("UNIQUE") || upper.starts_with("INDEX") || upper.starts_with("CHECK") { continue; }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0].trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']').to_string();
                let field_type = parts[1].to_uppercase();
                let mut constraints = Vec::new();
                let rest = upper.clone();
                if rest.contains("NOT NULL") { constraints.push("NOT NULL".into()); }
                if rest.contains("PRIMARY KEY") { constraints.push("PRIMARY KEY".into()); }
                if rest.contains("UNIQUE") { constraints.push("UNIQUE".into()); }
                if rest.contains("DEFAULT") { constraints.push("HAS DEFAULT".into()); }
                if rest.contains("REFERENCES") { constraints.push("FOREIGN KEY".into()); }
                let optional = !rest.contains("NOT NULL");
                fields.push(SchemaField { name, field_type, optional, constraints });
            }
        }
        if started && paren_depth <= 0 { break; }
    }
    fields
}

fn parse_rust(content: &str) -> (Vec<ImportNode>, Vec<ExportNode>, Vec<SchemaNode>, Vec<CallNode>) {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut schemas = Vec::new();
    let mut calls = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut current_function: Option<String> = None;
    let mut in_struct = false;
    let mut current_schema: Option<SchemaNode> = None;

    for line in lines.iter() {
        let trimmed = line.trim();

        if trimmed.starts_with("use ") {
            let path_str = trimmed.trim_start_matches("use ").trim_end_matches(';').trim();
            let name = path_str.split("::").last().unwrap_or(path_str).to_string();
            imports.push(ImportNode {
                path: path_str.to_string(),
                name,
                symbols: vec![],
            });
        }

        if trimmed.starts_with("pub fn ") || trimmed.starts_with("pub async fn ") {
            let is_async = trimmed.contains("async");
            let after = trimmed.split("fn ").nth(1).unwrap_or("");
            let name = after.split('(').next().unwrap_or("").trim().to_string();
            if !name.is_empty() {
                let params = extract_rust_params(after);
                let return_type = extract_rust_return(after);
                let signature = format!("fn {}({}){}", name, params.join(", "),
                    if return_type.is_empty() { String::new() } else { format!(" -> {}", return_type) });
                exports.push(ExportNode {
                    name: name.clone(),
                    kind: if is_async { "async fn".into() } else { "fn".into() },
                    signature,
                    params,
                    return_type,
                });
                current_function = Some(name);
            }
        } else if trimmed.starts_with("fn ") && !trimmed.starts_with("fn main") {
            let after = trimmed.split("fn ").nth(1).unwrap_or("");
            let name = after.split('(').next().unwrap_or("").trim().to_string();
            if !name.is_empty() {
                current_function = Some(name);
            }
        }

        // Track calls
        if let Some(ref caller) = current_function {
            for callee in extract_calls(trimmed) {
                if callee != *caller && !callee.is_empty() {
                    let is_async = trimmed.contains(".await") || trimmed.contains("await ");
                    calls.push(CallNode { caller: caller.clone(), callee, is_async });
                }
            }
        }

        // Struct/enum fields
        if trimmed.starts_with("pub struct ") && trimmed.contains('{') {
            let name = trimmed.trim_start_matches("pub struct ").split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string();
            if !name.is_empty() {
                in_struct = true;
                current_schema = Some(SchemaNode { name, kind: "struct".into(), fields: vec![], source: String::new() });
            }
        } else if trimmed.starts_with("pub enum ") {
            let name = trimmed.trim_start_matches("pub enum ").split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string();
            if !name.is_empty() {
                schemas.push(SchemaNode { name, kind: "enum".into(), fields: vec![], source: String::new() });
            }
        } else if in_struct {
            if trimmed == "}" {
                in_struct = false;
                if let Some(schema) = current_schema.take() {
                    schemas.push(schema);
                }
            } else if trimmed.starts_with("pub ") && trimmed.contains(':') {
                let field_part = trimmed.trim_start_matches("pub ");
                let parts: Vec<&str> = field_part.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let fname = parts[0].trim().to_string();
                    let ftype = parts[1].trim().trim_end_matches(',').to_string();
                    let optional = ftype.starts_with("Option<");
                    if let Some(ref mut schema) = current_schema {
                        schema.fields.push(SchemaField { name: fname, field_type: ftype, optional, constraints: vec![] });
                    }
                }
            }
        }
    }

    calls.sort_by(|a, b| (&a.caller, &a.callee).cmp(&(&b.caller, &b.callee)));
    calls.dedup_by(|a, b| a.caller == b.caller && a.callee == b.callee);

    (imports, exports, schemas, calls)
}

fn parse_python(content: &str) -> (Vec<ImportNode>, Vec<ExportNode>, Vec<EndpointNode>, Vec<SchemaNode>, Vec<String>, Vec<CallNode>) {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut endpoints = Vec::new();
    let mut schemas = Vec::new();
    let mut external = Vec::new();
    let mut calls = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut current_function: Option<String> = None;
    let mut in_class = false;
    let mut current_schema: Option<SchemaNode> = None;

    for line in lines.iter() {
        let trimmed = line.trim();

        if trimmed.starts_with("from ") || trimmed.starts_with("import ") {
            if trimmed.starts_with("from .") || trimmed.starts_with("from ..") {
                let module = trimmed.split_whitespace().nth(1).unwrap_or("");
                let symbols: Vec<String> = if trimmed.contains("import ") {
                    trimmed.split("import ").nth(1).unwrap_or("").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                } else { vec![] };
                imports.push(ImportNode { path: module.to_string(), name: module.to_string(), symbols });
            } else {
                let module = if trimmed.starts_with("from ") {
                    trimmed.split_whitespace().nth(1).unwrap_or("")
                } else {
                    trimmed.split_whitespace().nth(1).unwrap_or("")
                };
                let pkg = module.split('.').next().unwrap_or(module).to_string();
                if !external.contains(&pkg) {
                    external.push(pkg);
                }
            }
        }

        if trimmed.starts_with("def ") && !trimmed.starts_with("def _") {
            let after = trimmed.trim_start_matches("def ");
            let name = after.split('(').next().unwrap_or("").trim().to_string();
            if !name.is_empty() {
                let params = extract_python_params(after);
                let return_type = if after.contains("->") {
                    after.split("->").nth(1).unwrap_or("").trim().trim_end_matches(':').trim().to_string()
                } else { String::new() };
                let signature = format!("def {}({}){}", name, params.join(", "),
                    if return_type.is_empty() { String::new() } else { format!(" -> {}", return_type) });
                exports.push(ExportNode { name: name.clone(), kind: "function".into(), signature, params, return_type });
                current_function = Some(name);
            }
        }

        if trimmed.starts_with("class ") {
            let name = trimmed.trim_start_matches("class ").split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string();
            if !name.is_empty() {
                in_class = true;
                current_schema = Some(SchemaNode { name, kind: "class".into(), fields: vec![], source: String::new() });
            }
        } else if in_class && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            in_class = false;
            if let Some(schema) = current_schema.take() {
                schemas.push(schema);
            }
        } else if in_class && trimmed.starts_with("self.") && trimmed.contains('=') {
            let field_name = trimmed.trim_start_matches("self.").split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string();
            if !field_name.is_empty() {
                if let Some(ref mut schema) = current_schema {
                    if !schema.fields.iter().any(|f| f.name == field_name) {
                        schema.fields.push(SchemaField { name: field_name, field_type: String::new(), optional: false, constraints: vec![] });
                    }
                }
            }
        }

        // Track calls
        if let Some(ref caller) = current_function {
            for callee in extract_calls(trimmed) {
                if callee != *caller {
                    let is_async = trimmed.contains("await ");
                    calls.push(CallNode { caller: caller.clone(), callee, is_async });
                }
            }
        }

        // Flask/FastAPI endpoints
        if trimmed.starts_with("@app.") || trimmed.starts_with("@router.") {
            if let Some(ep) = parse_python_endpoint(trimmed) {
                endpoints.push(ep);
            }
        }
    }

    if in_class {
        if let Some(schema) = current_schema.take() {
            schemas.push(schema);
        }
    }

    calls.sort_by(|a, b| (&a.caller, &a.callee).cmp(&(&b.caller, &b.callee)));
    calls.dedup_by(|a, b| a.caller == b.caller && a.callee == b.callee);

    (imports, exports, endpoints, schemas, external, calls)
}

// ── Helpers ──

fn extract_module_path(line: &str) -> Option<String> {
    // import ... from 'path' or import ... from "path"
    if let Some(idx) = line.rfind("from ") {
        let after = &line[idx + 5..];
        let after = after.trim().trim_end_matches(';');
        let quote = after.chars().next()?;
        if quote == '\'' || quote == '"' {
            let end = after[1..].find(quote)?;
            return Some(after[1..1 + end].to_string());
        }
    }
    // require('path')
    if let Some(idx) = line.find("require(") {
        let after = &line[idx + 8..];
        let quote = after.chars().next()?;
        if quote == '\'' || quote == '"' {
            let end = after[1..].find(quote)?;
            return Some(after[1..1 + end].to_string());
        }
    }
    None
}

fn extract_import_symbols(line: &str) -> Vec<String> {
    if let Some(start) = line.find('{') {
        if let Some(end) = line.find('}') {
            return line[start + 1..end].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
    }
    // default import: import Name from ...
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "import" && !parts[1].starts_with('{') && !parts[1].starts_with('*') && parts[1] != "type" {
        return vec![parts[1].trim_end_matches(',').to_string()];
    }
    vec![]
}

fn resolve_relative_path(module_path: &str, file_dir: &Path, project_root: &Path) -> String {
    let resolved = file_dir.join(module_path);
    let normalized = normalize_path(&resolved);

    // Try common extensions
    let extensions = ["", ".ts", ".js", ".tsx", ".jsx", ".svelte", "/index.ts", "/index.js"];
    for ext in extensions {
        let candidate = PathBuf::from(format!("{}{}", normalized.display(), ext));
        if candidate.exists() {
            return candidate.strip_prefix(project_root).unwrap_or(&candidate).to_string_lossy().to_string();
        }
    }

    // Return as-is (relative to project root if possible)
    normalized.strip_prefix(project_root).unwrap_or(&normalized).to_string_lossy().to_string()
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => { result.pop(); }
            std::path::Component::CurDir => {}
            _ => result.push(component),
        }
    }
    result
}

fn parse_export_detailed(line: &str, _lines: &[&str], _line_idx: usize) -> Option<ExportNode> {
    let after = line.strip_prefix("export ")?.trim();
    let after = after.strip_prefix("default ").unwrap_or(after);

    // function/async function
    if after.starts_with("function ") || after.starts_with("async function ") {
        let is_async = after.starts_with("async");
        let fn_part = if is_async { after.strip_prefix("async function ")? } else { after.strip_prefix("function ")? };
        let name = fn_part.split('(').next()?.trim().to_string();
        let params = extract_js_params(fn_part);
        let return_type = extract_js_return_type(fn_part);
        let signature = format!("{}function {}({}){}", if is_async { "async " } else { "" }, name, params.join(", "),
            if return_type.is_empty() { String::new() } else { format!(": {}", return_type) });
        return Some(ExportNode { name, kind: "function".into(), signature, params, return_type });
    }

    // const/let with arrow function
    if after.starts_with("const ") || after.starts_with("let ") {
        let rest = after.split_whitespace().nth(1)?;
        let name = rest.split(|c: char| !c.is_alphanumeric() && c != '_').next()?.to_string();
        let is_fn = after.contains("=>") || after.contains("function");
        let kind = if is_fn { "function" } else { "const" };
        let params = if is_fn { extract_js_params(after) } else { vec![] };
        let return_type = extract_js_return_type(after);
        let signature = if is_fn {
            format!("const {} = ({}) => ...", name, params.join(", "))
        } else {
            format!("const {}", name)
        };
        return Some(ExportNode { name, kind: kind.into(), signature, params, return_type });
    }

    // class
    if after.starts_with("class ") {
        let name = after.strip_prefix("class ")?.split(|c: char| !c.is_alphanumeric() && c != '_').next()?.to_string();
        return Some(ExportNode { name: name.clone(), kind: "class".into(), signature: format!("class {}", name), params: vec![], return_type: String::new() });
    }

    // type/interface
    if after.starts_with("type ") || after.starts_with("interface ") {
        let kw = if after.starts_with("type") { "type" } else { "interface" };
        let rest = after.strip_prefix(kw)?.trim();
        let name = rest.split(|c: char| !c.is_alphanumeric() && c != '_').next()?.to_string();
        return Some(ExportNode { name: name.clone(), kind: kw.into(), signature: format!("{} {}", kw, name), params: vec![], return_type: String::new() });
    }

    // enum
    if after.starts_with("enum ") {
        let name = after.strip_prefix("enum ")?.split(|c: char| !c.is_alphanumeric() && c != '_').next()?.to_string();
        return Some(ExportNode { name: name.clone(), kind: "enum".into(), signature: format!("enum {}", name), params: vec![], return_type: String::new() });
    }

    None
}

fn parse_endpoint_detailed(line: &str) -> Option<EndpointNode> {
    let methods = ["get", "post", "put", "delete", "patch"];
    for method in methods {
        let patterns = [
            format!("app.{}(", method),
            format!("router.{}(", method),
        ];
        for pat in &patterns {
            if let Some(idx) = line.to_lowercase().find(&pat.to_lowercase()) {
                let after = &line[idx + pat.len()..];
                let quote = after.chars().next()?;
                if quote == '\'' || quote == '"' || quote == '`' {
                    let end = after[1..].find(quote)?;
                    let route = after[1..1 + end].to_string();
                    // Extract handler name and middleware
                    let rest = &after[2 + end..];
                    let parts: Vec<&str> = rest.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                    let handler = parts.last().unwrap_or(&"").trim_end_matches(')').trim().to_string();
                    let middleware: Vec<String> = if parts.len() > 1 {
                        parts[..parts.len()-1].iter().map(|s| s.trim().to_string()).collect()
                    } else { vec![] };
                    let params: Vec<String> = route.split('/').filter(|s| s.starts_with(':') || s.starts_with('{'))
                        .map(|s| s.trim_matches(|c| c == ':' || c == '{' || c == '}').to_string()).collect();
                    return Some(EndpointNode { method: method.to_uppercase(), route, handler, params, middleware });
                }
            }
        }
    }
    None
}

fn extract_function_name(line: &str) -> Option<String> {
    if line.contains("function ") {
        let after = line.split("function ").last()?;
        let name = after.split('(').next()?.trim().to_string();
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Some(name);
        }
    }
    // const name = ... => or const name = async (...) =>
    if (line.contains("const ") || line.contains("let ")) && line.contains("=>") {
        let after = line.split(|c| c == ' ' || c == '\t').find(|s| !s.is_empty() && *s != "const" && *s != "let" && *s != "export" && *s != "async")?;
        let name = after.split(|c: char| !c.is_alphanumeric() && c != '_').next()?.to_string();
        if !name.is_empty() { return Some(name); }
    }
    None
}

fn extract_calls(line: &str) -> Vec<String> {
    let mut calls = Vec::new();
    let mut chars = line.chars().peekable();
    let mut buf = String::new();

    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            buf.push(c);
        } else {
            if c == '(' && !buf.is_empty() {
                // Skip keywords
                let keywords = ["if", "else", "for", "while", "switch", "return", "new", "typeof", "instanceof", "import", "export", "from", "const", "let", "var", "function", "async", "await", "class"];
                if !keywords.contains(&buf.as_str()) && buf.chars().next().map_or(false, |c| c.is_lowercase()) {
                    calls.push(buf.clone());
                }
            }
            buf.clear();
            // Skip string literals
            if c == '\'' || c == '"' || c == '`' {
                while let Some(nc) = chars.next() {
                    if nc == c { break; }
                    if nc == '\\' { chars.next(); }
                }
            }
        }
    }
    calls
}

fn extract_js_params(line: &str) -> Vec<String> {
    if let Some(start) = line.find('(') {
        if let Some(end) = line[start..].find(')') {
            let inner = &line[start + 1..start + end];
            return inner.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
    }
    vec![]
}

fn extract_js_return_type(line: &str) -> String {
    // Look for ): ReturnType or ): Promise<X>
    if let Some(idx) = line.find("):") {
        let after = line[idx + 2..].trim();
        let ret = after.split(|c| c == '{' || c == ';').next().unwrap_or("").trim();
        if !ret.is_empty() { return ret.to_string(); }
    }
    String::new()
}

fn extract_rust_params(line: &str) -> Vec<String> {
    if let Some(start) = line.find('(') {
        if let Some(end) = line[start..].find(')') {
            let inner = &line[start + 1..start + end];
            return inner.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && *s != "&self" && *s != "&mut self" && *s != "self").collect();
        }
    }
    vec![]
}

fn extract_rust_return(line: &str) -> String {
    if let Some(idx) = line.find("->") {
        let after = line[idx + 2..].trim();
        let ret = after.split(|c| c == '{' || c == ';').next().unwrap_or("").trim();
        if !ret.is_empty() { return ret.to_string(); }
    }
    String::new()
}

fn extract_python_params(line: &str) -> Vec<String> {
    if let Some(start) = line.find('(') {
        if let Some(end) = line[start..].find(')') {
            let inner = &line[start + 1..start + end];
            return inner.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && *s != "self" && *s != "cls").collect();
        }
    }
    vec![]
}

fn extract_schema_name(line: &str, kind: &str) -> String {
    let prefix = format!("{} ", kind);
    if let Some(rest) = line.split(&prefix).nth(1) {
        return rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string();
    }
    // Try after export
    if let Some(rest) = line.split(&format!("export {}", prefix)).nth(1) {
        return rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string();
    }
    String::new()
}

fn parse_schema_field(line: &str) -> Option<SchemaField> {
    let trimmed = line.trim().trim_end_matches([',', ';']);
    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") { return None; }
    // name: type or name?: type
    let (name_part, type_part) = trimmed.split_once(':')?;
    let name = name_part.trim().trim_end_matches('?').to_string();
    let optional = name_part.contains('?');
    let field_type = type_part.trim().to_string();
    if name.is_empty() || name.contains(' ') { return None; }
    Some(SchemaField { name, field_type, optional, constraints: vec![] })
}

fn parse_python_endpoint(line: &str) -> Option<EndpointNode> {
    // @app.get("/path") or @router.post("/path")
    let methods = ["get", "post", "put", "delete", "patch"];
    for method in methods {
        let pat = format!(".{}(", method);
        if let Some(idx) = line.find(&pat) {
            let after = &line[idx + pat.len()..];
            let quote = after.chars().next()?;
            if quote == '\'' || quote == '"' {
                let end = after[1..].find(quote)?;
                let route = after[1..1 + end].to_string();
                let params: Vec<String> = route.split('/').filter(|s| s.starts_with('{'))
                    .map(|s| s.trim_matches(|c| c == '{' || c == '}').to_string()).collect();
                return Some(EndpointNode { method: method.to_uppercase(), route, handler: String::new(), params, middleware: vec![] });
            }
        }
    }
    None
}

/// Find files in the project that import the target file.
fn find_dependents(target_path: &Path, project_root: &Path) -> Vec<DependentNode> {
    let mut dependents = Vec::new();
    let target_stem = target_path.file_stem().unwrap_or_default().to_string_lossy();
    let target_rel = target_path.strip_prefix(project_root).unwrap_or(target_path).to_string_lossy().to_string();

    let skip_dirs: HashSet<&str> = ["node_modules", ".git", "dist", "build", "target", ".next", "__pycache__"].into_iter().collect();

    fn walk(dir: &Path, skip: &HashSet<&str>, files: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !skip.contains(name.as_ref()) {
                    walk(&path, skip, files);
                }
            } else {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "js" | "jsx" | "ts" | "tsx" | "mjs" | "svelte" | "rs" | "py") {
                    files.push(path);
                }
            }
        }
    }

    let mut files = Vec::new();
    walk(project_root, &skip_dirs, &mut files);

    for file in files {
        if file == target_path { continue; }
        let Ok(content) = std::fs::read_to_string(&file) else { continue };

        // Check if this file imports the target
        let imports_target = content.lines().any(|line| {
            let trimmed = line.trim();
            if let Some(module) = extract_module_path(trimmed) {
                // Check if the module path resolves to our target
                if module.contains(&*target_stem) {
                    return true;
                }
                // Check relative path resolution
                if module.starts_with('.') {
                    let file_dir = file.parent().unwrap_or(project_root);
                    let resolved = resolve_relative_path(&module, file_dir, project_root);
                    if resolved == target_rel || resolved.trim_end_matches(&['.', '/', '\\'][..]) == target_rel.trim_end_matches(&['.', '/', '\\'][..]) {
                        return true;
                    }
                }
            }
            false
        });

        if imports_target {
            let rel = file.strip_prefix(project_root).unwrap_or(&file).to_string_lossy().to_string();
            let name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
            dependents.push(DependentNode { path: rel, name, symbols: vec![] });
        }
    }

    dependents
}

// ── Tauri Command ──

#[tauri::command]
pub fn analyze_file_graph(
    file_path: String,
    project_root: String,
    state: tauri::State<'_, crate::modules::fs::ProjectRootState>,
) -> Result<FileGraph, String> {
    // Validate both paths are within the project root
    let root_guard = state.lock().map_err(|e| e.to_string())?;
    let root_canonical = root_guard
        .as_ref()
        .ok_or_else(|| "No project is open".to_string())?;
    let root = std::fs::canonicalize(&project_root)
        .map_err(|e| format!("Invalid project root: {}", e))?;
    if !root.starts_with(root_canonical) && root != *root_canonical {
        return Err("Access denied: project root mismatch".to_string());
    }
    drop(root_guard);

    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }
    let path_canonical = std::fs::canonicalize(&path)
        .map_err(|e| format!("Invalid file path: {}", e))?;
    if !path_canonical.starts_with(&root) {
        return Err("Access denied: file is outside the project directory".to_string());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    let lang = detect_language(&path);
    let file_dir = path.parent().unwrap_or(&root);
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let rel_path = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().to_string();

    let target = FileNode { path: rel_path, name: file_name };

    let (imports, exports, endpoints, schemas, external_deps, calls) = match lang {
        "javascript" | "typescript" | "svelte" => {
            let (imp, exp, ep, sch, ext, calls) = parse_js_ts(&content, file_dir, &root);
            (imp, exp, ep, sch, ext, calls)
        }
        "rust" => {
            let (imp, exp, sch, calls) = parse_rust(&content);
            (imp, exp, vec![], sch, vec![], calls)
        }
        "python" => {
            let (imp, exp, ep, sch, ext, calls) = parse_python(&content);
            (imp, exp, ep, sch, ext, calls)
        }
        _ => (vec![], vec![], vec![], vec![], vec![], vec![]),
    };

    let dependents = find_dependents(&path, &root);

    Ok(FileGraph { target, imports, dependents, exports, endpoints, schemas, external_deps, calls })
}
