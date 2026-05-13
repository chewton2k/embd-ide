use keyring::Entry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

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

// ── Key storage: keyring with file fallback ──
// On macOS in dev mode, keyring can silently fail due to unsigned binaries.
// We use a file fallback at ~/.leo-ide/keys.json as a reliable alternative.

fn keys_file_path() -> std::path::PathBuf {
    let dir = dirs::home_dir().unwrap_or_default().join(".leo-ide");
    std::fs::create_dir_all(&dir).ok();
    dir.join("keys.json")
}

fn read_keys_file() -> std::collections::HashMap<String, String> {
    let path = keys_file_path();
    if !path.exists() { return std::collections::HashMap::new(); }
    let Ok(bytes) = std::fs::read(&path) else { return std::collections::HashMap::new(); };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn write_keys_file(map: &std::collections::HashMap<String, String>) {
    let path = keys_file_path();
    if let Ok(bytes) = serde_json::to_vec_pretty(map) {
        if std::fs::write(&path, bytes).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                std::fs::set_permissions(&path, perms).ok();
            }
        }
    }
}

fn get_key(provider: &str) -> Result<Option<String>, String> {
    // Try keyring first
    if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
        if let Ok(pw) = entry.get_password() {
            if !pw.is_empty() { return Ok(Some(pw)); }
        }
    }
    // Fallback to file
    let map = read_keys_file();
    Ok(map.get(provider).cloned())
}

fn set_key(provider: &str, key: &str) -> Result<(), String> {
    // Write to both keyring and file for reliability
    if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
        if key.is_empty() {
            entry.delete_credential().ok();
        } else {
            entry.set_password(key).ok();
        }
    }
    // Always write to file as fallback
    let mut map = read_keys_file();
    if key.is_empty() {
        map.remove(provider);
    } else {
        map.insert(provider.to_string(), key.to_string());
    }
    write_keys_file(&map);
    Ok(())
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
}

#[derive(Serialize, Clone)]
pub struct StreamChunk {
    pub session_id: String,
    pub delta: String,
    pub done: bool,
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
    let api_key = get_key(&provider)?.ok_or_else(|| format!("No API key configured for {}.", display_provider(&provider)))?;

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
    let api_key = get_key(&provider)?.ok_or_else(|| format!("No API key configured for {}.", display_provider(&provider)))?;
    let model = request.model.unwrap_or_else(|| default_model(&provider));
    let session_id = request.session_id.clone();

    // Set up cancellation
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    {
        let mut tokens = state.cancel_tokens.lock().await;
        tokens.insert(session_id.clone(), cancel_tx);
    }

    let state_clone = state.inner().clone();
    let sid = session_id.clone();

    tokio::spawn(async move {
        let result = stream_response(&provider, &api_key, &model, &request.messages, &app, &sid, cancel_rx).await;

        // Clean up cancel token
        {
            let mut tokens = state_clone.cancel_tokens.lock().await;
            tokens.remove(&sid);
        }

        // Emit done or error
        match result {
            Ok(()) => {
                let _ = app.emit("ai-stream-chunk", StreamChunk { session_id: sid, delta: String::new(), done: true });
            }
            Err(e) => {
                let _ = app.emit("ai-stream-chunk", StreamChunk { session_id: sid.clone(), delta: format!("Error: {}", e), done: false });
                let _ = app.emit("ai-stream-chunk", StreamChunk { session_id: sid, delta: String::new(), done: true });
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
    app: &AppHandle,
    session_id: &str,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let (url, headers, body) = build_stream_request(provider, api_key, model, messages)?;

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
                                    if !delta.is_empty() {
                                        let _ = app.emit("ai-stream-chunk", StreamChunk {
                                            session_id: session_id.to_string(),
                                            delta,
                                            done: false,
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
            let body = json!({
                "model": model,
                "max_tokens": 4096,
                "stream": true,
                "system": system,
                "messages": msgs,
            });
            Ok((url, headers, body.to_string()))
        }
        _ => {
            // OpenAI-compatible (OpenRouter, OpenAI)
            let url = match provider {
                "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
                _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
            };
            let headers = vec![
                ("Authorization".into(), format!("Bearer {}", api_key)),
                ("content-type".into(), "application/json".into()),
            ];
            let msgs: Vec<Value> = messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect();
            let token_field = if provider == "openai" { "max_completion_tokens" } else { "max_tokens" };
            let body = json!({
                "model": model,
                token_field: 4096,
                "stream": true,
                "messages": msgs,
            });
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
        _ => "openrouter/auto".into(),
    }
}

fn display_provider(p: &str) -> &str {
    match p {
        "openrouter" => "OpenRouter",
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        _ => p,
    }
}
