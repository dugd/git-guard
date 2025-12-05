use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use serde_json::json;
use dotenvy::dotenv;
use termimad::{MadSkin, crossterm::style::Color};

use std::{env, process::Command, time::Duration};

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

    let system_prompt = "You are a cynical Senior Software Engineer. \
        Review the following git diff. Focus ONLY on logic errors, security risks, and bad patterns. \
        Do NOT use filler phrases like 'Let's dive in' or 'In conclusion'. \
        Do NOT be polite. Be harsh, direct, and technical. \
        Use Markdown. Format code blocks with language hints.";
    
    let body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": diff}
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

fn print_markdown(text: &str) {
    let mut skin = MadSkin::default();
    skin.bold.set_fg(Color::Red);
    skin.italic.set_fg(Color::Yellow);
    skin.set_headers_fg(Color::Magenta);
    skin.print_text(text);
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

    println!("Diff found ({} chars).", diff.len());

    // Init custom spinner
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        .template("{spinner:.green} Judging your code...").unwrap());
    pb.enable_steady_tick(Duration::from_millis(100));

    // Wait for senior code review
    let result = ask_gpt(&diff, &api_key).await;

    // Disable spinner
    pb.finish_and_clear();

    match result {
        Ok(review) => {
            println!("\n--- CODE REVIEW ---\n");
            print_markdown(&review);
            println!("\n-------------------");
        }
        Err(e) => eprintln!("AI Error: {}", e),
    }
}
