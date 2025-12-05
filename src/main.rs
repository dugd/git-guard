mod llm;

use std::{env, process::Command, time::Duration};

use indicatif::{ProgressBar, ProgressStyle};
use dotenvy::dotenv;
use termimad::{MadSkin, crossterm::style::Color};

use llm::{FileContext, review_changes};

// ignore
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

// ignore
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

    println!("=== Git Sensei (MVP 0.1) ===");

    // TODO: implement git.rs module
    let mock_files = vec![
        FileContext {
            path: "src/auth.rs".to_string(),
            diff: "+ let telegram_bot_api = \"1234567890:ABCDEFGHIJKLMNOPQRSTUVWXYZ123456789\";".to_string(),
            full_content: None,
        },
    ];

    println!("Analyzing {} files...", mock_files.len());

    // Init custom spinner
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        .template("{spinner:.green} Judging your code...").unwrap());

    pb.enable_steady_tick(Duration::from_millis(100));

    // Wait for senior code review
    let result = review_changes(mock_files, &api_key).await;

    // Disable spinner
    pb.finish_and_clear();

    match result {
        Ok(analysis) => {
            println!("\n--- CODE REVIEW ---\n");

            // general info
            println!("Verdict: {}", analysis.verdict);
            println!("Score: {}/10", analysis.score);

            // issues
            if !analysis.issues.is_empty() {
                println!("\nIssues Found:");
                for issue in analysis.issues {
                    println!("[{:?}] {}: {}", issue.severity, issue.file, issue.description);
                }
            } else {
                println!("Surprisingly decent.");
            }

            println!("\n-------------------");
        }
        Err(e) => eprintln!("Fatal Error: {:?}", e),
    }
}
