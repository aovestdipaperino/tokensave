//! `tokensave purge`: plan and remove tokensave-owned state (#1055).
//!
//! `tokensave uninstall` disconnects agent integrations and tokensave's git
//! hooks and keeps every index and setting. `purge` does that cleanup as well
//! and then deletes tokensave's own settings, caches and indexes, within a
//! scope the caller states explicitly:
//!
//! * global (the default): agent integrations, global git hooks, and the
//!   settings and caches under `~/.tokensave`;
//! * project (`--local`, `--project`): one or more projects' index
//!   directories, their project-scoped agent config and repository hooks;
//! * every registered project (`--all-projects`), on top of the global scope.
//!
//! The token-savings history in the global database cannot be rebuilt, so it
//! is deleted only with `--include-history`.
//!
//! Purge is plan-first. [`plan`] reads the filesystem and decides every path,
//! with the evidence that it belongs to tokensave; [`execute_removals`] then
//! deletes exactly those paths, re-checking each one first. `--dry-run`
//! prints the same plan and stops, so what it shows is what a real run does.
//!
//! Only entries tokensave recognizes are removed (see [`constants`]):
//! anything else in a state directory is reported and left in place. Symbolic
//! links and junctions are never followed, a project's state directory must
//! sit where tokensave puts it and hold an index, and paths from the global
//! registry get the same checks as paths typed on the command line. The
//! executable is never touched.

pub mod constants;
pub mod provenance;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use constants::{
    ATOMIC_TMP_INFIX, BENCHMARK_FILENAME, BETA_DIRNAME, GLOBAL_DB_FILENAME, GLOBAL_DB_SIDECARS,
    INSTALLED_AGENTS_KEY, LOCKED_SETTINGS, LOCK_SUFFIX, MAX_SIZE_DEPTH, MEMORY_MMAP_FILENAME,
    MONITOR_MMAP_FILENAME, PRICING_FILENAME, PROFILE_DIRNAME, PROJECT_OWNERSHIP_MARKERS,
    SERVERS_DIRNAME, SIZE_STEP, SIZE_UNITS, STATE_BASES_FILENAME, STATE_FILENAME,
    USER_CONFIG_FILENAME, WINDOWS_FILE_IN_USE,
};

/// What kind of data a planned removal holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// User settings and machine-local state; recreated with defaults.
    Settings,
    /// Caches and runtime files that tokensave rebuilds on demand.
    Cache,
    /// The global database: token-savings history and the project registry.
    /// Cannot be rebuilt.
    UsageHistory,
    /// A project's index directory; rebuilt by `tokensave init`.
    ProjectIndex,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Cache => "cache",
            Self::UsageHistory => "usage history",
            Self::ProjectIndex => "project index",
        }
    }
}

/// One path purge will delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub path: PathBuf,
    pub category: Category,
    /// Why this path is recognized as tokensave's.
    pub evidence: String,
    /// Approximate size in bytes (symbolic links are not followed).
    pub bytes: u64,
    /// A direct child to keep when removing a directory: the beta channel's
    /// state inside a stable state directory.
    pub keep_child: Option<&'static str>,
}

/// A path purge looked at and will not delete, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub path: PathBuf,
    pub reason: String,
}

/// A project named for purging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTarget {
    pub root: PathBuf,
    /// The path came from the global registry rather than the command line.
    pub registered: bool,
}

/// What the caller asked to purge.
#[derive(Debug, Clone)]
pub struct PurgeRequest {
    pub home: PathBuf,
    /// Agent integrations, global git hooks and the global settings/caches.
    pub global: bool,
    /// Also delete the global database (token-savings history).
    pub include_history: bool,
    /// The projects whose state to purge.
    pub projects: Vec<ProjectTarget>,
    /// `projects` is every registered project, so records that only exist to
    /// find project state (`state-bases.json`) can go too.
    pub all_projects: bool,
    /// Agents whose tokensave configuration to remove.
    pub agents: Vec<String>,
}

