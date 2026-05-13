use serde::Deserialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024; // 5 MB
const MAX_HISTORY_FILES: usize = 10;
const SECRET_PATTERN: &[&str] = &["api_key", "apikey", "api-key", "token", "password", "secret"];

pub struct LogState {
    writer: Mutex<Option<BufWriter<File>>>,
    log_path: PathBuf,
}

impl LogState {
    pub fn new() -> Self {
        let log_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".leo-ide")
            .join("logs");
        let _ = fs::create_dir_all(&log_dir);
        let log_path = log_dir.join("leo.jsonl");

        let writer = open_log_file(&log_path);

        // Prune old log files on startup
        prune_old_logs(&log_dir);

        Self {
            writer: Mutex::new(writer),
            log_path,
        }
    }
}

fn open_log_file(path: &PathBuf) -> Option<BufWriter<File>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
        .map(BufWriter::new)
}

fn prune_old_logs(log_dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(log_dir) else { return };
    let mut log_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map_or(false, |ext| ext == "jsonl")
                && p.file_name().map_or(false, |n| n != "leo.jsonl")
        })
        .collect();
    if log_files.len() <= MAX_HISTORY_FILES {
        return;
    }
    log_files.sort();
    for path in &log_files[..log_files.len() - MAX_HISTORY_FILES] {
        let _ = fs::remove_file(path);
    }
}

fn rotate_if_needed(state: &LogState) {
    let size = fs::metadata(&state.log_path).map(|m| m.len()).unwrap_or(0);
    if size < MAX_LOG_SIZE {
        return;
    }
    // Close current writer
    let mut guard = match state.writer.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    *guard = None;

    // Rename to timestamped file
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let rotated = state.log_path.with_file_name(format!("leo-{ts}.jsonl"));
    let _ = fs::rename(&state.log_path, &rotated);

    // Open fresh file
    *guard = open_log_file(&state.log_path);

    // Prune in background
    let log_dir = state.log_path.parent().unwrap_or(&state.log_path).to_path_buf();
    std::thread::spawn(move || prune_old_logs(&log_dir));
}

fn redact_value(s: &str) -> String {
    // Simple JSON-level redaction of secret-shaped keys
    let mut result = s.to_string();
    for pattern in SECRET_PATTERN {
        // Match keys like "apiKey":"value" or "api_key":"value"
        let search = format!("\"{}\"", pattern);
        if let Some(_) = result.to_lowercase().find(&search.to_lowercase()) {
            // Use regex-free approach: find the key, skip to value, replace
            if let Some(redacted) = redact_json_key(&result, pattern) {
                result = redacted;
            }
        }
    }
    result
}

fn redact_json_key(json: &str, key_pattern: &str) -> Option<String> {
    let lower = json.to_lowercase();
    let key_lower = key_pattern.to_lowercase();
    let mut result = json.to_string();
    let mut offset = 0;

    while let Some(pos) = lower[offset..].find(&format!("\"{}\"", key_lower)) {
        let abs_pos = offset + pos;
        // Find the colon after the key
        let after_key = abs_pos + key_pattern.len() + 2;
        if let Some(colon_offset) = result[after_key..].find(':') {
            let value_start = after_key + colon_offset + 1;
            let trimmed = result[value_start..].trim_start();
            let trim_offset = value_start + (result[value_start..].len() - trimmed.len());
            if trimmed.starts_with('"') {
                // Find end of string value
                let str_start = trim_offset + 1;
                let mut i = str_start;
                while i < result.len() {
                    if result.as_bytes()[i] == b'"' && (i == str_start || result.as_bytes()[i - 1] != b'\\') {
                        // Replace value
                        result = format!("{}\"[redacted]\"{}", &result[..trim_offset], &result[i + 1..]);
                        break;
                    }
                    i += 1;
                }
            }
        }
        offset = abs_pos + 1;
    }
    Some(result)
}

#[derive(Deserialize)]
pub struct LogError {
    message: String,
    stack: Option<String>,
}

#[tauri::command]
pub fn log_record(
    level: String,
    msg: String,
    ts: u64,
    data: Option<String>,
    err: Option<LogError>,
    state: tauri::State<'_, LogState>,
) {
    rotate_if_needed(&state);

    let data_redacted = data.as_deref().map(redact_value);

    let entry = serde_json::json!({
        "level": level,
        "msg": msg,
        "ts": ts,
        "data": data_redacted,
        "err": err.as_ref().map(|e| serde_json::json!({
            "message": &e.message,
            "stack": &e.stack,
        })),
    });

    let mut guard = match state.writer.lock() {
        Ok(g) => g,
        Err(_) => return, // Poisoned mutex — drop the log silently
    };

    if let Some(writer) = guard.as_mut() {
        let line = entry.to_string();
        let _ = writeln!(writer, "{}", line);
        // Flush immediately for error-level logs
        if level == "error" {
            let _ = writer.flush();
        }
    }
}
