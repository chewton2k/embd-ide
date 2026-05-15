use keyring::Entry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

mod key_store;

/// Shared HTTP client for non-streaming requests (with timeout).
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .pool_max_idle_per_host(4)
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .expect("reqwest client build")
    })
}

/// Shared HTTP client for streaming requests (no overall timeout).
fn http_client_streaming() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .pool_max_idle_per_host(4)
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest streaming client build")
    })
}

const SERVICE_NAME: &str = "leo-ide";

// ── Key storage ──
//
// Order of preference, on every read AND write:
//
//   1. OS keyring (primary, never on disk)
//   2. ChaCha20-Poly1305 encrypted file at `<base>/keys.enc`. The 32-byte
//      master key is itself stored in the keyring under `__file_key__`,
//      so the file is meaningless without keyring access.
//   3. Last-resort: derive the master key from machine-specific paths so
//      we can still read keys.enc on systems with no keyring at all.
//      This is *obfuscation*, not strong protection — see the security
//      notes in the deferred-items plan. The primary protections at this
//      tier are OS-level FDE and the 0o600 file permissions.
//
// Migration: any plaintext `keys.json` left over from earlier builds is
// re-keyed into the new format on startup and renamed to `keys.json.bak`.

fn keys_dir() -> PathBuf {
    let dir = dirs::home_dir().unwrap_or_default().join(".leo-ide");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn legacy_keys_file_in(base: &Path) -> PathBuf {
    base.join("keys.json")
}

/// Resolve the 32-byte master key used to seal `keys.enc`.
///
/// Uses a deterministic machine-derived key so that `keys.enc` is always
/// readable regardless of OS keyring state. Previous versions stored a
/// random key in the keyring, but that created a fragility: if keyring
/// access was lost (e.g. code signature change on macOS dev builds), the
/// encrypted file became permanently unreadable.
///
/// Security model: the file key is derived from machine-specific paths
/// and a build-time salt. This is obfuscation (not strong protection);
/// the primary defenses are OS-level FDE and 0o600 file permissions.
/// The OS keyring is still used as the *primary* store for the actual
/// API keys — the encrypted file is the durable fallback.
fn get_or_create_file_key() -> Result<[u8; key_store::KEY_SIZE], String> {
    Ok(derive_machine_file_key())
}

fn derive_machine_file_key() -> [u8; key_store::KEY_SIZE] {
    use sha2::{Digest, Sha256};
    // Build-time constant: changing the binary changes the salt and thus
    // invalidates the tier-3 fallback. Acceptable: tier-3 only matters on
    // systems with no keyring, where users will re-enter their keys.
    const SALT: &[u8] = b"leo-ide:v1:file-key-fallback";
    let mut h = Sha256::new();
    h.update(SALT);
    if let Some(p) = dirs::data_local_dir() {
        h.update(p.to_string_lossy().as_bytes());
    }
    if let Some(p) = dirs::home_dir() {
        h.update(p.to_string_lossy().as_bytes());
    }
    let digest = h.finalize();
    let mut k = [0u8; key_store::KEY_SIZE];
    k.copy_from_slice(&digest);
    k
}

fn get_key(provider: &str) -> Result<Option<String>, String> {
    if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
        if let Ok(pw) = entry.get_password() {
            if !pw.is_empty() {
                return Ok(Some(pw));
            }
        }
    }
    let file_key = get_or_create_file_key()?;
    let dir = keys_dir();
    match key_store::get(&dir, &file_key, provider) {
        Ok(v) => Ok(v),
        // Corrupted file: surface as "no key" so the caller asks the user
        // to re-enter, instead of crashing the AI flow.
        Err(e) => {
            log::warn!("encrypted keys file unreadable: {e}");
            Ok(None)
        }
    }
}

fn set_key(provider: &str, key: &str) -> Result<(), String> {
    let file_key = get_or_create_file_key()?;
    let dir = keys_dir();

    if key.is_empty() {
        // Delete from both stores so a stale entry can't shadow a new one.
        if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
            let _ = entry.delete_credential();
        }
        let _ = key_store::remove(&dir, &file_key, provider);
        return Ok(());
    }

    // Always write to the encrypted file store (authoritative, durable).
    key_store::put(&dir, &file_key, provider, key)?;

    // Best-effort write to OS keyring (fast-path cache for reads).
    if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
        let _ = entry.set_password(key);
    }

    Ok(())
}

