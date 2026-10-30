//! Git branch resolution utilities for multi-branch indexing.

mod gc;
pub use gc::{collect_if_enabled, collect_stale_indexes};

use std::path::{Path, PathBuf};

use crate::branch_meta::BranchMeta;

/// Returns the path of a database's `-wal` sidecar file.
fn wal_sidecar(db_path: &Path) -> PathBuf {
    let mut os = db_path.as_os_str().to_os_string();
    os.push("-wal");
    PathBuf::from(os)
}

/// File size in bytes, or 0 when the file does not exist.
fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |m| m.len())
}

/// Snapshots `parent_db` into `new_db_path` via `VACUUM INTO`, which writes a
/// consistent copy under the connection's read snapshot even while another
/// connection is committing. Fails if the parent is not a readable `SQLite`
/// file; `new_db_path` must not exist.
async fn vacuum_into(parent_db: &Path, new_db_path: &Path) -> std::result::Result<(), String> {
    let db = libsql::Builder::new_local(parent_db)
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let conn = db.connect().map_err(|e| e.to_string())?;
    // A lossy conversion would make VACUUM INTO "succeed" at a mangled
    // replacement-character path; bail to the file-copy fallback instead,
    // which handles non-UTF-8 paths losslessly.
    let dest = new_db_path
        .to_str()
        .ok_or_else(|| "non-UTF-8 destination path".to_string())?
        .to_owned();
    conn.execute("VACUUM INTO ?1", [dest])
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Copies `parent_db` to `new_db_path` without dropping data that sits in the
/// parent's uncheckpointed WAL (#292).
///
/// A long-lived MCP server keeps the parent DB open in WAL mode, so committed
/// rows can live only in the `-wal` sidecar for a long time. A bare
/// `fs::copy` of the `.db` then produces a truncated snapshot — nodes that
/// happen to be checkpointed still answer queries while e.g. the `files`
/// table comes up empty. Two layers close that hole:
///
/// 1. `VACUUM INTO` writes a transactionally consistent snapshot of the
///    parent (main file + WAL contents), correct even while a concurrent
///    connection is writing. It targets a unique temp file that is renamed
///    over the destination — the same atomic-publish idiom as
///    `save_branch_meta` — so two trackers racing on the same branch each
///    produce a complete DB and the loser atomically replaces the winner's,
///    and a stale destination left by an interrupted earlier attempt is
///    replaced rather than diverting to the fallback below.
/// 2. If the snapshot fails (the parent is not a readable `SQLite` file, or
///    an exotic locking failure), fall back to a best-effort
///    `wal_checkpoint(TRUNCATE)` followed by a plain file copy, bringing the
///    `-wal` sidecar along if the checkpoint could not empty it so `SQLite`
///    recovers it on first open of the new DB. This path is not
///    race-free against an active writer — the snapshot layer above is.
pub async fn copy_branch_db(parent_db: &Path, new_db_path: &Path) -> crate::errors::Result<()> {
    if let Err(error) =
        crate::db::compatibility::preflight(parent_db, crate::db::migrations::latest_version())
            .await
    {
        if crate::db::compatibility::is_refusal(&error) {
            return Err(error);
        }
    }
    let file_name = new_db_path.file_name().map_or_else(
        || "branch.db".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp =
        new_db_path.with_file_name(format!(".{file_name}.{}.{nanos}.tmp", std::process::id()));
    if vacuum_into(parent_db, &tmp).await.is_ok() {
        if let Err(e) = std::fs::rename(&tmp, new_db_path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        return Ok(());
    }
    let _ = std::fs::remove_file(&tmp);

    let parent_wal = wal_sidecar(parent_db);
    if file_len(&parent_wal) > 0 {
        // Errors are deliberately ignored: the non-empty-WAL check below
        // catches every failure mode uniformly by falling back to a sidecar
        // copy.
        if let Ok(db) = libsql::Builder::new_local(parent_db).build().await {
            if let Ok(conn) = db.connect() {
                let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").await;
            }
        }
    }
    std::fs::copy(parent_db, new_db_path)?;
    if file_len(&parent_wal) > 0 {
        std::fs::copy(&parent_wal, wal_sidecar(new_db_path))?;
    }
    Ok(())
}

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

/// Returns the set of local branch names, or `None` when the refs could not
/// be enumerated at all.
///
/// The distinction matters to every caller that deletes something: an empty
/// set means the repository genuinely has no branches, while `None` means we
/// do not know, and "we do not know" must never be read as "nothing exists".
///
/// This asks git rather than looking for `.git/refs/heads/<name>` on disk
/// (#501). That probe is wrong in two common layouts: inside a linked
/// worktree `.git` is a *file* pointing at the real git directory, so the
/// path never exists; and a `reftable` repository stores no loose refs at
/// all. In both, every branch looked deleted.
pub fn local_branches(project_root: &Path) -> Option<std::collections::HashSet<String>> {
    if let Some(branches) = local_branches_gix(project_root) {
        return Some(branches);
    }
    local_branches_git(project_root)
}

fn local_branches_gix(project_root: &Path) -> Option<std::collections::HashSet<String>> {
    let repo = gix::open(project_root).ok()?;
    let platform = repo.references().ok()?;
    let iter = platform.local_branches().ok()?;
    let mut out = std::collections::HashSet::new();
    for reference in iter {
        let reference = reference.ok()?;
        let name = reference.name().as_bstr().to_string();
        if let Some(short) = name.strip_prefix("refs/heads/") {
            out.insert(short.to_string());
        }
    }
    Some(out)
}

fn local_branches_git(project_root: &Path) -> Option<std::collections::HashSet<String>> {
    let output = std::process::Command::new("git")
        .args(["for-each-ref", "--format=%(refname:strip=2)", "refs/heads"])
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = std::str::from_utf8(&output.stdout).ok()?;
    Some(
        text.lines()
            .filter(|l| !l.is_empty())
            .map(std::string::ToString::to_string)
            .collect(),
    )
}

/// The refs that make `name` a branch of the repository: a local branch, or
/// a remote-tracking branch of `origin`.
fn branch_refs(name: &str) -> [String; 2] {
    [
        format!("refs/heads/{name}"),
        format!("refs/remotes/origin/{name}"),
    ]
}

fn has_branch_ref(repo: &gix::Repository, name: &str) -> bool {
    branch_refs(name)
        .iter()
        .any(|r| repo.find_reference(r.as_str()).is_ok())
}

/// Returns whether `name` exists as a local branch or as a remote-tracking
/// branch of `origin`, or `None` when the repository cannot be queried.
///
/// A ref `gix` cannot find is confirmed with `git show-ref` before it is
/// reported missing: callers rename metadata on a `false`, and a repository
/// layout `gix` does not read (#501) must not look like a deleted branch.
pub fn branch_exists(project_root: &Path, name: &str) -> Option<bool> {
    if gix::open(project_root).is_ok_and(|repo| has_branch_ref(&repo, name)) {
        return Some(true);
    }
    for refname in branch_refs(name) {
        let status = std::process::Command::new("git")
            .args(["show-ref", "--verify", "--quiet", &refname])
            .current_dir(project_root)
            .status()
            .ok()?;
        match status.code() {
            Some(0) => return Some(true),
            Some(1) => {}
            _ => return None,
        }
    }
    Some(false)
}

/// Auto-detects the default branch (main or master).
///
/// Strategy:
/// 1. Try `git symbolic-ref refs/remotes/origin/HEAD`, accepting its target
///    only while that branch still exists: `git fetch` never updates
///    `origin/HEAD`, so it can name a branch renamed upstream (#874)
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
                if has_branch_ref(&repo, name) {
                    return Some(name.to_string());
                }
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

/// How the recorded default branch compares with the repository (#874).
///
/// `branch-meta.json` records the default branch once, at init. A rename
/// (`git branch -m master main`) or a stale `origin/HEAD` leaves it naming a
/// branch that no longer exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultBranchState {
    /// The recorded default still exists, or the repository cannot be read.
    Current,
    /// The recorded default is gone and the detected default has no entry of
    /// its own, so the default entry can be renamed in place.
    Renamable { recorded: String, detected: String },
    /// The recorded default is gone and the detected default is already
    /// tracked with its own database. Never merged automatically.
    Duplicate { recorded: String, detected: String },
    /// The recorded default is gone and no replacement was detected.
    Undetected { recorded: String },
}

impl DefaultBranchState {
    /// A one-line explanation with the command that fixes it, or `None`
    /// when the recorded default is current.
    pub fn warning(&self) -> Option<String> {
        match self {
            Self::Current => None,
            Self::Renamable { recorded, detected } => Some(format!(
                "default branch '{recorded}' no longer exists; the repository's default is \
                 '{detected}'. The next tokensave command renames it, or run \
                 `tokensave branch set-default {detected}`."
            )),
            Self::Duplicate { recorded, detected } => Some(format!(
                "default branch '{recorded}' no longer exists, and '{detected}' is tracked \
                 with its own database. Run `tokensave branch set-default {detected}`, then \
                 `tokensave branch remove {recorded}` to drop the old index."
            )),
            Self::Undetected { recorded } => Some(format!(
                "default branch '{recorded}' no longer exists. Run \
                 `tokensave branch set-default <branch>` to choose the new default."
            )),
        }
    }
}

/// Compares `meta.default_branch` with the repository. Read-only.
pub fn default_branch_state(project_root: &Path, meta: &BranchMeta) -> DefaultBranchState {
    let recorded = &meta.default_branch;
    // An unborn branch (`git init` with no commit yet) has no ref but is real.
    if branch_exists(project_root, recorded) != Some(false)
        || current_branch(project_root).as_deref() == Some(recorded.as_str())
    {
        return DefaultBranchState::Current;
    }
    match detect_default_branch(project_root) {
        Some(detected) if detected == *recorded => DefaultBranchState::Current,
        Some(detected) if meta.is_tracked(&detected) => DefaultBranchState::Duplicate {
            recorded: recorded.clone(),
            detected,
        },
        Some(detected) => DefaultBranchState::Renamable {
            recorded: recorded.clone(),
            detected,
        },
        None => DefaultBranchState::Undetected {
            recorded: recorded.clone(),
        },
    }
}

/// Renames a stale default-branch entry when that is unambiguous: the
/// recorded default no longer exists and the detected default has no entry
/// of its own. The entry keeps its `db_file`; no database is copied or
/// deleted.
///
/// Returns the state found under the branch-operation lock. `Renamable`
/// means the rename was applied; any other state was left unchanged.
pub async fn reconcile_default_branch(
    project_root: &Path,
    tokensave_dir: &Path,
) -> crate::errors::Result<DefaultBranchState> {
    use crate::branch_meta;

    let Some(meta) = branch_meta::load_branch_meta(tokensave_dir) else {
        return Ok(DefaultBranchState::Current);
    };
    // Check before locking: the common, current case must stay free.
    let unlocked = default_branch_state(project_root, &meta);
    if !matches!(unlocked, DefaultBranchState::Renamable { .. }) {
        return Ok(unlocked);
    }
    let _branch_lock = crate::tokensave::acquire_branch_operation_lock(tokensave_dir).await?;
    let Some(mut meta) = branch_meta::load_branch_meta(tokensave_dir) else {
        return Ok(DefaultBranchState::Current);
    };
    let state = default_branch_state(project_root, &meta);
    if let DefaultBranchState::Renamable { recorded, detected } = &state {
        if meta.rename_branch(recorded, detected) {
            branch_meta::save_branch_meta(tokensave_dir, &meta)?;
        }
    }
    Ok(state)
}

/// What `set_default_branch` changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetDefaultOutcome {
    /// The branch already was the default.
    Unchanged,
    /// The branch was untracked: the previous default's entry was renamed.
    Renamed { previous: String },
    /// The branch was tracked: its entry became the default, and the
    /// previous default stays as an ordinary branch entry.
    Promoted { previous: String },
}

/// Makes `name` the default branch, keeping the data already indexed
/// (`tokensave branch set-default`).
pub async fn set_default_branch(
    project_root: &Path,
    tokensave_dir: &Path,
    name: &str,
) -> crate::errors::Result<SetDefaultOutcome> {
    use crate::branch_meta;
    use crate::errors::TokenSaveError;

    let _branch_lock = crate::tokensave::acquire_branch_operation_lock(tokensave_dir).await?;
    let mut meta =
        branch_meta::load_branch_meta(tokensave_dir).ok_or_else(|| TokenSaveError::Config {
            message: "no branch tracking configured; run `tokensave init` first".to_string(),
        })?;
    if name == meta.default_branch {
        return Ok(SetDefaultOutcome::Unchanged);
    }
    let previous = meta.default_branch.clone();
    let outcome = if meta.set_default(name) {
        SetDefaultOutcome::Promoted { previous }
    } else {
        if branch_exists(project_root, name) == Some(false)
            && current_branch(project_root).as_deref() != Some(name)
        {
            return Err(TokenSaveError::Config {
                message: format!("'{name}' is not a local or origin branch of this repository"),
            });
        }
        meta.rename_branch(&previous, name);
        SetDefaultOutcome::Renamed { previous }
    };
    branch_meta::save_branch_meta(tokensave_dir, &meta)?;
    Ok(outcome)
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

/// Returns the `branches/<name>.db` relative `db_file` to use for `branch`,
/// guaranteed not to collide with a *different* branch already recorded in
/// `meta`.
///
/// `sanitize_branch_name` is many-to-one: `feature/foo`, `feature_foo` and
/// `feature.foo` all map to `feature_foo`, so two distinct branches could
/// otherwise be pointed at the same DB file and silently overwrite each
/// other's index. When the readable stem is already claimed by a different
/// branch, a short stable hash of the *full* branch name is appended, so the
/// mapping stays deterministic (idempotent re-track) and collision-free.
pub fn unique_branch_db_file(meta: &crate::branch_meta::BranchMeta, branch: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let stem = sanitize_branch_name(branch);
    let candidate = format!("branches/{stem}.db");
    let collides = meta
        .branches
        .iter()
        .any(|(name, entry)| name != branch && entry.db_file == candidate);
    if !collides {
        return candidate;
    }
    let digest = Sha256::digest(branch.as_bytes());
    let short = digest.iter().take(4).fold(String::new(), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    });
    format!("branches/{stem}-{short}.db")
}

/// Tracks `branch` by copying the nearest-ancestor DB into a per-branch DB and
/// recording it in `BranchMeta`. This is the copy+record core shared by the
/// `tokensave branch add` CLI command and the transparent auto-track paths.
///
/// Does **not** run an incremental sync — that would require opening a
/// `TokenSave` (re-entrant with `open()`); callers that want a fresh index run
/// `sync()` afterwards, and the `post-commit`/`post-merge` hooks keep a tracked branch fresh.
///
/// Returns `Ok(true)` when the branch is newly tracked, `Ok(false)` when nothing
/// was done (no branch metadata yet → single-DB mode; the branch is the default;
/// or it is already tracked). Never touches the default branch's `tokensave.db`.
pub async fn track_branch_copy(
    project_root: &Path,
    tokensave_dir: &Path,
    branch: &str,
) -> crate::errors::Result<bool> {
    use crate::branch_meta;

    // No metadata → single-DB mode. Do NOT bootstrap tracking implicitly here;
    // that is `branch add`'s job. Preserves backward-compatible behavior.
    if branch_meta::load_branch_meta(tokensave_dir).is_none() {
        return Ok(false);
    }
    let _branch_lock = crate::tokensave::acquire_branch_operation_lock(tokensave_dir).await?;
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

    let db_file = unique_branch_db_file(&meta, branch);
    branch_meta::ensure_branches_dir(tokensave_dir)?;
    let new_db_path = tokensave_dir.join(&db_file);
    copy_branch_db(&parent_db, &new_db_path).await?;

    meta.add_branch(branch, &db_file, &parent);
    branch_meta::save_branch_meta(tokensave_dir, &meta)?;
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn unique_branch_db_file_avoids_collision() {
        use crate::branch_meta::BranchMeta;
        let mut meta = BranchMeta::new("main");
        // First branch takes the plain sanitized stem.
        let f1 = unique_branch_db_file(&meta, "feature/foo");
        assert_eq!(f1, "branches/feature_foo.db");
        meta.add_branch("feature/foo", &f1, "main");
        // A different branch that sanitizes to the same stem must not reuse it.
        let f2 = unique_branch_db_file(&meta, "feature_foo");
        assert_ne!(f2, f1);
        assert!(f2.starts_with("branches/feature_foo-") && f2.contains(".db"));
        // Re-querying an already-tracked branch is idempotent: its own entry is
        // excluded from the collision check so it keeps its plain stem.
        assert_eq!(unique_branch_db_file(&meta, "feature/foo"), f1);
    }

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

    #[tokio::test]
    async fn track_branch_copy_copies_ancestor_and_is_idempotent() {
        use crate::branch_meta;
        let dir = tempfile::TempDir::new().unwrap();
        let ts = dir.path(); // use as both project_root and tokensave_dir
                             // Seed default-branch metadata + its DB (no git repo → ancestor lookup
                             // falls back to the default branch). Not a real SQLite file, which
                             // also covers copy_branch_db's checkpoint-open failure path.
        branch_meta::save_branch_meta(ts, &branch_meta::BranchMeta::new("main")).unwrap();
        std::fs::write(ts.join("tokensave.db"), b"DBDATA").unwrap();

        // First call tracks the branch by copying the ancestor DB.
        assert!(track_branch_copy(ts, ts, "feature-x").await.unwrap());
        assert!(ts.join("branches").join("feature-x.db").exists());
        assert!(branch_meta::load_branch_meta(ts)
            .unwrap()
            .is_tracked("feature-x"));

        // Idempotent: already tracked, default branch, and no-metadata are no-ops.
        assert!(!track_branch_copy(ts, ts, "feature-x").await.unwrap());
        assert!(!track_branch_copy(ts, ts, "main").await.unwrap());
        let empty = tempfile::TempDir::new().unwrap();
        assert!(!track_branch_copy(empty.path(), empty.path(), "x")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn track_branch_copy_waits_for_an_in_progress_branch_operation() {
        use crate::branch_meta;
        use std::time::Duration;

        let dir = tempfile::TempDir::new().unwrap();
        branch_meta::save_branch_meta(dir.path(), &branch_meta::BranchMeta::new("main")).unwrap();
        std::fs::write(dir.path().join("tokensave.db"), b"DBDATA").unwrap();
        let lock = crate::tokensave::acquire_branch_operation_lock(dir.path())
            .await
            .unwrap();

        let mut operation = tokio::spawn({
            let root = dir.path().to_path_buf();
            async move { track_branch_copy(&root, &root, "feature-x").await }
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut operation)
                .await
                .is_err(),
            "a concurrent branch copy must wait for the in-progress operation"
        );

        drop(lock);
        assert!(operation.await.unwrap().unwrap());
    }
}
