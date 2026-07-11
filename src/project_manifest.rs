//! Optional `.tokensave/project.json` index manifest (#194).
//!
//! An explicit list of files/globs to index, each with an optional
//! `language` override forcing a specific extractor. Two things become
//! possible that `config.json` cannot express:
//!
//! - **Language overrides** — index `homedir/.bash_profile` or `*.shrc`
//!   as Bash even though extension-based dispatch would skip them.
//! - **External paths** — absolute or `~/…` entries opt in files outside
//!   the project root (e.g. real dotfiles under `$HOME`). These are
//!   stored in the graph under their resolved absolute path; they are
//!   opt-in, project-local, and should be treated as trusted input.
//!
//! `config.json` stays the walker/policy config (`exclude`, size limits,
//! `git_ignore`); `project.json` is additive: it never removes files the
//! normal walk would index.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use glob::{MatchOptions, Pattern};
use serde::{Deserialize, Serialize};

use crate::config::get_tokensave_dir;
use crate::errors::{Result, TokenSaveError};
use crate::extraction::LanguageRegistry;

/// Name of the index-manifest file stored inside the `.tokensave` directory.
pub const PROJECT_MANIFEST_FILENAME: &str = "project.json";

/// Raw on-disk shape of `.tokensave/project.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    /// Schema version of the manifest.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Explicit index entries.
    #[serde(default)]
    pub entries: Vec<ManifestEntry>,
}

fn default_version() -> u32 {
    1
}

/// One manifest entry: a path or glob, optionally forcing an extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// File, directory, or glob. Project-relative, absolute, or `~/…`.
    pub path: String,
    /// Optional language name forcing the extractor for matched files
    /// (e.g. `"bash"`). When omitted, extension dispatch applies.
    #[serde(default)]
    pub language: Option<String>,
    /// Free-form note; ignored.
    #[serde(default)]
    pub comment: Option<String>,
}

/// A single compiled pattern with its optional language override.
#[derive(Debug, Clone)]
struct CompiledEntry {
    /// Raw pattern text after `~` expansion, forward slashes.
    raw: String,
    pattern: Pattern,
    language: Option<String>,
}

/// Parsed and validated manifest, ready for matching.
#[derive(Debug, Clone, Default)]
pub struct CompiledManifest {
    /// Entries whose path is project-relative.
    local: Vec<CompiledEntry>,
    /// Entries whose path is absolute (including expanded `~/…`).
    external: Vec<CompiledEntry>,
}

/// Glob options matching the ones used for `config.json` include/exclude.
const MATCH_OPTS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: false,
    require_literal_leading_dot: false,
};

/// Returns the path to `project.json` within the `.tokensave` directory.
pub fn get_manifest_path(project_root: &Path) -> PathBuf {
    get_tokensave_dir(project_root).join(PROJECT_MANIFEST_FILENAME)
}

/// Expands a leading `~` or `~/` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if path == "~" {
        return dirs::home_dir()
            .map_or_else(|| path.to_string(), |h| h.to_string_lossy().to_string());
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.to_string_lossy().trim_end_matches('/'), rest);
        }
    }
    path.to_string()
}

impl CompiledManifest {
    /// Compiles a raw manifest, validating each entry.
    ///
    /// Unknown `language` values are a hard error (listing the valid names)
    /// so a typo fails the sync loudly instead of silently skipping files.
    pub fn compile(manifest: &ProjectManifest, registry: &LanguageRegistry) -> Result<Self> {
        let mut local = Vec::new();
        let mut external = Vec::new();
        for entry in &manifest.entries {
            if let Some(ref lang) = entry.language {
                if registry.extractor_for_language(lang).is_none() {
                    let mut known: Vec<&str> = registry
                        .extractors()
                        .iter()
                        .map(|e| e.language_name())
                        .collect();
                    known.sort_unstable();
                    known.dedup();
                    return Err(TokenSaveError::Config {
                        message: format!(
                            "project.json: unknown language '{}' for path '{}'; \
                             valid languages: {}",
                            lang,
                            entry.path,
                            known.join(", ")
                        ),
                    });
                }
            }
            let raw = expand_tilde(entry.path.trim()).replace('\\', "/");
            if raw.is_empty() || raw == "." {
                // "." documents the default walk; nothing to compile.
                continue;
            }
            let pattern = Pattern::new(&raw).map_err(|e| TokenSaveError::Config {
                message: format!("project.json: invalid glob '{}': {e}", entry.path),
            })?;
            let compiled = CompiledEntry {
                raw: raw.clone(),
                pattern,
                language: entry.language.clone(),
            };
            if Path::new(&raw).is_absolute() {
                external.push(compiled);
            } else {
                local.push(compiled);
            }
        }
        Ok(Self { local, external })
    }

