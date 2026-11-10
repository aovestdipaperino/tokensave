//! Moving a project between storage backends (#946).
//!
//! The backend is chosen at build time: a binary is either a libSQL build or
//! a Grafeo build, and neither can read the other's index. A switch
//! therefore happens in two steps, each run with `tokensave storage switch
//! <target>`:
//!
//! 1. **Prepare**, in a build of the current backend: the records a re-index
//!    cannot rebuild (session-memory decisions and code areas, from every
//!    index of the project) and the project configuration are written to a
//!    backend-neutral file, `storage-switch.json`, in the channel directory.
//!    The current index is not changed.
//! 2. **Finish**, in a build of the target backend: the target index is
//!    rebuilt with a full index, as `tokensave init` does, the saved records
//!    are added to it, and the old index is removed unless `--keep-old` was
//!    given.
//!
//! The old index stays untouched until the new one is complete, and the
//! switch file is removed only after the records are stored, so an
//! interrupted finish can be run again: it rebuilds the target from scratch.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::branch_meta;
use crate::config;
use crate::db::Database;
use crate::errors::{Result, TokenSaveError};
use crate::tokensave::{current_timestamp, TokenSave};
use crate::types::CodeAreaRecord;

/// Name of the switch file in the channel directory.
pub const HANDOFF_FILENAME: &str = "storage-switch.json";
/// Version of the switch file layout this build reads and writes.
const HANDOFF_FORMAT: u32 = 1;
/// Name of the default-branch index in a backend directory.
const INDEX_FILENAME: &str = "tokensave.db";
/// Name of the project configuration in a backend directory.
const CONFIG_FILENAME: &str = "config.json";
/// Name of the per-branch index directory in a backend directory.
const BRANCHES_DIRNAME: &str = "branches";
/// Name of the branch metadata in a backend directory.
const BRANCH_META_FILENAME: &str = "branch-meta.json";
/// Suffixes of the files and directories that belong to an index file:
/// SQLite's write-ahead log, shared memory and rollback journal, and
/// Grafeo's write-ahead-log directory.
const SIDECAR_SUFFIXES: &[&str] = &["-wal", "-shm", "-journal", ".wal"];

/// A project-index storage backend.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    /// libSQL, the default backend.
    Libsql,
    /// Grafeo, the experimental backend (`grafeo` cargo feature).
    Grafeo,
}

impl StorageBackend {
    /// The backend this build stores project indexes in.
    pub const fn current() -> Self {
        if cfg!(feature = "grafeo") {
            Self::Grafeo
        } else {
            Self::Libsql
        }
    }

    /// The other backend.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Libsql => Self::Grafeo,
            Self::Grafeo => Self::Libsql,
        }
    }

    /// The name used on the command line and in the switch file.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Libsql => "libsql",
            Self::Grafeo => "grafeo",
        }
    }

    /// The name used in messages.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Libsql => "libSQL",
            Self::Grafeo => "Grafeo",
        }
    }
}

impl std::fmt::Display for StorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returns the directory that holds `backend`'s project state.
pub fn index_dir(project_root: &Path, backend: StorageBackend) -> PathBuf {
    config::get_backend_dir(project_root, backend == StorageBackend::Grafeo)
}

/// Returns `backend`'s default-branch index file.
pub fn index_path(project_root: &Path, backend: StorageBackend) -> PathBuf {
    index_dir(project_root, backend).join(INDEX_FILENAME)
}

/// Returns the switch file of the project's channel.
pub fn handoff_path(project_root: &Path) -> PathBuf {
    config::get_channel_dir(project_root).join(HANDOFF_FILENAME)
}

/// A decision as stored in the switch file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SavedDecision {
    created_at: i64,
    text: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// A code area as stored in the switch file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SavedCodeArea {
    path: String,
    #[serde(default)]
    description: Option<String>,
    last_touched_at: i64,
    touch_count: u32,
}

/// The backend-neutral switch file.
#[derive(Debug, Serialize, Deserialize)]
struct Handoff {
    format: u32,
    from: StorageBackend,
    to: StorageBackend,
    created_at: i64,
    #[serde(default)]
    keep_old: bool,
    /// The source backend's `config.json`, verbatim.
    #[serde(default)]
    config: Option<serde_json::Value>,
    #[serde(default)]
    decisions: Vec<SavedDecision>,
    #[serde(default)]
    code_areas: Vec<SavedCodeArea>,
    /// Tracked branches other than the default, which are not carried over.
    #[serde(default)]
    dropped_branches: Vec<String>,
}

