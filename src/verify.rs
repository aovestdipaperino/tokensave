//! Read-only integrity checks. No migrations, indexing, or repair is performed.
//!
//! Rows are streamed; diagnostics retain at most 1,000 entries. Deep source
//! checks retain one file of at most 16 MiB. SQLite uses an 8 MiB page cache.
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::errors::{Result, TokenSaveError};

const MAX_DIAGNOSTICS: usize = 1_000;
const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: &'static str,
    pub entity: String,
    pub action: &'static str,
    pub repair_status: &'static str,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub completed: bool,
    pub checked_files: u64,
    pub findings: u64,
    pub omitted: u64,
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    fn add(
        &mut self,
        code: &'static str,
        severity: &'static str,
        entity: impl Into<String>,
        action: &'static str,
    ) {
        self.findings += 1;
        if self.diagnostics.len() == MAX_DIAGNOSTICS {
            self.omitted += 1;
            return;
        }
        self.diagnostics.push(Diagnostic {
            code,
            severity,
            entity: crate::text::utf8_prefix_at_or_before(&entity.into(), 1024).to_owned(),
            action,
            repair_status: "not_attempted",
        });
    }

    /// 0: clean, 1: findings (including source drift), 2: incomplete/unsupported.
    pub fn exit_code(&self) -> i32 {
        if self.completed {
            i32::from(self.findings != 0)
        } else {
            2
        }
    }
}

fn relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn database_path(root: &Path, report: &mut Report) -> Result<Option<PathBuf>> {
    let directory = root.join(".tokensave");
    let metadata = directory.join("branch-meta.json");
    if !metadata.exists() {
        return Ok(Some(directory.join("tokensave.db")));
    }
    let bytes = read_bounded(&metadata, 1024 * 1024)?;
    let meta: crate::branch_meta::BranchMeta = if let Ok(meta) = serde_json::from_slice(&bytes) {
        meta
    } else {
        report.add(
            "BRANCH_METADATA_INVALID",
            "error",
            "branch-meta.json",
            "Restore branch metadata from a known-good backup.",
        );
        return Ok(None);
    };
    if !meta.branches.contains_key(&meta.default_branch) {
        report.add(
            "BRANCH_DEFAULT_MISSING",
            "error",
            &meta.default_branch,
            "Restore the default branch entry from a known-good backup.",
        );
        return Ok(None);
    }
    let canonical_directory = directory.canonicalize()?;
    for (name, entry) in &meta.branches {
        if !relative(&entry.db_file) {
            report.add(
                "BRANCH_DATABASE_PATH_INVALID",
                "error",
                name,
                "Restore a database path within .tokensave.",
            );
            return Ok(None);
        }
        let path = directory.join(&entry.db_file);
        if !path.is_file() {
            report.add(
                "BRANCH_DATABASE_MISSING",
                "error",
                name,
                "Restore the missing branch database from backup.",
            );
        } else if !path.canonicalize()?.starts_with(&canonical_directory) {
            report.add(
                "BRANCH_DATABASE_PATH_INVALID",
                "error",
                name,
                "Restore a database path within .tokensave.",
            );
            return Ok(None);
        }
    }
    let current = crate::branch::current_branch(root);
    let selected = current
        .as_ref()
        .filter(|name| meta.branches.contains_key(*name))
        .unwrap_or(&meta.default_branch);
    if current.as_ref().is_some_and(|name| name != selected) {
        report.add("BRANCH_FALLBACK", "warning", selected, "Check out the indexed branch before comparing source files, or track the current branch.");
        // Comparing a different branch's files would report misleading source drift.
        return Ok(None);
    }
    Ok(Some(directory.join(&meta.branches[selected].db_file)))
}

fn read_bounded(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    // Reject special files before open: opening a FIFO could wait indefinitely.
    if !std::fs::metadata(path)?.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::other(
            "file exceeds verification size limit",
        ));
    }
    Ok(bytes)
}

/// Checks a project without executing normal CLI startup or opening a writable DB.
/// Repair is reserved but explicitly unsupported by this read-only foundation.
pub async fn verify(root: &Path, deep: bool, repair: bool) -> Report {
    let mut report = Report::default();
    if repair {
        report.add(
            "REPAIR_UNSUPPORTED",
            "error",
            "index",
            "Incremental repair is not implemented; inspect diagnostics without --repair.",
        );
        return report;
    }
    match inspect(root, deep, &mut report).await {
        Ok(completed) => report.completed = completed,
        Err(error) => report.add("VERIFY_FAILED", "error", error.to_string(), "Check permissions, concurrent writers, and database readability; preserve a backup before recovery."),
    }
    report
}

