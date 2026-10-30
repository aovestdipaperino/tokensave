use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::cli::BranchAction;
use crate::global;
use crate::Spinner;
use tokensave::config::{discover_project, ProjectDiscovery};
use tokensave::tokensave::TokenSave;

pub(crate) async fn handle_branch_action(action: BranchAction) -> tokensave::errors::Result<()> {
    use tokensave::branch;
    use tokensave::branch_meta;
    use tokensave::config::get_tokensave_dir;

    match action {
        BranchAction::List { path } => {
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            let tokensave_dir = get_tokensave_dir(&project_path);
            let Some(meta) = branch_meta::load_branch_meta(&tokensave_dir) else {
                eprintln!("No branch tracking configured. Run `tokensave branch add` to start.");
                return Ok(());
            };
            let current = branch::current_branch(&project_path);
            eprintln!("Default branch: {}", meta.default_branch);
            eprintln!();
            for (name, entry) in &meta.branches {
                let db_path = tokensave_dir.join(&entry.db_file);
                let size = if db_path.exists() {
                    let bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                    tokensave::display::format_bytes(bytes)
                } else {
                    "missing".to_string()
                };
                let marker = if current.as_deref() == Some(name.as_str()) {
                    " *"
                } else {
                    ""
                };
                let parent = entry
                    .parent
                    .as_deref()
                    .map(|p| format!(" (from {p})"))
                    .unwrap_or_default();
                let synced = branch_meta::format_timestamp(&entry.last_synced_at);
                eprintln!("  {name}{marker} — {size}{parent}, synced {synced}");
            }
        }
        BranchAction::Add {
            name,
            path,
            if_enabled,
        } => {
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            let tokensave_dir = get_tokensave_dir(&project_path);

            // #397: an automated caller (the `post-checkout` hook) passes
            // `--if-enabled` so the `auto_track` knob governs this path too.
            // Before this, `auto_track` was read only inside `TokenSave::open`
            // and the hook tracked unconditionally, so the knob was not
            // authoritative — on fresh installs as much as old ones. Checked
            // before anything is read or written, so a declined auto-track
            // costs nothing. Silent by design: a hook runs on every checkout
            // and must not narrate.
            if if_enabled {
                let config = tokensave::config::load_config(&project_path).unwrap_or_default();
                let enabled =
                    tokensave::config::env_bool_override("TOKENSAVE_AUTO_TRACK", config.auto_track);
                if !enabled {
                    return Ok(());
                }
            }

            let branch_name = match name {
                Some(n) => n,
                None => branch::current_branch(&project_path).ok_or_else(|| {
                    tokensave::errors::TokenSaveError::Config {
                        message:
                            "cannot detect current branch (detached HEAD?). Specify a branch name."
                                .to_string(),
                    }
                })?,
            };

            // Serialize the copy + metadata phase with the asynchronous
            // post-checkout hook and transparent auto-track paths. The lock
            // is deliberately released before the branch's sync below, so
            // `TokenSave::open` can perform its normal auto-track check.
            let _branch_lock =
                tokensave::tokensave::acquire_branch_operation_lock(&tokensave_dir).await?;

            // Load or bootstrap metadata
            let mut meta = branch_meta::load_branch_meta(&tokensave_dir).unwrap_or_else(|| {
                let default = branch::detect_default_branch(&project_path)
                    .unwrap_or_else(|| "main".to_string());
                branch_meta::BranchMeta::new(&default)
            });

            if meta.is_tracked(&branch_name) {
                eprintln!("Branch '{branch_name}' is already tracked.");
                return Ok(());
            }

            // Find parent DB to copy from
            let parent = branch::find_nearest_tracked_ancestor(&project_path, &branch_name, &meta)
                .unwrap_or_else(|| meta.default_branch.clone());
            let parent_db = branch::resolve_branch_db_path(&tokensave_dir, &parent, &meta)
                .ok_or_else(|| tokensave::errors::TokenSaveError::Config {
                    message: format!("parent branch '{parent}' has no DB"),
                })?;
            if !parent_db.exists() {
                return Err(tokensave::errors::TokenSaveError::Config {
                    message: format!("parent DB not found at '{}'", parent_db.display()),
                });
            }

            // Copy DB (collision-safe db_file: distinct branches that sanitize
            // to the same stem get a hash-suffixed name instead of sharing one).
            let db_file = branch::unique_branch_db_file(&meta, &branch_name);
            branch_meta::ensure_branches_dir(&tokensave_dir)?;
            let new_db_path = tokensave_dir.join(&db_file);
            let spinner = Spinner::new();
            spinner.set_message(&format!("copying DB from '{parent}'"));
            branch::copy_branch_db(&parent_db, &new_db_path).await?;

            // Save metadata BEFORE open() so it resolves the new branch to its DB
            meta.add_branch(&branch_name, &db_file, &parent);
            branch_meta::save_branch_meta(&tokensave_dir, &meta)?;
            drop(_branch_lock);

            // A sync reads the working directory, so it can only speak for the
            // branch that is actually checked out. Adding some *other* branch
            // used to call `TokenSave::open`, which resolves the DB for HEAD:
            // the new branch's DB stayed a bare copy of the parent while the
            // working tree — including files that exist on no branch at all —
            // was written into the *current* branch's DB (#501). When the
            // target is not checked out, the copy of the parent is the honest
            // answer, and the `post-checkout` hook refreshes it on arrival.
            let checked_out = branch::current_branch(&project_path);
            if checked_out.as_deref() != Some(branch_name.as_str()) {
                spinner.done(&format!(
                    "branch '{branch_name}' tracked — copied from '{parent}'"
                ));
                eprintln!(
                    "Not checked out, so nothing was indexed from the working tree. \
                     Check it out and run `tokensave sync` to bring it up to date."
                );
                return Ok(());
            }

            // Run incremental sync (hash-based delta) against the new branch DB
            spinner.set_message("syncing changes");
            // Safe now: the guard above established that HEAD is this branch,
            // so `open` resolves to the DB just registered for it.
            let cg = TokenSave::open(&project_path).await?;
            let result = cg.sync().await?;

            // Update sync timestamp after successful sync
            if let Some(mut meta) = branch_meta::load_branch_meta(&tokensave_dir) {
                meta.touch_synced(&branch_name);
                let _ = branch_meta::save_branch_meta(&tokensave_dir, &meta);
            }

            let skipped_msg = if result.skipped_paths.is_empty() {
                String::new()
            } else {
                format!(", {} skipped", result.skipped_paths.len())
            };
            spinner.done(&format!(
                "branch '{branch_name}' tracked — {} added, {} modified, {} removed{skipped_msg}",
                result.files_added, result.files_modified, result.files_removed
            ));
            if !result.skipped_paths.is_empty() {
                eprintln!();
                eprintln!(
                    "\x1b[33mSkipped ({}) — files found but not readable:\x1b[0m",
                    result.skipped_paths.len()
                );
                for (path, reason) in &result.skipped_paths {
                    eprintln!("  ! {path}: {reason}");
                }
            }
        }
        BranchAction::Remove { name, path } => {
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            let tokensave_dir = get_tokensave_dir(&project_path);
            let Some(mut meta) = branch_meta::load_branch_meta(&tokensave_dir) else {
                eprintln!("No branch tracking configured.");
                return Ok(());
            };
            if name == meta.default_branch {
                return Err(tokensave::errors::TokenSaveError::Config {
                    message: format!("cannot remove default branch '{name}'"),
                });
            }
            if let Some(entry) = meta.remove_branch(&name) {
                let db_path = tokensave_dir.join(&entry.db_file);
                if db_path.exists() {
                    std::fs::remove_file(&db_path)?;
                    // Also remove WAL/SHM sidecar files
                    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
                    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
                }
                branch_meta::save_branch_meta(&tokensave_dir, &meta)?;
                eprintln!("\x1b[32m✔\x1b[0m Branch '{name}' removed.");
            } else {
                eprintln!("Branch '{name}' is not tracked.");
            }
        }
        BranchAction::Removeall { path } => {
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            let tokensave_dir = get_tokensave_dir(&project_path);
            let Some(mut meta) = branch_meta::load_branch_meta(&tokensave_dir) else {
                eprintln!("No branch tracking configured.");
                return Ok(());
            };
            let removed = meta.remove_all_branches();
            if removed.is_empty() {
                eprintln!("No non-default branches to remove.");
            } else {
                for (name, entry) in &removed {
                    let db_path = tokensave_dir.join(&entry.db_file);
                    if db_path.exists() {
                        std::fs::remove_file(&db_path)?;
                        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
                        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
                    }
                    eprintln!("  removed '{name}'");
                }
                branch_meta::save_branch_meta(&tokensave_dir, &meta)?;
                eprintln!(
                    "\x1b[32m✔\x1b[0m Removed {} branch(es). Only '{}' remains.",
                    removed.len(),
                    meta.default_branch
                );
            }
        }
        BranchAction::Gc { path } => {
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            let tokensave_dir = get_tokensave_dir(&project_path);
            if branch_meta::load_branch_meta(&tokensave_dir).is_none() {
                eprintln!("No branch tracking configured.");
                return Ok(());
            }
            let removed = branch::collect_stale_indexes(&project_path, &[]).await?;
            if removed.is_empty() {
                eprintln!("No stale branches to clean up.");
            } else {
                for name in &removed {
                    eprintln!("  removed '{name}'");
                }
                eprintln!(
                    "\x1b[32m✔\x1b[0m Cleaned up {} stale branch(es).",
                    removed.len()
                );
            }
        }
    }
    Ok(())
}