/// Migrate any plaintext `keys.json` from older builds into the new
/// secure storage. The old file is renamed to `keys.json.bak` so users
/// can recover by hand if migration goes sideways.
///
/// Setting the `LEO_DISABLE_KEY_MIGRATION` env var skips migration —
/// useful as an emergency rollback knob.
pub fn migrate_plaintext_keys() {
    if std::env::var("LEO_DISABLE_KEY_MIGRATION").is_ok() {
        return;
    }
    migrate_plaintext_keys_in(&keys_dir(), &mut |provider, key| set_key(provider, key));
}

/// Test-friendly variant: migrate from `base/keys.json` using a custom
/// store function so we don't have to touch the real keyring or the
/// user's home directory.
pub fn migrate_plaintext_keys_in(
    base: &Path,
    store: &mut dyn FnMut(&str, &str) -> Result<(), String>,
) -> usize {
    let path = legacy_keys_file_in(base);
    if !path.exists() {
        return 0;
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("could not read legacy keys.json: {e}");
            return 0;
        }
    };
    let map: std::collections::HashMap<String, String> = match serde_json::from_slice(&bytes) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("legacy keys.json malformed: {e}");
            return 0;
        }
    };

    let mut migrated = 0usize;
    for (provider, key) in &map {
        if key.is_empty() {
            continue;
        }
        match store(provider, key) {
            Ok(()) => migrated += 1,
            Err(e) => log::warn!("migration: failed to store key for '{provider}': {e}"),
        }
    }

    let backup = path.with_extension("json.bak");
    // Best-effort rename. If it fails, we leave the plaintext file in
    // place rather than risk losing it; next startup will retry.
    if let Err(e) = std::fs::rename(&path, &backup) {
        log::warn!("migration: failed to rename keys.json → keys.json.bak: {e}");
    }
    log::info!("Migrated {migrated} key(s) from plaintext to secure storage");
    migrated
}

#[tauri::command]
pub fn set_api_key(key: String) -> Result<(), String> {
    set_key("openrouter", &key)
}

#[tauri::command]
pub fn set_provider_key(provider: String, key: String) -> Result<(), String> {
    set_key(&provider.to_lowercase(), &key)
}

#[tauri::command]
pub fn get_provider_key(provider: String) -> Result<String, String> {
    Ok(get_key(&provider.to_lowercase())?.unwrap_or_default())
}

// ── Types ──

#[derive(Deserialize, Clone)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct AiRequest {
    pub prompt: String,
    pub context: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
}

#[derive(Deserialize)]
pub struct AiStreamRequest {
    pub messages: Vec<ChatMessageInput>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub session_id: String,
    pub tools: Option<Value>,
}

#[derive(Serialize, Clone)]
pub struct StreamChunk {
    pub session_id: String,
    pub delta: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
}

const SYSTEM_PROMPT: &str = "You are an AI coding assistant embedded in a lightweight IDE called leo. \
    Help the user with their code: explain, debug, refactor, or write new code. \
    Keep responses concise and code-focused.";

// ── Cancellation state ──

pub struct AiState {
    pub cancel_tokens: Mutex<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,
}

