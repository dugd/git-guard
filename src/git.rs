use std::process::Command;

use anyhow::{Result, anyhow, Context};

use crate::llm::FileContext;

pub fn get_staged_files() -> Result<Vec<FileContext>> {
    // Ignore Deleted (D)
    let output = exec_git(&["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .context("Failed to get list of staged files")?;

    let paths = output.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty());

    let mut contexts = Vec::new();

    for path in paths {
        // Skip huge lock files
        if path.ends_with(".lock") { continue; }

        let diff = exec_git(&["diff", "--cached", "--", path])
            .context(format!("Failed to get diff for {}", path))?;

        let full_content = match exec_git(&["show", &format!(":{}", path)]) {
            Ok(content) => content,
            Err(_) => {
                // Skip
                String::new() 
            }
        };

        // TODO: find a way to skip binary files

        contexts.push(FileContext {
            path: path.to_string(),
            diff,
            full_content: Some(full_content),
        });
    }

    Ok(contexts)
}

/// System call git command. Secure, but slow.
fn exec_git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| anyhow!("Failed to execute git command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Git command failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}