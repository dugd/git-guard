use anyhow::{Result, Context, bail};
use glob::Pattern;
use indicatif::{ProgressBar, ProgressStyle};
use std::{env, time::Duration};

mod config;
mod git;
mod llm;
mod utils;

pub use config::{Config, ContextConfig};

/// execution function
pub async fn run() -> Result<()> {
    println!("=== Git Sensei (MVP 0.1) ===");
    
    let config = load_config()?;
    let context_config = ContextConfig::default();
    let api_key = get_api_key()?;
    
    let ignore_patterns = prepare_ignore_patterns(&config.ignore)?;
    
    // get files to review
    let files_to_review = get_files_to_review(&context_config, &ignore_patterns)?;
    
    if files_to_review.is_empty() {
        println!("Nothing to review.");
        return Ok(());
    }
    
    println!("Found {} changed file(s). Preparing context...", files_to_review.len());
    
    // AI analyze
    let analysis = analyze_changes(files_to_review, &api_key, &config).await?;
    
    // display results
    print_analysis_results(&analysis)?;
    
    check_critical_errors(&analysis)?;
    
    Ok(())
}

/// load configs with exception handling
fn load_config() -> Result<Config> {
    Config::load().or_else(|e| {
        eprintln!("Warning: Failed to load config, using defaults: {}", e);
        Ok(Config {
            model: "gpt-4o-mini".to_string(),
            toxicity: "High".to_string(),
            ignore: vec![],
        })
    })
}

/// get api key
fn get_api_key() -> Result<String> {
    env::var("OPENAI_API_KEY")
        .context("OPENAI_API_KEY environment variable is missing. Please set it in .env file or environment.")
}

/// prepare patterns
fn prepare_ignore_patterns(ignore_patterns: &[String]) -> Result<Vec<Pattern>> {
    let mut patterns = Vec::new();
    let mut has_errors = false;
    
    for pattern_str in ignore_patterns {
        match Pattern::new(pattern_str) {
            Ok(pattern) => patterns.push(pattern),
            Err(e) => {
                // warn
                eprintln!("Warning: Invalid ignore pattern '{}': {}", pattern_str, e);
                has_errors = true;
            }
        }
    }
    
    if has_errors && !ignore_patterns.is_empty() {
        eprintln!("Some ignore patterns were invalid. Continuing with valid ones...");
    }
    
    Ok(patterns)
}

/// get staged files
fn get_files_to_review(
    context_config: &ContextConfig,
    ignore_patterns: &[Pattern],
) -> Result<Vec<llm::FileContext>> {
    let staged_files = git::get_staged_files(context_config)
        .context("Failed to get staged files from git")?;
    
    let files_to_review: Vec<_> = staged_files
        .into_iter()
        .filter(|file| {
            // filtering
            for pattern in ignore_patterns {
                if pattern.matches(&file.path) {
                    println!("Ignoring {} (matched pattern: {})", file.path, pattern);
                    return false;
                }
            }
            true
        })
        .collect();
    
    Ok(files_to_review)
}

/// analyze with progressbar
async fn analyze_changes(
    files_to_review: Vec<llm::FileContext>,
    api_key: &str,
    config: &Config,
) -> Result<llm::ReviewAnalysis> {
    // init progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} Analyzing code changes...")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    
    // query
    let result = llm::review_changes(files_to_review, api_key, config)
        .await
        .context("Failed to analyze code changes");
    
    // remove progress bar
    pb.finish_and_clear();
    
    result
}

/// display the result
fn print_analysis_results(analysis: &llm::ReviewAnalysis) -> Result<()> {
    println!("VERDICT: {}", analysis.verdict);
    println!("SCORE:   {}/10", analysis.score);
    println!("{}", "-".repeat(40));
    
    if analysis.issues.is_empty() {
        println!("No issues found. Great job!");
        println!("{}", "-".repeat(40));
        return Ok(());
    }
    
    let mut critical_count = 0;
    let mut warning_count = 0;
    let mut nitpick_count = 0;
    
    for issue in &analysis.issues {
        match issue.severity {
            llm::Severity::Critical => critical_count += 1,
            llm::Severity::Warning => warning_count += 1,
            llm::Severity::Nitpick => nitpick_count += 1,
        }
        
        let level = match issue.severity {
            llm::Severity::Critical => "[CRITICAL]",
            llm::Severity::Warning => "[WARNING]",
            llm::Severity::Nitpick => "[NITPICK]",
        };
        
        println!("{} {}: {}", level, issue.file, issue.description);
        if let Some(line) = issue.line {
            println!("       Line: {}", line);
        }
        println!();
    }
    
    println!("{}", "-".repeat(40));
    println!("Summary: {} critical, {} warnings, {} nitpicks", 
             critical_count, warning_count, nitpick_count);
    
    Ok(())
}

/// check for critical errors
fn check_critical_errors(analysis: &llm::ReviewAnalysis) -> Result<()> {
    let critical_count = analysis.issues.iter()
        .filter(|issue| matches!(issue.severity, llm::Severity::Critical))
        .count();
    
    if critical_count > 0 {
        bail!("{} critical issue(s) found. Commit blocked.", critical_count);
    }
    
    Ok(())
}

/// exit with error helper
pub fn exit_with_error<E: std::fmt::Display>(error: E) -> ! {
    eprintln!("Error: {}", error);
    std::process::exit(1);
}