//! Shared, conservative garbage collection for per-branch databases.
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use crate::branch_meta::{load_branch_meta, save_branch_meta};
use crate::errors::{Result, TokenSaveError};

fn checked_out_branches(root: &Path) -> Option<HashSet<String>> {
    // Ask Git: linked worktrees, packed refs and reftable do not share a
    // loose-ref filesystem layout. Include locked and prunable worktrees.
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain", "-z"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = std::str::from_utf8(&output.stdout).ok()?;
    Some(
        text.split('\0')
            .filter_map(|field| field.strip_prefix("branch refs/heads/").map(str::to_owned))
            .collect(),
    )
}

fn identity(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Only standalone files directly in our branches directory are collectible.
/// Old or hand-edited metadata must never authorize deleting arbitrary paths.
fn collectible_path(dir: &Path, file: &str) -> Option<PathBuf> {
    let path = Path::new(file);
    let mut components = path.components();
    if components.next() != Some(Component::Normal("branches".as_ref()))
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || path.extension()? != "db"
    {
        return None;
    }
    let branches = dir.join("branches");
    match std::fs::symlink_metadata(&branches) {
        Ok(metadata) if metadata.file_type().is_symlink() => return None,
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return None,
        _ => {}
    }
    let db = dir.join(path);
    if std::fs::symlink_metadata(&db).is_ok_and(|m| m.file_type().is_symlink()) {
        return None;
    }
    Some(db)
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Remove indexes for branches Git has deleted. Explicit and automatic GC
/// share this implementation. `protected` includes the caller's open DB,
/// which can be an ancestor fallback even when its branch no longer exists.
///
/// Serialize with branch creation and sync. If Git cannot be queried, fail
/// closed. Never delete default, checked-out, shared, or served databases.
pub async fn collect_stale_indexes(root: &Path, protected: &[PathBuf]) -> Result<Vec<String>> {
    let dir = crate::config::get_tokensave_dir(root);
    if load_branch_meta(&dir).is_none() {
        return Ok(Vec::new());
    }
    let _branch_lock = crate::tokensave::acquire_branch_operation_lock(&dir).await?;
    let _sync_lock = crate::tokensave::try_acquire_sync_lock(root)?;
    let Some(mut meta) = load_branch_meta(&dir) else {
        return Ok(Vec::new());
    };
    let unavailable = || TokenSaveError::Config {
        message: format!(
            "cannot list branches or worktrees in '{}' — refusing to delete any branch DB",
            root.display()
        ),
    };
    let mut live = super::local_branches(root).ok_or_else(unavailable)?;
    live.extend(checked_out_branches(root).ok_or_else(unavailable)?);
    if let Some(current) = super::current_branch(root) {
        live.insert(current);
    }
    let mut protected: HashSet<_> = protected.iter().map(|p| identity(p)).collect();
    protected.extend(
        crate::servers::reap()
            .into_iter()
            .map(|server| identity(Path::new(&server.db_path))),
    );
    let mut candidates: Vec<_> = meta
        .branches
        .keys()
        .filter(|name| **name != meta.default_branch && !live.contains(*name))
        .cloned()
        .collect();
    candidates.sort();
    let mut removed = Vec::new();
    for name in candidates {
        let entry = &meta.branches[&name];
        let Some(db) = collectible_path(&dir, &entry.db_file) else {
            continue;
        };
        if protected.contains(&identity(&db))
            || meta.branches.iter().any(|(other, e)| {
                other != &name && identity(&dir.join(&e.db_file)) == identity(&db)
            })
        {
            continue;
        }
        // Git clients do not take TokenSave's locks; recheck immediately
        // before unlinking, rather than relying on the initial snapshot.
        if super::local_branches(root)
            .ok_or_else(unavailable)?
            .contains(&name)
            || checked_out_branches(root)
                .ok_or_else(unavailable)?
                .contains(&name)
        {
            continue;
        }
        remove_if_present(&db.with_extension("db-wal"))?;
        remove_if_present(&db.with_extension("db-shm"))?;
        remove_if_present(&db)?;
        meta.remove_branch(&name);
        // Persist each successful removal, so a later filesystem failure
        // cannot leave all earlier deletions recorded as still tracked.
        save_branch_meta(&dir, &meta)?;
        removed.push(name);
    }
    Ok(removed)
}

/// Best-effort maintenance: disabled on old configs unless explicitly enabled.
/// Reload configuration so long-lived MCP servers honor changes to the knob.
pub async fn collect_if_enabled(root: &Path, protected: &[PathBuf]) {
    let result = async {
        let config = crate::config::load_config(root)?;
        if !crate::config::env_bool_override("TOKENSAVE_AUTO_GC", config.auto_gc) {
            return Ok(Vec::new());
        }
        collect_stale_indexes(root, protected).await
    }
    .await;
    match result {
        Ok(removed) if !removed.is_empty() => {
            tracing::info!(?removed, "cleaned stale branch indexes");
        }
        Err(error) => tracing::warn!(%error, "automatic branch cleanup skipped"),
        _ => {}
    }
}
