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
    pub model: Option<String>,
}

#[derive(Serialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<OpenRouterMessage>,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterChoiceMessage,
}

#[derive(Deserialize)]
struct OpenRouterChoiceMessage {
    content: Option<String>,
}

#[tauri::command]
pub async fn ai_chat(state: tauri::State<'_, ApiKeyState>, request: AiRequest) -> Result<String, String> {
    let api_key = {
        let stored = state.lock().map_err(|e| e.to_string())?;
        stored.clone()
    };
    let api_key = api_key
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

    let model = request.model
        .unwrap_or_else(|| "openrouter/free".to_string());

    let body = OpenRouterRequest {
        model,
        max_tokens: 4096,
        messages: vec![
            OpenRouterMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            OpenRouterMessage {
                role: "user".to_string(),
                content: user_content,
            },
        ],
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, body));
    }

    let or_response: OpenRouterResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    or_response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| "Empty response from AI".to_string())
}
