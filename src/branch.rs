//! Git branch resolution utilities for multi-branch indexing.

use std::path::Path;

use crate::branch_meta::BranchMeta;

/// Resolves the current branch name using `gix`. Falls back to
/// `git symbolic-ref HEAD` for worktrees when gix cannot resolve HEAD
/// (e.g. with minimal feature flags that exclude worktree support).
///
/// Returns `None` for detached HEAD or if the repository cannot be opened.
pub fn current_branch(project_root: &Path) -> Option<String> {
    if let Some(branch) = current_branch_gix(project_root) {
        return Some(branch);
    }
    current_branch_git(project_root)
}

fn current_branch_gix(project_root: &Path) -> Option<String> {
    let repo = gix::open(project_root).ok()?;
    let head = repo.head().ok()?;
    let name = head.name().as_bstr();
    let name_str = std::str::from_utf8(name).ok()?;
    name_str
        .strip_prefix("refs/heads/")
        .map(std::string::ToString::to_string)
}

fn current_branch_git(project_root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = std::str::from_utf8(&output.stdout).ok()?;
    name.strip_prefix("refs/heads/")
        .and_then(|s| s.strip_suffix('\n'))
        .map(std::string::ToString::to_string)
}

/// Auto-detects the default branch (main or master).
///
/// Strategy:
/// 1. Try `git symbolic-ref refs/remotes/origin/HEAD`
/// 2. Fall back to checking if `main` or `master` exists locally
pub fn detect_default_branch(project_root: &Path) -> Option<String> {
    let repo = gix::open(project_root).ok()?;

    // Try symbolic-ref first (refs/remotes/origin/HEAD -> refs/remotes/origin/<branch>)
    if let Ok(reference) = repo.find_reference("refs/remotes/origin/HEAD") {
        if let Some(Ok(target)) = reference.follow() {
            if let Some(name) = target
                .name()
                .as_bstr()
                .to_string()
                .strip_prefix("refs/remotes/origin/")
            {
                return Some(name.to_string());
            }
        }
    }

    // Fall back to heuristics
    for candidate in &["main", "master"] {
        let refname = format!("refs/heads/{candidate}");
        if repo.find_reference(&refname).is_ok() {
            return Some((*candidate).to_string());
        }
    }

    None
}

/// Sanitizes a branch name for use as a filename.
///
/// Replaces `/` with `_`, strips characters unsafe for filenames,
/// and collapses `..` sequences to prevent path traversal.
pub fn sanitize_branch_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' | '.' => '_',
            c => c,
        })
        .collect();
    // Collapse runs of underscores
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_underscore = false;
    for c in sanitized.chars() {
        if c == '_' {
            if !prev_underscore {
                result.push(c);
            }
            prev_underscore = true;
        } else {
            result.push(c);
            prev_underscore = false;
        }
    }
    // Strip leading/trailing underscores
    result.trim_matches('_').to_string()
}

/// Resolves the DB path for a given branch.
///
/// If the branch is tracked in metadata, returns its `db_file` path.
/// Returns `None` if untracked or if the path would escape `tokensave_dir`.
pub fn resolve_branch_db_path(
    tokensave_dir: &Path,
    branch: &str,
    meta: &BranchMeta,
) -> Option<std::path::PathBuf> {
    let entry = meta.branches.get(branch)?;
    let resolved = tokensave_dir.join(&entry.db_file);
    // Prevent path traversal: resolved path must stay within tokensave_dir
    if let (Ok(canonical_dir), Ok(canonical_path)) =
        (tokensave_dir.canonicalize(), resolved.canonicalize())
    {
        if !canonical_path.starts_with(&canonical_dir) {
            return None;
        }
    }
    Some(resolved)
}

/// Finds the nearest tracked ancestor branch using `git merge-base`.
///
/// For each tracked branch in the metadata, computes the merge-base with
/// the given branch and picks the one with the most recent common ancestor.
pub fn find_nearest_tracked_ancestor(
    project_root: &Path,
    branch: &str,
    meta: &BranchMeta,
) -> Option<String> {
    let repo = gix::open(project_root).ok()?;

    let branch_ref = format!("refs/heads/{branch}");
    let branch_commit = repo
        .find_reference(&branch_ref)
        .ok()?
        .peel_to_commit()
        .ok()?;

    let mut best: Option<(String, gix::date::Time)> = None;

    for tracked_name in meta.branches.keys() {
        if tracked_name == branch {
            continue;
        }
        let tracked_ref = format!("refs/heads/{tracked_name}");
        let Some(tracked_commit) = repo
            .find_reference(&tracked_ref)
            .ok()
            .and_then(|mut r| r.peel_to_commit().ok())
        else {
            continue;
        };

        // Find merge-base between branch and tracked branch
        let Ok(base_id) = repo.merge_base(branch_commit.id, tracked_commit.id) else {
            continue;
        };

        let Ok(base_commit) = repo.find_commit(base_id) else {
            continue;
        };
        let time = base_commit
            .time()
            .ok()
            .unwrap_or_else(|| gix::date::Time::new(0, 0));
        if best
            .as_ref()
            .is_none_or(|(_, best_time)| time.seconds > best_time.seconds)
        {
            best = Some((tracked_name.clone(), time));
        }
    }

    best.map(|(name, _)| name)
}

