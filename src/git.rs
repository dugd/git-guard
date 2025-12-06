use anyhow::{Result, anyhow, Context};
use git2::{Delta, Diff, DiffDelta, DiffOptions, Patch, Repository, Tree};

use crate::{config::ContextConfig, llm::FileContext, utils::estimate_tokens};

pub fn get_staged_files(config: &ContextConfig) -> Result<Vec<FileContext>> {
    // Open repo
    let repo = open_repository()?;

    // Get current head tree
    let head_tree = get_head_tree(&repo)?;

    // Get diff head tree to index
    let diff = create_diff(&repo, head_tree.as_ref(), config)?;

    extract_file_contexts(&repo, &diff, config)
}

fn open_repository() -> Result<Repository> {
    Repository::open_from_env()
        .context("Failed to open git repository")
}

fn get_head_tree<'a>(repo: &'a Repository) -> Result<Option<Tree<'a>>> {
    match repo.head() {
        Ok(head) => {
            let tree = head.peel_to_tree()
                .context("Failed to peel HEAD to tree")?;
            Ok(Some(tree))
        }
        Err(_) => Ok(None),
    }
}

fn create_diff<'a>(
    repo: &'a Repository,
    head_tree: Option<&Tree<'a>>,
    config: &ContextConfig,
) -> Result<Diff<'a>> {
    let mut diff_opts = DiffOptions::new();
    diff_opts.include_typechange(true);
    diff_opts.context_lines(config.context_lines);
    
    repo.diff_tree_to_index(
        head_tree,
        Some(&repo.index()?),
        Some(&mut diff_opts),
    )
    .context("Failed to calculate diff")
}

fn extract_file_contexts(
    repo: &Repository, 
    diff: &Diff, 
    config: &ContextConfig,
) -> Result<Vec<FileContext>> {
    let mut contexts = Vec::new();

    let mut total_tokens = 0;

    for i in 0..diff.deltas().len() {
        if let Some(file_context) = process_delta(repo, diff, i, config.max_file_tokens)? {
            let full_tokens = if let Some(s) = file_context.full_content.as_ref() {
                estimate_tokens(s)
            } else { 0 };

            let diff_tokens = estimate_tokens(&file_context.diff);

            total_tokens += full_tokens + diff_tokens;
            if total_tokens > config.max_global_tokens {
                return Err(anyhow!("Reached the limit of tokens"));
            }

            contexts.push(file_context);
        }
    }

    Ok(contexts)
}

fn process_delta(repo: &Repository, diff: &Diff, i: usize, file_tokens_limit: usize) -> Result<Option<FileContext>> {
    let patch = Patch::from_diff(&diff, i)?;

    let Some(patch) = patch else { return Ok(None) };

    let delta = patch.delta();

    if !is_revelant_status(delta.status()) {
        return Ok(None);
    }

    let path_str = get_delta_path(&delta)?;

    if should_skip_file(&path_str) {
        return Ok(None);
    }

    let file_content = get_file_content(&repo, &delta, &path_str)?;
    let Some(file_content) = file_content else { return Ok(None) };

    // with numbers
    let full_content = with_line_numbers(&file_content);
    let full_content = if estimate_tokens(&full_content) > file_tokens_limit { 
        None 
    } else { Some(file_content) };

    // diff content
    let diff_str = get_patch_diff_content(&patch)?;

    Ok(Some(FileContext {
        path: path_str,
        diff: diff_str,
        full_content: full_content,
    }))
}

fn is_revelant_status(status: Delta) -> bool {
    matches!(
        status,
         Delta::Added | Delta::Modified | Delta::Renamed | Delta::Copied,
    )
}

fn get_delta_path(delta: &DiffDelta) -> Result<String> {
    let path_bytes = delta.new_file()
        .path()
        .ok_or_else(|| anyhow!("Delta has no path"))?;
    
    Ok(path_bytes.to_string_lossy().to_string())
}

fn should_skip_file(path: &str) -> bool {
    path.ends_with(".lock")
}

fn get_file_content(
    repo: &Repository,
    delta: &DiffDelta,
    path: &str,
) -> Result<Option<String>> {
    let oid = delta.new_file().id();
    let blob = repo.find_blob(oid)
        .context(format!("Failed to find blob for {}", path))?;

    if blob.is_binary() {
        return Ok(None);
    }

    match str::from_utf8(blob.content()) {
        Ok(c) => Ok(Some(c.to_string())),
        Err(_) => Ok(None), // binary
    }
}

fn with_line_numbers(content: &str) -> String {
    content.lines()
        .enumerate()
        .map(|(i, line)| format!("{:>4} | {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_patch_diff_content(patch: &Patch) -> Result<String> {
    let mut diff_output = String::new();

    for h_idx in 0..patch.num_hunks() {
        append_hunk_content(patch, h_idx, &mut diff_output)?;
    }

    Ok(diff_output)
}

fn append_hunk_content(patch: &Patch, hunk_idx: usize, output: &mut String) -> Result<()> {
    let (hunk, lines_count) = patch.hunk(hunk_idx)?;

    let header = str::from_utf8(hunk.header())
            .unwrap_or("@@ ? @@")
            .trim_end();

    output.push_str(header);
    output.push('\n');

    for line_idx in 0..lines_count {
        append_line_content(patch, hunk_idx, line_idx, output)?;
    }

    Ok(())
}

fn append_line_content(patch: &Patch, hunk_index: usize, line_index: usize, output: &mut String) -> Result<()> {
    let line = patch.line_in_hunk(hunk_index, line_index)?;
    
    let prefix = line.origin();
    let content = str::from_utf8(line.content()).unwrap_or("");
    
    output.push(prefix);
    output.push_str(content);
    
    Ok(())
}