/// Records collected from several indexes, without duplicates.
///
/// A branch index starts as a copy of its parent's, so the same record is
/// often found in several indexes. Decisions are kept once per identical
/// content; a code area keeps its latest touch, the description that goes
/// with it, and the highest touch count seen.
#[derive(Debug, Default)]
struct Records {
    decisions: BTreeSet<SavedDecision>,
    code_areas: BTreeMap<String, SavedCodeArea>,
}

impl Records {
    fn add_decision(&mut self, decision: SavedDecision) {
        self.decisions.insert(decision);
    }

    fn add_code_area(&mut self, area: SavedCodeArea) {
        match self.code_areas.entry(area.path.clone()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(area);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                let kept = slot.get_mut();
                kept.touch_count = kept.touch_count.max(area.touch_count);
                if area.last_touched_at > kept.last_touched_at {
                    kept.last_touched_at = area.last_touched_at;
                    if area.description.is_some() {
                        kept.description = area.description;
                    }
                } else if kept.description.is_none() {
                    kept.description = area.description;
                }
            }
        }
    }

    /// Adds the records stored in `db`.
    async fn add_from(&mut self, db: &Database) -> Result<()> {
        // A negative limit means no limit in both backends.
        for decision in db.get_decisions(None, None, -1).await? {
            self.add_decision(SavedDecision {
                created_at: decision.created_at,
                text: decision.text,
                reason: decision.reason,
                files: decision.files,
                tags: decision.tags,
            });
        }
        for area in db.get_code_areas(-1).await? {
            self.add_code_area(SavedCodeArea {
                path: area.path,
                description: area.description,
                last_touched_at: area.last_touched_at,
                touch_count: area.touch_count,
            });
        }
        Ok(())
    }

    /// Reads the records of every index in `dir`. An index that cannot be
    /// opened fails the read: its records would otherwise be lost.
    async fn add_from_dir(&mut self, dir: &Path) -> Result<()> {
        for db_path in index_files(dir).0 {
            if !db_path.exists() {
                continue;
            }
            let db =
                Database::open_read_only(&db_path)
                    .await
                    .map_err(|e| TokenSaveError::Config {
                        message: format!(
                            "cannot read the index at '{}': {e}\n  Stop any tokensave process \
                         using this project (MCP servers included) and run the command again.",
                            db_path.display()
                        ),
                    })?;
            let result = self.add_from(&db).await;
            db.close();
            result?;
        }
        Ok(())
    }

    /// Stores the records in `db`, oldest decision first so their ids
    /// follow the order they were recorded in.
    async fn store(&self, db: &Database) -> Result<()> {
        for d in &self.decisions {
            db.insert_decision(
                &d.text,
                d.reason.as_deref(),
                &d.files,
                &d.tags,
                d.created_at,
            )
            .await?;
        }
        for area in self.code_areas.values() {
            db.import_code_area(&CodeAreaRecord {
                id: 0,
                path: area.path.clone(),
                description: area.description.clone(),
                last_touched_at: area.last_touched_at,
                touch_count: area.touch_count,
            })
            .await?;
        }
        Ok(())
    }
}

/// What `prepare` saved.
#[derive(Debug)]
pub struct PrepareReport {
    /// The backend the project is switching from (this build's).
    pub from: StorageBackend,
    /// The backend the project is switching to.
    pub to: StorageBackend,
    /// Decisions saved.
    pub decisions: usize,
    /// Code areas saved.
    pub code_areas: usize,
    /// Tracked branches other than the default, which the switch drops.
    pub dropped_branches: Vec<String>,
    /// The switch file written.
    pub handoff: PathBuf,
}

/// What `finish` did.
#[derive(Debug)]
pub struct FinishReport {
    /// The backend the project switched from.
    pub from: StorageBackend,
    /// The backend the project switched to (this build's).
    pub to: StorageBackend,
    /// Decisions stored in the new index.
    pub decisions: usize,
    /// Code areas stored in the new index.
    pub code_areas: usize,
    /// Files indexed.
    pub file_count: usize,
    /// Nodes indexed.
    pub node_count: usize,
    /// Edges indexed.
    pub edge_count: usize,
    /// Tracked branches the source index had besides the default.
    pub dropped_branches: Vec<String>,
    /// The old backend's directory when its index was kept.
    pub kept_old: Option<PathBuf>,
    /// Problems that did not fail the switch, such as an old index file
    /// another process still holds.
    pub warnings: Vec<String>,
}

