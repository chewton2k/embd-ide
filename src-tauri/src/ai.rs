use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type ProviderKeysState = Arc<Mutex<HashMap<String, String>>>;

pub fn create_provider_keys_state() -> ProviderKeysState {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Legacy single-key command — maps to the OpenRouter provider so older
/// frontend code that calls `set_api_key` keeps working.
#[tauri::command]
pub fn set_api_key(state: tauri::State<'_, ProviderKeysState>, key: String) -> Result<(), String> {
    set_provider_key(state, "openrouter".to_string(), key)
}

#[tauri::command]
pub fn set_provider_key(
    state: tauri::State<'_, ProviderKeysState>,
    provider: String,
    key: String,
) -> Result<(), String> {
    let mut keys = state.lock().map_err(|e| e.to_string())?;
    let provider = provider.to_lowercase();
    if key.is_empty() {
        keys.remove(&provider);
    } else {
        keys.insert(provider, key);
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct AiRequest {
    pub prompt: String,
    pub context: Option<String>,
    pub model: Option<String>,
    /// "openrouter" (default), "openai", or "anthropic".
    pub provider: Option<String>,
}

const SYSTEM_PROMPT: &str = "You are an AI coding assistant embedded in a lightweight IDE called embd. \
    Help the user with their code: explain, debug, refactor, or write new code. \
    Keep responses concise and code-focused.";

#[tauri::command]
pub async fn ai_chat(
    state: tauri::State<'_, ProviderKeysState>,
    request: AiRequest,
) -> Result<String, String> {
    let provider = request
        .provider
        .clone()
        .unwrap_or_else(|| "openrouter".to_string())
        .to_lowercase();

    let api_key = {
        let keys = state.lock().map_err(|e| e.to_string())?;
        keys.get(&provider).cloned()
    };
    let api_key = api_key.ok_or_else(|| {
        format!(
            "No API key configured for {}. Set it in Settings → Models.",
            display_provider(&provider)
        )
    })?;

    let user_content = match &request.context {
        Some(ctx) if !ctx.is_empty() => {
            format!("Code context:\n```\n{}\n```\n\n{}", ctx, request.prompt)
        }
        _ => request.prompt.clone(),
    };

    match provider.as_str() {
        "openrouter" => {
            let model = request
                .model
                .unwrap_or_else(|| "openrouter/auto".to_string());
            call_openai_compatible(
                "https://openrouter.ai/api/v1/chat/completions",
                &api_key,
                &model,
                &user_content,
                None,
            )
            .await
        }
        "openai" => {
            let model = request.model.unwrap_or_else(|| "gpt-4o-mini".to_string());
            call_openai_compatible(
                "https://api.openai.com/v1/chat/completions",
                &api_key,
                &model,
                &user_content,
                None,
            )
            .await
        }
        "anthropic" => {
            let model = request
                .model
                .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());
            call_anthropic(&api_key, &model, &user_content).await
        }
        other => Err(format!("Unknown provider: {}", other)),
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

#[derive(Serialize)]
struct OAMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct OARequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<OAMessage<'a>>,
}

async fn call_openai_compatible(
    url: &str,
    api_key: &str,
    model: &str,
    user_content: &str,
    extra_header: Option<(&str, &str)>,
) -> Result<String, String> {
    let body = OARequest {
        model,
        max_tokens: 4096,
        messages: vec![
            OAMessage {
                role: "system",
                content: SYSTEM_PROMPT,
            },
            OAMessage {
                role: "user",
                content: user_content,
            },
        ],
    };

    let client = reqwest::Client::new();
    let mut req = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("content-type", "application/json");
    if let Some((k, v)) = extra_header {
        req = req.header(k, v);
    }
    let response = req
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, body));
    }

    let parsed: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    parsed
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Empty response from AI".to_string())
}

async fn call_anthropic(api_key: &str, model: &str, user_content: &str) -> Result<String, String> {
    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": SYSTEM_PROMPT,
        "messages": [
            { "role": "user", "content": user_content }
        ]
    });

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
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

    let parsed: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    parsed
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.iter().find_map(|item| {
            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
            } else {
                None
            }
        }))
        .ok_or_else(|| "Empty response from AI".to_string())
}
