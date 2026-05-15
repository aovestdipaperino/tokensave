use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::cli::BranchAction;
use crate::global;
use crate::Spinner;
use tokensave::tokensave::TokenSave;

pub(crate) async fn handle_branch_action(action: BranchAction) -> tokensave::errors::Result<()> {
    use tokensave::branch;
    use tokensave::branch_meta;
    use tokensave::config::get_tokensave_dir;

    match action {
        BranchAction::List { path } => {
            let project_path = tokensave::config::resolve_path(path);
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
        BranchAction::Add { name, path } => {
            let project_path = tokensave::config::resolve_path(path);
            let tokensave_dir = get_tokensave_dir(&project_path);

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

            // Copy DB
            let sanitized = branch::sanitize_branch_name(&branch_name);
            let branches_dir = branch_meta::ensure_branches_dir(&tokensave_dir)?;
            let new_db_path = branches_dir.join(format!("{sanitized}.db"));
            let spinner = Spinner::new();
            spinner.set_message(&format!("copying DB from '{parent}'"));
            std::fs::copy(&parent_db, &new_db_path)?;

            // Save metadata BEFORE open() so it resolves the new branch to its DB
            let db_file = format!("branches/{sanitized}.db");
            meta.add_branch(&branch_name, &db_file, &parent);
            branch_meta::save_branch_meta(&tokensave_dir, &meta)?;

            // Run incremental sync (hash-based delta) against the new branch DB
            spinner.set_message("syncing changes");
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
            let project_path = tokensave::config::resolve_path(path);
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
            let project_path = tokensave::config::resolve_path(path);
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
            let project_path = tokensave::config::resolve_path(path);
            let tokensave_dir = get_tokensave_dir(&project_path);
            let Some(mut meta) = branch_meta::load_branch_meta(&tokensave_dir) else {
                eprintln!("No branch tracking configured.");
                return Ok(());
            };

            // Find branches in metadata that no longer exist in git
            let stale: Vec<String> = meta
                .branches
                .keys()
                .filter(|name| *name != &meta.default_branch)
                .filter(|name| {
                    let ref_path = project_path.join(format!(".git/refs/heads/{name}"));
                    let packed = project_path.join(".git/packed-refs");
                    let suffix = format!("refs/heads/{name}");
                    let in_packed = packed.exists()
                        && std::fs::read_to_string(&packed)
                            .map(|c| c.lines().any(|line| line.ends_with(&suffix)))
                            .unwrap_or(false);
                    !ref_path.exists() && !in_packed
                })
                .cloned()
                .collect();

            if stale.is_empty() {
                eprintln!("No stale branches to clean up.");
            } else {
                for name in &stale {
                    if let Some(entry) = meta.remove_branch(name) {
                        let db_path = tokensave_dir.join(&entry.db_file);
                        if db_path.exists() {
                            std::fs::remove_file(&db_path)?;
                            let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
                            let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
                        }
                        eprintln!("  removed '{name}'");
                    }
                }
                branch_meta::save_branch_meta(&tokensave_dir, &meta)?;
                eprintln!(
                    "\x1b[32m✔\x1b[0m Cleaned up {} stale branch(es).",
                    stale.len()
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

    let home_tokensave: Option<PathBuf> = dirs::home_dir().map(|h| h.join(".tokensave"));

    let mut targets = global::gather_target_projects(all, &home_tokensave).await;
    if all {
        // wipe acts on the live `.tokensave/` directory; drop rows whose
        // directory is already gone (they're handled by `tokensave doctor`).
        targets.retain(|p| p.join(".tokensave/tokensave.db").exists());
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
        let ts_dir = project_root.join(".tokensave");
        if !ts_dir.exists() {
            continue;
        }
        match fs::remove_dir_all(&ts_dir) {
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

    let home_tokensave: Option<PathBuf> = dirs::home_dir().map(|h| h.join(".tokensave"));
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
        let ts_dir = path.join(".tokensave");
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

/// When invoked with no subcommand, offer to create the index if none exists.
pub(crate) async fn handle_no_command() -> tokensave::errors::Result<()> {
    let project_path = tokensave::config::resolve_path(None);
    if TokenSave::is_initialized(&project_path) {
        // Already initialized — show help via clap
        let _ = <crate::cli::Cli as clap::CommandFactory>::command().print_help();
        eprintln!();
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
        init_and_index(&project_path, &[], false).await?;
    }
    Ok(())
}

/// Initializes a new project (if needed) and runs a full index.
pub(crate) async fn init_and_index(
    project_path: &Path,
    skip_folders: &[String],
    verbose: bool,
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
        TokenSave::open(project_path).await?
    } else {
        let cg = TokenSave::init(project_path).await?;
        eprintln!("Initialized TokenSave at {}", project_path.display());
        // Offer to add .tokensave to .gitignore if not already there
        if !tokensave::config::is_in_gitignore(project_path) {
            eprint!("Add .tokensave to .gitignore? [Y/n] ");
            io::stderr().flush().ok();
            let mut answer = String::new();
            if io::stdin().lock().read_line(&mut answer).is_ok() {
                let answer = answer.trim();
                if answer.is_empty() || answer.eq_ignore_ascii_case("y") {
                    tokensave::config::add_to_gitignore(project_path);
                    eprintln!("Added .tokensave to .gitignore");
                }
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
        std::env::current_dir().ok().map(|p| p.to_string_lossy().into_owned())
    };

    if history {
        let rows = gdb.savings_history(project_filter.as_deref(), since as i64).await;
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

    let total = gdb.sum_savings(project_filter.as_deref(), since as i64).await;
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

/// Print the `--doctor` report after an incremental sync.
pub(crate) fn print_sync_doctor(result: &tokensave::tokensave::SyncResult) {
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
