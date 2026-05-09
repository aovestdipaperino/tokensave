// Rust guideline compliant 2025-10-17
// Updated 2026-03-23: compact bordered table for status output
use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process;

use tokensave::context::{format_context_as_json, format_context_as_markdown};
use tokensave::tokensave::TokenSave;
use tokensave::types::*;

/// Alias for the shared timestamp utility.
fn current_unix_timestamp() -> i64 {
    tokensave::tokensave::current_timestamp()
}

/// A self-animating spinner that ticks on a background thread.
/// Call `set_message` to update what is displayed; the background thread
/// redraws at ~80 ms intervals. Call `done` to stop and print a final line.
struct Spinner {
    message: std::sync::Arc<std::sync::Mutex<String>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    fn new() -> Self {
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

    fn set_message(&self, msg: &str) {
        *self.message.lock().unwrap() = msg.to_string();
    }

    fn done(mut self, message: &str) {
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

/// Code intelligence for Rust codebases.
#[derive(Parser)]
#[command(
    name = "tokensave",
    about = "Code intelligence for 15 languages — semantic graph queries instead of file reads",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new TokenSave project (full index)
    Init {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Folders to skip during indexing (can be repeated)
        #[arg(long = "skip-folder", num_args = 1..)]
        skip_folders: Vec<String>,
    },
    /// Incremental sync (project must already be initialized with `tokensave init`)
    Sync {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Force a full re-index
        #[arg(short, long)]
        force: bool,
        /// Folders to skip during indexing (can be repeated)
        #[arg(long = "skip-folder", num_args = 1..)]
        skip_folders: Vec<String>,
        /// List added, modified, and removed files after sync
        #[arg(long)]
        doctor: bool,
        /// Print per-phase diagnostics (file counts, timings) to help debug slow syncs
        #[arg(short, long)]
        verbose: bool,
    },
    /// Show project statistics
    Status {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Output as JSON
        #[arg(short, long)]
        json: bool,
        /// Show only the header (version, tokens, sync times)
        #[arg(short, long)]
        short: bool,
        /// Show node-kind breakdown
        #[arg(short, long)]
        details: bool,
    },
    /// Search for symbols
    Query {
        /// Search query
        search: String,
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Build context for a task
    Context {
        /// Task description
        task: String,
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
        /// Maximum symbols
        #[arg(short = 'n', long, default_value = "20")]
        max_nodes: usize,
        /// Output format (markdown or json)
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },
    /// List indexed files
    Files {
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
        /// Filter to files under this directory
        #[arg(long)]
        filter: Option<String>,
        /// Filter files matching this glob pattern
        #[arg(long)]
        pattern: Option<String>,
        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },
    /// Find test files affected by changed source files
    Affected {
        /// Changed file paths
        files: Vec<String>,
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
        /// Read file list from stdin (one per line)
        #[arg(long)]
        stdin: bool,
        /// Max dependency traversal depth
        #[arg(short, long, default_value = "5")]
        depth: usize,
        /// Custom glob filter for test files
        #[arg(short, long)]
        filter: Option<String>,
        /// Output as JSON
        #[arg(short, long)]
        json: bool,
        /// Only output file paths, no decoration
        #[arg(short, long)]
        quiet: bool,
    },
    /// Configure agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "install", visible_alias = "claude-install")]
    Install {
        /// Agent to configure (auto-detects if omitted)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Refresh settings for all already-installed agents
    Reinstall,
    /// Remove agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "uninstall", visible_alias = "claude-uninstall")]
    Uninstall {
        /// Agent to remove (removes all if omitted)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Extraction worker (spawned by tokensave itself; not for direct use).
    #[command(name = "extract-worker", hide = true)]
    ExtractWorker,
    /// PreToolUse hook handler (called by Claude Code, not by users directly)
    #[command(name = "hook-pre-tool-use", hide = true)]
    HookPreToolUse,
    /// UserPromptSubmit hook handler (resets session counter)
    #[command(name = "hook-prompt-submit", hide = true)]
    HookPromptSubmit,
    /// Stop hook handler (prints session token savings)
    #[command(name = "hook-stop", hide = true)]
    HookStop,
    /// Start MCP server over stdio
    Serve {
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Download and install the latest version from GitHub
    Upgrade,
    /// Show or switch the update channel (stable or beta)
    Channel {
        /// Target channel: "stable" or "beta" (omit to show current)
        channel: Option<String>,
    },
    /// Show the resettable project-local token counter
    #[command(name = "current-counter")]
    CurrentCounter {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Reset the project-local token counter to zero
    #[command(name = "reset-counter")]
    ResetCounter {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Disable uploading token counts to the worldwide counter
    #[command(name = "disable-upload-counter")]
    DisableUploadCounter,
    /// Enable uploading token counts to the worldwide counter
    #[command(name = "enable-upload-counter")]
    EnableUploadCounter,
    /// Show or change whether .gitignore rules are respected during indexing
    #[command(name = "gitignore")]
    Gitignore {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
        /// "on" to enable, "off" to disable, omit to show current setting
        action: Option<String>,
    },
    /// Check tokensave installation, configuration, and agent integration
    Doctor {
        /// Check only this agent (default: all agents)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Background file watcher daemon
    Daemon {
        /// Run in foreground (don't fork)
        #[arg(long)]
        foreground: bool,
        /// Stop the running daemon
        #[arg(long)]
        stop: bool,
        /// Show daemon status
        #[arg(long)]
        status: bool,
        /// Install autostart service (launchd/systemd)
        #[arg(long)]
        enable_autostart: bool,
        /// Remove autostart service
        #[arg(long)]
        disable_autostart: bool,
        /// Override debounce duration (e.g. "2s", "15s", "1m"). Overrides config.
        #[arg(long)]
        debounce: Option<String>,
    },
    /// Token cost summary from Claude Code sessions
    Cost {
        /// Time range: "today", "7d", "30d", "month", or "all"
        #[arg(default_value = "7d")]
        range: String,
        /// Group by model
        #[arg(long)]
        by_model: bool,
        /// Group by task category
        #[arg(long)]
        by_task: bool,
        /// Export format: csv or json
        #[arg(long)]
        export: Option<String>,
    },
    /// Live token savings monitor (global, all projects)
    Monitor,
    /// Manage multi-branch indexing
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },
    /// Wipe local tokensave DBs (current folder, parents, and children)
    Wipe {
        /// Wipe ALL tracked projects so the global DB ends empty
        #[arg(short, long)]
        all: bool,
    },
    /// List tokensave projects (current folder, parents, and children)
    List {
        /// List ALL tracked projects from the global DB
        #[arg(short, long)]
        all: bool,
    },
}

#[derive(Subcommand)]
enum BranchAction {
    /// List tracked branches and their DB sizes
    List {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Track a new branch (copies nearest ancestor DB + incremental sync)
    Add {
        /// Branch name to track (default: current branch)
        name: Option<String>,
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Remove a tracked branch and delete its DB
    Remove {
        /// Branch name to remove
        name: String,
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Remove all tracked branches (keeps only the default branch)
    Removeall {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Remove DBs for branches that no longer exist in git
    Gc {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
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
        None => return handle_no_command().await,
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
    try_flush(&mut user_config, is_force_flush);
    user_config.save();

    if is_first_run {
        eprintln!(
            "note: tokensave uploads anonymous token-saved counts to a worldwide counter.\n\
             \x20     Run `tokensave disable-upload-counter` to opt out."
        );
    }

    // Nudge beta users to switch to the stable channel.
    if tokensave::cloud::is_beta() {
        eprintln!(
            "\x1b[33mnote:\x1b[0m The beta channel has been merged into stable. \
             Run `tokensave channel stable` to switch."
        );
    }

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
            init_and_index(&project_path, &skip_folders, false).await?;

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
                init_and_index(&project_path, &skip_folders, verbose).await?;
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
                    print_sync_doctor(&result);
                }
                update_global_db(&cg).await;
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
                    init_and_index(&project_path, &[], false).await?
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
                check_for_update(&mut config, false, true);
            }
        }
        Commands::Query {
            search,
            path,
            limit,
        } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
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
            let cg = ensure_initialized(&project_path).await?;
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
            let cg = ensure_initialized(&project_path).await?;
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
            let cg = ensure_initialized(&project_path).await?;

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

            let affected = find_affected_tests(&cg, &changed, depth, filter.as_deref()).await?;

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
            let cg = match ensure_initialized(&project_path).await {
                Ok(cg) => cg,
                Err(_) => {
                    // CWD-based discovery failed (e.g. VS Code launched us from ~).
                    // Fall back to the global DB's registered projects.
                    let fallback = resolve_serve_from_global_db().await;
                    match fallback {
                        Some(p) => ensure_initialized(&p).await?,
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
            };

            // Compute scope prefix: relative path from project root to original cwd
            let scope_prefix = original_cwd.and_then(|cwd| {
                cwd.strip_prefix(&project_path)
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
            let cg = ensure_initialized(&project_path).await?;
            let value = cg.get_local_counter().await?;
            println!("{value}");
        }
        Commands::ResetCounter { path } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
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
        Commands::Monitor => {
            if let Err(e) = tokensave::monitor::run() {
                eprintln!("Monitor error: {e}");
                process::exit(1);
            }
        }
        Commands::Branch { action } => {
            handle_branch_action(action).await?;
        }
        Commands::Wipe { all } => {
            handle_wipe(all).await?;
        }
        Commands::List { all } => {
            handle_list(all).await?;
        }
    }
    Ok(())
}

async fn handle_branch_action(action: BranchAction) -> tokensave::errors::Result<()> {
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
async fn handle_wipe(all: bool) -> tokensave::errors::Result<()> {
    use std::fs;
    use std::path::PathBuf;

    let home_tokensave: Option<PathBuf> = dirs::home_dir().map(|h| h.join(".tokensave"));

    let mut targets = gather_target_projects(all, &home_tokensave).await;
    if all {
        // wipe acts on the live `.tokensave/` directory; drop rows whose
        // directory is already gone (they're handled by `tokensave doctor`).
        targets.retain(|p| p.join(".tokensave/tokensave.db").exists());
    }

    if !all && targets.is_empty() {
        eprintln!("No tokensave projects found in current folder, parents, or children.");
        return Ok(());
    }

    print_flash_warning(all, &targets);

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
async fn handle_list(all: bool) -> tokensave::errors::Result<()> {
    use std::path::PathBuf;
    use tokensave::display::format_token_count;

    let home_tokensave: Option<PathBuf> = dirs::home_dir().map(|h| h.join(".tokensave"));
    let project_paths = gather_target_projects(all, &home_tokensave).await;

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
            tokensave_dir_size(&ts_dir)
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

/// Returns the total size in bytes of every file under `dir`. Best-effort.
fn tokensave_dir_size(dir: &Path) -> u64 {
    fn walk(p: &Path, acc: &mut u64) {
        let Ok(entries) = std::fs::read_dir(p) else {
            return;
        };
        for entry in entries.flatten() {
            // One stat per entry instead of file_type() + metadata():
            // `metadata()` already carries the file-type bits, so calling
            // both means a redundant syscall on filesystems that don't
            // cache the dirent stat.
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                walk(&entry.path(), acc);
            } else if meta.is_file() {
                *acc = acc.saturating_add(meta.len());
            }
        }
    }
    let mut total = 0u64;
    walk(dir, &mut total);
    total
}

/// Returns the project paths the `wipe` / `list` commands should act on.
///
/// `--all` returns every path tracked in the global DB (including stale rows).
/// Otherwise returns the local discovery from cwd / ancestors / descendants.
async fn gather_target_projects(
    all: bool,
    home_tokensave: &Option<std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    if all {
        let Some(gdb) = tokensave::global_db::GlobalDb::open().await else {
            return Vec::new();
        };
        gdb.list_project_paths()
            .await
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect()
    } else {
        gather_local_projects(home_tokensave)
    }
}

/// Returns project roots whose `.tokensave` dir lives in cwd, an ancestor, or a descendant.
fn gather_local_projects(home_tokensave: &Option<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    gather_local_projects_from(&cwd, home_tokensave)
}

/// Same as [`gather_local_projects`] but takes the starting directory explicitly.
///
/// Pure (apart from filesystem reads) — easier to test than the cwd-driven wrapper.
fn gather_local_projects_from(
    cwd: &Path,
    home_tokensave: &Option<std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    use std::collections::HashSet;
    use std::path::PathBuf;

    // Canonicalize the home `.tokensave` once so symlinked HOME paths still
    // get correctly skipped during the ancestor + descendant walks. A user
    // whose `$HOME` is `/Users/x` but whose canonical home is
    // `/private/var/...` would otherwise leak the global DB into the wipe set.
    let canon_home_ts: Option<PathBuf> =
        home_tokensave.as_ref().and_then(|p| p.canonicalize().ok());

    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    let is_home_tokensave = |ts: &Path| -> bool {
        if let Some(ref canon) = canon_home_ts {
            if ts.canonicalize().ok().as_ref() == Some(canon) {
                return true;
            }
        }
        false
    };

    let is_project_dir = |ts: &Path| -> bool {
        !is_home_tokensave(ts) && ts.is_dir() && ts.join("tokensave.db").exists()
    };

    let mut cursor: Option<&Path> = Some(cwd);
    while let Some(dir) = cursor {
        let ts = dir.join(".tokensave");
        if is_project_dir(&ts) && seen.insert(dir.to_path_buf()) {
            out.push(dir.to_path_buf());
        }
        cursor = dir.parent();
    }

    find_descendant_tokensave(cwd, &canon_home_ts, &mut seen, &mut out);

    out
}

/// Iteratively walks `start` looking for `.tokensave/tokensave.db` projects.
///
/// Skips common heavy directories (node_modules, target, .git, etc.) and never
/// descends into a `.tokensave` once found. Tracks canonicalized directories
/// to break symlink/junction cycles, and uses an explicit worklist instead of
/// recursion so deep trees can't overflow the stack.
fn find_descendant_tokensave(
    start: &Path,
    canon_home_ts: &Option<std::path::PathBuf>,
    seen: &mut std::collections::HashSet<std::path::PathBuf>,
    out: &mut Vec<std::path::PathBuf>,
) {
    use std::collections::HashSet;

    let mut visited: HashSet<std::path::PathBuf> = HashSet::new();
    let mut work: Vec<std::path::PathBuf> = vec![start.to_path_buf()];

    while let Some(dir) = work.pop() {
        // Cycle guard — best-effort. If canonicalize fails (permission, broken
        // symlink) we fall back to the raw path, which still dedupes most cases.
        let canon = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if !visited.insert(canon) {
            continue;
        }

        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            // `file_type()` does not traverse symlinks, so symlinks-to-dirs
            // report `is_symlink()` and are skipped here. That's the primary
            // cycle defense; the `visited` set above is belt-and-suspenders.
            if !ft.is_dir() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == ".tokensave" {
                // Only canonicalize when the entry could match the home skip;
                // doing it for every dir entry would mean one syscall per
                // entry on tree walks of arbitrary size.
                if let Some(canon) = canon_home_ts {
                    if path.canonicalize().ok().as_ref() == Some(canon) {
                        continue;
                    }
                }
                if path.join("tokensave.db").exists() {
                    if let Some(parent) = path.parent() {
                        let pb = parent.to_path_buf();
                        if seen.insert(pb.clone()) {
                            out.push(pb);
                        }
                    }
                }
                continue;
            }
            if matches!(
                name_str.as_ref(),
                "node_modules"
                    | "target"
                    | ".git"
                    | "vendor"
                    | "dist"
                    | "build"
                    | ".next"
                    | ".venv"
                    | "__pycache__"
            ) {
                continue;
            }
            work.push(path);
        }
    }
}

/// Prints the big flashing warning shown before a wipe.
fn print_flash_warning(all: bool, targets: &[std::path::PathBuf]) {
    // Banner is `INNER_WIDTH` display columns wide. The colored title row is
    // padded with red-background spaces so the highlight reaches the same
    // width as the `═` rules above and below — a fixed-width visual block
    // rather than a short red strip floating between long horizontal lines.
    const INNER_WIDTH: usize = 64;
    let title = "⚠  DESTRUCTIVE ACTION — TOKENSAVE WIPE  ⚠";
    // Visible columns: ⚠(2) + "  "(2) + 35 + "  "(2) + ⚠(2) = 43.
    // Modern terminals render U+26A0 as a 2-col emoji glyph; older terminals
    // that pick the text presentation will leave a 2-col gap, which is mild.
    const TITLE_COLS: usize = 43;
    let pad_total = INNER_WIDTH.saturating_sub(TITLE_COLS);
    let pad_left = " ".repeat(pad_total / 2);
    let pad_right = " ".repeat(pad_total - pad_total / 2);
    let banner = "═".repeat(INNER_WIDTH);
    let blank_red = " ".repeat(INNER_WIDTH);

    eprintln!();
    eprintln!("\x1b[1;31m{banner}\x1b[0m");
    eprintln!("\x1b[1;5;37;41m{blank_red}\x1b[0m");
    eprintln!("\x1b[1;5;37;41m{pad_left}{title}{pad_right}\x1b[0m");
    eprintln!("\x1b[1;5;37;41m{blank_red}\x1b[0m");
    eprintln!("\x1b[1;31m{banner}\x1b[0m");
    eprintln!();
    if all {
        eprintln!(
            "\x1b[1;31mThis will wipe \x1b[5mALL\x1b[25;1;31m tracked tokensave projects \
             AND empty the global DB.\x1b[0m"
        );
    } else {
        eprintln!(
            "\x1b[1;31mThis will wipe local tokensave DBs in the current folder \
             (parents and children).\x1b[0m"
        );
    }
    eprintln!();
    if targets.is_empty() {
        eprintln!("  \x1b[33m(no project .tokensave directories found)\x1b[0m");
    } else {
        eprintln!("Targets:");
        for t in targets {
            eprintln!("  \x1b[31m✗\x1b[0m {}/.tokensave", t.display());
        }
    }
    if all {
        if let Some(p) = tokensave::global_db::global_db_path() {
            eprintln!("  \x1b[31m✗\x1b[0m {} (global DB)", p.display());
        }
    }
    eprintln!();
    eprintln!("\x1b[1;5;33mThis cannot be undone.\x1b[0m");
    eprintln!();
}

/// When invoked with no subcommand, offer to create the index if none exists.
async fn handle_no_command() -> tokensave::errors::Result<()> {
    let project_path = tokensave::config::resolve_path(None);
    if TokenSave::is_initialized(&project_path) {
        // Already initialized — show help via clap
        let _ = <Cli as clap::CommandFactory>::command().print_help();
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
async fn init_and_index(
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
    update_global_db(&cg).await;
    Ok(cg)
}

/// Print the `--doctor` report after an incremental sync.
fn print_sync_doctor(result: &tokensave::tokensave::SyncResult) {
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

/// Opens an existing project, or tells the user to run `tokensave init` first.
async fn ensure_initialized(project_path: &Path) -> tokensave::errors::Result<TokenSave> {
    if TokenSave::is_initialized(project_path) {
        return TokenSave::open(project_path).await;
    }
    Err(tokensave::errors::TokenSaveError::Config {
        message: format!(
            "no TokenSave index found at '{}' — run 'tokensave init' first",
            project_path.display()
        ),
    })
}

/// Fallback for `serve`: when CWD-based discovery fails, check the global DB
/// for registered projects. Returns the single registered project path, or
/// `None` if zero or multiple projects are registered.
async fn resolve_serve_from_global_db() -> Option<std::path::PathBuf> {
    let gdb = tokensave::global_db::GlobalDb::open().await?;
    let mut paths: Vec<String> = gdb.list_project_paths().await;
    // Keep only projects whose .tokensave dir still exists on disk.
    paths.retain(|p| {
        std::path::Path::new(p)
            .join(".tokensave/tokensave.db")
            .exists()
    });
    if paths.len() == 1 {
        Some(std::path::PathBuf::from(paths.remove(0)))
    } else if paths.len() > 1 {
        eprintln!("Multiple tokensave projects found — pass -p <path> to select one:");
        for p in &paths {
            eprintln!("  {p}");
        }
        None
    } else {
        None
    }
}

/// Best-effort: register this project in the user-level global DB and
/// accumulate the token-saved delta into the pending upload counter.
async fn update_global_db(cg: &TokenSave) {
    let tokens = cg.get_tokens_saved().await.unwrap_or(0);
    if let Some(gdb) = tokensave::global_db::GlobalDb::open().await {
        let previous = gdb.get_project_tokens(cg.project_root()).await;
        gdb.upsert(cg.project_root(), tokens).await;

        // Accumulate delta into pending upload
        if tokens > previous {
            let mut config = tokensave::user_config::UserConfig::load();
            config.pending_upload += tokens - previous;
            config.save();
        }
    }
}

/// Best-effort: try to flush pending tokens to the worldwide counter.
/// `force` = true on status/sync commands (always attempt), false on others
/// (only flush if stale > 30s).
fn try_flush(config: &mut tokensave::user_config::UserConfig, force: bool) {
    if config.pending_upload == 0 || !config.upload_enabled {
        return;
    }
    let now = current_unix_timestamp();

    // Cooldown: skip if last flush attempt failed less than 60s ago
    if config.last_flush_attempt_at > config.last_upload_at
        && now - config.last_flush_attempt_at < 60
    {
        return;
    }

    // Staleness check for non-force commands
    if !force && now - config.last_upload_at < 30 {
        return;
    }

    config.last_flush_attempt_at = now;
    if let Some(worldwide_total) = tokensave::cloud::flush_pending(config.pending_upload) {
        config.pending_upload = 0;
        config.last_upload_at = now;
        config.last_worldwide_total = worldwide_total;
        config.last_worldwide_fetch_at = now;
    }
}

/// Best-effort version check with 5-minute network cache. If `skip_cache` is
/// true, always fetches from GitHub (used during sync where the call runs in
/// parallel). If `skip_suppression` is false, the warning is suppressed for 15
/// minutes after it was last shown; if true it is always shown (used for status).
fn check_for_update(
    config: &mut tokensave::user_config::UserConfig,
    skip_cache: bool,
    skip_suppression: bool,
) {
    let current_version = env!("CARGO_PKG_VERSION");
    let now = current_unix_timestamp();

    let latest = if !skip_cache && now - config.last_version_check_at < 300 {
        // Use cached value
        if config.cached_latest_version.is_empty() {
            return;
        }
        config.cached_latest_version.clone()
    } else if let Some(v) = tokensave::cloud::fetch_latest_version() {
        config.cached_latest_version = v.clone();
        config.last_version_check_at = now;
        config.save();
        v
    } else {
        return;
    };

    // The status page (skip_suppression=true) warns on any newer version;
    // the CLI only warns on minor+ bumps to avoid nagging on patch releases.
    let dominated = if skip_suppression {
        tokensave::cloud::is_newer_version(current_version, &latest)
    } else {
        tokensave::cloud::is_newer_minor_version(current_version, &latest)
    };

    if dominated && (skip_suppression || now - config.last_version_warning_at >= 900) {
        eprintln!(
            "\n\x1b[33mUpdate available: v{} → v{}\x1b[0m\n  Run: \x1b[1mtokensave upgrade\x1b[0m",
            current_version, latest
        );
        if !skip_suppression {
            config.last_version_warning_at = now;
            config.save();
        }
    }
}

// display, doctor, and is_test_file functions moved to:
// - src/display.rs (status table rendering)
// - src/doctor.rs (health checks)
// - src/tokensave.rs (is_test_file)

/// BFS through file dependents to find test files affected by changes.
async fn find_affected_tests(
    cg: &TokenSave,
    changed_files: &[String],
    max_depth: usize,
    custom_filter: Option<&str>,
) -> tokensave::errors::Result<Vec<String>> {
    debug_assert!(
        !changed_files.is_empty(),
        "find_affected_tests called with no changed files"
    );
    debug_assert!(
        max_depth > 0,
        "find_affected_tests max_depth must be positive"
    );
    use std::collections::{HashSet, VecDeque};

    let custom_glob = custom_filter.and_then(|p| glob::Pattern::new(p).ok());

    let matches_test = |path: &str| -> bool {
        if let Some(ref g) = custom_glob {
            g.matches(path)
        } else {
            tokensave::tokensave::is_test_file(path)
        }
    };

    let mut affected: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();

    // Seed: changed files that are themselves tests go directly into the result
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    for file in changed_files {
        if matches_test(file) {
            affected.insert(file.clone());
        }
        if visited.insert(file.clone()) {
            queue.push_back((file.clone(), 0));
        }
    }

    // BFS through file dependents
    while let Some((file, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let dependents = cg.get_file_dependents(&file).await?;
        for dep in dependents {
            if !visited.insert(dep.clone()) {
                continue;
            }
            if matches_test(&dep) {
                affected.insert(dep.clone());
            } else {
                queue.push_back((dep, depth + 1));
            }
        }
    }

    let mut result: Vec<String> = affected.into_iter().collect();
    result.sort();
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod gather_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Plant a `.tokensave/tokensave.db` marker so `is_project_dir` returns true.
    fn make_project(root: &Path) {
        let ts = root.join(".tokensave");
        fs::create_dir_all(&ts).unwrap();
        fs::write(ts.join("tokensave.db"), b"").unwrap();
    }

    #[test]
    fn finds_project_at_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().canonicalize().unwrap();
        make_project(&cwd);

        let out = gather_local_projects_from(&cwd, &None);
        assert_eq!(out, vec![cwd]);
    }

    #[test]
    fn finds_project_at_ancestor_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let nested = root.join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        make_project(&root);

        let out = gather_local_projects_from(&nested, &None);
        assert!(
            out.contains(&root),
            "ancestor project must be detected, got {out:?}"
        );
    }

    #[test]
    fn finds_project_at_descendant_only() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().canonicalize().unwrap();
        let child = cwd.join("sub").join("proj");
        fs::create_dir_all(&child).unwrap();
        make_project(&child);

        let out = gather_local_projects_from(&cwd, &None);
        assert!(
            out.contains(&child),
            "descendant project must be detected, got {out:?}"
        );
    }

    #[test]
    fn finds_both_ancestor_and_descendant_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let cwd = root.join("mid");
        fs::create_dir_all(&cwd).unwrap();
        let child = cwd.join("child");
        fs::create_dir_all(&child).unwrap();
        make_project(&root);
        make_project(&child);

        let out = gather_local_projects_from(&cwd, &None);
        assert!(out.contains(&root));
        assert!(out.contains(&child));
        let unique: std::collections::HashSet<_> = out.iter().collect();
        assert_eq!(unique.len(), out.len(), "duplicates: {out:?}");
    }

    #[test]
    fn skips_projects_inside_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().canonicalize().unwrap();
        let buried = cwd.join("node_modules").join("pkg");
        fs::create_dir_all(&buried).unwrap();
        make_project(&buried);

        let out = gather_local_projects_from(&cwd, &None);
        assert!(
            !out.contains(&buried),
            "projects inside node_modules must be skipped, got {out:?}"
        );
    }

    #[test]
    fn skips_home_tokensave_via_canonical_path() {
        // Simulate a symlinked HOME: `home_alias` → `home_real`. The user
        // passes `home_alias/.tokensave` as the skip path, but the descendant
        // walk encounters the directory through `home_real/.tokensave`.
        // Canonicalization must resolve them as equal so the global DB
        // directory is not picked up as a wipe target.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let home_real = root.join("home_real");
        fs::create_dir_all(&home_real).unwrap();
        make_project(&home_real); // pretend `~/.tokensave` is a project (it shouldn't be wiped)

        // Try to symlink: home_alias -> home_real. If the platform doesn't
        // allow symlinks (e.g. Windows without dev mode) we just skip the
        // canonical-equivalence check and verify the direct-path skip works.
        let home_alias = root.join("home_alias");
        let symlink_ok = symlink_dir(&home_real, &home_alias).is_ok();

        let cwd = root.clone();
        let alias_ts: PathBuf = if symlink_ok {
            home_alias.join(".tokensave")
        } else {
            home_real.join(".tokensave")
        };

        let out = gather_local_projects_from(&cwd, &Some(alias_ts));
        assert!(
            !out.contains(&home_real),
            "home `.tokensave` (canonical) must be skipped, got {out:?}"
        );
        if symlink_ok {
            assert!(
                !out.contains(&home_alias),
                "home `.tokensave` (alias) must be skipped, got {out:?}"
            );
        }
    }

    #[cfg(unix)]
    fn symlink_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(src, dst)
    }

    #[cfg(windows)]
    fn symlink_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(src, dst)
    }

    #[test]
    fn empty_dir_yields_empty_result() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().canonicalize().unwrap();
        let out = gather_local_projects_from(&cwd, &None);
        assert!(out.is_empty(), "got {out:?}");
    }
}
// direct test 1774739850
// daemon-test-1774740132
