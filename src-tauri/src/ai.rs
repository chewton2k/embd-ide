use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub type ApiKeyState = Arc<Mutex<Option<String>>>;

pub fn create_api_key_state() -> ApiKeyState {
    Arc::new(Mutex::new(None))
}

#[tauri::command]
pub fn set_api_key(state: tauri::State<'_, ApiKeyState>, key: String) -> Result<(), String> {
    let mut stored = state.lock().map_err(|e| e.to_string())?;
    if key.is_empty() {
        *stored = None;
    } else {
        *stored = Some(key);
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct AiRequest {
    pub prompt: String,
    pub context: Option<String>,
    pub api_key: Option<String>, // kept for backwards compat, prefer stored key
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
    system: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[tauri::command]
pub async fn ai_chat(state: tauri::State<'_, ApiKeyState>, request: AiRequest) -> Result<String, String> {
    // Prefer stored key, fall back to request key for backwards compat
    let api_key = {
        let stored = state.lock().map_err(|e| e.to_string())?;
        stored.clone()
    };
    let api_key = api_key
        .or(request.api_key)
        .ok_or_else(|| "No API key configured. Set it in Settings.".to_string())?;

    if api_key.is_empty() {
        return Err("No API key configured. Set it in Settings.".to_string());
    }

    let system_prompt = "You are an AI coding assistant embedded in a lightweight IDE called embd. \
        Help the user with their code: explain, debug, refactor, or write new code. \
        Keep responses concise and code-focused.";

    let mut user_content = request.prompt;
    if let Some(ctx) = request.context {
        user_content = format!("Code context:\n```\n{}\n```\n\n{}", ctx, user_content);
    }

    let body = ClaudeRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 4096,
        messages: vec![ClaudeMessage {
            role: "user".to_string(),
            content: user_content,
        }],
        system: Some(system_prompt.to_string()),
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("API error: {}", status));
    }

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    claude_response
        .content
        .first()
        .and_then(|block| block.text.clone())
        .ok_or_else(|| "Empty response from AI".to_string())
}