/// Handles the `wipe` and `wipe --all` commands.
pub(crate) async fn handle_wipe(all: bool) -> tokensave::errors::Result<()> {
    use std::fs;
    use std::path::PathBuf;

    let home_tokensave: Option<PathBuf> =
        dirs::home_dir().map(|h| tokensave::config::get_tokensave_dir(&h));

    let mut targets = global::gather_target_projects(all, &home_tokensave).await;
    if all {
        // wipe acts on the live `.tokensave/` directory; drop rows whose
        // directory is already gone (they're handled by `tokensave doctor`).
        targets.retain(|p| {
            tokensave::config::get_tokensave_dir(p)
                .join("tokensave.db")
                .exists()
        });
    }

    if !all && targets.is_empty() {
        eprintln!("No tokensave projects found in current folder, parents, or children.");
        return Ok(());
    }

    global::print_flash_warning(all, &targets);

    eprint!("Type \x1b[1;32mgo!\x1b[0m to confirm (anything else aborts): ");
    io::stderr().flush().ok();
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer).map_err(|e| {
        tokensave::errors::TokenSaveError::Config {
            message: format!("failed to read stdin: {e}"),
        }
    })?;
    if answer.trim() != "go!" {
        eprintln!("\x1b[33mAborted — nothing was wiped.\x1b[0m");
        return Ok(());
    }

    let mut removed = 0usize;
    let mut errors = 0usize;
    let mut wiped_paths: Vec<PathBuf> = Vec::new();

    // `targets` is already unique: `gather_local_projects` dedupes via its
    // own `seen`, and the `--all` branch reads from `projects.path` which is
    // a primary key. No need for a second per-loop dedupe.
    for project_root in &targets {
        let ts_dir = tokensave::config::get_tokensave_dir(project_root);
        if !ts_dir.exists() {
            continue;
        }
        let removal = if !tokensave::cloud::is_beta() && ts_dir.join("beta").exists() {
            // Beta owns this subtree; wiping stable must preserve it.
            (|| -> std::io::Result<()> {
                for entry in fs::read_dir(&ts_dir)? {
                    let entry = entry?;
                    if entry.file_name() == "beta" {
                        continue;
                    }
                    if entry.file_type()?.is_dir() {
                        fs::remove_dir_all(entry.path())?;
                    } else {
                        fs::remove_file(entry.path())?;
                    }
                }
                Ok(())
            })()
        } else {
            fs::remove_dir_all(&ts_dir)
        };
        match removal {
            Ok(()) => {
                removed += 1;
                wiped_paths.push(project_root.clone());
                eprintln!("  \x1b[32m✔\x1b[0m removed {}", ts_dir.display());
            }
            Err(e) => {
                errors += 1;
                eprintln!("  \x1b[31m✗\x1b[0m {} ({e})", ts_dir.display());
            }
        }
    }

    if all {
        if let Some(global_dir) = home_tokensave.as_ref() {
            for ext in ["db", "db-wal", "db-shm"] {
                let p = global_dir.join(format!("global.{ext}"));
                let _ = fs::remove_file(&p);
            }
            eprintln!(
                "  \x1b[32m✔\x1b[0m emptied global DB at {}/global.db",
                global_dir.display()
            );
        }
    } else if !wiped_paths.is_empty() {
        if let Some(gdb) = tokensave::global_db::GlobalDb::open().await {
            let path_strs: Vec<String> = wiped_paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            gdb.delete_projects(&path_strs).await;
        }
    }

    eprintln!();
    let suffix = if errors > 0 {
        format!(" ({errors} error(s))")
    } else {
        String::new()
    };
    eprintln!("\x1b[32mWiped {removed} project(s){suffix}.\x1b[0m");
    Ok(())
}

