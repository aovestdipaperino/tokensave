//! Path scoping for the opt-in filesystem watcher (#937).
//!
//! The watcher itself lives in [`crate::mcp::fs_watch`]; this module answers
//! the questions that need the project's indexing rules: which directories may
//! be watched, and what a batch of changed paths means for the index. Both
//! reuse the scan walk's own rules, so a path the scan would never index is
//! never indexed because of a notification.
use super::indexing::ScanDirPrune;
use super::{
    get_tokensave_dir, is_excluded, is_excluded_dir, is_included, HashMap, HashSet, Path, PathBuf,
    TokenSave,
};

/// What a debounced batch of changed paths asks of the index.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WatchPlan {
    /// Project-relative files to pass to the incremental sync: modified or
    /// deleted files the index already records, and new in-scope files.
    pub files: Vec<String>,
    /// In-scope directories that appeared (created or renamed into place).
    /// A per-directory watcher registers them; their contents arrived with
    /// no event of their own, so they also set [`Self::reconcile`].
    pub new_dirs: Vec<PathBuf>,
    /// The batch cannot be applied path by path — a directory appeared or
    /// disappeared — and needs the bounded reconciliation scan instead.
    pub reconcile: bool,
}

/// One changed path from a notification batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedChange {
    /// The absolute path the notification named.
    pub path: PathBuf,
    /// Whether the path was created, removed or renamed, rather than only
    /// written to. Only structural changes can make a directory appear or
    /// disappear; a plain write to a directory is how some platforms report
    /// a change to one of its children.
    pub structural: bool,
}

/// Whether `name` is a temporary or backup file an editor or tool writes
/// next to the real one during a save. Such files never hold source and
/// vanish moments later, so the watcher drops them before any other check.
pub fn is_transient_file_name(name: &str) -> bool {
    const SUFFIXES: &[&str] = &[
        "~",
        ".swp",
        ".swo",
        ".swx",
        ".tmp",
        ".temp",
        ".bak",
        ".orig",
        ".part",
        ".crdownload",
        "___jb_tmp___",
        "___jb_old___",
    ];
    // Vim probes a directory's writability with a file named `4913`; Emacs
    // writes `.#name` lock links and `#name#` autosaves; GNOME writes
    // `.goutputstream-*`.
    name == "4913"
        || name.starts_with(".#")
        || (name.starts_with('#') && name.ends_with('#'))
        || name.starts_with(".goutputstream-")
        || SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// Whether the gitignore-aware walker, configured as the scan walk configures
/// it, yields `abs` from its parent directory. The walker reads ignore files
/// from parent directories too, so the root's `.gitignore`, nested ones,
/// `.git/info/exclude` and the global ignore file all apply exactly as they
/// do to a full scan. `cache` keeps one listing per parent directory.
fn passes_gitignore(abs: &Path, cache: &mut HashMap<PathBuf, HashSet<PathBuf>>) -> bool {
    let Some(parent) = abs.parent() else {
        return false;
    };
    let entries = cache.entry(parent.to_path_buf()).or_insert_with(|| {
        ignore::WalkBuilder::new(parent)
            .max_depth(Some(1))
            .follow_links(true)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .add_custom_ignore_filename(".gitignore")
            .build()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.depth() == 1)
            .map(ignore::DirEntry::into_path)
            .collect()
    });
    entries.contains(abs)
}