/// The result of `tokensave storage switch`.
pub enum SwitchOutcome {
    /// This build saved the records for a build of the target backend.
    Prepared(PrepareReport),
    /// This build, of the target backend, completed the switch.
    Finished(FinishReport, Box<TokenSave>),
}

/// Runs `tokensave storage switch <target>`: prepares the switch when
/// `target` is another backend than this build's, finishes it otherwise.
/// `on_file(current, total, path)` reports the full index's progress.
pub async fn switch<F>(
    project_root: &Path,
    target: StorageBackend,
    keep_old: bool,
    on_file: F,
) -> Result<SwitchOutcome>
where
    F: Fn(usize, usize, &str),
{
    if target == StorageBackend::current() {
        let (report, tokensave) = finish(project_root, keep_old, on_file).await?;
        Ok(SwitchOutcome::Finished(report, Box::new(tokensave)))
    } else {
        Ok(SwitchOutcome::Prepared(
            prepare(project_root, target, keep_old).await?,
        ))
    }
}

/// Saves this build's non-derived records for a switch to `target`. The
/// current index is not changed.
pub async fn prepare(
    project_root: &Path,
    target: StorageBackend,
    keep_old: bool,
) -> Result<PrepareReport> {
    let from = StorageBackend::current();
    if target == from {
        return Err(TokenSaveError::Config {
            message: format!("this build already stores the index in {}", from.label()),
        });
    }
    let dir = index_dir(project_root, from);
    if !dir.join(INDEX_FILENAME).exists() {
        return Err(TokenSaveError::Config {
            message: format!(
                "no {} index found at '{}'; there is nothing to switch from",
                from.label(),
                dir.display()
            ),
        });
    }

    let mut records = Records::default();
    records.add_from_dir(&dir).await?;
    let (_, dropped_branches) = index_files(&dir);
    let config = read_config(&dir)?;

    let handoff = Handoff {
        format: HANDOFF_FORMAT,
        from,
        to: target,
        created_at: current_timestamp(),
        keep_old,
        config,
        decisions: records.decisions.iter().cloned().collect(),
        code_areas: records.code_areas.values().cloned().collect(),
        dropped_branches,
    };
    let path = handoff_path(project_root);
    write_handoff(&path, &handoff)?;
    Ok(PrepareReport {
        from,
        to: target,
        decisions: handoff.decisions.len(),
        code_areas: handoff.code_areas.len(),
        dropped_branches: handoff.dropped_branches,
        handoff: path,
    })
}

/// Completes a switch prepared for this build's backend: rebuilds this
/// backend's index with a full index, stores the saved records in it, and
/// removes the old backend's index unless `keep_old` is set here or was set
/// when the switch was prepared.
///
/// Records already in this backend's index (left from before an earlier
/// switch, or written while both builds were used side by side) are kept.
pub async fn finish<F>(
    project_root: &Path,
    keep_old: bool,
    on_file: F,
) -> Result<(FinishReport, TokenSave)>
where
    F: Fn(usize, usize, &str),
{
    let to = StorageBackend::current();
    let path = handoff_path(project_root);
    let Some(handoff) = read_handoff(&path)? else {
        return Err(no_switch_prepared(project_root, to));
    };
    if handoff.to != to {
        return Err(TokenSaveError::Config {
            message: format!(
                "the switch prepared in '{}' moves the project to {}, but this build uses {}. \
                 Finish it with a {} build, or delete that file to cancel it.",
                path.display(),
                handoff.to.label(),
                to.label(),
                handoff.to.label()
            ),
        });
    }

    let mut records = Records::default();
    for decision in handoff.decisions {
        records.add_decision(decision);
    }
    for area in handoff.code_areas {
        records.add_code_area(area);
    }
    let target_dir = index_dir(project_root, to);
    records.add_from_dir(&target_dir).await?;

    // The target index is stale or partial; the full index rebuilds it.
    remove_index(&target_dir).map_err(|e| TokenSaveError::Config {
        message: format!(
            "cannot remove the existing {} index in '{}': {e}",
            to.label(),
            target_dir.display()
        ),
    })?;
    if let Some(config) = &handoff.config {
        write_config(&target_dir, config)?;
    }

    let tokensave = TokenSave::init(project_root).await?;
    let result = tokensave.index_all_with_progress(on_file).await?;
    records.store(tokensave.db()).await?;

    let mut warnings = Vec::new();
    if let Err(e) = std::fs::remove_file(&path) {
        warnings.push(format!(
            "could not remove the switch file '{}': {e}; delete it by hand",
            path.display()
        ));
    }
    let old_dir = index_dir(project_root, handoff.from);
    let kept_old = if keep_old || handoff.keep_old {
        Some(old_dir)
    } else {
        if let Err(e) = remove_index(&old_dir) {
            warnings.push(format!(
                "could not remove the old {} index in '{}': {e}; remove it once no \
                 process uses it",
                handoff.from.label(),
                old_dir.display()
            ));
        }
        None
    };

    Ok((
        FinishReport {
            from: handoff.from,
            to,
            decisions: records.decisions.len(),
            code_areas: records.code_areas.len(),
            file_count: result.file_count,
            node_count: result.node_count,
            edge_count: result.edge_count,
            dropped_branches: handoff.dropped_branches,
            kept_old,
            warnings,
        },
        tokensave,
    ))
}

