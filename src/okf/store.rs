//! Imported OKF bundles, kept in the project state directory (#858).
//!
//! `tokensave okf import` copies a bundle byte for byte into
//! `<state dir>/okf/<name>/bundle/` and records how it was imported in
//! `<state dir>/okf/<name>/import.json`. The stored copy is itself an OKF
//! bundle, so imported knowledge stays a portable interchange layer that is
//! independent of the code-graph database: importing never changes the
//! database schema or forces a reindex.
//!
//! A bundle is staged in a temporary directory beside the store and moved
//! into place only when complete. Replacing a bundle moves the old copy aside
//! first and restores it if the new copy cannot be moved into place.

use std::io::Read as _;
use std::path::Path;
use std::path::PathBuf;

use super::bundle;
use super::bundle::Content;
use super::bundle::Limits;
use super::bundle::Role;
use super::time;
use super::validate;
use crate::errors::Result;
use crate::errors::TokenSaveError;

/// Directory of the project state directory that holds imported bundles.
pub const STORE_DIR: &str = "okf";
/// Directory of an imported bundle that holds the copied files.
pub const BUNDLE_DIR: &str = "bundle";
/// File beside [`BUNDLE_DIR`] that records the import.
pub const RECORD_FILE: &str = "import.json";
/// Version of the [`ImportRecord`] layout.
pub const RECORD_FORMAT: u32 = 1;
/// Longest bundle name accepted.
pub const MAX_NAME_LEN: usize = 64;

const MAX_RECORD_BYTES: u64 = 64 * 1024;

/// Resource limits for importing and loading imported bundles. Lower than
/// the validator's defaults, because imported bundles are read again by the
/// MCP server.
pub fn import_limits() -> Limits {
    Limits {
        max_entries: 50_000,
        max_file_bytes: 4 * 1024 * 1024,
        max_total_bytes: 64 * 1024 * 1024,
    }
}

/// How a bundle was imported, stored as `import.json`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImportRecord {
    /// Layout version of this record ([`RECORD_FORMAT`]).
    pub format: u32,
    pub name: String,
    /// The source directory as given on the command line.
    pub source: String,
    /// ISO 8601 UTC time of the import.
    pub imported_at: String,
    /// `tokensave/<version>` that imported the bundle.
    pub imported_by: String,
    /// The bundle's root `okf_version`, when declared.
    pub declared_version: Option<String>,
    pub conformant: bool,
    pub errors: usize,
    pub warnings: usize,
    /// Concept files with a readable, non-empty `type`.
    pub concepts: usize,
    /// Concept files without one; they are kept but not searchable.
    pub unreadable_concepts: usize,
    /// Regular files copied, markdown or not.
    pub files: usize,
    pub bytes: u64,
}

/// Options for [`import`].
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Bundle name; derived from the source directory name when `None`.
    pub name: Option<String>,
    /// Replace an imported bundle of the same name.
    pub replace: bool,
    /// Refuse bundles that are not conformant.
    pub strict: bool,
    pub limits: Limits,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            name: None,
            replace: false,
            strict: false,
            limits: import_limits(),
        }
    }
}

/// The result of an import: the record and the validation of the stored copy.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportOutcome {
    pub record: ImportRecord,
    pub report: validate::Report,
    /// Files of the source that were not copied, with the reason.
    pub skipped: Vec<String>,
    /// Whether an imported bundle of the same name was replaced.
    pub replaced: bool,
}

fn invalid(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

/// The directory that holds the imported bundles of `project_root`.
pub fn store_dir(project_root: &Path) -> PathBuf {
    crate::config::get_tokensave_dir(project_root).join(STORE_DIR)
}

/// Whether `name` is a valid bundle name: `[A-Za-z0-9_][A-Za-z0-9_.-]*`, at
/// most [`MAX_NAME_LEN`] bytes, not ending in `.` and not a reserved Windows
/// device name.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        && !name.ends_with('.')
        && super::export::safe_segment(name, false) == name
}

/// A valid bundle name derived from a directory name.
fn derive_name(source: &Path) -> String {
    let base = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut name = super::export::safe_segment(&base, false);
    name.truncate(MAX_NAME_LEN);
    while name.ends_with('.') {
        name.pop();
    }
    if is_valid_name(&name) {
        name
    } else {
        "bundle".to_string()
    }
}

fn check_name(name: &str) -> Result<()> {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(invalid(format!(
            "invalid bundle name `{name}`: use 1 to {MAX_NAME_LEN} characters from \
             [A-Za-z0-9_.-], starting with a letter, digit or `_`"
        )))
    }
}

/// Fails unless `project_root` has a tokensave state directory.
fn require_initialized(project_root: &Path) -> Result<()> {
    if crate::config::get_tokensave_dir(project_root).is_dir() {
        Ok(())
    } else {
        Err(invalid(
            "project is not initialized; run `tokensave init` first",
        ))
    }
}