/// Handles the `list` and `list --all` commands.
pub(crate) async fn handle_list(all: bool) -> tokensave::errors::Result<()> {
    use std::path::PathBuf;
    use tokensave::display::format_token_count;

    let home_tokensave: Option<PathBuf> =
        dirs::home_dir().map(|h| tokensave::config::get_tokensave_dir(&h));
    let project_paths = global::gather_target_projects(all, &home_tokensave).await;

    if project_paths.is_empty() {
        if all {
            println!("No tokensave projects tracked in the global DB.");
        } else {
            println!("No tokensave projects found in current folder, parents, or children.");
        }
        return Ok(());
    }

    let gdb = tokensave::global_db::GlobalDb::open().await;
    let mut rows: Vec<ListRow> = Vec::with_capacity(project_paths.len());
    let mut total_size: u64 = 0;
    let mut total_tokens: u64 = 0;

    for path in &project_paths {
        let ts_dir = tokensave::config::get_tokensave_dir(path);
        let on_disk = ts_dir.exists();
        let size = if on_disk {
            global::tokensave_dir_size(&ts_dir)
        } else {
            0
        };
        let tokens = match &gdb {
            Some(db) => db.get_project_tokens(path).await,
            None => 0,
        };
        total_size = total_size.saturating_add(size);
        total_tokens = total_tokens.saturating_add(tokens);
        rows.push(ListRow {
            path: path.clone(),
            on_disk,
            size,
            tokens,
        });
    }

    rows.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.path.cmp(&b.path)));

    let path_w = rows
        .iter()
        .map(|r| {
            r.path.display().to_string().chars().count()
                + if r.on_disk { 0 } else { " (stale)".len() }
        })
        .max()
        .unwrap_or(0);

    println!("Found {} tokensave project(s):", rows.len());
    println!();
    for r in &rows {
        let path_str = if r.on_disk {
            r.path.display().to_string()
        } else {
            format!("{} \x1b[33m(stale)\x1b[0m", r.path.display())
        };
        let pad = path_w.saturating_sub(
            r.path.display().to_string().chars().count()
                + if r.on_disk { 0 } else { " (stale)".len() },
        );
        let size_str = if r.on_disk {
            tokensave::display::format_bytes(r.size)
        } else {
            "—".to_string()
        };
        let tokens_str = if r.tokens == 0 {
            "—".to_string()
        } else {
            format_token_count(r.tokens)
        };
        println!(
            "  {path_str}{pad}  {size:>10}  {tokens:>10} tokens",
            pad = " ".repeat(pad),
            size = size_str,
            tokens = tokens_str
        );
    }
    println!();
    let total_tokens_str = if total_tokens == 0 {
        "—".to_string()
    } else {
        format_token_count(total_tokens)
    };
    println!(
        "Total: {} on disk · {} tokens saved",
        tokensave::display::format_bytes(total_size),
        total_tokens_str
    );
    Ok(())
}

#[derive(Debug)]
struct ListRow {
    path: std::path::PathBuf,
    on_disk: bool,
    size: u64,
    tokens: u64,
}

/// True when the global DB has zero registered projects (or can't be opened
/// at all) — i.e. the user has not run `tokensave init` anywhere yet.
async fn is_fresh_install() -> bool {
    match tokensave::global_db::GlobalDb::open().await {
        Some(gdb) => gdb.list_project_paths().await.is_empty(),
        None => true,
    }
}

/// Where a project that does not exist yet is created for `cwd`: an incomplete
/// project's own root (re-running `init` completes it), else the enclosing git
/// working tree root, else `cwd` itself.
fn init_target(cwd: PathBuf, discovery: ProjectDiscovery) -> PathBuf {
    match discovery {
        ProjectDiscovery::Initialized(root) | ProjectDiscovery::Incomplete(root) => root,
        ProjectDiscovery::Uninitialized { git_root } => git_root.unwrap_or(cwd),
    }
}

/// Resolves the directory `tokensave init` initializes.
///
/// An explicit path is an override and is used as given. Without one, `cwd` is
/// resolved through [`discover_project`]: when it lies below an existing
/// project, that project is reported and `None` is returned, so `init` exits 0
/// without creating a nested `.tokensave/` or indexing anything. `cwd` being
/// the project root itself is returned as is, keeping `init`'s refusal to
/// re-initialize (#372). Otherwise the project is created at the git root.
pub(crate) fn resolve_init_root(
    path: Option<String>,
) -> tokensave::errors::Result<Option<PathBuf>> {
    if path.is_some() {
        return Ok(Some(tokensave::config::resolve_path(path)));
    }
    let cwd = tokensave::config::resolve_path(None);
    match discover_project(&cwd) {
        ProjectDiscovery::Initialized(root) if root != cwd => {
            // Reporting a project whose config cannot load as "usable" would
            // only defer the failure to the first sync or query.
            if let Err(e) = tokensave::config::load_config(&root) {
                return Err(tokensave::errors::TokenSaveError::Config {
                    message: format!(
                        "TokenSave is initialized at '{}', but its configuration cannot be \
                         loaded: {e}\n  Fix or remove .tokensave/config.json there, then rerun.",
                        root.display()
                    ),
                });
            }
            eprintln!(
                "TokenSave is already initialized in:\n  {}\n\nCurrent directory:\n  {}\n\n\
                 Using existing project configuration and index.\n\
                 No initialization or indexing required.",
                root.display(),
                cwd.display()
            );
            Ok(None)
        }
        discovery => Ok(Some(init_target(cwd, discovery))),
    }
}

/// When invoked with no subcommand, offer to create the index if none exists.
pub(crate) async fn handle_no_command() -> tokensave::errors::Result<()> {
    let cwd = tokensave::config::resolve_path(None);
    let discovery = discover_project(&cwd);
    let project_path = init_target(cwd, discovery);
    if TokenSave::is_initialized(&project_path) {
        // Already initialized — render help to stderr, never stdout. Agent
        // permission hooks invoke a bare `tokensave` and parse stdout as JSON;
        // help text there is a fatal parse error that fail-closes the wrapped
        // command. An empty stdout with exit 0 reads as "no opinion" instead.
        // See #347, #348, #351.
        let mut cmd = <crate::cli::Cli as clap::CommandFactory>::command();
        eprint!("{}", cmd.render_help());
        eprintln!();
        return Ok(());
    }
    if is_fresh_install().await {
        eprintln!("\x1b[1;36mWelcome to tokensave!\x1b[0m");
        eprintln!(
            "Looks like a new installation. To get started, run \x1b[1mtokensave init\x1b[0m \
             in your project root."
        );
        eprintln!();
    }
    // Never prompt when stdin isn't a terminal. A hook pipes its JSON payload
    // into a bare `tokensave`; reading it here would consume that payload and
    // could be mistaken for a "yes", spuriously initializing the project (#351).
    if !io::stdin().is_terminal() {
        eprintln!(
            "No TokenSave index found at '{}'. Run `tokensave init` to create one.",
            project_path.display()
        );
        return Ok(());
    }
    eprint!(
        "No TokenSave index found at '{}'. Create one now? [Y/n] ",
        project_path.display()
    );
    io::stderr().flush().ok();
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer).map_err(|e| {
        tokensave::errors::TokenSaveError::Config {
            message: format!("failed to read stdin: {}", e),
        }
    })?;
    let answer = answer.trim();
    if answer.is_empty() || answer.eq_ignore_ascii_case("y") {
        init_and_index(&project_path, &[], false, None).await?;
    }
    Ok(())
}

