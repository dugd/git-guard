use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result, anyhow};
use serde_json::json;

use crate::config::Config;

// API contracts

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

// Response contracts

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,   // block
    Warning,    // shame
    Nitpick,    // annoy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub file: String,
    pub line: Option<usize>,
    pub severity: Severity,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewAnalysis {
    pub verdict: String, // "Total garbage", "Meh", "God tier", etc.
    pub score: u8, // 0-10
    pub issues: Vec<Issue>,
}

// Request contracts

#[derive(Debug, Serialize)]
pub struct FileContext {
    pub path: String,
    pub diff: String,
    pub full_content: Option<String>
}

// Logic

pub async fn review_changes(files: Vec<FileContext>, api_key: &str, config: &Config) -> Result<ReviewAnalysis> {
    let client = Client::new();

    let persona_adjective = match config.toxicity.as_str() {
        "Low" => "strict but polite",
        "Medium" => "cynical", // By default
        _ => "extremely toxic and ruthless",
    };

    let system_prompt = format!(r#"
    You are 'Git Sensei', a {persona_adjective} Principal Software Engineer.
    Your goal is to review code commits before they are pushed.
    
    PERSONA RULES:
    1. Be cynical and ironic, but technically precise. 
    2. Do NOT be mean just for the sake of it. If code is good, admit it (grudgingly).
    3. You prefer "Boring Code" over "Clever Code".
    
    TECHNICAL STANDARDS (RUST):
    - "Critical": Logic bugs, security leaks, panics (unwrap/expect on runtime data), blocking I/O in async.
    - "Warning": Complex logic, bad naming, unoptimized clones, weird architecture.
    - "Nitpick": Formatting, minor consistency.
    
    IMPORTANT:
    - Do NOT complain about standard idioms (e.g. `continue` in loops, `match` statements).
    - Do NOT complain about error handling if `?` or `match` is used correctly.
    - Use line numbers from the provided context.
    
    You MUST respond with valid JSON:
    {{
        "verdict": "string (short, witty summary)",
        "score": number (0-10),
        "issues": [
            {{
                "file": "string",
                "line": number,
                "severity": "critical" | "warning" | "nitpick",
                "description": "string (short & punchy)"
            }}
        ]
    }}
    "#);

    let user_content = json!({
        "files_to_review": files,
    }).to_string();

    let body = json!({
        "model": config.model,
        "response_format": { "type": "json_object"},
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.2 // Consistent JSON
    });

    let res = client.post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .context("Failed to send request to OpenAI")?;

    if !res.status().is_success() {
        return Err(anyhow!("OpenAI API error: {}", res.status()));
    }

    let response_data: OpenAiResponse = res.json()
        .await
        .context("Failed to parse OpenAI generic response")?;

    let content = response_data.choices.first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("API returned empty choices"))?;

    let analysis: ReviewAnalysis = serde_json::from_str(&content)
        .context(format!("Failed to deserialize AI JSON: {}", content))?;

    Ok(analysis)
}