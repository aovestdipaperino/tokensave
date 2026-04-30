//! Git integration helpers for churn analysis.
//! Shells out to `git log` at runtime to gather temporal data.
//! No data is persisted to the tokensave DB.

use std::collections::HashMap;
use std::path::Path;

use crate::errors::Result;

/// Returns a map of `file_path` → `commit_count` for the last `days` days.
/// Shells out to `git log --format= --name-only --since='{days} days ago'`.
/// Returns an empty map if git is not available or not a repo.
pub async fn file_churn(project_root: &Path, days: u32) -> Result<HashMap<String, usize>> {
    let output = tokio::process::Command::new("git")
        .args([
            "log",
            "--format=",
            "--name-only",
            &format!("--since={days} days ago"),
        ])
        .current_dir(project_root)
        .output()
        .await;

    // git not found or other OS error → return empty map gracefully
    let Ok(output) = output else {
        return Ok(HashMap::new());
    };

    if !output.status.success() {
        // Not a git repo, or another non-fatal git error
        return Ok(HashMap::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut churn: HashMap<String, usize> = HashMap::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        *churn.entry(trimmed.to_string()).or_insert(0) += 1;
    }
    Ok(churn)
}