/// The error for a finish with no switch file.
fn no_switch_prepared(project_root: &Path, current: StorageBackend) -> TokenSaveError {
    let other = current.other();
    let message = if index_path(project_root, other).exists() {
        format!(
            "no switch to {current_label} is prepared. This build cannot read the {other_label} \
             index; first run `tokensave storage switch {current}` with a {other_label} build, \
             which saves the decisions and code areas, then run it again with this build",
            current_label = current.label(),
            other_label = other.label(),
        )
    } else {
        format!(
            "this project already uses {}; there is nothing to switch",
            current.label()
        )
    };
    TokenSaveError::Config { message }
}

/// Returns the index files in a backend directory (the default-branch
/// index and every tracked branch's), and the tracked branches other than
/// the default.
///
/// A branch entry whose path leaves the directory is ignored, so a damaged
/// branch metadata file can never name a file outside it.
fn index_files(dir: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let mut files = vec![dir.join(INDEX_FILENAME)];
    let mut dropped = Vec::new();
    if let Some(meta) = branch_meta::load_branch_meta(dir) {
        for (name, entry) in &meta.branches {
            if *name != meta.default_branch {
                dropped.push(name.clone());
            }
            let relative = Path::new(&entry.db_file);
            if !relative
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            {
                continue;
            }
            let file = dir.join(relative);
            if !files.contains(&file) {
                files.push(file);
            }
        }
    }
    dropped.sort();
    (files, dropped)
}

/// Removes every index in a backend directory, with their sidecars, the
/// branch directory and the branch metadata. The configuration is kept.
fn remove_index(dir: &Path) -> std::io::Result<()> {
    for file in index_files(dir).0 {
        remove_path(&file)?;
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for suffix in SIDECAR_SUFFIXES {
            remove_path(&file.with_file_name(format!("{name}{suffix}")))?;
        }
    }
    remove_path(&dir.join(BRANCHES_DIRNAME))?;
    remove_path(&dir.join(BRANCH_META_FILENAME))
}

/// Removes a file or directory if it exists.
fn remove_path(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn read_config(dir: &Path) -> Result<Option<serde_json::Value>> {
    let path = dir.join(CONFIG_FILENAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| TokenSaveError::Config {
            message: format!("cannot parse '{}': {e}", path.display()),
        })
}

fn write_config(dir: &Path, config: &serde_json::Value) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(config).map_err(|e| TokenSaveError::Config {
        message: format!("cannot serialize the project configuration: {e}"),
    })?;
    write_atomically(&dir.join(CONFIG_FILENAME), text.as_bytes())
}

fn read_handoff(path: &Path) -> Result<Option<Handoff>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let handoff: Handoff = serde_json::from_str(&text).map_err(|e| TokenSaveError::Config {
        message: format!(
            "cannot parse the switch file '{}': {e}. Prepare the switch again.",
            path.display()
        ),
    })?;
    if handoff.format != HANDOFF_FORMAT {
        return Err(TokenSaveError::Config {
            message: format!(
                "the switch file '{}' has format {}, but this build reads format \
                 {HANDOFF_FORMAT}. Prepare the switch again with a build of the same version.",
                path.display(),
                handoff.format
            ),
        });
    }
    Ok(Some(handoff))
}

fn write_handoff(path: &Path, handoff: &Handoff) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(handoff).map_err(|e| TokenSaveError::Config {
        message: format!("cannot serialize the switch file: {e}"),
    })?;
    write_atomically(path, text.as_bytes())
}

