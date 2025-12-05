use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result, anyhow};
use serde_json::json;

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

pub async fn review_changes(files: Vec<FileContext>, api_key: &str) -> Result<ReviewAnalysis> {
    let client = Client::new();

    let system_prompt = r#"
    You are 'Git Sensei', a Toxic Senior Software Engineer.
    Your goal is to review code commits and BLOCK them if they are garbage.
    
    Analyze the provided code changes for:
    1. Logic errors (Critical)
    2. Security risks (Critical)
    3. Bad patterns / Spaghetti code (Warning)
    4. Naming conventions (Nitpick)

    BE HARSH. Do not be polite.
    
    You MUST respond with valid JSON matching this structure:
    {
        "verdict": "string (short in 1-2 words, cynical summary)",
        "score": number (0-10),
        "issues": [
            {
                "file": "string",
                "line": numberOrNull,
                "severity": "critical" | "warning" | "nitpick",
                "description": "string"
            }
        ]
    }
    "#;

    let user_content = json!({
        "files_to_review": files,
    }).to_string();

    let body = json!({
        "model": "gpt-4o-mini",
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