use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

// ── State ──

pub struct KnowledgeState {
    db: Mutex<Option<Connection>>,
}

impl KnowledgeState {
    pub fn new() -> Self {
        Self { db: Mutex::new(None) }
    }
}

// ── Types ──

#[derive(Serialize, Clone)]
pub struct FileInfo {
    pub path: String,
    pub language: String,
    pub summary: String,
    pub exports: String,
}

#[derive(Serialize, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Clone)]
pub struct IndexProgress {
    pub done: u32,
    pub total: u32,
}

// ── DB Path ──

fn db_path(project_root: &str) -> PathBuf {
    let hash = format!("{:x}", Sha256::digest(project_root.as_bytes()));
    let dir = dirs::home_dir().unwrap_or_default().join(".leo-ide").join("knowledge");
    std::fs::create_dir_all(&dir).ok();
    dir.join(format!("{}.db", &hash[..16]))
}

// ── Schema ──

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            hash TEXT NOT NULL,
            language TEXT,
            size INTEGER,
            last_indexed INTEGER,
            summary TEXT DEFAULT '',
            exports TEXT DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            title TEXT,
            created_at INTEGER,
            updated_at INTEGER,
            messages TEXT
        );
        CREATE TABLE IF NOT EXISTS project_meta (
            key TEXT PRIMARY KEY,
            value TEXT
        );"
    ).map_err(|e| format!("Schema init failed: {}", e))
}

// ── Cleanup ──

fn cleanup_old_data(conn: &Connection) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let thirty_days_ago = now - (30 * 24 * 60 * 60);
    let seven_days_ago = now - (7 * 24 * 60 * 60);

    // Keep only the 50 most recent conversations, delete anything older than 30 days
    conn.execute("DELETE FROM conversations WHERE updated_at < ?1", params![thirty_days_ago]).ok();
    conn.execute(
        "DELETE FROM conversations WHERE id NOT IN (SELECT id FROM conversations ORDER BY updated_at DESC LIMIT 50)",
        [],
    ).ok();

    // Remove file entries that haven't been re-indexed in 7 days (file was probably deleted)
    conn.execute("DELETE FROM files WHERE last_indexed < ?1", params![seven_days_ago]).ok();

    // Vacuum to reclaim space
    conn.execute_batch("VACUUM;").ok();
}

// ── Commands ──

#[tauri::command]
pub async fn knowledge_init(
    project_root: String,
    state: tauri::State<'_, Arc<KnowledgeState>>,
) -> Result<(), String> {
    let path = db_path(&project_root);
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open DB: {}", e))?;
    init_schema(&conn)?;
    cleanup_old_data(&conn);
    let mut db = state.db.lock().await;
    *db = Some(conn);
    Ok(())
}