fn read_capped(path: &Path, max: u64) -> std::io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= max).then_some(bytes))
}

/// Copies the regular files and directories the reader saw from `source`
/// into `target`. Returns the number of files, bytes and skipped entries.
fn copy_entries(
    source: &Path,
    target: &Path,
    entries: &std::collections::BTreeSet<String>,
    limits: Limits,
) -> Result<(usize, u64, Vec<String>)> {
    std::fs::create_dir_all(target)?;
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut skipped = Vec::new();
    for rel in entries {
        if crate::cancel::is_requested() {
            return Err(invalid("import cancelled"));
        }
        if let Some(dir) = rel.strip_suffix('/') {
            std::fs::create_dir_all(bundle::join(target, dir))?;
            continue;
        }
        let from = bundle::join(source, rel);
        // The reader skipped symbolic links; recheck in case the tree changed.
        let is_file = std::fs::symlink_metadata(&from).is_ok_and(|m| m.file_type().is_file());
        if !is_file {
            skipped.push(format!("{rel} (not a regular file)"));
            continue;
        }
        let Some(content) = read_capped(&from, limits.max_file_bytes)? else {
            skipped.push(format!(
                "{rel} (larger than {} bytes)",
                limits.max_file_bytes
            ));
            continue;
        };
        bytes = bytes.saturating_add(content.len() as u64);
        if bytes > limits.max_total_bytes {
            return Err(invalid(format!(
                "bundle is larger than the import limit of {} bytes",
                limits.max_total_bytes
            )));
        }
        let to = bundle::join(target, rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&to, content)?;
        files += 1;
    }
    Ok((files, bytes, skipped))
}

fn skip_reason(reason: &bundle::SkipReason) -> &'static str {
    match reason {
        bundle::SkipReason::Symlink => "symbolic link",
        bundle::SkipReason::HiddenDirectory => "hidden directory",
        bundle::SkipReason::TooLarge => "too large",
        bundle::SkipReason::Unreadable => "unreadable",
    }
}