impl TokenSave {
    /// The project-relative, `/`-separated form of an absolute `path`
    /// reported by a notification, or `None` outside the project. Matches
    /// against the canonical root as well, since some backends (`FSEvents`)
    /// report resolved paths for a project opened through a symlink.
    pub(crate) fn watch_relative(&self, path: &Path) -> Option<String> {
        // `strip_prefix` is textual, so `root/../x` would pass it.
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return None;
        }
        let normalize = |rel: &Path| rel.to_string_lossy().replace('\\', "/");
        if let Ok(rel) = path.strip_prefix(&self.project_root) {
            return Some(normalize(rel));
        }
        let canonical = self.project_root.canonicalize().ok()?;
        path.strip_prefix(&canonical).ok().map(normalize)
    }

    /// Whether `rel` lies inside the git metadata directory or `TokenSave`'s
    /// own state directory. Writes there are `TokenSave`'s own database and
    /// lock traffic, or git's; reacting to them would feed back into itself.
    pub(crate) fn is_watch_internal(&self, rel: &str) -> bool {
        if rel.split('/').any(|part| part == ".git") {
            return true;
        }
        let state_dir = get_tokensave_dir(&self.project_root);
        let Ok(state_rel) = state_dir.strip_prefix(&self.project_root) else {
            return false;
        };
        let state_rel = state_rel.to_string_lossy().replace('\\', "/");
        !state_rel.is_empty() && (rel == state_rel || rel.starts_with(&format!("{state_rel}/")))
    }

    /// Whether every ancestor directory of `rel` (and `rel` itself when
    /// `include_self`) passes the scan walk's directory pruning: no exclude
    /// glob, no hidden directory without an include glob, no symlink back
    /// onto the root, and — when `git_ignore` is on — no ignored directory.
    ///
    /// The walk never descends into an ignored directory, so a path below
    /// one is out of scope even when no rule names the path itself.
    fn watch_ancestors_in_scope(
        &self,
        rel: &str,
        include_self: bool,
        gitignore_cache: &mut HashMap<PathBuf, HashSet<PathBuf>>,
    ) -> bool {
        let prune = ScanDirPrune::new(self);
        let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
        let dir_count = if include_self {
            parts.len()
        } else {
            parts.len().saturating_sub(1)
        };
        let mut abs = self.project_root.clone();
        for (depth, part) in parts.iter().take(dir_count).enumerate() {
            abs.push(part);
            let is_symlink = std::fs::symlink_metadata(&abs).is_ok_and(|m| m.is_symlink());
            if !prune.keeps(&abs, depth + 1, is_symlink) {
                return false;
            }
            if self.config.git_ignore {
                if !passes_gitignore(&abs, gitignore_cache) {
                    return false;
                }
            } else if *part == "target" {
                // The plain walk also skips `target/`.
                return false;
            }
        }
        true
    }

    /// Whether the scan walk would index the existing file at `rel`.
    ///
    /// `gitignore_cache` holds, per parent directory, the entries the
    /// gitignore-aware walker yields there, so a batch of new files in one
    /// directory reads its ignore files once.
    fn watch_file_in_scope(
        &self,
        rel: &str,
        abs: &Path,
        supported_exts: &[&str],
        gitignore_cache: &mut HashMap<PathBuf, HashSet<PathBuf>>,
    ) -> bool {
        let force_included = crate::config::is_force_included(rel, &self.config);
        if force_included {
            // A force-included file skips ignore rules but not exclude globs.
            let parent = rel.rsplit_once('/').map_or("", |(dir, _)| dir);
            let mut prefix = String::new();
            for part in parent.split('/').filter(|p| !p.is_empty()) {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(part);
                if is_excluded_dir(&prefix, &self.config) {
                    return false;
                }
            }
        } else {
            if !self.watch_ancestors_in_scope(rel, false, gitignore_cache) {
                return false;
            }
            let name = rel.rsplit('/').next().unwrap_or(rel);
            if name.starts_with('.') && !crate::config::is_dependency_lockfile(abs) {
                let manifest_allows = self
                    .manifest()
                    .is_some_and(|m| m.matches_local_file(rel) || m.local_dir_may_contain(rel));
                if !is_included(rel, &self.config) && !manifest_allows {
                    return false;
                }
            }
            if self.config.git_ignore && !passes_gitignore(abs, gitignore_cache) {
                return false;
            }
        }
        if is_excluded(rel, &self.config) {
            return false;
        }
        let mut skipped = HashMap::new();
        self.accept_file(abs, supported_exts, &mut skipped)
            .is_some()
    }

    /// The extensions the scan walk admits, owned so the caller can borrow
    /// them as `&str` the way [`Self::accept_file`] expects.
    fn watch_supported_extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = self
            .registry
            .supported_extensions()
            .into_iter()
            .map(str::to_string)
            .collect();
        exts.extend(self.artifact_extensions());
        if let Some(manifest) = self.manifest() {
            exts.extend(manifest.alias_extensions().map(str::to_string));
        }
        exts
    }

    /// The directories under `start` that the scan walk enters, `start`
    /// included when it is itself in scope, for a watcher that registers
    /// one non-recursive watch per directory.
    ///
    /// Returns `None` when more than `limit` directories qualify: the caller
    /// then does not watch at all rather than watching part of the tree.
    pub fn watch_directories(&self, start: &Path, limit: usize) -> Option<Vec<PathBuf>> {
        if start != self.project_root {
            let rel = self.watch_relative(start)?;
            if self.is_watch_internal(&rel)
                || !self.watch_ancestors_in_scope(&rel, true, &mut HashMap::new())
            {
                return Some(Vec::new());
            }
        }
        let prune = ScanDirPrune::new(self);
        let skip_target = !self.config.git_ignore;
        let git_ignore = self.config.git_ignore;
        let mut builder = ignore::WalkBuilder::new(start);
        builder
            .follow_links(true)
            .hidden(false)
            .git_ignore(git_ignore)
            .git_global(git_ignore)
            .git_exclude(git_ignore)
            .ignore(git_ignore)
            .parents(git_ignore);
        if git_ignore {
            builder.add_custom_ignore_filename(".gitignore");
        }
        let root = self.project_root.clone();
        builder.filter_entry(move |e| {
            if !e.file_type().is_some_and(|ft| ft.is_dir()) {
                return false;
            }
            if skip_target && e.depth() > 0 && e.file_name() == "target" {
                return false;
            }
            // Depth is measured from the project root, so the hidden-
            // directory rule applies to `start` itself when it is not the
            // root.
            let depth = e
                .path()
                .strip_prefix(&root)
                .map_or(e.depth(), |rel| rel.components().count());
            prune.keeps(e.path(), depth, e.path_is_symlink())
        });
        let mut dirs = Vec::new();
        for entry in builder.build() {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_some_and(|ft| ft.is_dir()) {
                continue;
            }
            if dirs.len() >= limit {
                return None;
            }
            dirs.push(entry.into_path());
        }
        Some(dirs)
    }

    /// Turns a debounced batch of changed paths into what the index needs.
    ///
    /// Internal and transient paths are dropped. An existing file is passed
    /// on when the index records it or the scan walk would index it; a
    /// missing file only when the index records it (a deletion to apply).
    /// An in-scope directory that appeared, or an extensionless missing path
    /// that may have been a directory, makes the batch a reconciliation.
    pub async fn plan_watch_batch(&self, changes: &[WatchedChange]) -> WatchPlan {
        let mut plan = WatchPlan::default();
        let exts = self.watch_supported_extensions();
        let ext_refs: Vec<&str> = exts.iter().map(String::as_str).collect();
        let mut gitignore_cache = HashMap::new();
        let mut seen = HashSet::new();
        for change in changes {
            let Some(rel) = self.watch_relative(&change.path) else {
                continue;
            };
            if rel.is_empty() || self.is_watch_internal(&rel) || !seen.insert(rel.clone()) {
                continue;
            }
            let name = rel.rsplit('/').next().unwrap_or(&rel);
            if is_transient_file_name(name) {
                continue;
            }
            let abs = self.project_root.join(&rel);
            let meta = std::fs::metadata(&abs);
            if meta.as_ref().is_ok_and(std::fs::Metadata::is_dir) {
                if change.structural
                    && self.watch_ancestors_in_scope(&rel, true, &mut gitignore_cache)
                {
                    plan.new_dirs.push(abs);
                    plan.reconcile = true;
                }
                continue;
            }
            let known = matches!(self.db.get_file(&rel).await, Ok(Some(_)));
            if meta.is_ok() {
                if known || self.watch_file_in_scope(&rel, &abs, &ext_refs, &mut gitignore_cache) {
                    plan.files.push(rel);
                }
            } else if known {
                plan.files.push(rel);
            } else if change.structural && Path::new(&rel).extension().is_none() {
                // Possibly a directory that was removed or renamed away: its
                // indexed files produced no event of their own.
                plan.reconcile = true;
            }
        }
        plan.files.sort();
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::is_transient_file_name;

    #[test]
    fn transient_names_are_recognised() {
        for name in [
            "main.rs~",
            ".main.rs.swp",
            "4913",
            ".#main.rs",
            "#main.rs#",
            "main.rs.tmp",
            "main.rs___jb_tmp___",
            ".goutputstream-ABC123",
        ] {
            assert!(is_transient_file_name(name), "{name}");
        }
        for name in [
            "main.rs",
            "Cargo.toml",
            "Cargo.lock",
            "lib.py",
            "README.md",
            "#include.h",
        ] {
            assert!(!is_transient_file_name(name), "{name}");
        }
    }
}
