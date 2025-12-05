mod llm;
mod git;

use std::{env, process, time::Duration};

use indicatif::{ProgressBar, ProgressStyle};
use dotenvy::dotenv;

use llm::review_changes;
use git::get_staged_files;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let api_key = env::var("OPENAI_API_KEY").expect("'OPENAI_API_KEY' is missing");

    println!("=== Git Sensei (MVP 0.1) ===");

    let staged_files = match get_staged_files() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Git Error: {}", e);
            process::exit(1);
        }
    };

    if staged_files.is_empty() {
        println!("Staging area is empty. Go write some code.");
        process::exit(0);
    }

    println!("Found {} changed file(s). Preparing context...", staged_files.len());

    // Init custom spinner
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        .template("{spinner:.green} Judging your code...").unwrap());

    pb.enable_steady_tick(Duration::from_millis(100));

    // Wait for senior code review
    let result = review_changes(staged_files, &api_key).await;

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