    /// True when the manifest has no effective entries.
    pub fn is_empty(&self) -> bool {
        self.local.is_empty() && self.external.is_empty()
    }

    /// Language override for a graph path (project-relative for local files,
    /// absolute for external ones). First matching entry with a language wins.
    pub fn language_for(&self, path: &str) -> Option<&str> {
        self.local
            .iter()
            .chain(self.external.iter())
            .find(|e| e.language.is_some() && e.pattern.matches_with(path, MATCH_OPTS))
            .and_then(|e| e.language.as_deref())
    }

    /// True if a project-relative file path is explicitly listed, making it
    /// indexable even when its extension has no registered extractor and
    /// letting it pass the hidden-file filter.
    pub fn matches_local_file(&self, rel_path: &str) -> bool {
        self.local
            .iter()
            .any(|e| e.pattern.matches_with(rel_path, MATCH_OPTS))
    }

    /// True if a project-relative directory may contain manifest-listed
    /// files, so walkers don't prune it (relevant for hidden directories
    /// like `homedir/.bashrc.d`).
    pub fn local_dir_may_contain(&self, rel_dir: &str) -> bool {
        let prefix = format!("{}/", rel_dir.trim_end_matches('/'));
        self.local.iter().any(|e| {
            e.raw.starts_with(&prefix)
                || e.pattern.matches_with(rel_dir, MATCH_OPTS)
                || e.pattern.matches_with(&format!("{prefix}_"), MATCH_OPTS)
        })
    }