/// Prints a saved or fresh benchmark result for `init` and `config benchmark`.
fn print_benchmark(result: &tokensave::global_benchmark::SavedBenchmark) {
    eprintln!("  Measured:    {}", result.measurement.summary());
    let workers = result.recommended_workers();
    eprintln!(
        "  Recommended: {} profile, {workers} worker{} ({})",
        result.recommendation.profile.as_str(),
        if workers == 1 { "" } else { "s" },
        result.recommendation.reason
    );
}

fn benchmark_file_display() -> String {
    tokensave::user_config::benchmark_path().map_or_else(
        || "~/.tokensave/benchmark.toml".to_string(),
        |p| p.display().to_string(),
    )
}

/// Explains why the benchmark is about to run.
fn announce_benchmark(status: &tokensave::global_benchmark::CacheStatus, forced: bool) {
    use tokensave::global_benchmark::CacheStatus;
    match status {
        _ if forced => eprintln!("Running hardware benchmark (--force)..."),
        CacheStatus::Invalid { reason, .. } => {
            eprintln!("Saved global hardware benchmark can't be reused: {reason}.");
            eprintln!("Running hardware benchmark...");
        }
        _ => {
            eprintln!("No global hardware benchmark found.");
            eprintln!("Running initial hardware benchmark...");
        }
    }
}

/// Picks the `cpu_usage` to pin for a project being initialized: only
/// `--cpu-usage` pins one. When the project will follow the global hardware
/// benchmark, makes sure a valid one is saved, measuring only if none is, so
/// the next project on this machine reuses it (#752).
pub(crate) fn prepare_initial_cpu_usage(
    project_path: &Path,
    requested: Option<tokensave::cpu::CpuUsage>,
) -> tokensave::errors::Result<Option<tokensave::cpu::CpuUsage>> {
    use tokensave::global_benchmark::{self as gb, Outcome, Trigger};
    if requested.is_some() {
        return Ok(requested);
    }
    // A setting that takes precedence over the benchmark makes it unused.
    if tokensave::config::configured_cpu_usage(project_path)?.is_some()
        || gb::env_cpu_usage().is_some()
    {
        return Ok(None);
    }
    let prefs = tokensave::user_config::resource_preferences();
    if prefs.cpu_usage.is_some() {
        return Ok(None);
    }
    match gb::ensure(prefs.benchmark, Trigger::Automatic, |status| {
        announce_benchmark(status, false);
    }) {
        Outcome::Cached(_) => eprintln!("Using cached global hardware benchmark."),
        Outcome::Measured { result, saved } => {
            print_benchmark(&result);
            match saved {
                Ok(()) => eprintln!(
                    "Global benchmark results saved to {}.",
                    benchmark_file_display()
                ),
                Err(e) => eprintln!(
                    "\x1b[33mwarning:\x1b[0m could not save the hardware benchmark: {e}. \
                     Using the built-in default; run `tokensave config benchmark` to retry."
                ),
            }
        }
        Outcome::Disabled => {}
        Outcome::Skipped(status) => {
            let why = match status {
                gb::CacheStatus::Invalid { reason, .. } => reason,
                _ => "none is saved".to_string(),
            };
            eprintln!(
                "No usable global hardware benchmark ({why}) and automatic benchmarking is off \
                 ([benchmark] auto_run = false). Run `tokensave config benchmark` to measure."
            );
        }
    }
    Ok(None)
}

fn print_json(report: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(report).unwrap_or_else(|_| report.to_string())
    );
}

/// `tokensave config benchmark`: manages the machine-level hardware
/// benchmark in `~/.tokensave/benchmark.toml` (#752). Exits 1 when the
/// requested action could not be completed.
pub(crate) fn run_config_benchmark(force: bool, show: bool, reset: bool, json: bool) {
    use tokensave::global_benchmark::{self as gb, CacheStatus, Outcome, Trigger};
    let path = benchmark_file_display();
    let fail = |message: String, report: serde_json::Value| -> ! {
        if json {
            print_json(&report);
        } else {
            eprintln!("\x1b[31merror:\x1b[0m {message}");
        }
        std::process::exit(1);
    };

    if reset {
        match gb::reset() {
            Ok(removed) => {
                if json {
                    print_json(&serde_json::json!({
                        "action": "reset", "path": path, "removed": removed,
                    }));
                } else if removed {
                    println!(
                        "Deleted {path}. The next `tokensave init` or \
                         `tokensave config benchmark` measures this machine again."
                    );
                } else {
                    println!("No global hardware benchmark was saved ({path}).");
                }
            }
            Err(e) => fail(
                e.clone(),
                serde_json::json!({ "action": "reset", "path": path, "error": e }),
            ),
        }
        return;
    }

    let prefs = tokensave::user_config::resource_preferences();
    if show {
        let (_, status) = gb::status();
        let (profile, source) = gb::effective_cpu_usage(None);
        let (state, reason, saved) = match &status {
            CacheStatus::Missing => ("missing", None, None),
            CacheStatus::Valid(saved) => ("valid", None, Some(saved)),
            CacheStatus::Invalid { reason, saved } => ("invalid", Some(reason), saved.as_ref()),
        };
        if json {
            print_json(&serde_json::json!({
                "action": "show",
                "path": path,
                "status": state,
                "reason": reason,
                "saved": saved,
                "recommended_workers": saved.map(gb::SavedBenchmark::recommended_workers),
                "settings": {
                    "enabled": prefs.benchmark.enabled,
                    "auto_run": prefs.benchmark.auto_run,
                    "cpu_usage": prefs.cpu_usage,
                },
                "effective": { "cpu_usage": profile, "source": source },
            }));
            return;
        }
        println!("Global hardware benchmark: {path}");
        match (state, reason) {
            ("valid", _) => println!("  Status:      valid for this machine"),
            ("missing", _) => println!("  Status:      not measured yet"),
            (_, Some(reason)) => println!("  Status:      not usable: {reason}"),
            _ => {}
        }
        if let Some(saved) = saved {
            println!("  Measured:    {}", saved.measurement.summary());
            println!(
                "  Recommended: {} profile, {} worker(s) ({})",
                saved.recommendation.profile.as_str(),
                saved.recommended_workers(),
                saved.recommendation.reason
            );
            println!(
                "  Measured at: {} by tokensave {}",
                saved.measured_at_utc(),
                saved.tokensave_version
            );
        }
        println!(
            "  Settings:    enabled = {}, auto_run = {}, global cpu_usage = {}",
            prefs.benchmark.enabled,
            prefs.benchmark.auto_run,
            prefs
                .cpu_usage
                .map_or("(not set)", tokensave::cpu::CpuUsage::as_str)
        );
        println!(
            "  Projects without their own cpu_usage use: {} ({})",
            profile.as_str(),
            source.describe()
        );
        return;
    }

    if !prefs.benchmark.enabled {
        let message = "the hardware benchmark is disabled ([benchmark] enabled = false in \
                       ~/.tokensave/config.toml)"
            .to_string();
        fail(
            message.clone(),
            serde_json::json!({ "action": "benchmark", "path": path, "error": message }),
        );
    }
    let trigger = if force {
        Trigger::Force
    } else {
        Trigger::OnRequest
    };
    let outcome = gb::ensure(prefs.benchmark, trigger, |status| {
        if !json {
            announce_benchmark(status, force);
        }
    });
    let (result, measured, saved) = match outcome {
        Outcome::Cached(result) => (result, false, Ok(())),
        Outcome::Measured { result, saved } => (result, true, saved),
        // Ruled out above: enabled is true and the trigger is explicit.
        Outcome::Disabled | Outcome::Skipped(_) => return,
    };
    if json {
        let report = serde_json::json!({
            "action": "benchmark",
            "path": path,
            "status": if measured { "measured" } else { "cached" },
            "saved": saved.is_ok(),
            "error": saved.as_ref().err(),
            "result": result,
            "recommended_workers": result.recommended_workers(),
        });
        if saved.is_err() {
            fail(String::new(), report);
        }
        print_json(&report);
        return;
    }
    if !measured {
        eprintln!("Using cached global hardware benchmark (run with --force to measure again).");
    }
    print_benchmark(&result);
    match saved {
        Ok(()) if measured => eprintln!("Global benchmark results saved to {path}."),
        Ok(()) => {}
        Err(e) => fail(
            format!("could not save the hardware benchmark: {e}"),
            serde_json::Value::Null,
        ),
    }
}