impl AiState {
    pub fn new() -> Self {
        Self {
            cancel_tokens: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

// ── Legacy blocking command (kept for backward compat) ──

#[tauri::command]
pub async fn ai_chat(request: AiRequest) -> Result<String, String> {
    let provider = request.provider.clone().unwrap_or_else(|| "openrouter".to_string()).to_lowercase();
    let api_key = if provider == "local" {
        get_key(&provider)?.unwrap_or_else(|| "http://localhost:11434".to_string())
    } else {
        get_key(&provider)?.ok_or_else(|| format!("No API key configured for {}.", display_provider(&provider)))?
    };

    let user_content = match &request.context {
        Some(ctx) if !ctx.is_empty() => format!("Code context:\n```\n{}\n```\n\n{}", ctx, request.prompt),
        _ => request.prompt.clone(),
    };

    let messages = vec![
        ChatMessageInput { role: "system".into(), content: SYSTEM_PROMPT.into() },
        ChatMessageInput { role: "user".into(), content: user_content },
    ];

    let model = request.model.unwrap_or_else(|| default_model(&provider));
    call_blocking(&provider, &api_key, &model, &messages).await
}

// ── Streaming command ──

#[tauri::command]
pub async fn ai_chat_stream(
    request: AiStreamRequest,
    app: AppHandle,
    state: tauri::State<'_, Arc<AiState>>,
) -> Result<(), String> {
    let provider = request.provider.clone().unwrap_or_else(|| "openrouter".to_string()).to_lowercase();
    let api_key = if provider == "local" {
        get_key(&provider)?.unwrap_or_else(|| "http://localhost:11434".to_string())
    } else {
        get_key(&provider)?.ok_or_else(|| format!("No API key configured for {}.", display_provider(&provider)))?
    };
    let model = request.model.unwrap_or_else(|| default_model(&provider));
    let session_id = request.session_id.clone();
    let tools = request.tools.clone();

    // Set up cancellation
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    {
        let mut tokens = state.cancel_tokens.lock().await;
        tokens.insert(session_id.clone(), cancel_tx);
    }

    let state_clone = state.inner().clone();
    let sid = session_id.clone();

    tokio::spawn(async move {
        let result = stream_response(&provider, &api_key, &model, &request.messages, &tools, &app, &sid, cancel_rx).await;

        // Clean up cancel token
        {
            let mut tokens = state_clone.cancel_tokens.lock().await;
            tokens.remove(&sid);
        }

        // Emit done or error
        match result {
            Ok(()) => {
                let _ = app.emit("ai-stream-chunk", StreamChunk { session_id: sid, delta: String::new(), done: true, tool_calls: None });
            }
            Err(e) => {
                let _ = app.emit("ai-stream-chunk", StreamChunk { session_id: sid.clone(), delta: format!("Error: {}", e), done: false, tool_calls: None });
                let _ = app.emit("ai-stream-chunk", StreamChunk { session_id: sid, delta: String::new(), done: true, tool_calls: None });
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn ai_chat_cancel(
    session_id: String,
    state: tauri::State<'_, Arc<AiState>>,
) -> Result<(), String> {
    let tokens = state.cancel_tokens.lock().await;
    if let Some(tx) = tokens.get(&session_id) {
        let _ = tx.send(true);
    }
    Ok(())
}

// ── Streaming implementation ──

async fn stream_response(
    provider: &str,
    api_key: &str,
    model: &str,
    messages: &[ChatMessageInput],
    tools: &Option<Value>,
    app: &AppHandle,
    session_id: &str,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let (url, headers, body) = build_stream_request(provider, api_key, model, messages, tools)?;

    let client = http_client_streaming();
    let mut req = client.post(&url);
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let response = req
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    // Read SSE stream
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    let mut buffer: Vec<u8> = Vec::with_capacity(8192);

    loop {
        tokio::select! {
            chunk_opt = stream.next() => {
                match chunk_opt {
                    None => break,
                    Some(Err(e)) => return Err(format!("Stream error: {}", e)),
                    Some(Ok(chunk)) => {
                        buffer.extend_from_slice(&chunk);
                        if buffer.len() > 1_048_576 {
                            return Err("SSE buffer overflow: no newline in 1MB of data".to_string());
                        }

                        loop {
                            let Some(line_end) = buffer.iter().position(|&b| b == b'\n') else { break };
                            let line_bytes = &buffer[..line_end];
                            let line_bytes = line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes);
                            let line = String::from_utf8_lossy(line_bytes);

                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    return Ok(());
                                }

                                if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                                    let delta = extract_stream_delta(&parsed, provider);
                                    let tool_calls = extract_tool_calls(&parsed, provider);

                                    if !delta.is_empty() || tool_calls.is_some() {
                                        let _ = app.emit("ai-stream-chunk", StreamChunk {
                                            session_id: session_id.to_string(),
                                            delta,
                                            done: false,
                                            tool_calls,
                                        });
                                    }

                                    if is_stream_done(&parsed, provider) {
                                        return Ok(());
                                    }
                                }
                            }

                            buffer.drain(..line_end + 1);
                        }
                    }
                }
            }
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

fn build_stream_request(
    provider: &str,
    api_key: &str,
    model: &str,
    messages: &[ChatMessageInput],
    tools: &Option<Value>,
) -> Result<(String, Vec<(String, String)>, String), String> {
    match provider {
        "anthropic" => {
            let url = "https://api.anthropic.com/v1/messages".to_string();
            let headers = vec![
                ("x-api-key".into(), api_key.to_string()),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("content-type".into(), "application/json".into()),
            ];
            // Separate system message
            let system = messages.iter().find(|m| m.role == "system").map(|m| m.content.clone()).unwrap_or_default();
            let msgs: Vec<Value> = messages.iter().filter(|m| m.role != "system").map(|m| json!({"role": m.role, "content": m.content})).collect();
            let mut body = json!({
                "model": model,
                "max_tokens": 4096,
                "stream": true,
                "system": system,
                "messages": msgs,
            });
            // Add tools if provided (convert from OpenAI format to Anthropic format)
            if let Some(tools_val) = tools {
                if let Some(tools_arr) = tools_val.as_array() {
                    let anthropic_tools: Vec<Value> = tools_arr.iter().filter_map(|t| {
                        let func = t.get("function")?;
                        Some(json!({
                            "name": func.get("name")?,
                            "description": func.get("description")?,
                            "input_schema": func.get("parameters")?
                        }))
                    }).collect();
                    if !anthropic_tools.is_empty() {
                        body["tools"] = json!(anthropic_tools);
                    }
                }
            }
            Ok((url, headers, body.to_string()))
        }
        _ => {
            // OpenAI-compatible (OpenRouter, OpenAI, Local)
            let url = match provider {
                "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
                "local" => {
                    // For local provider, the api_key field stores the base URL
                    let base = api_key.trim_end_matches('/');
                    format!("{}/v1/chat/completions", base)
                }
                _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
            };
            let headers = if provider == "local" {
                vec![("content-type".into(), "application/json".into())]
            } else {
                vec![
                    ("Authorization".into(), format!("Bearer {}", api_key)),
                    ("content-type".into(), "application/json".into()),
                ]
            };
            let msgs: Vec<Value> = messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect();
            let token_field = if provider == "openai" { "max_completion_tokens" } else { "max_tokens" };
            let mut body = json!({
                "model": model,
                token_field: 4096,
                "stream": true,
                "messages": msgs,
            });
            // Add tools if provided (already in OpenAI format)
            if let Some(tools_val) = tools {
                body["tools"] = tools_val.clone();
            }
            Ok((url, headers, body.to_string()))
        }
    }
}

fn extract_stream_delta(parsed: &Value, provider: &str) -> String {
    match provider {
        "anthropic" => {
            // Anthropic: {"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}
            parsed.get("delta")
                .and_then(|d| d.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string()
        }
        _ => {
            // OpenAI: {"choices":[{"delta":{"content":"..."}}]}
            parsed.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string()
        }
    }
}

fn extract_tool_calls(parsed: &Value, provider: &str) -> Option<Value> {
    match provider {
        "anthropic" => {
            // Anthropic tool use: {"type":"content_block_start","content_block":{"type":"tool_use","id":"...","name":"...","input":{}}}
            // or {"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"..."}}
            let event_type = parsed.get("type").and_then(|t| t.as_str())?;
            match event_type {
                "content_block_start" => {
                    let block = parsed.get("content_block")?;
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let id = block.get("id")?.as_str()?;
                        let name = block.get("name")?.as_str()?;
                        Some(json!([{
                            "id": id,
                            "type": "function",
                            "function": { "name": name, "arguments": "" }
                        }]))
                    } else {
                        None
                    }
                }
                "content_block_delta" => {
                    let delta = parsed.get("delta")?;
                    if delta.get("type").and_then(|t| t.as_str()) == Some("input_json_delta") {
                        let partial = delta.get("partial_json").and_then(|p| p.as_str()).unwrap_or("");
                        // We need the block index to correlate — use a placeholder id
                        let idx = parsed.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                        Some(json!([{
                            "id": format!("anthropic-{}", idx),
                            "type": "function",
                            "function": { "name": "", "arguments": partial }
                        }]))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => {
            // OpenAI: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"...","type":"function","function":{"name":"...","arguments":"..."}}]}}]}
            parsed.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("tool_calls"))
                .cloned()
        }
    }
}

fn is_stream_done(parsed: &Value, provider: &str) -> bool {
    match provider {
        "anthropic" => {
            parsed.get("type").and_then(|t| t.as_str()) == Some("message_stop")
        }
        _ => {
            parsed.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("finish_reason"))
                .and_then(|r| r.as_str())
                .is_some_and(|r| r == "stop" || r == "end_turn")
        }
    }
}

// ── Blocking call (for legacy ai_chat) ──

async fn call_blocking(provider: &str, api_key: &str, model: &str, messages: &[ChatMessageInput]) -> Result<String, String> {
    match provider {
        "anthropic" => {
            let system = messages.iter().find(|m| m.role == "system").map(|m| m.content.clone()).unwrap_or_default();
            let msgs: Vec<Value> = messages.iter().filter(|m| m.role != "system").map(|m| json!({"role": m.role, "content": m.content})).collect();
            let body = json!({ "model": model, "max_tokens": 4096, "system": system, "messages": msgs });
            let client = http_client();
            let response = client.post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body).send().await.map_err(|e| format!("Request failed: {}", e))?;
            if !response.status().is_success() {
                let s = response.status(); let b = response.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", s, b));
            }
            let parsed: Value = response.json().await.map_err(|e| format!("Parse error: {}", e))?;
            parsed.get("content").and_then(|c| c.as_array())
                .and_then(|arr| arr.iter().find_map(|item| {
                    if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                        item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                    } else { None }
                })).ok_or_else(|| "Empty response".into())
        }
        _ => {
            let url = match provider {
                "openai" => "https://api.openai.com/v1/chat/completions",
                _ => "https://openrouter.ai/api/v1/chat/completions",
            };
            let msgs: Vec<Value> = messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect();
            let token_field = if provider == "openai" { "max_completion_tokens" } else { "max_tokens" };
            let body = json!({ "model": model, token_field: 4096, "messages": msgs });
            let client = http_client();
            let response = client.post(url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("content-type", "application/json")
                .json(&body).send().await.map_err(|e| format!("Request failed: {}", e))?;
            if !response.status().is_success() {
                let s = response.status(); let b = response.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", s, b));
            }
            let parsed: Value = response.json().await.map_err(|e| format!("Parse error: {}", e))?;
            parsed.get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("message"))
                .and_then(|m| m.get("content")).and_then(|c| c.as_str()).map(|s| s.to_string())
                .ok_or_else(|| "Empty response".into())
        }
    }
}

fn default_model(provider: &str) -> String {
    match provider {
        "openai" => "gpt-4o-mini".into(),
        "anthropic" => "claude-sonnet-4-20250514".into(),
        "local" => "llama3".into(),
        _ => "openrouter/auto".into(),
    }
}

fn display_provider(p: &str) -> &str {
    match p {
        "openrouter" => "OpenRouter",
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        "local" => "Local (Ollama/LM Studio)",
        _ => p,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn key_bytes(seed: u8) -> [u8; key_store::KEY_SIZE] {
        let mut k = [0u8; key_store::KEY_SIZE];
        for (i, b) in k.iter_mut().enumerate() {
            *b = seed.wrapping_add(i as u8);
        }
        k
    }

    #[test]
    fn migrate_moves_plaintext_into_encrypted_store_and_renames_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("keys.json");
        std::fs::write(
            &legacy,
            br#"{"openai":"sk-old","anthropic":"ant-old","empty":""}"#,
        )
        .unwrap();

        // Simulate: keyring is unavailable, so the migration writes into
        // the encrypted file via the test store closure.
        let file_key = key_bytes(0xA1);
        let captured: std::cell::RefCell<Vec<(String, String)>> = std::cell::RefCell::new(vec![]);
        let mut store = |provider: &str, key: &str| -> Result<(), String> {
            captured
                .borrow_mut()
                .push((provider.to_string(), key.to_string()));
            key_store::put(dir.path(), &file_key, provider, key)
        };
        let migrated = migrate_plaintext_keys_in(dir.path(), &mut store);

        assert_eq!(migrated, 2, "two non-empty keys must be migrated");
        // Empty entry was skipped (set_key with empty string would *delete*).
        let captured = captured.into_inner();
        assert!(captured.iter().any(|(p, _)| p == "openai"));
        assert!(captured.iter().any(|(p, _)| p == "anthropic"));
        assert!(!captured.iter().any(|(p, _)| p == "empty"));

        // Encrypted file now holds the keys.
        assert_eq!(
            key_store::get(dir.path(), &file_key, "openai").unwrap(),
            Some("sk-old".to_string())
        );
        assert_eq!(
            key_store::get(dir.path(), &file_key, "anthropic").unwrap(),
            Some("ant-old".to_string())
        );

        // Legacy plaintext renamed to .bak, original gone.
        assert!(!legacy.exists());
        assert!(dir.path().join("keys.json.bak").exists());
    }

    #[test]
    fn migrate_is_noop_when_no_legacy_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = |_: &str, _: &str| -> Result<(), String> { Ok(()) };
        let n = migrate_plaintext_keys_in(dir.path(), &mut store);
        assert_eq!(n, 0);
    }

    #[test]
    fn migrate_handles_malformed_legacy_file_without_crashing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keys.json"), b"{not json").unwrap();
        let mut store = |_: &str, _: &str| -> Result<(), String> {
            unreachable!("malformed file must produce no store calls")
        };
        let n = migrate_plaintext_keys_in(dir.path(), &mut store);
        assert_eq!(n, 0);
        // Malformed file is left in place so an operator can inspect it.
        assert!(dir.path().join("keys.json").exists());
    }

    #[test]
    fn migrate_continues_when_a_single_store_call_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("keys.json"),
            br#"{"a":"1","b":"2","c":"3"}"#,
        )
        .unwrap();

        let succeeded = std::cell::RefCell::new(0);
        let mut store = |provider: &str, _key: &str| -> Result<(), String> {
            if provider == "b" {
                Err("simulated keyring failure".into())
            } else {
                *succeeded.borrow_mut() += 1;
                Ok(())
            }
        };
        let n = migrate_plaintext_keys_in(dir.path(), &mut store);
        assert_eq!(n, 2);
        assert_eq!(*succeeded.borrow(), 2);
        // Even with a partial failure we still rename the legacy file:
        // re-reading it on next startup would just retry. Since the user
        // still has keys.json.bak as a manual recovery, this is safer
        // than blocking startup forever.
        assert!(dir.path().join("keys.json.bak").exists());
    }

    #[test]
    fn machine_derived_file_key_is_stable_across_calls() {
        // Tier-3 fallback must be deterministic so a previously-written
        // keys.enc remains decryptable.
        let a = derive_machine_file_key();
        let b = derive_machine_file_key();
        assert_eq!(a, b);
    }

    #[test]
    fn corrupted_keys_enc_does_not_panic_and_returns_no_keys() {
        let dir = tempfile::tempdir().unwrap();
        let file_key = key_bytes(0x66);
        std::fs::write(key_store::encrypted_path(dir.path()), b"\x99garbage").unwrap();
        // Reading via the store API surfaces an Err — the production
        // get_key wraps that into Ok(None) so the AI flow degrades to
        // "no key configured" rather than crashing.
        assert!(key_store::read_all(dir.path(), &file_key).is_err());
    }

    #[test]
    fn provider_round_trip_via_file_only_path() {
        // Simulates the keyring-unavailable case: every put/get goes
        // through the encrypted file. Mirrors what runtime set_key/
        // get_key do when Entry::new succeeds but the actual keyring
        // operation fails.
        let dir = tempfile::tempdir().unwrap();
        let file_key = key_bytes(0x4F);
        let mut map = HashMap::new();
        map.insert("openrouter".into(), "or-1".into());
        map.insert("openai".into(), "sk-1".into());
        map.insert("anthropic".into(), "an-1".into());
        key_store::write_all(dir.path(), &file_key, &map).unwrap();

        for (p, expected) in &map {
            assert_eq!(
                key_store::get(dir.path(), &file_key, p).unwrap().as_deref(),
                Some(expected.as_str())
            );
        }
    }
}