#[tauri::command]
pub async fn knowledge_index(
    project_root: String,
    app: AppHandle,
) -> Result<(), String> {
    let root = PathBuf::from(&project_root);

    tokio::task::spawn_blocking(move || {
        let skip: HashSet<&str> = ["node_modules", ".git", "dist", "build", "target", ".next", "__pycache__", ".svelte-kit"].into_iter().collect();
        let mut files: Vec<PathBuf> = Vec::new();
        walk_files(&root, &skip, &mut files);

        let total = files.len() as u32;
        let _ = app.emit("indexing-progress", IndexProgress { done: 0, total });

        // Open DB in this thread
        let db_p = db_path(&root.to_string_lossy());
        let conn = match Connection::open(&db_p) {
            Ok(c) => c,
            Err(_) => return,
        };
        init_schema(&conn).ok();

        for (i, file) in files.iter().enumerate() {
            let rel = file.strip_prefix(&root).unwrap_or(file).to_string_lossy().to_string();
            let Ok(content) = std::fs::read_to_string(file) else { continue };
            let hash = format!("{:x}", Sha256::digest(content.as_bytes()));

            // Check if already indexed with same hash
            let existing_hash: Option<String> = conn
                .query_row("SELECT hash FROM files WHERE path = ?1", params![rel], |r| r.get(0))
                .ok();
            if existing_hash.as_deref() == Some(&hash) { continue; }

            let lang = detect_lang(file);
            let size = content.len() as i64;
            let summary = extract_summary(&content, &lang);
            let exports = extract_exports(&content, &lang);
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

            conn.execute(
                "INSERT OR REPLACE INTO files (path, hash, language, size, last_indexed, summary, exports) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![rel, hash, lang, size, now, summary, exports],
            ).ok();

            if (i + 1) % 20 == 0 || i + 1 == files.len() {
                let _ = app.emit("indexing-progress", IndexProgress { done: (i + 1) as u32, total });
            }
        }
    }).await.map_err(|e| format!("Indexing failed: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn knowledge_get_context(
    project_root: String,
    query: String,
    current_file: Option<String>,
) -> Result<Vec<FileInfo>, String> {
    let db_p = db_path(&project_root);
    let conn = Connection::open(&db_p).map_err(|e| format!("DB open failed: {}", e))?;

    let mut results: Vec<FileInfo> = Vec::new();
    let keywords: Vec<&str> = query.split_whitespace().filter(|w| w.len() > 2).collect();

    // 1. Files mentioned by name in query
    for kw in &keywords {
        let pattern = format!("%{}%", kw);
        let mut stmt = conn.prepare("SELECT path, language, summary, exports FROM files WHERE path LIKE ?1 LIMIT 3").map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(FileInfo { path: row.get(0)?, language: row.get(1)?, summary: row.get(2)?, exports: row.get(3)? })
        }).map_err(|e| e.to_string())?;
        for r in rows.flatten() {
            if !results.iter().any(|x| x.path == r.path) { results.push(r); }
        }
    }

    // 2. Files in same directory as current file
    if let Some(ref cf) = current_file {
        let dir = cf.rsplit_once('/').map(|(d, _)| format!("{}/%", d)).unwrap_or_default();
        if !dir.is_empty() {
            let mut stmt = conn.prepare("SELECT path, language, summary, exports FROM files WHERE path LIKE ?1 LIMIT 5").map_err(|e| e.to_string())?;
            let rows = stmt.query_map(params![dir], |row| {
                Ok(FileInfo { path: row.get(0)?, language: row.get(1)?, summary: row.get(2)?, exports: row.get(3)? })
            }).map_err(|e| e.to_string())?;
            for r in rows.flatten() {
                if !results.iter().any(|x| x.path == r.path) { results.push(r); }
            }
        }
    }

    // 3. Files matching keywords in exports/summary
    for kw in &keywords {
        let pattern = format!("%{}%", kw);
        let mut stmt = conn.prepare("SELECT path, language, summary, exports FROM files WHERE exports LIKE ?1 OR summary LIKE ?1 LIMIT 3").map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(FileInfo { path: row.get(0)?, language: row.get(1)?, summary: row.get(2)?, exports: row.get(3)? })
        }).map_err(|e| e.to_string())?;
        for r in rows.flatten() {
            if !results.iter().any(|x| x.path == r.path) { results.push(r); }
        }
    }

    // Limit to 10 most relevant
    results.truncate(10);
    Ok(results)
}

// ── Conversation persistence ──