/// Re-runs the resource benchmark, saves it as the global benchmark and
/// updates a `cpu_usage` the project pins (#731, #752).
///
/// `--dry-run` only reports, so it also works where no project is initialized.
pub(crate) fn run_tune(
    path: Option<String>,
    dry_run: bool,
    json: bool,
) -> tokensave::errors::Result<()> {
    let project_path = tokensave::config::resolve_path_with_discovery(path);
    let initialized = TokenSave::is_initialized(&project_path);
    if !dry_run && !initialized {
        let plain = format!("no TokenSave index found at '{}'", project_path.display());
        if json {
            // A machine-readable mode has to produce a document, including on
            // the error path — `tokensave verify --json` does the same.
            let report = serde_json::json!({
                "error": format!(
                    "{plain}. Run `tokensave init` first, or `tokensave tune --dry-run` \
                     to only see the recommendation."
                ),
                "project": project_path.display().to_string(),
                "dry_run": dry_run,
                "written": false,
            });
            print_json(&report);
        } else {
            eprintln!(
                "\x1b[31merror:\x1b[0m {plain}.\n\
                 Run \x1b[1mtokensave init\x1b[0m first, or \x1b[1mtokensave tune --dry-run\x1b[0m \
                 to only see the recommendation."
            );
        }
        std::process::exit(1);
    }
    if !json {
        eprintln!("Benchmarking this machine...");
    }
    let measurement = tokensave::hardware::measure();
    let recommendation = tokensave::hardware::recommend(&measurement);

    // Only a profile the project pins is updated; a project without one
    // follows the global benchmark saved below.
    let mut config = if initialized {
        Some(tokensave::config::load_config(&project_path)?)
    } else {
        None
    };
    let previous = config.as_ref().and_then(|c| c.cpu_usage);
    let written = match config.as_mut() {
        Some(config) if !dry_run && previous.is_some_and(|p| p != recommendation.profile) => {
            config.cpu_usage = Some(recommendation.profile);
            tokensave::config::save_config(&project_path, config)?;
            true
        }
        _ => false,
    };
    let global_saved = if dry_run
        || !tokensave::user_config::resource_preferences()
            .benchmark
            .enabled
    {
        None
    } else {
        Some(tokensave::global_benchmark::save_measurement(measurement))
    };

    if json {
        let report = serde_json::json!({
            "project": project_path.display().to_string(),
            "measurement": measurement,
            "effective_cpus": measurement.effective_cpus(),
            "recommended": recommendation.profile,
            "reason": recommendation.reason,
            "previous": previous,
            "dry_run": dry_run,
            "written": written,
            "global_saved": global_saved.as_ref().is_some_and(Result::is_ok),
        });
        print_json(&report);
        return Ok(());
    }

    println!("Measured:    {}", measurement.summary());
    println!(
        "Recommended: {} ({})",
        recommendation.profile.as_str(),
        recommendation.reason
    );
    match (previous, written) {
        (Some(prev), true) => println!(
            "Updated cpu_usage: {} -> {}. Restart `tokensave serve` to apply it to a running server.",
            prev.as_str(),
            recommendation.profile.as_str()
        ),
        (Some(prev), false) if prev == recommendation.profile => {
            println!("cpu_usage is already {}.", prev.as_str());
        }
        (Some(prev), false) => println!(
            "Dry run: cpu_usage stays {}. Run without --dry-run to apply.",
            prev.as_str()
        ),
        (None, _) if !initialized => {
            println!("Dry run: no project is initialized here, nothing written.");
        }
        (None, _) if dry_run => println!("Dry run: nothing written."),
        (None, _) => println!(
            "This project has no cpu_usage of its own and follows the global hardware benchmark."
        ),
    }
    match global_saved {
        Some(Ok(_)) => println!(
            "Saved as the global hardware benchmark ({}).",
            benchmark_file_display()
        ),
        Some(Err(e)) => {
            eprintln!("\x1b[33mwarning:\x1b[0m could not save the global hardware benchmark: {e}")
        }
        None => {}
    }
    Ok(())
}

