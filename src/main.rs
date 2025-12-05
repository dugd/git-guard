use serde::Deserialize;
use serde_json::json;
use dotenvy::dotenv;

use std::{env, process::Command};

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<Choice>
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

/// Get staged diff using the git command
fn get_staged_diff() -> Result<String, String> {
    // System call
    let output = Command::new("git")
        .arg("diff")
        .arg("--cached")
        .output()
        .map_err(|e| format!("Failed to execute git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("Git error: {}", stderr));
    }

    let diff = String::from_utf8_lossy(&output.stdout).to_string();

    if diff.trim().is_empty() {
        return Err("No staged changes found. Did you forget 'git add'?".to_string())
    }

    Ok(diff)
}

/// Ask chat gpt for code review
async fn ask_gpt(diff: &str, api_key: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let system_prompt = "You are a Toxic Senior Software Engineer. \
        You are reviewing a Junior's code via git diff. \
        Your goal is to find bugs, bad practices, and security issues. \
        Be cynical, sarcastic, and brief. Use technical slang. \
        If the code is good, admit it reluctantly. \
        Output in Markdown format.";
    
    let body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": format!("Review this staged git diff:\n\n{}", diff)}
        ]
    });

    let res = client.post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("API Error: {}", res.status()));
    }

    let response_data: OpenAiResponse = res.json()
        .await
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    response_data.choices.first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| "No content in response".to_string())
}


#[tokio::main]
async fn main() {
    dotenv().ok();

    // Used
    let api_key = env::var("OPENAI_API_KEY").expect("'OPENAI_API_KEY' is missing");

    println!("=== Git Sensei (Demo) ===");

    let diff = match get_staged_diff() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Git Error: {}", e);
            return;
        }
    };

    println!("Diff found ({} chars). Asking the Senior...", diff.len());

    match ask_gpt(&diff, &api_key).await {
        Ok(review) => {
            println!("\n--- CODE REVIEW ---\n");
            println!("{}", review);
            println!("\n-------------------");
        }
        Err(e) => eprintln!("AI Error: {}", e),
    }
}