/// Writes `bytes` to a temporary file renamed over `path`, so a reader
/// never sees a partial file.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// The latest modification time of an index file and its sidecars.
fn index_mtime(file: &Path) -> Option<std::time::SystemTime> {
    let name = file.file_name()?.to_string_lossy().into_owned();
    std::iter::once(file.to_path_buf())
        .chain(
            SIDECAR_SUFFIXES
                .iter()
                .map(|suffix| file.with_file_name(format!("{name}{suffix}"))),
        )
        .filter_map(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok())
        .max()
}

/// Returns a one-line hint about the project's storage, or `None`:
///
/// - a switch was prepared and waits for a build of its target backend;
/// - the other backend's index is newer than this build's, so this build
///   would serve stale data and miss the records written since.
///
/// Reads file metadata only; it never changes anything.
pub fn storage_hint(project_root: &Path) -> Option<String> {
    let current = StorageBackend::current();
    if let Ok(Some(handoff)) = read_handoff(&handoff_path(project_root)) {
        return Some(if handoff.to == current {
            format!(
                "a switch from {} to {} is prepared; run `tokensave storage switch {current}` \
                 to finish it",
                handoff.from.label(),
                current.label()
            )
        } else {
            format!(
                "a switch to {} is prepared; finish it by running `tokensave storage switch {}` \
                 with a {} build",
                handoff.to.label(),
                handoff.to,
                handoff.to.label()
            )
        });
    }
    let other = current.other();
    let other_time = index_mtime(&index_path(project_root, other))?;
    let newer = index_mtime(&index_path(project_root, current)).is_none_or(|own| other_time > own);
    newer.then(|| {
        format!(
            "the {other_label} index of this project is newer than its {current_label} index; to \
             move its decisions and code areas here, run `tokensave storage switch {current}` \
             with a {other_label} build, then again with this build",
            other_label = other.label(),
            current_label = current.label(),
        )
    })
}

/// Prints [`storage_hint`] to stderr, at most once per process.
pub fn print_storage_hint_once(project_root: &Path) {
    static PRINTED: AtomicBool = AtomicBool::new(false);
    if PRINTED.load(Ordering::Relaxed) {
        return;
    }
    if let Some(hint) = storage_hint(project_root) {
        if !PRINTED.swap(true, Ordering::Relaxed) {
            eprintln!("[tokensave] hint: {hint}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(path: &str, touched: i64, count: u32, desc: Option<&str>) -> SavedCodeArea {
        SavedCodeArea {
            path: path.to_string(),
            description: desc.map(str::to_string),
            last_touched_at: touched,
            touch_count: count,
        }
    }

    #[test]
    fn duplicate_code_areas_keep_the_latest_touch_and_highest_count() {
        let mut records = Records::default();
        records.add_code_area(area("src/a.rs", 10, 5, Some("old")));
        records.add_code_area(area("src/a.rs", 20, 2, None));
        records.add_code_area(area("src/a.rs", 15, 7, Some("middle")));
        let kept = records.code_areas.get("src/a.rs");
        assert_eq!(kept, Some(&area("src/a.rs", 20, 7, Some("old"))));
    }

    #[test]
    fn identical_decisions_are_kept_once() {
        let decision = SavedDecision {
            created_at: 1,
            text: "use X".to_string(),
            reason: None,
            files: vec!["a.rs".to_string()],
            tags: Vec::new(),
        };
        let mut records = Records::default();
        records.add_decision(decision.clone());
        records.add_decision(decision);
        assert_eq!(records.decisions.len(), 1);
    }

    #[test]
    fn branch_entries_outside_the_directory_are_ignored() {
        let dir = tempfile::TempDir::new().map_err(|e| e.to_string());
        let Ok(dir) = dir else {
            return;
        };
        let mut meta = branch_meta::BranchMeta::new("main");
        meta.branches.insert(
            "evil".to_string(),
            branch_meta::BranchEntry {
                db_file: "../outside.db".to_string(),
                parent: Some("main".to_string()),
                created_at: String::new(),
                last_synced_at: String::new(),
            },
        );
        assert!(branch_meta::save_branch_meta(dir.path(), &meta).is_ok());
        let (files, dropped) = index_files(dir.path());
        assert_eq!(files, vec![dir.path().join(INDEX_FILENAME)]);
        assert_eq!(dropped, vec!["evil".to_string()]);
    }
}