/// Everything a purge will do, decided before anything changes.
#[derive(Debug, Clone, Default)]
pub struct PurgePlan {
    pub removals: Vec<Removal>,
    /// Paths deliberately kept, such as history without `--include-history`.
    pub kept: Vec<Note>,
    /// Paths refused because they could not be verified as tokensave's.
    pub skipped: Vec<Note>,
    /// Targets that have nothing to remove.
    pub absent: Vec<PathBuf>,
    /// Agents whose global tokensave configuration will be removed.
    pub global_agents: Vec<String>,
    /// Whether tokensave's global git hooks are installed and will be removed.
    pub global_hooks: bool,
    /// Projects whose project-scoped agent config and repository hooks will
    /// be cleaned.
    pub project_roots: Vec<PathBuf>,
    /// Agents whose project-scoped configuration is cleaned in each project.
    pub local_agents: Vec<String>,
    /// Registry rows to drop when the global database itself is kept.
    pub registry_rows: Vec<String>,
    /// Directories to remove afterwards if the purge left them empty.
    pub prune_if_empty: Vec<PathBuf>,
    /// User-facing notes about side effects (for example, settings reset).
    pub notes: Vec<String>,
}

impl PurgePlan {
    /// Total size of the planned removals.
    pub fn total_bytes(&self) -> u64 {
        self.removals
            .iter()
            .fold(0u64, |acc, r| acc.saturating_add(r.bytes))
    }

    /// Whether the plan changes nothing at all.
    pub fn is_empty(&self) -> bool {
        self.removals.is_empty()
            && self.global_agents.is_empty()
            && !self.global_hooks
            && self.project_roots.is_empty()
            && self.registry_rows.is_empty()
    }
}

/// Builds the plan for `req`. Reads the filesystem; writes nothing.
pub fn plan(req: &PurgeRequest) -> PurgePlan {
    let mut plan = PurgePlan::default();
    let shared = req.home.join(crate::config::TOKENSAVE_DIR);
    if req.global {
        plan.global_agents.clone_from(&req.agents);
        plan.global_hooks = crate::agents::global_git_hooks_installed();
        plan_global(req, &shared, &mut plan);
    }
    let history_deleted = plan
        .removals
        .iter()
        .any(|r| r.category == Category::UsageHistory);
    for target in &req.projects {
        plan_project(target, &shared, history_deleted, &mut plan);
    }
    if !plan.project_roots.is_empty() {
        req.agents.clone_into(&mut plan.local_agents);
    }
    plan
}

/// Plans the global state directory. On the stable channel that is
/// `~/.tokensave` minus the beta subdirectory; on the beta channel it is
/// `~/.tokensave/beta`, and the settings and caches shared with stable stay.
fn plan_global(req: &PurgeRequest, shared: &Path, plan: &mut PurgePlan) {
    let beta = crate::cloud::is_beta();
    let channel = crate::config::get_global_tokensave_dir(&req.home);
    if beta {
        plan.kept.push(Note {
            path: shared.to_path_buf(),
            reason: "settings and caches here are shared with the stable channel; \
                     run purge from a stable build to remove them"
                .to_string(),
        });
    }
    match fs::symlink_metadata(&channel) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            plan.absent.push(channel);
            return;
        }
        Err(e) => {
            plan.skipped.push(Note {
                path: channel,
                reason: format!("could not be inspected: {e}"),
            });
            return;
        }
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            plan.skipped.push(Note {
                path: channel,
                reason: "is a symbolic link, junction or file, not tokensave's state directory; \
                         not followed"
                    .to_string(),
            });
            return;
        }
        Ok(_) => {}
    }
    let Ok(entries) = fs::read_dir(&channel) else {
        plan.skipped.push(Note {
            path: channel,
            reason: "could not be read".to_string(),
        });
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut config_removed = false;
    for name in names {
        let path = channel.join(&name);
        if !beta && name == BETA_DIRNAME {
            plan.kept.push(Note {
                path,
                reason: "beta channel state; run purge from a beta build to remove it".to_string(),
            });
            continue;
        }
        let Some((category, evidence)) = classify_global_entry(&name, beta) else {
            plan.skipped.push(Note {
                path,
                reason: "not a file tokensave creates; left in place".to_string(),
            });
            continue;
        };
        if category == Category::UsageHistory && !req.include_history {
            plan.kept.push(Note {
                path,
                reason: "token-savings history cannot be rebuilt; \
                         pass --include-history to delete it"
                    .to_string(),
            });
            continue;
        }
        if name == STATE_BASES_FILENAME && !req.all_projects {
            plan.kept.push(Note {
                path,
                reason: "records where relocated project indexes live; \
                         kept while project indexes are kept (see --all-projects)"
                    .to_string(),
            });
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            plan.skipped.push(Note {
                path,
                reason: "is a symbolic link or junction; not followed".to_string(),
            });
            continue;
        }
        config_removed |= name == USER_CONFIG_FILENAME;
        plan.removals.push(Removal {
            bytes: size_of(&path),
            path,
            category,
            evidence: evidence.to_string(),
            keep_child: None,
        });
    }
    if config_removed {
        plan.notes.push(
            "Every setting in config.toml returns to its default, including an opt-out of \
             the worldwide counter. Set TOKENSAVE_TELEMETRY=0 or run \
             `tokensave disable-upload-counter` afterwards to keep it off."
                .to_string(),
        );
    }
    plan.prune_if_empty.push(channel);
}

