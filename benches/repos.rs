//! Definition + on-demand shallow clone of the large repositories used by the
//! bench. Each repo is pinned to a constant ref so successive bench runs hit
//! identical source, and cloned with `--depth 1` (via init + fetch) to avoid
//! pulling full history.
//!
//! All repos live under the directory pointed at by `TOKENSAVE_BENCH_REPOS_DIR`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug)]
pub struct Repo {
    pub name: &'static str,
    pub url: &'static str,
    /// A constant ref (tag preferred, SHA also fine). Anything that
    /// `git fetch --depth 1 origin <ref>` will resolve.
    pub git_ref: &'static str,
}

pub const REPOS: &[Repo] = &[
    Repo {
        name: "polkadot-sdk",
        url: "https://github.com/paritytech/polkadot-sdk",
        git_ref: "polkadot-stable2412",
    },
    Repo {
        name: "emacs",
        url: "https://github.com/emacs-mirror/emacs",
        git_ref: "emacs-30.1",
    },
    Repo {
        name: "scipy",
        url: "https://github.com/scipy/scipy",
        git_ref: "v1.14.1",
    },
    Repo {
        name: "node",
        url: "https://github.com/nodejs/node",
        git_ref: "v22.11.0",
    },
];

/// Returns the bench repos root, or `None` if `TOKENSAVE_BENCH_REPOS_DIR` is unset.
pub fn repos_root() -> Option<PathBuf> {
    std::env::var_os("TOKENSAVE_BENCH_REPOS_DIR").map(PathBuf::from)
}

/// Optional comma-separated filter (`TOKENSAVE_BENCH_REPOS`). When set, only
/// repos whose name appears in the list are processed.
pub fn selected_repos() -> Vec<Repo> {
    let filter = std::env::var("TOKENSAVE_BENCH_REPOS").ok();
    match filter {
        None => REPOS.to_vec(),
        Some(s) => {
            let wanted: Vec<&str> = s
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            REPOS
                .iter()
                .filter(|r| wanted.iter().any(|w| w.eq_ignore_ascii_case(r.name)))
                .copied()
                .collect()
        }
    }
}

/// Run a git command, inheriting stdout/stderr so the user sees fetch/clone
/// progress in real time. `output()` would capture the pipes and make a
/// multi-GB shallow clone look like the bench is frozen.
fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let status = cmd.status().map_err(|e| format!("spawn git: {e}"))?;
    if !status.success() {
        return Err(format!("git {} failed ({status})", args.join(" ")));
    }
    Ok(())
}

/// Ensure `repo` is checked out at its pinned ref under `root/<name>`.
/// Skips work if the marker file `.bench-ref` already records the right ref.
pub fn ensure_cloned(root: &Path, repo: Repo) -> Result<PathBuf, String> {
    let dir = root.join(repo.name);
    let marker = dir.join(".bench-ref");
    if marker.exists() {
        if let Ok(existing) = std::fs::read_to_string(&marker) {
            if existing.trim() == repo.git_ref {
                return Ok(dir);
            }
        }
    }

    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create dir: {e}"))?;
        run_git(&["init", "-q", "-b", "bench"], Some(&dir))?;
        run_git(&["remote", "add", "origin", repo.url], Some(&dir))?;
    }

    if std::env::var_os("TOKENSAVE_BENCH_SKIP_CLONE").is_some() {
        return Err(format!(
            "{} not at ref {} but TOKENSAVE_BENCH_SKIP_CLONE is set",
            repo.name, repo.git_ref
        ));
    }

    eprintln!(
        "[bench] fetching {} @ {} (depth 1)...",
        repo.name, repo.git_ref
    );
    // `--progress` forces progress output even when stderr isn't a TTY (criterion
    // wraps the bench binary, so without it `git fetch` falls back to silence on
    // a multi-GB fetch).
    run_git(
        &[
            "fetch",
            "--progress",
            "--depth",
            "1",
            "origin",
            repo.git_ref,
        ],
        Some(&dir),
    )?;
    run_git(&["checkout", "--force", "FETCH_HEAD"], Some(&dir))?;
    std::fs::write(&marker, repo.git_ref).map_err(|e| format!("write marker: {e}"))?;
    Ok(dir)
}

/// Revert all changes (tracked and untracked) made in `repo_dir` during the
/// bench: `git stash --include-untracked` followed by `git stash drop`. Run
/// at end-of-bench so write queries don't leak modifications past the run.
///
/// Returns `Ok(())` even if there is nothing to stash (`git stash` exits 0
/// in that case, just with a "No local changes" message on stderr).
pub fn restore_repo(repo_dir: &Path) -> Result<(), String> {
    // Check whether there is anything to stash; `stash drop` errors if the
    // stash list is empty, so we want to skip it cleanly when the tree is clean.
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("git status: {e}"))?;
    if dirty.stdout.is_empty() {
        return Ok(());
    }

    run_git(
        &[
            "stash",
            "push",
            "--include-untracked",
            "--quiet",
            "-m",
            "tokensave-bench",
        ],
        Some(repo_dir),
    )?;
    run_git(&["stash", "drop", "--quiet"], Some(repo_dir))?;
    Ok(())
}
