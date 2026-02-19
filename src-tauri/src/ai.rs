use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct AiRequest {
    pub prompt: String,
    pub context: Option<String>,
    pub api_key: String,
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
pub async fn ai_chat(request: AiRequest) -> Result<String, String> {
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
        .header("x-api-key", &request.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, error_text));
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