/// Classifies a direct child of the global state directory.
fn classify_global_entry(name: &str, beta: bool) -> Option<(Category, &'static str)> {
    if name == GLOBAL_DB_FILENAME || GLOBAL_DB_SIDECARS.contains(&name) {
        return Some((Category::UsageHistory, "tokensave's global database"));
    }
    // Everything below lives only in the shared directory, which belongs to
    // the stable channel's purge.
    if beta {
        return None;
    }
    match name {
        USER_CONFIG_FILENAME => return Some((Category::Settings, "tokensave's user settings")),
        STATE_FILENAME => return Some((Category::Settings, "tokensave's machine-local state")),
        STATE_BASES_FILENAME => {
            return Some((Category::Settings, "tokensave's relocated-state record"));
        }
        BENCHMARK_FILENAME => return Some((Category::Cache, "hardware benchmark cache")),
        PRICING_FILENAME => return Some((Category::Cache, "model pricing cache")),
        MEMORY_MMAP_FILENAME => return Some((Category::Cache, "memory telemetry ring")),
        MONITOR_MMAP_FILENAME => return Some((Category::Cache, "sync monitor ring")),
        SERVERS_DIRNAME => return Some((Category::Cache, "running-server registry")),
        PROFILE_DIRNAME => return Some((Category::Cache, "profiler aggregates")),
        _ => {}
    }
    for settings in LOCKED_SETTINGS {
        if name.strip_suffix(LOCK_SUFFIX) == Some(settings) {
            return Some((Category::Cache, "lock file of a tokensave settings file"));
        }
        if is_atomic_write_leftover(name, settings) {
            return Some((Category::Cache, "leftover of an interrupted settings write"));
        }
    }
    None
}

/// `config.tmp.<pid>.<n>`: what `write_atomic` leaves behind when a write is
/// interrupted between creating the temporary and renaming it.
fn is_atomic_write_leftover(name: &str, settings: &str) -> bool {
    let stem = Path::new(settings)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(settings);
    let Some(rest) = name
        .strip_prefix(stem)
        .and_then(|r| r.strip_prefix(ATOMIC_TMP_INFIX))
    else {
        return false;
    };
    let mut parts = rest.split('.');
    let numeric =
        |p: Option<&str>| p.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    numeric(parts.next()) && numeric(parts.next()) && parts.next().is_none()
}

