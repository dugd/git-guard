use anyhow::{Result, anyhow, Context};
use git2::{Delta, DiffOptions, Patch, Repository};

use crate::llm::FileContext;

pub fn get_staged_files() -> Result<Vec<FileContext>> {
    // Open repo
    let repo = Repository::open_from_env()
        .context("Failed to open git repository")?;

    // Get current head tree
    let head_tree = match repo.head() {
        Ok(head) => Some(head.peel_to_tree().context("Failed to peel HEAD to tree")?),
        Err(_) => None,
    };

    // configure diff
    let mut diff_opts = DiffOptions::new();
    diff_opts.include_typechange(true);

    // diff head tree to index
    let diff = repo.diff_tree_to_index(
        head_tree.as_ref(), 
        Some(&repo.index()?),
        Some(&mut diff_opts)
    ).context("Failed to calculate diff")?;

    let mut contexts = Vec::new();
    let num_deltas = diff.deltas().len();

    for i in 0..num_deltas {
        let patch = Patch::from_diff(&diff, i)?;

        if let Some(patch) = patch {
            // metadata
            let delta = patch.delta();

            match delta.status() {
                Delta::Added | Delta::Modified | Delta::Renamed | Delta::Copied => {
                    let path_bytes = delta.new_file().path()
                        .ok_or_else(|| anyhow!("Delta has no path"))?;

                    let path_str = path_bytes.to_string_lossy().to_string();

                    // filter verbose files
                    if path_str.ends_with(".lock") { continue; }

                    let oid = delta.new_file().id();
                    let blob = repo.find_blob(oid)
                        .context(format!("Failed to find blob for {}", path_str))?;

                    if blob.is_binary() { continue; }

                    // full content
                    let raw_content = match str::from_utf8(blob.content()) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    // with numbers
                    let full_content = with_line_numbers(raw_content);

                    // diff content
                    let diff_str = get_patch_diff_content(&patch)?;

                    contexts.push(FileContext {
                        path: path_str,
                        diff: diff_str,
                        full_content: Some(full_content),
                    });
                },
                _ => {},
            }
        }
    }

    Ok(contexts)
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
        let (hunk, lines_count) = patch.hunk(h_idx)?;

        let header = str::from_utf8(hunk.header())
            .unwrap_or("@@ ? @@")
            .trim_end();

        diff_output.push_str(header);
        diff_output.push('\n');

        for l_idx in 0..lines_count {
            let line = patch.line_in_hunk(h_idx, l_idx)?;

            let prefix = line.origin();

            let content = str::from_utf8(line.content()).unwrap_or("");

            diff_output.push(prefix);
            diff_output.push_str(content);
        }
    }

    Ok(diff_output)
}