/// Counts concepts that have a readable, non-empty `type`, and those that
/// do not.
fn count_concepts(bundle: &bundle::Bundle) -> (usize, usize) {
    let mut readable = 0usize;
    let mut unreadable = 0usize;
    for file in bundle.files.iter().filter(|f| f.role == Role::Concept) {
        let has_type = matches!(&file.content, Content::Parsed { .. })
            && file
                .frontmatter()
                .and_then(|fm| fm.get("type"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|t| !t.trim().is_empty());
        if has_type {
            readable += 1;
        } else {
            unreadable += 1;
        }
    }
    (readable, unreadable)
}

/// Imports the bundle at `source` into the store of `project_root`.
///
/// The bundle is copied as read, then the copy is validated. A bundle that
/// is not conformant is still imported, as OKF §12 asks consumers to read
/// best-effort, unless `options.strict` is set. Concepts without a readable
/// `type` are kept in the copy but are not searchable.
///
/// # Errors
/// Fails when the project is not initialized, the name is invalid or taken
/// (without `replace`), a limit is exceeded, the bundle is refused under
/// `strict`, or a file operation fails.
pub fn import(
    project_root: &Path,
    source: &Path,
    options: &ImportOptions,
) -> Result<ImportOutcome> {
    require_initialized(project_root)?;
    let source_bundle = bundle::read(source, options.limits)?;
    if !source_bundle.complete {
        return Err(invalid(format!(
            "bundle exceeds the import limits ({} entries, {} bytes per file, {} bytes in \
             total) or the import was cancelled",
            options.limits.max_entries,
            options.limits.max_file_bytes,
            options.limits.max_total_bytes
        )));
    }
    let name = match &options.name {
        Some(name) => {
            check_name(name)?;
            name.clone()
        }
        None => derive_name(
            &source
                .canonicalize()
                .unwrap_or_else(|_| source.to_path_buf()),
        ),
    };
    let store = store_dir(project_root);
    std::fs::create_dir_all(&store)?;
    let target = store.join(&name);
    let exists = target.exists();
    if exists && !options.replace {
        return Err(invalid(format!(
            "a bundle named `{name}` is already imported; pass --replace or choose \
             another --name"
        )));
    }

    let staging = tempfile::Builder::new()
        .prefix(".import-")
        .tempdir_in(&store)?;
    let staged_bundle = staging.path().join(BUNDLE_DIR);
    let (files, bytes, mut skipped) = copy_entries(
        source,
        &staged_bundle,
        &source_bundle.entries,
        options.limits,
    )?;
    skipped.extend(
        source_bundle
            .skipped
            .iter()
            .map(|s| format!("{} ({})", s.path, skip_reason(&s.reason))),
    );

    let stored = bundle::read(&staged_bundle, options.limits)?;
    let report = validate::validate(&stored, time::now());
    if options.strict && !report.conformant {
        return Err(invalid(format!(
            "bundle is not conformant ({} errors); run `tokensave okf validate` for \
             details, or import without --strict",
            report.errors
        )));
    }
    let (concepts, unreadable_concepts) = count_concepts(&stored);
    let record = ImportRecord {
        format: RECORD_FORMAT,
        name: name.clone(),
        source: source.display().to_string(),
        imported_at: time::format_utc(time::now()),
        imported_by: format!("tokensave/{}", env!("CARGO_PKG_VERSION")),
        declared_version: report.declared_version.clone(),
        conformant: report.conformant,
        errors: report.errors,
        warnings: report.warnings,
        concepts,
        unreadable_concepts,
        files,
        bytes,
    };
    std::fs::write(
        staging.path().join(RECORD_FILE),
        serde_json::to_vec_pretty(&record)?,
    )?;

    publish(&store, staging, &target, exists)?;
    Ok(ImportOutcome {
        record,
        report,
        skipped,
        replaced: exists,
    })
}

/// Moves the staged bundle to `target`, replacing an existing one.
fn publish(store: &Path, staging: tempfile::TempDir, target: &Path, replace: bool) -> Result<()> {
    let aside = if replace {
        let holder = tempfile::Builder::new()
            .prefix(".replaced-")
            .tempdir_in(store)?;
        let aside = holder.path().join(BUNDLE_DIR);
        std::fs::rename(target, &aside)?;
        Some((holder, aside))
    } else {
        None
    };
    let staged = staging.keep();
    if let Err(e) = std::fs::rename(&staged, target) {
        // Restore the previous bundle and drop the staged copy.
        if let Some((_, aside)) = &aside {
            let _ = std::fs::rename(aside, target);
        }
        let _ = std::fs::remove_dir_all(&staged);
        return Err(e.into());
    }
    // `holder` removes the replaced bundle when dropped.
    drop(aside);
    Ok(())
}

fn read_record(dir: &Path) -> Option<ImportRecord> {
    let bytes = read_capped(&dir.join(RECORD_FILE), MAX_RECORD_BYTES).ok()??;
    let record: ImportRecord = serde_json::from_slice(&bytes).ok()?;
    (record.format == RECORD_FORMAT).then_some(record)
}

/// One imported bundle found in the store.
#[derive(Debug, Clone)]
pub struct StoredBundle {
    pub record: ImportRecord,
    /// Directory holding the copied bundle files.
    pub bundle_dir: PathBuf,
    /// Modification time and size of `import.json`, which change on every
    /// import; used to detect a replaced bundle.
    pub stamp: (Option<std::time::SystemTime>, u64),
}

/// The imported bundles of `project_root`, by name. Entries without a
/// readable `import.json` (for example an interrupted import) are skipped.
///
/// # Errors
/// Fails only when the store exists but cannot be listed.
pub fn list(project_root: &Path) -> Result<Vec<StoredBundle>> {
    let store = store_dir(project_root);
    let entries = match std::fs::read_dir(&store) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_valid_name(&name) || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let dir = entry.path();
        let Some(record) = read_record(&dir) else {
            continue;
        };
        if record.name != name {
            continue;
        }
        let stamp = std::fs::metadata(dir.join(RECORD_FILE))
            .map_or((None, 0), |m| (m.modified().ok(), m.len()));
        out.push(StoredBundle {
            record,
            bundle_dir: dir.join(BUNDLE_DIR),
            stamp,
        });
    }
    out.sort_by(|a, b| a.record.name.cmp(&b.record.name));
    Ok(out)
}

/// Removes the imported bundle `name`. Returns `false` when there is none.
///
/// # Errors
/// Fails when the name is invalid or the bundle cannot be removed.
pub fn remove(project_root: &Path, name: &str) -> Result<bool> {
    check_name(name)?;
    let store = store_dir(project_root);
    let target = store.join(name);
    if !target.is_dir() {
        return Ok(false);
    }
    // Move it aside first so a concurrent reader never sees half a bundle.
    let holder = tempfile::Builder::new()
        .prefix(".removed-")
        .tempdir_in(&store)?;
    std::fs::rename(&target, holder.path().join(BUNDLE_DIR))?;
    holder.close()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_portable_identifiers() {
        assert!(is_valid_name("acme_retail"));
        assert!(is_valid_name("v0.2-docs"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name(".hidden"));
        assert!(!is_valid_name("-dash"));
        assert!(!is_valid_name("a/b"));
        assert!(!is_valid_name("trailing."));
        assert!(!is_valid_name("con"));
        assert!(!is_valid_name(&"x".repeat(MAX_NAME_LEN + 1)));
    }

    #[test]
    fn derived_names_are_valid() {
        assert_eq!(derive_name(Path::new("/tmp/acme retail")), "acme_retail");
        assert_eq!(derive_name(Path::new("/tmp/.vault")), "_.vault");
        assert_eq!(derive_name(Path::new("/tmp/nul")), "nul_");
        assert_eq!(derive_name(Path::new("/")), "_");
    }
}
