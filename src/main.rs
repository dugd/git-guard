use std::{env, process::Command};

use dotenvy::dotenv;

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

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Temporarily unused
    let _api_key = env::var("OPENAI_API_KEY").expect("'OPENAI_API_KEY' is missing");

    println!("=== Git Sensei (Demo) ===");

    match get_staged_diff() {
        Ok(diff) => {
            println!("Got diff ({} chars). Preparing to roast...", diff.len());
            println!("Preview:\n{:.200}...", diff);
        },
        Err(e) => eprint!("Error: {}", e),
    }
}
