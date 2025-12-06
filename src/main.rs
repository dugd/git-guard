mod llm;
mod git;
mod utils;
mod config;

use std::{env, process, time::Duration};

use glob::Pattern;
use indicatif::{ProgressBar, ProgressStyle};
use dotenvy::dotenv;

use crate::config::{Config, ContextConfig};
use crate::llm::{review_changes, Severity};
use crate::git::get_staged_files;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let api_key = env::var("OPENAI_API_KEY").expect("'OPENAI_API_KEY' is missing");
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Failed to load config, using defaults: {}", e);
        Config { 
            model: "gpt-4o-mini".to_string(), 
            toxicity: "High".to_string(),
            ignore: vec![],
        }
    });
    let context_config = ContextConfig::default();

    let ignore_patterns: Vec<Pattern> = config.ignore.iter()
        .filter_map(|p| {
            match Pattern::new(p) {
                Ok(pat) => Some(pat),
                Err(e) => {
                    eprintln!("Invalid ignore pattern '{}': {}", p, e);
                    None
                }
            }
        })
        .collect();

    println!("=== Git Sensei (MVP 0.1) ===");

    let staged_files = match get_staged_files(&context_config) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Git Error: {}", e);
            process::exit(1);
        }
    };

    let files_to_review: Vec<_> = staged_files.into_iter()
        .filter(|f| {
            for pattern in &ignore_patterns {
                if pattern.matches(&f.path) {
                    println!("Ignoring {} (matched {})", f.path, pattern);
                    return false;
                }
            }
            true
        })
        .collect();

    if files_to_review.is_empty() {
        println!("Nothing to review.");
        process::exit(0);
    }

    println!("Found {} changed file(s). Preparing context...", files_to_review.len());

    // Init custom spinner
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        .template("{spinner:.green} Judging your code...").unwrap());

    pb.enable_steady_tick(Duration::from_millis(100));

    // Wait for senior code review
    let result = review_changes(files_to_review, &api_key, &config).await;

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
