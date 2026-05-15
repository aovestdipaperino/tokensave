// Rust guideline compliant 2025-10-17
// Updated 2026-03-23: compact bordered table for status output
use clap::Parser;
use std::io::{self, BufRead, Write};
use std::process;

use tokensave::context::{format_context_as_json, format_context_as_markdown};
use tokensave::tokensave::TokenSave;
use tokensave::types::*;

mod cli;
mod commands;
mod global;
mod serve;

use cli::*;

/// Alias for the shared timestamp utility.
pub(crate) fn current_unix_timestamp() -> i64 {
    tokensave::tokensave::current_timestamp()
}

/// A self-animating spinner that ticks on a background thread.
/// Call `set_message` to update what is displayed; the background thread
/// redraws at ~80 ms intervals. Call `done` to stop and print a final line.
pub(crate) struct Spinner {
    message: std::sync::Arc<std::sync::Mutex<String>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    pub(crate) fn new() -> Self {
        let message = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let msg = message.clone();
        let stp = stop.clone();
        // Hide cursor while spinner is active.
        let _ = write!(std::io::stderr(), "\x1b[?25l");
        let _ = std::io::stderr().flush();
        let handle = std::thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut idx = 0usize;
            while !stp.load(std::sync::atomic::Ordering::Relaxed) {
                let text = msg.lock().unwrap().clone();
                if !text.is_empty() {
                    let frame = frames[idx % frames.len()];
                    idx += 1;
                    // Truncate to avoid line wrapping on typical terminals.
                    let display: std::borrow::Cow<str> = if text.len() > 50 {
                        format!("…{}", &text[text.len() - 49..]).into()
                    } else {
                        text.as_str().into()
                    };
                    let mut stderr = std::io::stderr();
                    let _ = write!(stderr, "\r\x1b[2K{} {}", frame, display);
                    let _ = stderr.flush();
                }
                std::thread::sleep(std::time::Duration::from_millis(80));
            }
        });
        Self {
            message,
            stop,
            handle: Some(handle),
        }
    }

    pub(crate) fn set_message(&self, msg: &str) {
        *self.message.lock().unwrap() = msg.to_string();
    }

    pub(crate) fn done(mut self, message: &str) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let mut stderr = std::io::stderr();
        // Show cursor again, then print the done line.
        let _ = write!(stderr, "\x1b[?25h");
        let _ = writeln!(stderr, "\r\x1b[2K\x1b[32m✔\x1b[0m {}", message);
        let _ = stderr.flush();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        // If the spinner wasn't explicitly finished (e.g. `?` propagated an
        // error), still stop the thread, clear the line, and restore the
        // cursor so the terminal is left in a sane state.
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let mut stderr = std::io::stderr();
        let _ = write!(stderr, "\r\x1b[2K\x1b[?25h");
        let _ = stderr.flush();
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run(cli: Cli) -> tokensave::errors::Result<()> {
    let command = match cli.command {
        Some(cmd) => cmd,
        None => return commands::handle_no_command().await,
    };

    // Worker mode bypasses every normal startup path (no config load, no
    // worldwide-counter ping, no agent checks). The token handshake inside
    // run_worker is the only authentication; this dispatch must happen
    // before anything else can side-effect on disk or network.
    if matches!(command, Commands::ExtractWorker) {
        tokensave::extraction_worker::run_worker();
    }

    // First-run notice (check BEFORE any config save creates the file)
    let is_first_run = tokensave::user_config::UserConfig::is_fresh();

    // Best-effort flush of pending worldwide counter tokens.
    // `matches!` borrows `command` temporarily; the borrow is dropped
    // before the `match command` move below, so this compiles.
    let is_force_flush = matches!(
        command,
        Commands::Init { .. } | Commands::Sync { .. } | Commands::Status { .. }
    );
    let mut user_config = tokensave::user_config::UserConfig::load();
    global::try_flush(&mut user_config, is_force_flush);
    user_config.save();

    if is_first_run {
        eprintln!(
            "note: tokensave uploads anonymous token-saved counts to a worldwide counter.\n\
             \x20     Run `tokensave disable-upload-counter` to opt out."
        );
    }

    // The "beta merged into stable" nudge that lived here through 4.3.x was
    // retired in 4.3.12. The beta channel is open again as of v5.0.0-beta.1
    // and beta users now stay on beta until they explicitly switch off.

    // Best-effort check: warn if install needs re-running
    if !matches!(command, Commands::Install { .. } | Commands::Reinstall) {
        tokensave::agents::claude::check_install_stale();
    }

    // Silent reinstall: if the running version is newer than the one that last
    // installed agents, re-run the install for every tracked agent so that
    // permissions, hooks, and MCP config stay in sync with the new binary.
    if !matches!(
        command,
        Commands::Install { .. } | Commands::Reinstall | Commands::Uninstall { .. }
    ) {
        let running = env!("CARGO_PKG_VERSION");
        if !user_config.installed_agents.is_empty()
            && !running.is_empty()
            && (user_config.last_installed_version.is_empty()
                || tokensave::cloud::is_newer_version(&user_config.last_installed_version, running))
        {
            if let (Some(home), Some(bin)) = (
                tokensave::agents::home_dir(),
                tokensave::agents::which_tokensave(),
            ) {
                let mut all_ok = true;
                for id in &user_config.installed_agents {
                    if let Ok(ag) = tokensave::agents::get_integration(id) {
                        let ctx = tokensave::agents::InstallContext {
                            home: home.clone(),
                            tokensave_bin: bin.clone(),
                            tool_permissions: tokensave::agents::expected_tool_perms(),
                        };
                        if ag.install(&ctx).is_err() {
                            all_ok = false;
                        }
                    }
                }
                if all_ok {
                    user_config.last_installed_version = running.to_string();
                    user_config.save();
                }
            }
        }
    }

    match command {
        Commands::Init { path, skip_folders } => {
            let project_path = tokensave::config::resolve_path(path);
            if TokenSave::is_initialized(&project_path) {
                eprintln!(
                    "\x1b[31merror:\x1b[0m TokenSave is already initialized at '{}'.\n\
                     Use \x1b[1mtokensave sync\x1b[0m to update the index, or \
                     \x1b[1mtokensave sync --force\x1b[0m to rebuild it.",
                    project_path.display()
                );
                std::process::exit(1);
            }
            // Check for updates in parallel with indexing
            let version_handle = std::thread::spawn(tokensave::cloud::fetch_latest_version);
            commands::init_and_index(&project_path, &skip_folders, false).await?;

            // Print update notice from parallel check (suppressed for 15 min)
            if let Ok(Some(latest)) = version_handle.join() {
                let current_version = env!("CARGO_PKG_VERSION");
                let now = current_unix_timestamp();
                let mut config = tokensave::user_config::UserConfig::load();
                config.cached_latest_version = latest.clone();
                config.last_version_check_at = now;
                config.save();
                if tokensave::cloud::is_newer_version(current_version, &latest)
                    && now - config.last_version_warning_at >= 900
                {
                    eprintln!(
                        "\n\x1b[33mUpdate available: v{} → v{}\x1b[0m\n  Run: \x1b[1mtokensave upgrade\x1b[0m",
                        current_version, latest
                    );
                    config.last_version_warning_at = now;
                    config.save();
                }
            }
        }
        Commands::Sync {
            path,
            force,
            skip_folders,
            doctor,
            verbose,
        } => {
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            if !TokenSave::is_initialized(&project_path) {
                eprintln!(
                    "\x1b[31merror:\x1b[0m no TokenSave index found at '{}'.\n\
                     Run \x1b[1mtokensave init\x1b[0m to create one first.",
                    project_path.display()
                );
                std::process::exit(1);
            }
            // Warn if legacy .codegraph directory exists
            if project_path.join(".codegraph").is_dir() {
                eprintln!(
                    "warning: found legacy .codegraph/ directory at '{}'. \
                     tokensave now uses .tokensave/ — the old directory can be safely deleted.",
                    project_path.display()
                );
            }
            // Check for updates in parallel with indexing
            let version_handle = std::thread::spawn(tokensave::cloud::fetch_latest_version);

            if force {
                commands::init_and_index(&project_path, &skip_folders, verbose).await?;
            } else {
                let mut cg = TokenSave::open(&project_path).await?;
                cg.add_skip_folders(&skip_folders);
                let spinner = Spinner::new();
                let sync_start = std::time::Instant::now();
                let result = cg
                    .sync_with_progress_verbose(
                        |current, total, detail| {
                            if current == 0 {
                                // Phase message (scanning, hashing, detecting, resolving)
                                spinner.set_message(detail);
                            } else {
                                // Per-file progress with ETA
                                let elapsed = sync_start.elapsed().as_secs_f64();
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
                                spinner.set_message(&format!(
                                    "[{current}/{total}] syncing {detail}{eta}"
                                ));
                            }
                        },
                        |msg| {
                            if verbose {
                                eprintln!("  \x1b[2m[verbose]\x1b[0m {msg}");
                            }
                        },
                    )
                    .await?;
                let skipped_msg = if result.skipped_paths.is_empty() {
                    String::new()
                } else {
                    format!(", {} skipped", result.skipped_paths.len())
                };
                spinner.done(&format!(
                    "sync done — {} added, {} modified, {} removed{skipped_msg} in {}ms",
                    result.files_added,
                    result.files_modified,
                    result.files_removed,
                    result.duration_ms
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
                if doctor {
                    commands::print_sync_doctor(&result);
                }
                global::update_global_db(&cg).await;
            }

            // Print update notice from parallel check (suppressed for 15 min)
            if let Ok(Some(latest)) = version_handle.join() {
                let current_version = env!("CARGO_PKG_VERSION");
                let now = current_unix_timestamp();
                let mut config = tokensave::user_config::UserConfig::load();
                config.cached_latest_version = latest.clone();
                config.last_version_check_at = now;
                config.save();
                if tokensave::cloud::is_newer_version(current_version, &latest)
                    && now - config.last_version_warning_at >= 900
                {
                    eprintln!(
                        "\n\x1b[33mUpdate available: v{} → v{}\x1b[0m\n  Run: \x1b[1mtokensave upgrade\x1b[0m",
                        current_version, latest
                    );
                    config.last_version_warning_at = now;
                    config.save();
                }
            }
        }
        Commands::Status {
            path,
            json,
            short,
            details,
        } => {
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            let cg = if TokenSave::is_initialized(&project_path) {
                TokenSave::open(&project_path).await?
            } else {
                eprint!(
                    "No TokenSave index found at '{}'. Create one now? [Y/n] ",
                    project_path.display()
                );
                io::stderr().flush().ok();
                let mut answer = String::new();
                io::stdin().lock().read_line(&mut answer).map_err(|e| {
                    tokensave::errors::TokenSaveError::Config {
                        message: format!("failed to read stdin: {e}"),
                    }
                })?;
                let answer = answer.trim();
                if answer.is_empty() || answer.eq_ignore_ascii_case("y") {
                    commands::init_and_index(&project_path, &[], false).await?
                } else {
                    return Ok(());
                }
            };
            let stats = cg.get_stats().await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&stats).unwrap_or_default()
                );
            } else {
                let tokens_saved = cg.get_tokens_saved().await.unwrap_or(0);
                // Register project and read global total in one open.
                // Subtract this project's count so "Global" means "all other projects".
                let gdb = tokensave::global_db::GlobalDb::open().await;
                let global_tokens_saved = match &gdb {
                    Some(db) => {
                        db.upsert(&project_path, tokens_saved).await;
                        db.global_tokens_saved()
                            .await
                            .map(|total| total.saturating_sub(tokens_saved))
                            .filter(|&other| other > 0)
                    }
                    None => None,
                };
                // Fetch worldwide total (1s timeout, 60s client cache TTL)
                let mut config = tokensave::user_config::UserConfig::load();
                let now = current_unix_timestamp();
                let worldwide = if now - config.last_worldwide_fetch_at < 60 {
                    // Use cached value
                    if config.last_worldwide_total > 0 {
                        Some(config.last_worldwide_total)
                    } else {
                        None
                    }
                } else if let Some(total) = tokensave::cloud::fetch_worldwide_total() {
                    config.last_worldwide_total = total;
                    config.last_worldwide_fetch_at = now;
                    config.save();
                    Some(total)
                } else if config.last_worldwide_total > 0 {
                    Some(config.last_worldwide_total) // fallback to cache
                } else {
                    None
                };
                // Fetch country flags (30 min cache)
                let country_flags = if now - config.last_flags_fetch_at < 1800 {
                    config.cached_country_flags.clone()
                } else {
                    let fresh = tokensave::cloud::fetch_country_flags();
                    if !fresh.is_empty() {
                        config.cached_country_flags = fresh.clone();
                        config.last_flags_fetch_at = now;
                        config.save();
                    }
                    if fresh.is_empty() && !config.cached_country_flags.is_empty() {
                        config.cached_country_flags.clone()
                    } else {
                        fresh
                    }
                };
                if !short {
                    print!("{}", include_str!("resources/logo.ansi"));
                }
                let branch_info = cg.active_branch().map(|_| {
                    let ts_dir = tokensave::config::get_tokensave_dir(&project_path);
                    let meta = tokensave::branch_meta::load_branch_meta(&ts_dir);
                    let has_tracking = meta.as_ref().is_some_and(|m| !m.branches.is_empty());
                    let display_branch = if has_tracking {
                        cg.serving_branch().unwrap_or("[single-db]").to_string()
                    } else {
                        "[single-db]".to_string()
                    };
                    let parent =
                        meta.and_then(|m| m.branches.get(cg.serving_branch()?)?.parent.clone());
                    tokensave::display::BranchInfo {
                        branch: display_branch,
                        parent,
                        is_fallback: cg.is_fallback(),
                    }
                });
                // Ingest new session data so cost info is up-to-date.
                if let Some(ref db) = gdb {
                    tokensave::accounting::parser::ingest(db).await;
                }
                // Best-effort cost summary for the status header.
                let cost_info = match &gdb {
                    Some(db) => {
                        tokensave::accounting::quick_cost_summary(
                            db,
                            tokens_saved,
                            global_tokens_saved.unwrap_or(0),
                        )
                        .await
                    }
                    None => None,
                };
                if short {
                    tokensave::display::print_status_header(
                        &stats,
                        tokens_saved,
                        global_tokens_saved,
                        worldwide,
                        &country_flags,
                        branch_info.as_ref(),
                        cost_info.as_ref(),
                    );
                } else {
                    tokensave::display::print_status_table(
                        &stats,
                        tokens_saved,
                        global_tokens_saved,
                        worldwide,
                        &country_flags,
                        branch_info.as_ref(),
                        cost_info.as_ref(),
                        details,
                    );
                }

                // Warn if .tokensave is not in .gitignore
                if !tokensave::config::is_in_gitignore(&project_path) {
                    eprintln!(
                        "\n\x1b[33mWarning: .tokensave is not in .gitignore — \
                         run `echo .tokensave >> .gitignore` to exclude it from git.\x1b[0m"
                    );
                }

                // Version check (5 min cache, always show for status)
                global::check_for_update(&mut config, false, true);
            }
        }
        Commands::Query {
            search,
            path,
            limit,
        } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;
            let results = cg.search(&search, limit).await?;
            if results.is_empty() {
                println!("No results found for '{}'", search);
            } else {
                for r in &results {
                    println!(
                        "{} ({}) - {}:{}",
                        r.node.name,
                        r.node.kind.as_str(),
                        r.node.file_path,
                        r.node.start_line
                    );
                    if let Some(sig) = &r.node.signature {
                        println!("  {}", sig);
                    }
                }
            }
        }
        Commands::Context {
            task,
            path,
            max_nodes,
            format,
        } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;
            let output_format = if format == "json" {
                OutputFormat::Json
            } else {
                OutputFormat::Markdown
            };
            let options = BuildContextOptions {
                max_nodes,
                format: output_format.clone(),
                ..Default::default()
            };
            let context = cg.build_context(&task, &options).await?;
            match output_format {
                OutputFormat::Json => {
                    println!("{}", format_context_as_json(&context));
                }
                OutputFormat::Markdown => {
                    println!("{}", format_context_as_markdown(&context));
                }
            }
        }
        Commands::Files {
            path,
            filter,
            pattern,
            json,
        } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;
            let mut files = cg.get_all_files().await?;
            files.sort_by(|a, b| a.path.cmp(&b.path));

            // Apply directory prefix filter
            if let Some(ref dir) = filter {
                let prefix = if dir.ends_with('/') {
                    dir.clone()
                } else {
                    format!("{}/", dir)
                };
                files.retain(|f| f.path.starts_with(&prefix) || f.path == dir.as_str());
            }

            // Apply glob pattern filter
            if let Some(ref pat) = pattern {
                if let Ok(glob) = glob::Pattern::new(pat) {
                    files.retain(|f| glob.matches(&f.path));
                } else {
                    eprintln!("warning: invalid glob pattern '{}', ignoring", pat);
                }
            }

            if json {
                let items: Vec<serde_json::Value> = files
                    .iter()
                    .map(|f| {
                        serde_json::json!({
                            "path": f.path,
                            "size": f.size,
                            "node_count": f.node_count,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&items).unwrap_or_default()
                );
            } else {
                println!("{} indexed files", files.len());
                for f in &files {
                    println!("  {} ({} bytes, {} symbols)", f.path, f.size, f.node_count);
                }
            }
        }
        Commands::Affected {
            files,
            path,
            stdin,
            depth,
            filter,
            json,
            quiet,
        } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;

            // Collect changed files from args and/or stdin
            let mut changed: Vec<String> = files;
            if stdin {
                let stdin_handle = io::stdin();
                for line in stdin_handle.lock().lines().map_while(Result::ok) {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() {
                        changed.push(trimmed);
                    }
                }
            }

            if changed.is_empty() {
                eprintln!("No files specified. Pass file paths as arguments or use --stdin.");
                return Ok(());
            }

            let affected =
                serve::find_affected_tests(&cg, &changed, depth, filter.as_deref()).await?;

            if json {
                let output = serde_json::json!({
                    "changed_files": changed,
                    "affected_tests": affected,
                    "count": affected.len(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).unwrap_or_default()
                );
            } else if quiet {
                for f in &affected {
                    println!("{}", f);
                }
            } else {
                if affected.is_empty() {
                    println!("No affected test files found.");
                } else {
                    println!("{} affected test file(s):", affected.len());
                    for f in &affected {
                        println!("  {}", f);
                    }
                }
            }
        }
        Commands::Install { agent } => {
            let home = tokensave::agents::home_dir().ok_or_else(|| {
                tokensave::errors::TokenSaveError::Config {
                    message: "could not determine home directory".to_string(),
                }
            })?;
            let tokensave_bin = tokensave::agents::which_tokensave().ok_or_else(|| {
                tokensave::errors::TokenSaveError::Config {
                    message: "tokensave not found on PATH. Install it first:\n  \
                          cargo install tokensave\n  \
                          brew install aovestdipaperino/tap/tokensave"
                        .to_string(),
                }
            })?;
            let mut user_cfg = tokensave::user_config::UserConfig::load();
            tokensave::agents::migrate_installed_agents(&home, &mut user_cfg);

            let mut installed_names: Vec<String> = Vec::new();
            let mut removed_names: Vec<String> = Vec::new();

            if let Some(id) = agent {
                let ag = tokensave::agents::get_integration(&id)?;
                let name = ag.name().to_string();
                let ctx = tokensave::agents::InstallContext {
                    home: home.clone(),
                    tokensave_bin: tokensave_bin.clone(),
                    tool_permissions: tokensave::agents::expected_tool_perms(),
                };
                ag.install(&ctx)?;
                if !user_cfg.installed_agents.contains(&id) {
                    user_cfg.installed_agents.push(id);
                    installed_names.push(name);
                }
                user_cfg.save();
            } else {
                let (to_install, to_uninstall) = tokensave::agents::pick_integrations_interactive(
                    &home,
                    &user_cfg.installed_agents,
                )?;

                for id in &to_uninstall {
                    let ag = tokensave::agents::get_integration(id)?;
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: tokensave_bin.clone(),
                        tool_permissions: tokensave::agents::expected_tool_perms(),
                    };
                    ag.uninstall(&ctx)?;
                    removed_names.push(ag.name().to_string());
                    user_cfg.installed_agents.retain(|a| a != id);
                }
                for id in &to_install {
                    let ag = tokensave::agents::get_integration(id)?;
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: tokensave_bin.clone(),
                        tool_permissions: tokensave::agents::expected_tool_perms(),
                    };
                    ag.install(&ctx)?;
                    installed_names.push(ag.name().to_string());
                    if !user_cfg.installed_agents.contains(id) {
                        user_cfg.installed_agents.push(id.clone());
                    }
                }
                user_cfg.save();
            }

            eprintln!();
            if installed_names.is_empty() && removed_names.is_empty() {
                eprintln!("No changes.");
            } else {
                for name in &installed_names {
                    eprintln!("\x1b[32m+\x1b[0m {name}");
                }
                for name in &removed_names {
                    eprintln!("\x1b[31m-\x1b[0m {name}");
                }
            }

            user_cfg.last_installed_version = env!("CARGO_PKG_VERSION").to_string();
            user_cfg.save();

            tokensave::agents::offer_git_post_commit_hook(&tokensave_bin);
            tokensave::daemon::offer_daemon_autostart();
        }
        Commands::Reinstall => {
            let home = tokensave::agents::home_dir().ok_or_else(|| {
                tokensave::errors::TokenSaveError::Config {
                    message: "could not determine home directory".to_string(),
                }
            })?;
            let tokensave_bin = tokensave::agents::which_tokensave().ok_or_else(|| {
                tokensave::errors::TokenSaveError::Config {
                    message: "tokensave not found on PATH".to_string(),
                }
            })?;
            let mut user_cfg = tokensave::user_config::UserConfig::load();
            tokensave::agents::migrate_installed_agents(&home, &mut user_cfg);

            if user_cfg.installed_agents.is_empty() {
                eprintln!("No installed agents found. Run `tokensave install` first.");
            } else {
                let agents = user_cfg.installed_agents.clone();
                eprintln!(
                    "Reinstalling {} agent(s): {}",
                    agents.len(),
                    agents.join(", ")
                );
                for id in &agents {
                    let ag = tokensave::agents::get_integration(id)?;
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: tokensave_bin.clone(),
                        tool_permissions: tokensave::agents::expected_tool_perms(),
                    };
                    ag.install(&ctx)?;
                }
                eprintln!("\x1b[32m✔\x1b[0m All agents reinstalled");
                user_cfg.last_installed_version = env!("CARGO_PKG_VERSION").to_string();
                user_cfg.save();
            }
        }
        Commands::Uninstall { agent } => {
            let home = tokensave::agents::home_dir().ok_or_else(|| {
                tokensave::errors::TokenSaveError::Config {
                    message: "could not determine home directory".to_string(),
                }
            })?;
            let mut user_cfg = tokensave::user_config::UserConfig::load();
            tokensave::agents::migrate_installed_agents(&home, &mut user_cfg);

            if let Some(id) = agent {
                let ag = tokensave::agents::get_integration(&id)?;
                let ctx = tokensave::agents::InstallContext {
                    home,
                    tokensave_bin: String::new(),
                    tool_permissions: tokensave::agents::expected_tool_perms(),
                };
                ag.uninstall(&ctx)?;
                user_cfg.installed_agents.retain(|a| a != &id);
                user_cfg.save();
            } else {
                for id in user_cfg.installed_agents.clone() {
                    if let Ok(ag) = tokensave::agents::get_integration(&id) {
                        let ctx = tokensave::agents::InstallContext {
                            home: home.clone(),
                            tokensave_bin: String::new(),
                            tool_permissions: tokensave::agents::expected_tool_perms(),
                        };
                        ag.uninstall(&ctx).ok();
                    }
                }
                user_cfg.installed_agents.clear();
                user_cfg.save();
                eprintln!("All agent integrations removed.");
            }
        }
        Commands::ExtractWorker => {
            // Handled by the early dispatch at the top of run(); this arm
            // exists only for clap match exhaustiveness.
            unreachable!("extract-worker handled by early dispatch")
        }
        Commands::HookPreToolUse => {
            tokensave::hooks::hook_pre_tool_use();
        }
        Commands::HookPromptSubmit => {
            tokensave::hooks::hook_prompt_submit().await;
        }
        Commands::HookStop => {
            tokensave::hooks::hook_stop().await;
        }
        Commands::Serve { path } => {
            if std::env::var("DISABLE_TOKENSAVE").as_deref() == Ok("true") {
                // Allow users to opt out per-project by setting
                // DISABLE_TOKENSAVE=true in their MCP server config (#19).
                // The process exits cleanly so the host does not retry.
                return Ok(());
            }
            let original_cwd = std::env::current_dir().ok();
            let project_path = tokensave::config::resolve_path_with_discovery(path);
            // Track the first stdin line if we need to peek at `initialize` roots.
            let mut peeked_line: Option<String> = None;
            let cg = match serve::ensure_initialized(&project_path).await {
                Ok(cg) => cg,
                Err(_) => {
                    // CWD-based discovery failed (e.g. VS Code launched us from ~).
                    // Fall back to the global DB's registered projects.
                    match serve::resolve_serve_from_global_db().await {
                        Some(p) => serve::ensure_initialized(&p).await?,
                        None => {
                            // Last resort: peek at the first stdin line for MCP
                            // `initialize` roots (e.g. VS Code multi-folder workspace).
                            match serve::resolve_serve_from_mcp_roots(&mut peeked_line).await {
                                Some(p) => serve::ensure_initialized(&p).await?,
                                None => {
                                    return Err(tokensave::errors::TokenSaveError::Config {
                                        message: format!(
                                            "no TokenSave index found at '{}' and no projects registered in the global database — run 'tokensave init' in your project first",
                                            project_path.display()
                                        ),
                                    }
                                    .into());
                                }
                            }
                        }
                    }
                }
            };

            // Compute scope prefix: relative path from project root to original cwd
            let scope_prefix = original_cwd.and_then(|cwd| {
                cwd.strip_prefix(cg.project_root())
                    .ok()
                    .filter(|rel| !rel.as_os_str().is_empty())
                    .map(|rel| rel.to_string_lossy().into_owned())
            });

            // If the daemon isn't running, watch this project for local changes.
            let watcher_cancel = if tokensave::daemon::running_daemon_pid().is_none() {
                let config = tokensave::user_config::UserConfig::load();
                let debounce = tokensave::daemon::parse_duration(&config.daemon_debounce)
                    .unwrap_or(std::time::Duration::from_secs(15));
                if let Some(pw) =
                    tokensave::project_watcher::ProjectWatcher::new(project_path.clone(), debounce)
                {
                    let token = tokio_util::sync::CancellationToken::new();
                    tokio::spawn(pw.run(token.clone()));
                    Some(token)
                } else {
                    None
                }
            } else {
                None
            };

            let server = tokensave::mcp::McpServer::new(cg, scope_prefix).await;
            let mut transport = tokensave::mcp::StdioTransport::new();
            // If we peeked at stdin to read `initialize` roots, replay that line.
            if let Some(line) = peeked_line {
                server.handle_and_write(&line, &mut transport).await;
            }
            server.run(&mut transport).await?;

            // Stop the watcher when the server exits.
            if let Some(token) = watcher_cancel {
                token.cancel();
            }
        }
        Commands::Upgrade => {
            tokensave::upgrade::run_upgrade()?;
        }
        Commands::Channel { channel } => match channel {
            Some(target) => {
                tokensave::upgrade::switch_channel(&target)?;
            }
            None => tokensave::upgrade::show_channel(),
        },
        Commands::CurrentCounter { path } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;
            let value = cg.get_local_counter().await?;
            println!("{value}");
        }
        Commands::ResetCounter { path } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;
            let prev = cg.get_local_counter().await?;
            cg.reset_local_counter().await?;
            eprintln!("Local counter reset (was {prev})");
        }
        Commands::DisableUploadCounter => {
            let mut config = tokensave::user_config::UserConfig::load();
            config.upload_enabled = false;
            config.save();
            eprintln!("Worldwide counter upload disabled. You can re-enable with `tokensave enable-upload-counter`.");
        }
        Commands::EnableUploadCounter => {
            let mut config = tokensave::user_config::UserConfig::load();
            config.upload_enabled = true;
            config.save();
            eprintln!("Worldwide counter upload enabled.");
        }
        Commands::Gitignore { path, action } => {
            let project_path = tokensave::config::resolve_path(path);
            let mut config = tokensave::config::load_config(&project_path)?;
            match action.as_deref() {
                Some("on") => {
                    config.git_ignore = true;
                    tokensave::config::save_config(&project_path, &config)?;
                    eprintln!(
                        "gitignore enabled — .gitignore rules will be respected during indexing."
                    );
                    eprintln!("Run `tokensave sync` to re-index with the new setting.");
                }
                Some("off") => {
                    config.git_ignore = false;
                    tokensave::config::save_config(&project_path, &config)?;
                    eprintln!(
                        "gitignore disabled — .gitignore rules will be ignored during indexing."
                    );
                    eprintln!("Run `tokensave sync` to re-index with the new setting.");
                }
                Some(other) => {
                    return Err(tokensave::errors::TokenSaveError::Config {
                        message: format!("unknown action '{other}': expected 'on' or 'off'"),
                    });
                }
                None => {
                    let status = if config.git_ignore { "on" } else { "off" };
                    eprintln!("gitignore: {status}");
                }
            }
        }
        Commands::Doctor { agent } => {
            tokensave::doctor::run_doctor(agent.as_deref()).await;
        }
        Commands::Daemon {
            foreground,
            stop,
            status,
            enable_autostart,
            disable_autostart,
            debounce,
        } => {
            if stop {
                tokensave::daemon::stop()?;
            } else if status {
                let code = tokensave::daemon::status();
                std::process::exit(code);
            } else if enable_autostart {
                tokensave::daemon::enable_autostart()?;
            } else if disable_autostart {
                tokensave::daemon::disable_autostart()?;
            } else {
                let upgraded = tokensave::daemon::run(foreground, debounce).await?;
                if upgraded {
                    // Exit with non-zero code so the service manager (launchd
                    // KeepAlive / systemd Restart=on-failure / Windows SCM
                    // failure actions) restarts with the new binary.
                    std::process::exit(1);
                }
            }
        }
        Commands::Cost {
            range,
            by_model,
            by_task,
            export,
        } => {
            // Refresh LiteLLM pricing if cache is older than 24h
            tokensave::accounting::pricing::refresh_if_stale();

            let gdb = match tokensave::global_db::GlobalDb::open().await {
                Some(db) => db,
                None => {
                    eprintln!("Could not open global database.");
                    process::exit(1);
                }
            };

            // Ingest new session data before querying
            let ingest_stats = tokensave::accounting::parser::ingest(&gdb).await;
            if ingest_stats.turns_inserted > 0 {
                eprintln!(
                    "Ingested {} new turns from Claude Code sessions.",
                    ingest_stats.turns_inserted
                );
            }

            let since = tokensave::accounting::metrics::parse_range(&range);
            let tokens_saved = gdb.global_tokens_saved().await.unwrap_or(0);
            let summary =
                tokensave::accounting::metrics::cost_summary(&gdb, since, tokens_saved).await;

            let Some(s) = summary else {
                println!("No session data found. Use Claude Code and then run `tokensave cost` to see spending.");
                return Ok(());
            };

            if let Some(ref fmt) = export {
                match fmt.as_str() {
                    "json" => {
                        let obj = serde_json::json!({
                            "range": range,
                            "total_cost_usd": s.total_cost,
                            "total_input_tokens": s.total_input_tokens,
                            "total_output_tokens": s.total_output_tokens,
                            "tokens_saved": s.tokens_saved,
                            "efficiency_ratio": s.efficiency_ratio,
                            "by_model": s.by_model.iter().map(|(m, c, t)| serde_json::json!({"model": m, "cost": c, "tokens": t})).collect::<Vec<_>>(),
                            "by_category": s.by_category.iter().map(|(cat, c, n)| serde_json::json!({"category": cat, "cost": c, "turns": n})).collect::<Vec<_>>(),
                        });
                        println!("{}", serde_json::to_string_pretty(&obj).unwrap_or_default());
                    }
                    "csv" => {
                        if by_model {
                            println!("model,cost_usd,tokens");
                            for (model, cost, tokens) in &s.by_model {
                                println!("{model},{cost:.4},{tokens}");
                            }
                        } else if by_task {
                            println!("category,cost_usd,turns");
                            for (cat, cost, turns) in &s.by_category {
                                println!("{cat},{cost:.4},{turns}");
                            }
                        } else {
                            println!(
                                "total_cost_usd,input_tokens,output_tokens,tokens_saved,efficiency"
                            );
                            println!(
                                "{:.4},{},{},{},{:.4}",
                                s.total_cost,
                                s.total_input_tokens,
                                s.total_output_tokens,
                                s.tokens_saved,
                                s.efficiency_ratio
                            );
                        }
                    }
                    _ => eprintln!("Unknown export format '{fmt}'. Use 'json' or 'csv'."),
                }
            } else if by_model {
                let total = s.total_cost.max(0.001);
                println!(
                    "  {:<24} {:>10} {:>10} {:>6}",
                    "Model", "Cost", "Tokens", "Share"
                );
                for (model, cost, tokens) in &s.by_model {
                    let share = cost / total * 100.0;
                    let tok_str = tokensave::display::format_token_count(*tokens);
                    println!(
                        "  {:<24} {:>9} {:>10} {:>5.0}%",
                        model,
                        format!("${cost:.2}"),
                        tok_str,
                        share
                    );
                }
            } else if by_task {
                println!("  {:<16} {:>10} {:>6}", "Category", "Cost", "Turns");
                for (cat, cost, turns) in &s.by_category {
                    println!("  {:<16} {:>9} {:>6}", cat, format!("${cost:.2}"), turns);
                }
            } else {
                // Default summary
                let today_since = tokensave::accounting::metrics::parse_range("today");
                let today_cost = gdb.total_cost_since(today_since).await.unwrap_or(0.0);
                let today_breakdown = gdb
                    .token_breakdown_since(today_since)
                    .await
                    .unwrap_or((0, 0, 0));

                let fmt_row = |label: &str, cost: f64, input: u64, output: u64, cache_read: u64| {
                    let input_s = tokensave::display::format_token_count(input);
                    let output_s = tokensave::display::format_token_count(output);
                    let cache_pct = if input + cache_read > 0 {
                        (cache_read as f64 / (input + cache_read) as f64) * 100.0
                    } else {
                        0.0
                    };
                    println!(
                        "  {:<10} {:>9} {:>10} {:>10} {:>9.0}%",
                        label,
                        format!("${cost:.2}"),
                        input_s,
                        output_s,
                        cache_pct
                    );
                };

                println!(
                    "  {:<10} {:>10} {:>10} {:>10} {:>10}",
                    "Period", "Cost", "Input", "Output", "Cache-hit"
                );
                fmt_row(
                    "Today",
                    today_cost,
                    today_breakdown.0,
                    today_breakdown.1,
                    today_breakdown.2,
                );
                fmt_row(
                    &range,
                    s.total_cost,
                    s.total_input_tokens,
                    s.total_output_tokens,
                    s.total_cache_read_tokens,
                );

                if s.tokens_saved > 0 {
                    let saved_str = tokensave::display::format_token_count(s.tokens_saved);
                    println!();
                    println!(
                        "  Savings  {} tokens ({:.0}% efficiency)",
                        saved_str,
                        s.efficiency_ratio * 100.0
                    );
                }
            }
        }
        Commands::Bench { queries, json, path, max_nodes } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;

            let queries_path = match queries {
                Some(p) => std::path::PathBuf::from(p),
                None => {
                    // Prefer the project-local default if present.
                    let local = project_path.join("benchmarks/queries/default.toml");
                    if local.exists() {
                        local
                    } else {
                        // Fall back to the binary-embedded default — write to a temp file
                        // so the file-based loader can use it without a separate code path.
                        let embedded = include_str!("../benchmarks/queries/default.toml");
                        let tmp = std::env::temp_dir().join("tokensave-bench-default.toml");
                        std::fs::write(&tmp, embedded).map_err(|e| {
                            tokensave::errors::TokenSaveError::Config {
                                message: format!("failed to write embedded query file: {e}"),
                            }
                        })?;
                        tmp
                    }
                }
            };

            let report = tokensave::bench::run_bench(
                &cg,
                &queries_path,
                tokensave::bench::BenchOptions {
                    format: if json {
                        tokensave::bench::OutputFormat::Json
                    } else {
                        tokensave::bench::OutputFormat::Markdown
                    },
                    max_nodes,
                },
            )
            .await?;

            if json {
                println!("{}", tokensave::bench::format_report_json(&report));
            } else {
                println!("{}", tokensave::bench::format_report_markdown(&report));
            }
        }
        Commands::Gain { all, history, range, json } => {
            commands::handle_gain(all, history, &range, json).await?;
        }
        Commands::Monitor => {
            if let Err(e) = tokensave::monitor::run() {
                eprintln!("Monitor error: {e}");
                process::exit(1);
            }
        }
        Commands::Branch { action } => {
            commands::handle_branch_action(action).await?;
        }
        Commands::Wipe { all } => {
            commands::handle_wipe(all).await?;
        }
        Commands::List { all } => {
            commands::handle_list(all).await?;
        }
    }
    Ok(())
}

// handle_branch_action, handle_wipe, handle_list, handle_no_command,
// init_and_index, and print_sync_doctor have been moved to src/commands.rs.
//
// update_global_db, try_flush, check_for_update, gather_target_projects,
// gather_local_projects, gather_local_projects_from, find_descendant_tokensave,
// print_flash_warning, and tokensave_dir_size have been moved to src/global.rs.
// direct test 1774739850
// daemon-test-1774740132