async fn inspect(root: &Path, deep: bool, report: &mut Report) -> Result<bool> {
    if !root.join(".tokensave/config.json").is_file() {
        report.add(
            "PROJECT_NOT_INITIALIZED",
            "error",
            root.display().to_string(),
            "Run tokensave init in the intended project.",
        );
        return Ok(false);
    }
    let Some(path) = database_path(root, report)? else {
        return Ok(false);
    };
    if !path.is_file() {
        report.add(
            "DATABASE_MISSING",
            "error",
            path.display().to_string(),
            "Restore the database from backup.",
        );
        return Ok(false);
    }
    let db = libsql::Builder::new_local(&path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await?;
    let conn = db.connect()?;
    conn.execute_batch("PRAGMA query_only=1; PRAGMA busy_timeout=1000; PRAGMA cache_size=-8192; PRAGMA mmap_size=0;").await?;
    // An owned read transaction gives every SQL check the same committed snapshot.
    // Dropping it rolls back on every return/error, without holding a writer lock.
    let tx = conn.transaction().await?;
    let mut rows = tx.query("PRAGMA user_version", ()).await?;
    let version: i64 = rows
        .next()
        .await?
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing schema version".into(),
        })?
        .get(0)?;
    drop(rows);
    if version != i64::from(crate::db::migrations::latest_version()) {
        report.add(
            "SCHEMA_INCOMPATIBLE",
            "error",
            version.to_string(),
            "Use a compatible Tokensave version; verification never migrates schemas.",
        );
        return Ok(false);
    }
    let pragma = if deep {
        "PRAGMA integrity_check"
    } else {
        "PRAGMA quick_check"
    };
    let mut rows = tx.query(pragma, ()).await?;
    let mut corrupt = false;
    while let Some(row) = rows.next().await? {
        let message: String = row.get(0)?;
        if message != "ok" {
            corrupt = true;
            report.add(
                "DATABASE_INTEGRITY",
                "error",
                message,
                "Preserve the database and restore from a known-good backup.",
            );
        }
    }
    drop(rows);
    if corrupt {
        return Ok(false);
    }
    for (code, sql) in [
        ("DANGLING_EDGE", "SELECT COUNT(*) FROM edges e WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=e.source) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=e.target)"),
        ("ORPHAN_NODE", "SELECT COUNT(*) FROM nodes n WHERE NOT EXISTS (SELECT 1 FROM files f WHERE f.path=n.file_path)"),
        ("DANGLING_PARENT", "SELECT COUNT(*) FROM nodes n WHERE n.parent_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM nodes p WHERE p.id=n.parent_id)"),
        ("DANGLING_REFERENCE", "SELECT COUNT(*) FROM unresolved_refs r WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=r.from_node_id)"),
        ("ORPHAN_VECTOR", "SELECT COUNT(*) FROM vectors v WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=v.node_id)"),
        ("ORPHAN_AMBIGUOUS_CALL", "SELECT COUNT(*) FROM ambiguous_calls a WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=a.from_node_id)"),
        ("INVALID_DISPATCH_RELATION", "SELECT COUNT(*) FROM trait_dispatch_callers d WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.concrete_method_id) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.trait_method_id) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.caller_id)"),
    ] {
        let mut rows = tx.query(sql, ()).await?;
        if let Some(row) = rows.next().await? {
            let count: i64 = row.get(0)?;
            if count != 0 {
                report.add(code, "error", format!("{count} records"), "Preserve a backup; targeted structural repair is not yet implemented.");
            }
        }
    }
    let canonical_root = root.canonicalize()?;
    let mut rows = tx
        .query(
            "SELECT path, content_hash, modified_at FROM files ORDER BY path",
            (),
        )
        .await?;
    while let Some(row) = rows.next().await? {
        report.checked_files += 1;
        let path: String = row.get(0)?;
        let hash: String = row.get(1)?;
        let modified: i64 = row.get(2)?;
        if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            report.add(
                "INVALID_FINGERPRINT",
                "error",
                &path,
                "Reindex the affected file after preserving a backup.",
            );
        }
        if !relative(&path) {
            report.add(
                "INVALID_FILE_PATH",
                "error",
                &path,
                "Restore a project-relative indexed file path.",
            );
            continue;
        }
        let absolute = root.join(&path);
        match absolute.canonicalize() {
            Ok(actual) if actual.starts_with(&canonical_root) => {}
            Ok(_) => {
                report.add(
                    "SOURCE_OUTSIDE_PROJECT",
                    "warning",
                    &path,
                    "Verify this symlink target manually.",
                );
                continue;
            }
            Err(error) => {
                let code = if error.kind() == std::io::ErrorKind::NotFound {
                    "SOURCE_MISSING"
                } else {
                    "SOURCE_UNREADABLE"
                };
                report.add(
                    code,
                    "warning",
                    &path,
                    "Run tokensave sync after confirming the source change.",
                );
                continue;
            }
        }
        if deep {
            match read_bounded(&absolute, MAX_SOURCE_BYTES)
                .and_then(|bytes| crate::sync::decode_source_bytes(&bytes))
            {
                Ok(source) if crate::sync::content_hash(&source) != hash => report.add(
                    "SOURCE_STALE",
                    "warning",
                    &path,
                    "Run tokensave sync; source drift is not database corruption.",
                ),
                Ok(_) => {}
                Err(_) => report.add(
                    "SOURCE_CHECK_SKIPPED",
                    "warning",
                    &path,
                    "Check readability, encoding, and the 16 MiB deep-check limit manually.",
                ),
            }
        } else if crate::sync::file_stat(&absolute).is_none_or(|(mtime, _)| mtime != modified) {
            report.add(
                "SOURCE_STALE",
                "warning",
                &path,
                "Run tokensave sync; source drift is not database corruption.",
            );
        }
    }
    Ok(true)
}