    /// Expands the external (absolute) entries into concrete file paths.
    ///
    /// Returns forward-slash absolute paths, capped at `max_file_size`.
    /// Missing paths and unreadable globs are skipped silently — dotfile
    /// sets legitimately differ between machines.
    pub fn expand_external_files(&self, max_file_size: u64) -> Vec<String> {
        let mut out = Vec::new();
        for entry in &self.external {
            let mut push = |p: &Path| {
                if let Ok(meta) = std::fs::metadata(p) {
                    if meta.is_file() && meta.len() <= max_file_size {
                        out.push(p.to_string_lossy().replace('\\', "/"));
                    }
                }
            };
            let has_glob_meta = entry.raw.contains(['*', '?', '[']);
            if has_glob_meta {
                if let Ok(paths) = glob::glob_with(&entry.raw, MATCH_OPTS) {
                    for p in paths.flatten() {
                        push(&p);
                    }
                }
            } else {
                push(Path::new(&entry.raw));
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Loads and compiles `.tokensave/project.json` for a project root.
///
/// Returns `Ok(None)` when the file does not exist. Parse errors, invalid
/// globs, and unknown languages are hard errors.
pub fn load_manifest(
    project_root: &Path,
    registry: &LanguageRegistry,
) -> Result<Option<CompiledManifest>> {
    let path = get_manifest_path(project_root);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let manifest: ProjectManifest =
        serde_json::from_str(&contents).map_err(|e| TokenSaveError::Config {
            message: format!("failed to parse '{}': {e}", path.display()),
        })?;
    Ok(Some(CompiledManifest::compile(&manifest, registry)?))
}

/// Cache slot: manifest keyed by file mtime so long-running processes pick
/// up edits without re-parsing on every file.
type CacheEntry = (Option<SystemTime>, Option<std::sync::Arc<CompiledManifest>>);

static MANIFEST_CACHE: Mutex<Option<std::collections::HashMap<PathBuf, CacheEntry>>> =
    Mutex::new(None);

/// Cached manifest lookup for hot paths (per-file extractor dispatch).
///
/// Errors degrade to "no manifest" here — the loud validation error is
/// surfaced by the sync entry points, which call [`load_manifest`] directly.
pub fn manifest_for(
    project_root: &Path,
    registry: &LanguageRegistry,
) -> Option<std::sync::Arc<CompiledManifest>> {
    let path = get_manifest_path(project_root);
    let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let mut guard = MANIFEST_CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(std::collections::HashMap::new);
    if let Some((cached_mtime, cached)) = cache.get(project_root) {
        if *cached_mtime == mtime {
            return cached.clone();
        }
    }
    let loaded = load_manifest(project_root, registry)
        .ok()
        .flatten()
        .filter(|m| !m.is_empty())
        .map(std::sync::Arc::new);
    cache.insert(project_root.to_path_buf(), (mtime, loaded.clone()));
    loaded
}

/// Resolves the extractor for a file, honoring a manifest language override
/// before falling back to extension-based dispatch.
pub fn resolve_extractor<'r>(
    registry: &'r LanguageRegistry,
    project_root: &Path,
    file_path: &str,
) -> Option<&'r dyn crate::extraction::LanguageExtractor> {
    if let Some(manifest) = manifest_for(project_root, registry) {
        if let Some(lang) = manifest.language_for(file_path) {
            return registry.extractor_for_language(lang);
        }
    }
    registry.extractor_for_file(file_path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn registry() -> LanguageRegistry {
        LanguageRegistry::new()
    }

    fn compile(entries: Vec<ManifestEntry>) -> CompiledManifest {
        CompiledManifest::compile(
            &ProjectManifest {
                version: 1,
                entries,
            },
            &registry(),
        )
        .unwrap()
    }

    fn entry(path: &str, language: Option<&str>) -> ManifestEntry {
        ManifestEntry {
            path: path.to_string(),
            language: language.map(str::to_string),
            comment: None,
        }
    }

    #[test]
    fn unknown_language_is_a_hard_error() {
        let m = ProjectManifest {
            version: 1,
            entries: vec![entry("foo/*.x", Some("klingon"))],
        };
        let err = CompiledManifest::compile(&m, &registry()).unwrap_err();
        assert!(err.to_string().contains("klingon"), "{err}");
        assert!(err.to_string().contains("valid languages"), "{err}");
    }

    #[test]
    fn language_override_matches_globs_and_literals() {
        let m = compile(vec![
            entry("homedir/.bash_profile", Some("bash")),
            entry("homedir/.bashrc.d/*.shrc", Some("bash")),
        ]);
        assert_eq!(m.language_for("homedir/.bash_profile"), Some("bash"));
        assert_eq!(
            m.language_for("homedir/.bashrc.d/prompt.shrc"),
            Some("bash")
        );
        assert_eq!(m.language_for("src/main.rs"), None);
    }

    #[test]
    fn local_file_and_dir_matching() {
        let m = compile(vec![entry("homedir/.bashrc.d/*.shrc", Some("bash"))]);
        assert!(m.matches_local_file("homedir/.bashrc.d/a.shrc"));
        assert!(!m.matches_local_file("homedir/.bashrc.d/a.txt"));
        assert!(m.local_dir_may_contain("homedir"));
        assert!(m.local_dir_may_contain("homedir/.bashrc.d"));
        assert!(!m.local_dir_may_contain("src"));
    }

    #[test]
    fn dot_entry_is_a_no_op() {
        let m = compile(vec![entry(".", None)]);
        assert!(m.is_empty());
    }

    #[test]
    fn resolve_extractor_honors_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".tokensave")).unwrap();
        std::fs::write(
            dir.path().join(".tokensave/project.json"),
            r#"{"version":1,"entries":[{"path":"dotfiles/.bash_profile","language":"bash"}]}"#,
        )
        .unwrap();
        let reg = registry();
        let e = resolve_extractor(&reg, dir.path(), "dotfiles/.bash_profile").unwrap();
        assert_eq!(e.language_name(), "Bash");
        // No override → normal dispatch still applies.
        assert!(resolve_extractor(&reg, dir.path(), "x.unknownext").is_none());
        assert!(resolve_extractor(&reg, dir.path(), "x.rs").is_some());
    }

    #[test]
    fn expand_external_files_finds_concrete_files() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join(".bash_profile");
        std::fs::write(&f, "export A=1\n").unwrap();
        let m = compile(vec![entry(&f.to_string_lossy(), Some("bash"))]);
        let files = m.expand_external_files(1_000_000);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".bash_profile"));
        assert_eq!(m.language_for(&files[0]), Some("bash"));
    }
}