#[tauri::command]
pub async fn knowledge_save_conversation(
    project_root: String,
    id: String,
    title: String,
    messages: String,
) -> Result<(), String> {
    let db_p = db_path(&project_root);
    let conn = Connection::open(&db_p).map_err(|e| format!("DB open failed: {}", e))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO conversations (id, title, created_at, updated_at, messages) VALUES (?1, ?2, COALESCE((SELECT created_at FROM conversations WHERE id = ?1), ?3), ?3, ?4)",
        params![id, title, now, messages],
    ).map_err(|e| format!("Save failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn knowledge_list_conversations(project_root: String) -> Result<Vec<ConversationSummary>, String> {
    let db_p = db_path(&project_root);
    let conn = Connection::open(&db_p).map_err(|e| format!("DB open failed: {}", e))?;
    let mut stmt = conn.prepare("SELECT id, title, created_at, updated_at FROM conversations ORDER BY updated_at DESC LIMIT 50").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(ConversationSummary { id: row.get(0)?, title: row.get(1)?, created_at: row.get(2)?, updated_at: row.get(3)? })
    }).map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub async fn knowledge_load_conversation(project_root: String, id: String) -> Result<String, String> {
    let db_p = db_path(&project_root);
    let conn = Connection::open(&db_p).map_err(|e| format!("DB open failed: {}", e))?;
    conn.query_row("SELECT messages FROM conversations WHERE id = ?1", params![id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Not found: {}", e))
}

#[tauri::command]
pub async fn knowledge_delete_conversations(project_root: String) -> Result<(), String> {
    let db_p = db_path(&project_root);
    let conn = Connection::open(&db_p).map_err(|e| format!("DB open failed: {}", e))?;
    conn.execute("DELETE FROM conversations", []).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Helpers ──

fn walk_files(dir: &Path, skip: &HashSet<&str>, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !skip.contains(name.as_ref()) && !name.starts_with('.') {
                walk_files(&path, skip, files);
            }
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "js"|"jsx"|"ts"|"tsx"|"svelte"|"rs"|"py"|"go"|"java"|"css"|"html"|"json"|"md"|"toml"|"yaml"|"yml"|"sql"|"sh"|"vue") {
                if path.metadata().map(|m| m.len() < 500_000).unwrap_or(false) {
                    files.push(path);
                }
            }
        }
    }
}

fn detect_lang(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("js" | "jsx" | "mjs") => "javascript",
        Some("ts" | "tsx" | "mts") => "typescript",
        Some("svelte") => "svelte",
        Some("rs") => "rust",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("css") => "css",
        Some("html") => "html",
        Some("json") => "json",
        Some("md") => "markdown",
        _ => "other",
    }.to_string()
}

fn extract_summary(content: &str, _lang: &str) -> String {
    let lines: Vec<&str> = content.lines().take(5).collect();
    // First meaningful comment or first few lines
    let mut summary = String::new();
    for line in &lines {
        let t = line.trim();
        if t.starts_with("//") || t.starts_with("#") || t.starts_with("/*") || t.starts_with("*") {
            summary.push_str(t.trim_start_matches(|c| c == '/' || c == '*' || c == '#' || c == ' '));
            summary.push(' ');
        }
    }
    if summary.is_empty() {
        summary = lines.join(" ");
    }
    summary.chars().take(200).collect()
}

fn extract_exports(content: &str, lang: &str) -> String {
    let mut exports = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        match lang {
            "javascript" | "typescript" | "svelte" => {
                if t.starts_with("export ") {
                    let name = t.split_whitespace().nth(2).or_else(|| t.split_whitespace().nth(1)).unwrap_or("").split('(').next().unwrap_or("").trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    if !name.is_empty() && name != "{" { exports.push(name.to_string()); }
                }
            }
            "rust" => {
                if t.starts_with("pub fn ") || t.starts_with("pub async fn ") {
                    let name = t.split("fn ").nth(1).unwrap_or("").split('(').next().unwrap_or("").trim();
                    if !name.is_empty() { exports.push(name.to_string()); }
                } else if t.starts_with("pub struct ") || t.starts_with("pub enum ") {
                    let name = t.split_whitespace().nth(2).unwrap_or("").split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("");
                    if !name.is_empty() { exports.push(name.to_string()); }
                }
            }
            "python" => {
                if t.starts_with("def ") && !t.starts_with("def _") {
                    let name = t.trim_start_matches("def ").split('(').next().unwrap_or("").trim();
                    if !name.is_empty() { exports.push(name.to_string()); }
                } else if t.starts_with("class ") {
                    let name = t.trim_start_matches("class ").split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("");
                    if !name.is_empty() { exports.push(name.to_string()); }
                }
            }
            _ => {}
        }
        if exports.len() >= 20 { break; }
    }
    exports.join(", ")
}