/// Initializes a new project (if needed) and runs a full index.
///
/// For a new project, `cpu_usage` pins the project's CPU usage profile when
/// given; otherwise the project follows the global settings and the global
/// hardware benchmark, which is measured only if none is saved (#752).
pub(crate) async fn init_and_index(
    project_path: &Path,
    skip_folders: &[String],
    verbose: bool,
    cpu_usage: Option<tokensave::cpu::CpuUsage>,
) -> tokensave::errors::Result<TokenSave> {
    debug_assert!(
        project_path.is_dir(),
        "init_and_index: project_path is not a directory"
    );
    debug_assert!(
        project_path.is_absolute(),
        "init_and_index: project_path must be absolute"
    );
    let mut cg = if TokenSave::is_initialized(project_path) {
        TokenSave::open_rebuilding_failed_migration(project_path).await?
    } else {
        let pin = prepare_initial_cpu_usage(project_path, cpu_usage)?;
        let cg = TokenSave::init_with_cpu_usage(project_path, pin).await?;
        eprintln!("Initialized TokenSave at {}", project_path.display());
        let source = cg.cpu_usage_source();
        let note = if cpu_usage.is_some()
            && source == tokensave::global_benchmark::CpuUsageSource::Project
        {
            "from --cpu-usage"
        } else {
            source.describe()
        };
        eprintln!("CPU usage profile: {} ({note})", cg.cpu_usage().as_str());
        // Offer to exclude .tokensave from git if it isn't already. Default to
        // the local, untracked .git/info/exclude so we don't leave a committable
        // diff; offer the tracked .gitignore as an explicit opt-in.
        //
        // Skipped entirely outside a git working tree — neither answer has any
        // effect there. When stdin isn't a TTY, the prompt is skipped so a
        // scripted or agent-driven `init` never consumes a line of the caller's
        // stdin and takes it for an answer (#288), but the write itself needs
        // no answer, so the default local exclusion is still applied (#373).
        // Success is reported only when the helper confirms the entry was
        // actually written.
        if tokensave::config::is_inside_git_repo(project_path)
            && !tokensave::config::is_in_gitignore(project_path)
        {
            if io::stdin().is_terminal() {
                eprint!(
                    "Exclude .tokensave from git? [Y] .git/info/exclude (local) / [g] .gitignore (tracked) / [n] no "
                );
                io::stderr().flush().ok();
                let mut answer = String::new();
                if io::stdin().lock().read_line(&mut answer).is_ok() {
                    let answer = answer.trim();
                    let reported = if answer.eq_ignore_ascii_case("g") {
                        tokensave::config::add_to_gitignore(project_path)
                            .then_some("Added .tokensave to .gitignore")
                    } else if answer.is_empty() || answer.eq_ignore_ascii_case("y") {
                        tokensave::config::add_to_git_info_exclude(project_path)
                            .then_some("Added .tokensave/ to .git/info/exclude (local, untracked)")
                    } else {
                        None
                    };
                    if let Some(message) = reported {
                        eprintln!("{message}");
                    }
                }
            } else if tokensave::config::add_to_git_info_exclude(project_path) {
                eprintln!("Added .tokensave/ to .git/info/exclude (local, untracked)");
            }
        }
        cg
    };
    cg.add_skip_folders(skip_folders);
    let spinner = Spinner::new();
    let index_start = std::time::Instant::now();
    let result = cg
        .index_all_with_progress_verbose(
            |current, total, file| {
                let elapsed = index_start.elapsed().as_secs_f64();
                let eta = if current > 1 {
                    let per_file = elapsed / (current - 1) as f64;
                    let remaining = per_file * (total - current) as f64;
                    if remaining >= 1.0 {
                        format!(" (ETA: {remaining:.0}s)")
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                spinner.set_message(&format!("[{current}/{total}] indexing {file}{eta}"));
            },
            |msg| {
                if verbose {
                    eprintln!("  \x1b[2m[verbose]\x1b[0m {msg}");
                }
            },
        )
        .await?;
    spinner.done(&format!(
        "indexing done — {} files, {} nodes, {} edges in {}ms",
        result.file_count, result.node_count, result.edge_count, result.duration_ms
    ));
    if !verbose {
        // Verbose already emitted the full per-extension list mid-run, so the
        // compact headline (and its "rerun with --verbose" hint) would repeat it.
        print_skipped_extension_summary(&result.skipped_extensions);
    }
    if let Some(warning) = cg.warn_skipped_hidden_dirs() {
        eprintln!("{warning}");
    }
    global::update_global_db(&cg).await;
    Ok(cg)
}

/// Convert raw tokens-saved into a USD estimate using Sonnet input pricing.
/// Sonnet is the default agent target; output-token savings are not relevant
/// for retrieval savings.
pub(crate) fn estimate_dollars_saved(saved_tokens: u64) -> f64 {
    use tokensave::accounting::pricing;
    pricing::refresh_if_stale();
    let price = pricing::lookup("claude-sonnet-4")
        .map(|p| p.input_per_mtok)
        .unwrap_or(3.0);
    (saved_tokens as f64) * price / 1_000_000.0
}

pub async fn handle_gain(
    all: bool,
    history: bool,
    range: &str,
    json_output: bool,
) -> tokensave::errors::Result<()> {
    let gdb = match tokensave::global_db::GlobalDb::open().await {
        Some(db) => db,
        None => {
            eprintln!("Could not open the global database (~/.tokensave/global.db).");
            return Ok(());
        }
    };

    let since = tokensave::accounting::metrics::parse_range(range);
    let project_filter: Option<String> = if all {
        None
    } else {
        std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    };

    if history {
        let rows = gdb
            .savings_history(project_filter.as_deref(), since as i64)
            .await;
        if json_output {
            let arr: Vec<_> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "day": r.day,
                        "saved_tokens": r.saved_tokens,
                        "calls": r.calls,
                        "usd": estimate_dollars_saved(r.saved_tokens),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        } else {
            tokensave::display::print_gain_history(&rows, estimate_dollars_saved);
        }
        return Ok(());
    }

    let total = gdb
        .sum_savings(project_filter.as_deref(), since as i64)
        .await;
    let usd = estimate_dollars_saved(total.saved_tokens);

    if json_output {
        let out = serde_json::json!({
            "range": range,
            "project": project_filter.clone().unwrap_or_else(|| "ALL".to_string()),
            "saved_tokens": total.saved_tokens,
            "calls": total.calls,
            "usd": usd,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        tokensave::display::print_gain_total(
            project_filter.as_deref().unwrap_or("ALL projects"),
            range,
            total.saved_tokens,
            total.calls,
            usd,
        );
    }
    Ok(())
}

/// Handle `tokensave discover`: surface file-navigation turns that a tokensave
/// graph query could have served far more cheaply.
///
/// Ingests any new session data first (mirroring `tokensave cost`), then runs
/// the deterministic [`tokensave::accounting::discover::analyze`] over the
/// `turns` table for the requested range and prints a ranked summary. The
/// recoverable figure is a clearly-labeled conservative lower bound; see the
/// `discover` module docs for the estimation assumptions.
pub async fn handle_discover(since: &str, json_output: bool) -> tokensave::errors::Result<()> {
    use tokensave::accounting::discover;

    let gdb = match tokensave::global_db::GlobalDb::open().await {
        Some(db) => db,
        None => {
            eprintln!("Could not open the global database (~/.tokensave/global.db).");
            return Ok(());
        }
    };

    // Ingest new session data before querying, same as `tokensave cost`.
    let ingest_stats = tokensave::accounting::parser::ingest(&gdb).await;
    if ingest_stats.turns_inserted > 0 {
        eprintln!(
            "Ingested {} new turns from Claude Code sessions.",
            ingest_stats.turns_inserted
        );
    }

    let since_ts = tokensave::accounting::metrics::parse_range(since);
    let rows = gdb.nav_turns_since(since_ts).await;
    let report = discover::analyze(&rows);

    if json_output {
        let buckets: Vec<_> = report
            .buckets
            .iter()
            .map(|b| {
                serde_json::json!({
                    "bucket": b.bucket.as_str(),
                    "tool": b.bucket.tool_name(),
                    "suggestion": b.bucket.suggestion(),
                    "turns": b.turns,
                    "turns_with_measured_sizes": b.turns_with_measured_sizes,
                    "addressable_input_tokens": b.addressable_input_tokens,
                    "recoverable_input_tokens": b.recoverable_input_tokens(),
                })
            })
            .collect();
        let out = serde_json::json!({
            "since": since,
            "recoverable_fraction": discover::RECOVERABLE_FRACTION,
            "total_turns": report.total_turns,
            "replaceable_turns": report.total_replaceable_turns(),
            "turns_with_measured_sizes": report.total_turns_with_measured_sizes(),
            "total_addressable_input_tokens": report.total_addressable_input_tokens(),
            "total_recoverable_input_tokens": report.total_recoverable_input_tokens(),
            "buckets": buckets,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return Ok(());
    }

    println!("Missed-opportunity scan (since: {since})");
    if report.buckets.is_empty() {
        println!(
            "  No replaceable file-navigation turns found in {} examined turns.",
            report.total_turns
        );
        return Ok(());
    }

    println!(
        "  {:<8} {:<32} {:>6} {:>14} {:>14}",
        "Tool", "TokenSave alternative", "Turns", "Addressable", "Recoverable"
    );
    for b in &report.buckets {
        println!(
            "  {:<8} {:<32} {:>6} {:>14} {:>14}",
            b.bucket.tool_name(),
            b.bucket.suggestion(),
            b.turns,
            tokensave::display::format_token_count(b.addressable_input_tokens),
            tokensave::display::format_token_count(b.recoverable_input_tokens()),
        );
    }

    println!();
    println!(
        "  {} navigation turns; addressable input tokens \u{2248} {}.",
        report.total_replaceable_turns(),
        tokensave::display::format_token_count(report.total_addressable_input_tokens()),
    );
    println!(
        "  Conservative recoverable \u{2248} {} (lower bound: {:.0}% of addressable; \
         excludes Bash grep/find/cat/rg, whose command text is not stored).",
        tokensave::display::format_token_count(report.total_recoverable_input_tokens()),
        discover::RECOVERABLE_FRACTION * 100.0,
    );

    // Turns ingested before #474 carry no tool-result size, so a range reaching
    // back before the upgrade reports navigation turns worth zero tokens. Say
    // why, or the honest "nothing measured here yet" reads as the very bug
    // #474 reported — a figure that is implausibly small.
    //
    // Which turns carry a size is a fact to read, not to infer from the total
    // being zero: that inference called a genuinely-measured zero "unknown",
    // and said nothing at all about a range straddling the upgrade, where the
    // total is a real but partial figure (#523).
    let replaceable = report.total_replaceable_turns();
    let measured = report.total_turns_with_measured_sizes();
    if replaceable > 0 && measured == 0 {
        println!(
            "  These turns were recorded before tool-result sizes were measured, so \
             their addressable total is unknown rather than zero. Turns ingested from \
             now on carry it."
        );
    } else if measured < replaceable {
        println!(
            "  {} of {replaceable} of these turns were recorded before tool-result sizes \
             were measured, so the addressable total above counts the other {measured} \
             and is a lower bound.",
            replaceable - measured
        );
    }

    Ok(())
}

/// Cap on how many per-extension entries the compact skipped summary lists
/// before rolling the rest into a single "and N more" tail.
const MAX_SUMMARY_EXTS: usize = 5;

/// Build the compact skipped-file headline shown after `init` and `sync`
/// (#345), or `None` when nothing was skipped.
///
/// Aggregated by extension so a large repository never prints thousands of
/// paths; `--doctor` and `--verbose` remain the detailed modes.
pub(crate) fn skipped_extension_headline(skipped: &[(String, usize)]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let total: usize = skipped.iter().map(|(_, count)| count).sum();
    let listed = skipped
        .iter()
        .take(MAX_SUMMARY_EXTS)
        .map(|(ext, count)| format!(".{ext} ({count})"))
        .collect::<Vec<_>>()
        .join(", ");
    let rest = skipped.len().saturating_sub(MAX_SUMMARY_EXTS);
    let more = if rest > 0 {
        format!(", and {rest} more extension(s)")
    } else {
        String::new()
    };
    let plural = if total == 1 { "" } else { "s" };
    Some(format!(
        "Skipped {total} tracked file{plural} (unsupported extension): {listed}{more}"
    ))
}

/// Print the compact skipped-file summary after `init` or `sync` so an index
/// that omitted unsupported languages cannot be mistaken for a complete one.
pub(crate) fn print_skipped_extension_summary(skipped: &[(String, usize)]) {
    if let Some(headline) = skipped_extension_headline(skipped) {
        eprintln!();
        eprintln!("\x1b[33m{headline}\x1b[0m");
        eprintln!(
            // Naming `artifact_extensions` here is the difference between a
            // dead end and a next step (#442): a text format with no extractor
            // still gets a `files` row when listed there, and a row is what
            // makes it reachable by `tokensave_files` and by literal search.
            "\x1b[2m  No extractor is registered for these extensions, so no symbols were \
             indexed from them.\n  Map source extensions with \"language_aliases\" in .tokensave/config.json\n  (for example {{\".t\": \"perl\"}}, when Perl is enabled), or use project.json\n  entries with a language for file/glob overrides; then run tokensave sync -f.\n  A text format worth searching by path or content \
             (templates, stylesheets, configs) can be added to \"artifact_extensions\" in \
             .tokensave/config.json — tracked by path, never parsed.\n  Rerun with --doctor \
             (or --verbose) for the full list.\x1b[0m"
        );
    }
}

/// Print the `--doctor` report after an incremental sync.
pub(crate) fn print_sync_doctor(result: &tokensave::tokensave::SyncResult) {
    // Unsupported-extension summary (#262, #270): makes "not indexed because
    // no extractor exists" distinguishable from a wrong project root, an
    // ignore rule, or a stale index.
    if !result.skipped_extensions.is_empty() {
        eprintln!("\n\x1b[33mSkipped extensions (no registered extractor):\x1b[0m");
        for (ext, count) in &result.skipped_extensions {
            let hint = match ext.as_str() {
                "patch" | "ldif" | "jsonnet" | "libsonnet" => "text format: opt into artifact_extensions (no symbols)",
                "pem" => "credential material: exclude; avoid content indexing",
                _ => "source: configure language_aliases or project.json with an enabled language; text: artifact_extensions",
            };
            eprintln!("  .{ext}: {count} file(s) — {hint}");
        }
    }
    let has_changes = !result.added_paths.is_empty()
        || !result.modified_paths.is_empty()
        || !result.removed_paths.is_empty();
    if !has_changes {
        eprintln!("\n\x1b[2mNo files changed.\x1b[0m");
        return;
    }
    eprintln!();
    if !result.added_paths.is_empty() {
        eprintln!("\x1b[32mAdded ({}):\x1b[0m", result.added_paths.len());
        for p in &result.added_paths {
            eprintln!("  + {p}");
        }
    }
    if !result.modified_paths.is_empty() {
        eprintln!("\x1b[33mModified ({}):\x1b[0m", result.modified_paths.len());
        for p in &result.modified_paths {
            eprintln!("  ~ {p}");
        }
    }
    if !result.removed_paths.is_empty() {
        eprintln!("\x1b[31mRemoved ({}):\x1b[0m", result.removed_paths.len());
        for p in &result.removed_paths {
            eprintln!("  - {p}");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod gain_tests {
    use super::estimate_dollars_saved;

    #[test]
    fn dollars_uses_sonnet_input_price_by_default() {
        // 1_000_000 tokens × $3 / MTok = $3.00 (Sonnet input price)
        let usd = estimate_dollars_saved(1_000_000);
        assert!((usd - 3.0).abs() < 0.01, "expected ~$3.00, got ${usd}");
    }

    #[test]
    fn dollars_handles_small_counts() {
        // 1_000 tokens × $3 / MTok = $0.003
        let usd = estimate_dollars_saved(1_000);
        assert!((usd - 0.003).abs() < 0.001);
    }

    #[test]
    fn dollars_zero_for_zero_tokens() {
        assert_eq!(estimate_dollars_saved(0), 0.0);
    }
}

#[cfg(test)]
mod skipped_summary_tests {
    use super::skipped_extension_headline;

    fn skipped(entries: &[(&str, usize)]) -> Vec<(String, usize)> {
        entries
            .iter()
            .map(|(ext, count)| ((*ext).to_string(), *count))
            .collect()
    }

    #[test]
    fn no_headline_when_nothing_skipped() {
        assert_eq!(skipped_extension_headline(&[]), None);
    }

    #[test]
    fn single_file_uses_singular_wording() {
        assert_eq!(
            skipped_extension_headline(&skipped(&[("v", 1)])).unwrap(),
            "Skipped 1 tracked file (unsupported extension): .v (1)"
        );
    }

    #[test]
    fn totals_are_summed_across_extensions() {
        let headline = skipped_extension_headline(&skipped(&[("v", 2), ("sv", 1)])).unwrap();
        assert_eq!(
            headline,
            "Skipped 3 tracked files (unsupported extension): .v (2), .sv (1)"
        );
    }

    #[test]
    fn long_lists_are_rolled_up() {
        let headline = skipped_extension_headline(&skipped(&[
            ("a", 1),
            ("b", 1),
            ("c", 1),
            ("d", 1),
            ("e", 1),
            ("f", 1),
            ("g", 1),
        ]))
        .unwrap();
        assert!(
            headline.ends_with(".e (1), and 2 more extension(s)"),
            "got: {headline}"
        );
        assert!(
            headline.starts_with("Skipped 7 tracked files"),
            "got: {headline}"
        );
    }
}

/// Handles a git `post-checkout` event (#342 Q1).
///
/// This is the branching that used to live inline in the installed hook
/// script. Moving it into the binary means the hook file itself is one
/// delegating line that never has to be rewritten again: every later change to
/// what a checkout triggers ships with the binary. It is also testable here,
/// which it was not as shell.
///
/// The semantics are carried over unchanged:
///
/// * git reports the initial checkout of a fresh clone — and of every new
///   `git worktree add` — by passing the all-zeros sentinel as the previous
///   HEAD. That checkout is **also** a branch checkout and can land on a
///   branch that is not the default one (`git clone -b feature`,
///   `git worktree add -b feature`), so it runs `init` and **then** tracks the
///   branch. Sequentially, because tracking copies the index `init` creates.
/// * Any other branch checkout (`branch_flag == "1"`) tracks the
///   just-checked-out branch alone.
/// * File checkouts (`branch_flag == "0"`) trigger nothing.
///
/// Tracking always goes through the `auto_track` gate (#397), so the knob
/// stays authoritative on this path. Silent by design and never fails the
/// checkout: a hook runs on every branch switch and must neither narrate nor
/// be able to break `git checkout`.
pub async fn hook_post_checkout(prev_head: Option<&str>, branch_flag: Option<&str>) {
    use tokensave::agents::hooks::{classify_checkout, is_under_temp_dir, CheckoutAction};

    let project_path = tokensave::config::resolve_path(None);

    // #569: under a `Global` hook install, this also fires inside ephemeral
    // clones unrelated tools (CocoaPods, npm, ...) stage in the system temp
    // dir and expect to own exclusively. Canonicalize both sides first: on
    // macOS `/tmp` is a symlink to `/private/tmp`, and comparing the
    // un-resolved forms would never match.
    let system_temp = std::env::temp_dir();
    let system_temp = std::fs::canonicalize(&system_temp).unwrap_or(system_temp);
    let canonical_project_path =
        std::fs::canonicalize(&project_path).unwrap_or_else(|_| project_path.clone());
    if is_under_temp_dir(&canonical_project_path, &system_temp) {
        return;
    }

    match classify_checkout(prev_head, branch_flag) {
        CheckoutAction::InitThenTrack => {
            // `init` on an already-initialised project returns an error;
            // either way tracking still runs, matching the shell's
            // `init || exit 0` followed by the track.
            let _ = init_and_index(&project_path, &[], false, None).await;
            track_current_branch_if_enabled(&project_path).await;
        }
        CheckoutAction::TrackOnly => track_current_branch_if_enabled(&project_path).await,
        CheckoutAction::Nothing => {}
    }
}

/// Tracks the current branch when `auto_track` allows it, swallowing every
/// failure. Shared by both arms of [`hook_post_checkout`].
async fn track_current_branch_if_enabled(project_path: &std::path::Path) {
    let config = tokensave::config::load_config(project_path).unwrap_or_default();
    if !tokensave::config::env_bool_override("TOKENSAVE_AUTO_TRACK", config.auto_track) {
        return;
    }
    let _ = handle_branch_action(BranchAction::Add {
        name: None,
        path: Some(project_path.display().to_string()),
        if_enabled: true,
    })
    .await;
}