/// Plans one project's state directory, project-scoped integrations and
/// repository hooks.
fn plan_project(
    target: &ProjectTarget,
    shared: &Path,
    history_deleted: bool,
    plan: &mut PurgePlan,
) {
    let root = &target.root;
    if !root.is_dir() {
        if target.registered {
            plan.skipped.push(Note {
                path: root.clone(),
                reason: "registered project root no longer exists; relocated state for it is \
                         removed by `tokensave state prune`"
                    .to_string(),
            });
            if !history_deleted {
                plan.registry_rows.push(root.to_string_lossy().into_owned());
            }
        } else {
            plan.skipped.push(Note {
                path: root.clone(),
                reason: "is not a directory".to_string(),
            });
        }
        return;
    }
    let root = crate::state_dir::canonical_root(root);
    if !plan.project_roots.contains(&root) {
        plan.project_roots.push(root.clone());
    }
    if !history_deleted {
        for key in [
            crate::global_db::normalize_project_key(&root),
            target.root.to_string_lossy().into_owned(),
        ] {
            if !plan.registry_rows.contains(&key) {
                plan.registry_rows.push(key);
            }
        }
    }

    let location = crate::config::locate_project_state(&root);
    let dir = location.dir().to_path_buf();
    if plan.removals.iter().any(|r| r.path == dir) {
        return;
    }
    // A project at the home directory would name the global state directory.
    let shared_canonical = crate::state_dir::canonical_root(shared);
    if crate::state_dir::canonical_root(&dir).starts_with(&shared_canonical)
        && matches!(location, crate::state_dir::Location::InRepo(_))
    {
        plan.skipped.push(Note {
            path: dir,
            reason: "is tokensave's global state directory, not a project index; \
                     purge it with the global scope"
                .to_string(),
        });
        return;
    }
    // The state directory must be where tokensave puts it, and nothing on the
    // way down to it may be a link that leads somewhere else.
    let anchor = match &location {
        crate::state_dir::Location::InRepo(_) => root.clone(),
        crate::state_dir::Location::Relocated { base, .. } => {
            base.join(crate::state_dir::PROJECTS_DIR)
        }
    };
    let Ok(relative) = dir.strip_prefix(&anchor) else {
        plan.skipped.push(Note {
            path: dir,
            reason: "is not inside the project or the configured state directory".to_string(),
        });
        return;
    };
    if relative.as_os_str().is_empty() {
        plan.skipped.push(Note {
            path: dir,
            reason: "is not a project state directory".to_string(),
        });
        return;
    }
    let mut step = anchor;
    for component in relative.components() {
        step.push(component);
        match fs::symlink_metadata(&step) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                plan.absent.push(dir);
                return;
            }
            Err(e) => {
                plan.skipped.push(Note {
                    path: dir,
                    reason: format!("could not be inspected: {e}"),
                });
                return;
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                plan.skipped.push(Note {
                    path: step,
                    reason: "is a symbolic link or junction; not followed".to_string(),
                });
                return;
            }
            Ok(meta) if !meta.is_dir() => {
                plan.skipped.push(Note {
                    path: step,
                    reason: "is a file, not a state directory".to_string(),
                });
                return;
            }
            Ok(_) => {}
        }
    }
    let Some(marker) = PROJECT_OWNERSHIP_MARKERS
        .iter()
        .find(|m| dir.join(m).is_file())
    else {
        plan.skipped.push(Note {
            path: dir,
            reason: "holds no tokensave index; left in place".to_string(),
        });
        return;
    };
    let keep_child = (!crate::cloud::is_beta()
        && fs::symlink_metadata(dir.join(BETA_DIRNAME)).is_ok())
    .then_some(BETA_DIRNAME);
    if keep_child.is_some() {
        plan.kept.push(Note {
            path: dir.join(BETA_DIRNAME),
            reason: "beta channel index; run purge from a beta build to remove it".to_string(),
        });
    }
    plan.removals.push(Removal {
        bytes: size_of(&dir),
        path: dir,
        category: Category::ProjectIndex,
        evidence: format!("contains {marker}"),
        keep_child,
    });
}

/// Approximate size of `path`, without following symbolic links.
fn size_of(path: &Path) -> u64 {
    fn walk(path: &Path, depth: usize, acc: &mut u64) {
        let Ok(meta) = fs::symlink_metadata(path) else {
            return;
        };
        if meta.is_file() {
            *acc = acc.saturating_add(meta.len());
        } else if meta.is_dir() && depth < MAX_SIZE_DEPTH {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    walk(&entry.path(), depth + 1, acc);
                }
            }
        }
    }
    let mut total = 0;
    walk(path, 0, &mut total);
    total
}

/// What [`execute_removals`] did.
#[derive(Debug, Default)]
pub struct RemovalReport {
    pub removed: Vec<PathBuf>,
    /// Paths that were already gone when their turn came.
    pub already_absent: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
}