/// Tracks `branch` by copying the nearest-ancestor DB into a per-branch DB and
/// recording it in `BranchMeta`. This is the copy+record core shared by the
/// `tokensave branch add` CLI command and the transparent auto-track paths.
///
/// Does **not** run an incremental sync — that would require opening a
/// `TokenSave` (re-entrant with `open()`); callers that want a fresh index run
/// `sync()` afterwards, and the `post-commit` hook keeps a tracked branch fresh.
///
/// Returns `Ok(true)` when the branch is newly tracked, `Ok(false)` when nothing
/// was done (no branch metadata yet → single-DB mode; the branch is the default;
/// or it is already tracked). Never touches the default branch's `tokensave.db`.
pub fn track_branch_copy(
    project_root: &Path,
    tokensave_dir: &Path,
    branch: &str,
) -> crate::errors::Result<bool> {
    use crate::branch_meta;

    // No metadata → single-DB mode. Do NOT bootstrap tracking implicitly here;
    // that is `branch add`'s job. Preserves backward-compatible behavior.
    let Some(mut meta) = branch_meta::load_branch_meta(tokensave_dir) else {
        return Ok(false);
    };

    // The default branch is served by the top-level tokensave.db; never copy it.
    if branch == meta.default_branch || meta.is_tracked(branch) {
        return Ok(false);
    }

    // Pick the parent DB to copy from (nearest tracked ancestor, else default).
    let parent = find_nearest_tracked_ancestor(project_root, branch, &meta)
        .unwrap_or_else(|| meta.default_branch.clone());
    let Some(parent_db) = resolve_branch_db_path(tokensave_dir, &parent, &meta) else {
        return Ok(false);
    };
    if !parent_db.exists() {
        return Ok(false);
    }

    let sanitized = sanitize_branch_name(branch);
    let branches_dir = branch_meta::ensure_branches_dir(tokensave_dir)?;
    let new_db_path = branches_dir.join(format!("{sanitized}.db"));
    // ponytail: copies only the .db, matching `tokensave branch add`. If the
    // ancestor has an uncheckpointed WAL (concurrent sync), that data is missed;
    // upgrade path = checkpoint-before-copy applied to BOTH paths together.
    std::fs::copy(&parent_db, &new_db_path)?;

    let db_file = format!("branches/{sanitized}.db");
    meta.add_branch(branch, &db_file, &parent);
    branch_meta::save_branch_meta(tokensave_dir, &meta)?;
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_simple() {
        assert_eq!(sanitize_branch_name("main"), "main");
    }

    #[test]
    fn sanitize_slashes() {
        assert_eq!(sanitize_branch_name("feature/foo/bar"), "feature_foo_bar");
    }

    #[test]
    fn sanitize_special_chars() {
        assert_eq!(sanitize_branch_name("fix: bug <1>"), "fix_bug_1");
    }

    #[test]
    fn sanitize_dots_prevented() {
        // ".." becomes all underscores, collapsed and trimmed to empty
        assert_eq!(sanitize_branch_name(".."), "");
        // dots and slashes become underscores, collapsed
        assert_eq!(sanitize_branch_name("foo/../bar"), "foo_bar");
    }

    #[test]
    fn track_branch_copy_copies_ancestor_and_is_idempotent() {
        use crate::branch_meta;
        let dir = tempfile::TempDir::new().unwrap();
        let ts = dir.path(); // use as both project_root and tokensave_dir
                             // Seed default-branch metadata + its DB (no git repo → ancestor lookup
                             // falls back to the default branch).
        branch_meta::save_branch_meta(ts, &branch_meta::BranchMeta::new("main")).unwrap();
        std::fs::write(ts.join("tokensave.db"), b"DBDATA").unwrap();

        // First call tracks the branch by copying the ancestor DB.
        assert!(track_branch_copy(ts, ts, "feature-x").unwrap());
        assert!(ts.join("branches").join("feature-x.db").exists());
        assert!(branch_meta::load_branch_meta(ts)
            .unwrap()
            .is_tracked("feature-x"));

        // Idempotent: already tracked, default branch, and no-metadata are no-ops.
        assert!(!track_branch_copy(ts, ts, "feature-x").unwrap());
        assert!(!track_branch_copy(ts, ts, "main").unwrap());
        let empty = tempfile::TempDir::new().unwrap();
        assert!(!track_branch_copy(empty.path(), empty.path(), "x").unwrap());
    }
}
