//! Borrowed-index detection for git worktrees.
//!
//! A tokensave index lives in a `.tokensave/` directory and is resolved by
//! walking up parent directories to the nearest one (see
//! [`config::discover_project_root`](crate::config::discover_project_root)).
//! That walk is unaware of git worktrees: when a worktree is created *inside*
//! the main checkout (e.g. agent tooling that puts worktrees under
//! `.claude/worktrees/<name>/` or `.worktrees/<name>/`), a command run from
//! the worktree walks up and silently resolves the MAIN checkout's index.
//!
//! Every query then returns results from the main tree's code — usually a
//! different branch — rather than the worktree the user is actually editing.
//! Symbols added or changed only in the worktree are invisible to the agent.
//! This module detects that "borrowed index" situation so callers can warn.
//!
//! Detection is best-effort: when git is unavailable or the path isn't a
//! repo, it reports "no mismatch" and callers carry on unchanged.
//!
//! Ported from `codegraph/src/sync/worktree.ts` (#312).

use std::path::{Path, PathBuf};
use std::process::Command;

/// A mismatch between the caller's git working tree and the resolved
/// tokensave index root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeIndexMismatch {
    /// The git working tree the command was invoked from.
    pub worktree_root: PathBuf,
    /// The (different) working tree whose `.tokensave/` index is being
    /// served.
    pub index_root: PathBuf,
}

/// Absolute, symlink-resolved toplevel of the git working tree that `dir`
/// belongs to, or `None` when `dir` isn't inside a git repo (or `git` is
/// missing on PATH).
///
/// `git rev-parse --show-toplevel` returns the per-worktree root: the main
/// checkout and each linked worktree report their own distinct directory,
/// which is exactly the distinction this module relies on.
pub fn git_worktree_root(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    realpath(Path::new(trimmed))
}

/// Detect when `start_path` lives in one git working tree but the resolved
/// tokensave index (`index_root`) belongs to a *different* working tree.
///
/// Returns `None` — meaning "nothing to warn about" — when:
///   - `start_path` isn't in a git repo (or git is unavailable),
///   - the index already lives in `start_path`'s own working tree, or
///   - `index_root` isn't itself a working-tree root (an unrelated parent
///     directory that merely happens to contain a `.tokensave/`), which
///     keeps non-git and monorepo-subdir layouts from producing false
///     warnings.
pub fn detect_worktree_index_mismatch(
    start_path: &Path,
    index_root: &Path,
) -> Option<WorktreeIndexMismatch> {
    let worktree_root = git_worktree_root(start_path)?;
    let resolved_index_root = realpath(index_root).unwrap_or_else(|| index_root.to_path_buf());
    if worktree_root == resolved_index_root {
        return None;
    }
    // Only flag when the index root is itself a real working-tree root.
    // This distinguishes "borrowed another worktree's index" from "index
    // sits in a plain ancestor directory", and avoids warning outside git
    // entirely.
    if git_worktree_root(&resolved_index_root)? != resolved_index_root {
        return None;
    }
    Some(WorktreeIndexMismatch {
        worktree_root,
        index_root: resolved_index_root,
    })
}

/// Verbose multi-line warning for `tokensave status` and similar contexts
/// where the answer can sit alongside a heads-up block.
pub fn worktree_mismatch_warning(m: &WorktreeIndexMismatch) -> String {
    format!(
        "This tokensave index belongs to a different git working tree.\n  \
         Running in: {}\n  \
         Index from: {}\n\
         Results reflect that tree's code (often a different branch), not this worktree — \
         symbols changed only here are missing. Run `tokensave init` in this worktree for a \
         worktree-local index.",
        m.worktree_root.display(),
        m.index_root.display()
    )
}

/// Compact, single-line variant for prefixing an MCP tool response. Read
/// tools return their answer inline, so the heads-up has to ride on the
/// same payload the agent is already reading — a multi-line block would
/// bury the result.
pub fn worktree_mismatch_notice(m: &WorktreeIndexMismatch) -> String {
    format!(
        "WARNING: tokensave results below come from a different git worktree ({}), \
         not where you're working ({}) — they may reflect another branch, and symbols \
         changed only here are missing. Run `tokensave init` here for a worktree-local index.",
        m.index_root.display(),
        m.worktree_root.display()
    )
}

/// Resolve symlinks where possible so tmp/realpath quirks don't break
/// equality checks. Falls back to a plain `absolutize` when canonicalize
/// fails (e.g. directory was deleted between rev-parse and the fs call).
fn realpath(p: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(p).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn run_git(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .expect("git not on PATH — required for worktree tests");
        assert!(status.success(), "git {args:?} failed in {}", cwd.display());
    }

    #[test]
    fn no_mismatch_outside_git() {
        let tmp = tempdir().unwrap();
        let index = tmp.path().join("index");
        let start = tmp.path().join("start");
        fs::create_dir_all(&index).unwrap();
        fs::create_dir_all(&start).unwrap();
        assert!(detect_worktree_index_mismatch(&start, &index).is_none());
    }

    #[test]
    fn no_mismatch_when_index_lives_in_same_worktree() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(&project).unwrap();
        run_git(&project, &["init", "--quiet"]);
        // start_path is inside the same working tree as the index
        let sub = project.join("src");
        fs::create_dir_all(&sub).unwrap();
        assert!(detect_worktree_index_mismatch(&sub, &project).is_none());
    }

    #[test]
    fn flags_mismatch_when_started_from_linked_worktree() {
        // Two real git working trees: a main checkout and a linked
        // worktree. start_path = the linked worktree; index_root = the
        // main checkout. Expect a mismatch.
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        fs::create_dir_all(&main).unwrap();
        run_git(&main, &["init", "--quiet"]);
        // git worktree add requires at least one commit
        fs::write(main.join("README.md"), "hi").unwrap();
        run_git(&main, &["add", "."]);
        run_git(
            &main,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "--quiet",
                "-m",
                "init",
            ],
        );
        let worktree = tmp.path().join("wt");
        run_git(
            &main,
            &["worktree", "add", "--detach", worktree.to_str().unwrap()],
        );
        let mismatch = detect_worktree_index_mismatch(&worktree, &main)
            .expect("expected mismatch when started from linked worktree but index is main");
        assert_eq!(
            mismatch.worktree_root,
            std::fs::canonicalize(&worktree).unwrap()
        );
        assert_eq!(mismatch.index_root, std::fs::canonicalize(&main).unwrap());
    }

    #[test]
    fn no_mismatch_when_index_root_is_plain_ancestor() {
        // index_root is a parent of the worktree but NOT a working-tree
        // root itself (no .git). Should not flag.
        let tmp = tempdir().unwrap();
        let outer = tmp.path().join("outer"); // not a repo
        let inner = outer.join("inner-repo");
        fs::create_dir_all(&inner).unwrap();
        run_git(&inner, &["init", "--quiet"]);
        // start in inner-repo, index_root = outer (plain dir, no .git)
        assert!(detect_worktree_index_mismatch(&inner, &outer).is_none());
    }
}
