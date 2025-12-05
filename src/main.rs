mod llm;
mod git;

use std::{env, process, time::Duration};

use indicatif::{ProgressBar, ProgressStyle};
use dotenvy::dotenv;

use crate::llm::{review_changes, Severity};
use crate::git::get_staged_files;

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
            println!("VERDICT: {}", analysis.verdict);
            println!("SCORE:   {}/10", analysis.score);
            println!("----------------------------------------");

            let mut critical_errors = 0;

            if !analysis.issues.is_empty() {
                for issue in analysis.issues {
                    let level = match issue.severity {
                        Severity::Critical => {
                            critical_errors += 1;
                            "[CRITICAL]"
                        },
                        Severity::Warning => "[WARNING]",
                        Severity::Nitpick => "[NITPICK]",
                    };
                    
                    println!("{} {}: {}", level, issue.file, issue.description);
                    if let Some(line) = issue.line {
                         println!("           Line: {}", line);
                    }
                }
            } else {
                println!("No issues found.");
            }
            println!("----------------------------------------");

            if critical_errors > 0 {
                eprintln!("FAILURE: {} critical issues blocked commit.", critical_errors);
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("FATAL: Analysis failed: {:?}", e);
            process::exit(1);
        }
    }
}