/// Deletes the plan's removals, then any directory in `prune_if_empty` the
/// removals left empty.
///
/// Each path is re-checked first: a path that became a symbolic link since
/// the plan was made is refused rather than followed. One failure does not
/// stop the rest; the report lists every outcome.
pub fn execute_removals(plan: &PurgePlan) -> RemovalReport {
    let mut report = RemovalReport::default();
    for removal in &plan.removals {
        match remove_path(&removal.path, removal.keep_child) {
            Ok(true) => report.removed.push(removal.path.clone()),
            Ok(false) => report.already_absent.push(removal.path.clone()),
            Err(e) => report
                .failed
                .push((removal.path.clone(), describe_io_error(&e))),
        }
    }
    for dir in &plan.prune_if_empty {
        // `remove_dir` only removes an empty directory, which is the point.
        let _ = fs::remove_dir(dir);
    }
    report
}

/// Removes `path` without following symbolic links, keeping `keep_child`
/// when `path` is a directory. `Ok(false)` when it was already gone.
///
/// Shared by `purge` and `wipe`: both must keep the beta channel's state
/// inside a stable state directory.
pub fn remove_path(path: &Path, keep_child: Option<&str>) -> io::Result<bool> {
    let meta = match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        other => other?,
    };
    if meta.file_type().is_symlink() {
        return Err(io::Error::other(
            "became a symbolic link or junction after planning; not followed",
        ));
    }
    if !meta.is_dir() {
        fs::remove_file(path)?;
        return Ok(true);
    }
    let Some(keep) = keep_child else {
        // `remove_dir_all` removes links inside the tree without following
        // them.
        fs::remove_dir_all(path)?;
        return Ok(true);
    };
    let mut first_error = None;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_name() == keep {
            continue;
        }
        if let Err(e) = remove_child(&entry.path()) {
            first_error.get_or_insert(e);
        }
    }
    first_error.map_or(Ok(true), Err)
}

/// Removes a directory child without following it if it is a link.
fn remove_child(path: &Path) -> io::Result<()> {
    let file_type = fs::symlink_metadata(path)?.file_type();
    if file_type.is_symlink() {
        // A directory link on Windows (symlink or junction) needs
        // `remove_dir`; everywhere else a link is removed like a file.
        return fs::remove_file(path).or_else(|_| fs::remove_dir(path));
    }
    if file_type.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// An error message with a hint for the failure a running server causes.
fn describe_io_error(e: &io::Error) -> String {
    let in_use = e.kind() == io::ErrorKind::PermissionDenied
        || (cfg!(windows)
            && e.raw_os_error()
                .is_some_and(|c| WINDOWS_FILE_IN_USE.contains(&c)));
    if in_use {
        format!(
            "{e} — a running tokensave process may hold it; close the agents using tokensave \
             (`tokensave servers` lists them) and run purge again"
        )
    } else {
        e.to_string()
    }
}

/// Formats a byte count for the plan.
pub fn human_size(bytes: u64) -> String {
    let mut unit = 0;
    let mut whole = bytes;
    let mut tenths = 0;
    while whole >= SIZE_STEP && unit + 1 < SIZE_UNITS.len() {
        tenths = (whole % SIZE_STEP) * 10 / SIZE_STEP;
        whole /= SIZE_STEP;
        unit += 1;
    }
    if unit == 0 {
        format!("{whole} {}", SIZE_UNITS[unit])
    } else {
        format!("{whole}.{tenths} {}", SIZE_UNITS[unit])
    }
}

/// The agents a purge cleans: those recorded in `installed_agents` plus any
/// whose config still carries tokensave. Read-only, unlike
/// [`crate::agents::migrate_installed_agents`], so a dry run writes nothing.
pub fn agents_to_clean(home: &Path) -> Vec<String> {
    let mut agents: Vec<String> = fs::read_to_string(
        home.join(crate::config::TOKENSAVE_DIR)
            .join(USER_CONFIG_FILENAME),
    )
    .ok()
    .and_then(|text| text.parse::<toml::Table>().ok())
    .and_then(|table| {
        table
            .get(INSTALLED_AGENTS_KEY)
            .and_then(|v| v.as_array())
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_str().map(str::to_string))
                    .collect()
            })
    })
    .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    agents.retain(|id| crate::agents::get_integration(id).is_ok() && seen.insert(id.clone()));
    let missing = crate::agents::detect_missing_installed_agents(home, &agents);
    agents.extend(missing);
    agents
}
