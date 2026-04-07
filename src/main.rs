// Rust guideline compliant 2025-10-17
// Updated 2026-03-23: compact bordered table for status output
use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process;

use tokensave::tokensave::TokenSave;
use tokensave::context::{format_context_as_json, format_context_as_markdown};
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

    fn done(self, message: &str) {
        self.stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(h) = self.handle {
            let _ = h.join();
        }
        let mut stderr = std::io::stderr();
        // Show cursor again, then print the done line.
        let _ = write!(stderr, "\x1b[?25h");
        let _ = writeln!(stderr, "\r\x1b[2K\x1b[32m✔\x1b[0m {}", message);
        let _ = stderr.flush();
    }
}

/// Code intelligence for Rust codebases.
#[derive(Parser)]
#[command(name = "tokensave", about = "Code intelligence for 15 languages — semantic graph queries instead of file reads")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync the index (creates it if missing, incremental by default)
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
    /// Remove agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "uninstall", visible_alias = "claude-uninstall")]
    Uninstall {
        /// Agent to remove (removes all if omitted)
        #[arg(long)]
        agent: Option<String>,
    },
    /// PreToolUse hook handler (called by Claude Code, not by users directly)
    #[command(name = "hook-pre-tool-use", hide = true)]
    HookPreToolUse,
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

    // First-run notice (check BEFORE any config save creates the file)
    let is_first_run = tokensave::user_config::UserConfig::is_fresh();

    // Best-effort flush of pending worldwide counter tokens.
    // `matches!` borrows `command` temporarily; the borrow is dropped
    // before the `match command` move below, so this compiles.
    let is_force_flush = matches!(command, Commands::Sync { .. } | Commands::Status { .. });
    let mut user_config = tokensave::user_config::UserConfig::load();
    try_flush(&mut user_config, is_force_flush);
    user_config.save();

    if is_first_run {
        eprintln!(
            "note: tokensave uploads anonymous token-saved counts to a worldwide counter.\n\
             \x20     Run `tokensave disable-upload-counter` to opt out."
        );
    }

    // Best-effort check: warn if install needs re-running
    if !matches!(command, Commands::Install { .. }) {
        tokensave::agents::claude::check_install_stale();
    }

    match command {
        Commands::Sync { path, force, skip_folders, doctor } => {
            let project_path = tokensave::config::resolve_path(path);
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

            if force || !TokenSave::is_initialized(&project_path) {
                if !force {
                    eprintln!("No existing index found — performing full index");
                }
                init_and_index(&project_path, &skip_folders).await?;
            } else {
                let mut cg = TokenSave::open(&project_path).await?;
                cg.add_skip_folders(&skip_folders);
                let spinner = Spinner::new();
                let sync_start = std::time::Instant::now();
                let result = cg
                    .sync_with_progress(|current, total, detail| {
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
                            spinner.set_message(&format!("[{current}/{total}] syncing {detail}{eta}"));
                        }
                    })
                    .await?;
                spinner.done(&format!(
                    "sync done — {} added, {} modified, {} removed in {}ms",
                    result.files_added,
                    result.files_modified,
                    result.files_removed,
                    result.duration_ms
                ));
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
        Commands::Status { path, json, short } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = if TokenSave::is_initialized(&project_path) {
                TokenSave::open(&project_path).await?
            } else {
                eprint!(
                    "No TokenSave index found at '{}'. Create one now? [Y/n] ",
                    project_path.display()
                );
                io::stderr().flush().ok();
                let mut answer = String::new();
                io::stdin()
                    .lock()
                    .read_line(&mut answer)
                    .map_err(|e| tokensave::errors::TokenSaveError::Config {
                        message: format!("failed to read stdin: {e}"),
                    })?;
                let answer = answer.trim();
                if answer.is_empty() || answer.eq_ignore_ascii_case("y") {
                    init_and_index(&project_path, &[]).await?
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
                let global_tokens_saved = match tokensave::global_db::GlobalDb::open().await {
                    Some(gdb) => {
                        gdb.upsert(&project_path, tokens_saved).await;
                        gdb.global_tokens_saved().await
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
                if short {
                    tokensave::display::print_status_header(&stats, tokens_saved, global_tokens_saved, worldwide, &country_flags);
                } else {
                    tokensave::display::print_status_table(&stats, tokens_saved, global_tokens_saved, worldwide, &country_flags);
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
                    println!(
                        "  {} ({} bytes, {} symbols)",
                        f.path, f.size, f.node_count
                    );
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
                for line in stdin_handle.lock().lines() {
                    if let Ok(line) = line {
                        let trimmed = line.trim().to_string();
                        if !trimmed.is_empty() {
                            changed.push(trimmed);
                        }
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
            let home = tokensave::agents::home_dir().ok_or_else(|| tokensave::errors::TokenSaveError::Config {
                message: "could not determine home directory".to_string(),
            })?;
            let tokensave_bin = tokensave::agents::which_tokensave().ok_or_else(|| tokensave::errors::TokenSaveError::Config {
                message: "tokensave not found on PATH. Install it first:\n  \
                          cargo install tokensave\n  \
                          brew install aovestdipaperino/tap/tokensave".to_string(),
            })?;
            let mut user_cfg = tokensave::user_config::UserConfig::load();
            tokensave::agents::migrate_installed_agents(&home, &mut user_cfg);

            if let Some(id) = agent {
                let ag = tokensave::agents::get_integration(&id)?;
                let ctx = tokensave::agents::InstallContext {
                    home: home.clone(),
                    tokensave_bin: tokensave_bin.clone(),
                    tool_permissions: tokensave::agents::EXPECTED_TOOL_PERMS,
                };
                ag.install(&ctx)?;
                if !user_cfg.installed_agents.contains(&id) {
                    user_cfg.installed_agents.push(id);
                }
                user_cfg.save();
            } else {
                let (to_install, to_uninstall) =
                    tokensave::agents::pick_integrations_interactive(&home, &user_cfg.installed_agents)?;

                for id in &to_uninstall {
                    let ag = tokensave::agents::get_integration(id)?;
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: tokensave_bin.clone(),
                        tool_permissions: tokensave::agents::EXPECTED_TOOL_PERMS,
                    };
                    ag.uninstall(&ctx)?;
                    user_cfg.installed_agents.retain(|a| a != id);
                }
                for id in &to_install {
                    let ag = tokensave::agents::get_integration(id)?;
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: tokensave_bin.clone(),
                        tool_permissions: tokensave::agents::EXPECTED_TOOL_PERMS,
                    };
                    ag.install(&ctx)?;
                    if !user_cfg.installed_agents.contains(id) {
                        user_cfg.installed_agents.push(id.clone());
                    }
                }
                user_cfg.save();
            }

            tokensave::agents::offer_git_post_commit_hook(&tokensave_bin);
            tokensave::daemon::offer_daemon_autostart();
        }
        Commands::Uninstall { agent } => {
            let home = tokensave::agents::home_dir().ok_or_else(|| tokensave::errors::TokenSaveError::Config {
                message: "could not determine home directory".to_string(),
            })?;
            let mut user_cfg = tokensave::user_config::UserConfig::load();
            tokensave::agents::migrate_installed_agents(&home, &mut user_cfg);

            if let Some(id) = agent {
                let ag = tokensave::agents::get_integration(&id)?;
                let ctx = tokensave::agents::InstallContext {
                    home,
                    tokensave_bin: String::new(),
                    tool_permissions: tokensave::agents::EXPECTED_TOOL_PERMS,
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
                            tool_permissions: tokensave::agents::EXPECTED_TOOL_PERMS,
                        };
                        ag.uninstall(&ctx).ok();
                    }
                }
                user_cfg.installed_agents.clear();
                user_cfg.save();
                eprintln!("All agent integrations removed.");
            }
        }
        Commands::HookPreToolUse => {
            tokensave::hooks::hook_pre_tool_use();
        }
        Commands::Serve { path } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
            let server = tokensave::mcp::McpServer::new(cg).await;
            let mut transport = tokensave::mcp::StdioTransport::new();
            server.run(&mut transport).await?;
        }
        Commands::Upgrade => {
            tokensave::upgrade::run_upgrade()?;
        }
        Commands::Channel { channel } => {
            match channel {
                Some(target) => { tokensave::upgrade::switch_channel(&target)?; }
                None => tokensave::upgrade::show_channel(),
            }
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
                    eprintln!("gitignore enabled — .gitignore rules will be respected during indexing.");
                    eprintln!("Run `tokensave sync` to re-index with the new setting.");
                }
                Some("off") => {
                    config.git_ignore = false;
                    tokensave::config::save_config(&project_path, &config)?;
                    eprintln!("gitignore disabled — .gitignore rules will be ignored during indexing.");
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
        Commands::Daemon { foreground, stop, status, enable_autostart, disable_autostart } => {
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
                let upgraded = tokensave::daemon::run(foreground).await?;
                if upgraded {
                    // Exit with non-zero code so the service manager (launchd
                    // KeepAlive / systemd Restart=on-failure / Windows SCM
                    // failure actions) restarts with the new binary.
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
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
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| tokensave::errors::TokenSaveError::Config {
            message: format!("failed to read stdin: {}", e),
        })?;
    let answer = answer.trim();
    if answer.is_empty() || answer.eq_ignore_ascii_case("y") {
        init_and_index(&project_path, &[]).await?;
    }
    Ok(())
}


/// Initializes a new project (if needed) and runs a full index.
async fn init_and_index(project_path: &Path, skip_folders: &[String]) -> tokensave::errors::Result<TokenSave> {
    debug_assert!(project_path.is_dir(), "init_and_index: project_path is not a directory");
    debug_assert!(project_path.is_absolute(), "init_and_index: project_path must be absolute");
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
    let result = cg.index_all_with_progress(|current, total, file| {
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
    }).await?;
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

/// Opens an existing project, or tells the user to run `tokensave sync` first.
async fn ensure_initialized(project_path: &Path) -> tokensave::errors::Result<TokenSave> {
    if TokenSave::is_initialized(project_path) {
        return TokenSave::open(project_path).await;
    }
    Err(tokensave::errors::TokenSaveError::Config {
        message: format!(
            "no TokenSave index found at '{}' — run 'tokensave sync' first",
            project_path.display()
        ),
    })
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
fn check_for_update(config: &mut tokensave::user_config::UserConfig, skip_cache: bool, skip_suppression: bool) {
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

    if dominated && (skip_suppression || now - config.last_version_warning_at >= 900)
    {
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
    debug_assert!(!changed_files.is_empty(), "find_affected_tests called with no changed files");
    debug_assert!(max_depth > 0, "find_affected_tests max_depth must be positive");
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
// direct test 1774739850
// daemon-test-1774740132
