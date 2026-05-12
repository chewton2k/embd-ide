use keyring::Entry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SERVICE_NAME: &str = "leo-ide";

fn keyring_entry(provider: &str) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, provider).map_err(|e| format!("Keyring error: {}", e))
}

fn get_key(provider: &str) -> Result<Option<String>, String> {
    let entry = keyring_entry(provider)?;
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to read key from keychain: {}", e)),
    }
}

fn set_key(provider: &str, key: &str) -> Result<(), String> {
    let entry = keyring_entry(provider)?;
    if key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Failed to delete key from keychain: {}", e)),
        }
    } else {
        entry
            .set_password(key)
            .map_err(|e| format!("Failed to store key in keychain: {}", e))
    }
}

/// Legacy single-key command — maps to the OpenRouter provider.
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

#[derive(Deserialize)]
pub struct AiRequest {
    pub prompt: String,
    pub context: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
}

const SYSTEM_PROMPT: &str = "You are an AI coding assistant embedded in a lightweight IDE called leo. \
    Help the user with their code: explain, debug, refactor, or write new code. \
    Keep responses concise and code-focused.";

#[tauri::command]
pub async fn ai_chat(request: AiRequest) -> Result<String, String> {
    let provider = request
        .provider
        .clone()
        .unwrap_or_else(|| "openrouter".to_string())
        .to_lowercase();

    let api_key = get_key(&provider)?.ok_or_else(|| {
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
            let model = request.model.unwrap_or_else(|| "openrouter/auto".to_string());
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
            let model = request.model.unwrap_or_else(|| "gpt-5-mini".to_string());
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
            let model = request.model.unwrap_or_else(|| "claude-sonnet-4-6".to_string());
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
            OAMessage { role: "system", content: SYSTEM_PROMPT },
            OAMessage { role: "user", content: user_content },
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
    let response = req.json(&body).send().await.map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, body));
    }

    let parsed: Value = response.json().await.map_err(|e| format!("Failed to parse response: {}", e))?;

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
        "messages": [{ "role": "user", "content": user_content }]
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

    let parsed: Value = response.json().await.map_err(|e| format!("Failed to parse response: {}", e))?;

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
