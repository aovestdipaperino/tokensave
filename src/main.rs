// Rust guideline compliant 2025-10-17
// Updated 2026-03-23: compact bordered table for status output
use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process;

use tokensave::agents::DoctorCounters;
use tokensave::tokensave::TokenSave;
use tokensave::context::{format_context_as_json, format_context_as_markdown};
use tokensave::types::*;

/// Returns the current UNIX timestamp in seconds.
fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// A self-animating spinner that ticks on a background thread.
///
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
    },
    /// Show project statistics
    Status {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Output as JSON
        #[arg(short, long)]
        json: bool,
        /// Show country flags of worldwide users
        #[arg(long)]
        show_flags: bool,
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
        /// Agent to configure (default: claude)
        #[arg(long, default_value = "claude")]
        agent: String,
    },
    /// Remove agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "uninstall", visible_alias = "claude-uninstall")]
    Uninstall {
        /// Agent to remove (default: claude)
        #[arg(long, default_value = "claude")]
        agent: String,
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
    /// Disable uploading token counts to the worldwide counter
    #[command(name = "disable-upload-counter")]
    DisableUploadCounter,
    /// Enable uploading token counts to the worldwide counter
    #[command(name = "enable-upload-counter")]
    EnableUploadCounter,
    /// Check tokensave installation, configuration, and agent integration
    Doctor {
        /// Check only this agent (default: all agents)
        #[arg(long)]
        agent: Option<String>,
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
        Commands::Sync { path, force } => {
            let project_path = resolve_path(path);
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
                init_and_index(&project_path).await?;
            } else {
                let cg = TokenSave::open(&project_path).await?;
                let spinner = Spinner::new();
                let result = cg
                    .sync_with_progress(|phase, detail| {
                        let msg = if detail.is_empty() {
                            phase.to_string()
                        } else {
                            format!("{phase} {detail}")
                        };
                        spinner.set_message(&msg);
                    })
                    .await?;
                spinner.done(&format!(
                    "sync done — {} added, {} modified, {} removed in {}ms",
                    result.files_added,
                    result.files_modified,
                    result.files_removed,
                    result.duration_ms
                ));
                update_global_db(&cg).await;
            }

            // Print update notice from parallel check
            if let Ok(Some(latest)) = version_handle.join() {
                let current_version = env!("CARGO_PKG_VERSION");
                let mut config = tokensave::user_config::UserConfig::load();
                config.cached_latest_version = latest.clone();
                config.last_version_check_at = current_unix_timestamp();
                config.save();
                if tokensave::cloud::is_newer_version(current_version, &latest) {
                    let method = tokensave::cloud::detect_install_method();
                    let cmd = tokensave::cloud::upgrade_command(&method);
                    eprintln!(
                        "\n\x1b[33mUpdate available: v{} → v{}\x1b[0m\n  Run: \x1b[1m{}\x1b[0m",
                        current_version, latest, cmd
                    );
                }
            }
        }
        Commands::Status { path, json, show_flags } => {
            let project_path = resolve_path(path);
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
                    init_and_index(&project_path).await?
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
                // Register project and read global total in one open
                let global_tokens_saved = match tokensave::global_db::GlobalDb::open().await {
                    Some(gdb) => {
                        gdb.upsert(&project_path, tokens_saved).await;
                        gdb.global_tokens_saved().await
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
                let country_flags = if show_flags {
                    tokensave::cloud::fetch_country_flags()
                } else {
                    Vec::new()
                };
                print!("{}", include_str!("resources/logo.ansi"));
                print_status_table(&stats, tokens_saved, global_tokens_saved, worldwide, &country_flags);

                // Version check (5 min cache)
                check_for_update(&mut config, false);
            }
        }
        Commands::Query {
            search,
            path,
            limit,
        } => {
            let project_path = resolve_path(path);
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
            let project_path = resolve_path(path);
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
            let project_path = resolve_path(path);
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
            let project_path = resolve_path(path);
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
            let ag = tokensave::agents::get_agent(&agent)?;
            let home = tokensave::agents::home_dir().ok_or_else(|| tokensave::errors::TokenSaveError::Config {
                message: "could not determine home directory".to_string(),
            })?;
            let tokensave_bin = tokensave::agents::which_tokensave().ok_or_else(|| tokensave::errors::TokenSaveError::Config {
                message: "tokensave not found on PATH. Install it first:\n  \
                          cargo install tokensave\n  \
                          brew install aovestdipaperino/tap/tokensave".to_string(),
            })?;
            let ctx = tokensave::agents::InstallContext {
                home,
                tokensave_bin: tokensave_bin.clone(),
                tool_permissions: tokensave::agents::EXPECTED_TOOL_PERMS,
            };
            ag.install(&ctx)?;
            tokensave::agents::offer_git_post_commit_hook(&tokensave_bin);
        }
        Commands::Uninstall { agent } => {
            let ag = tokensave::agents::get_agent(&agent)?;
            let home = tokensave::agents::home_dir().ok_or_else(|| tokensave::errors::TokenSaveError::Config {
                message: "could not determine home directory".to_string(),
            })?;
            let ctx = tokensave::agents::InstallContext {
                home,
                tokensave_bin: String::new(),
                tool_permissions: tokensave::agents::EXPECTED_TOOL_PERMS,
            };
            ag.uninstall(&ctx)?;
        }
        Commands::HookPreToolUse => {
            hook_pre_tool_use();
        }
        Commands::Serve { path } => {
            let project_path = resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
            let server = tokensave::mcp::McpServer::new(cg).await;
            server.run().await?;
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
        Commands::Doctor { agent } => {
            run_doctor(agent.as_deref());
        }
    }
    Ok(())
}

/// When invoked with no subcommand, offer to create the index if none exists.
async fn handle_no_command() -> tokensave::errors::Result<()> {
    let project_path = resolve_path(None);
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
        init_and_index(&project_path).await?;
    }
    Ok(())
}

/// Initializes a new project (if needed) and runs a full index.
async fn init_and_index(project_path: &Path) -> tokensave::errors::Result<TokenSave> {
    debug_assert!(project_path.is_dir(), "init_and_index: project_path is not a directory");
    debug_assert!(project_path.is_absolute(), "init_and_index: project_path must be absolute");
    let cg = if TokenSave::is_initialized(project_path) {
        TokenSave::open(project_path).await?
    } else {
        let cg = TokenSave::init(project_path).await?;
        eprintln!("Initialized TokenSave at {}", project_path.display());
        cg
    };
    let spinner = Spinner::new();
    let result = cg.index_all_with_progress(|file| {
        spinner.set_message(&format!("indexing {}", file));
    }).await?;
    spinner.done(&format!(
        "indexing done — {} files, {} nodes, {} edges in {}ms",
        result.file_count, result.node_count, result.edge_count, result.duration_ms
    ));
    update_global_db(&cg).await;
    Ok(cg)
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

/// Best-effort version check with 5-minute cache. If `skip_cache` is true,
/// always fetches from GitHub (used during sync where the call runs in parallel).
fn check_for_update(config: &mut tokensave::user_config::UserConfig, skip_cache: bool) {
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

    if tokensave::cloud::is_newer_version(current_version, &latest) {
        let method = tokensave::cloud::detect_install_method();
        let cmd = tokensave::cloud::upgrade_command(&method);
        eprintln!(
            "\n\x1b[33mUpdate available: v{} → v{}\x1b[0m\n  Run: \x1b[1m{}\x1b[0m",
            current_version, latest, cmd
        );
    }
}

/// Formats a token count into a human-readable string (e.g. "12.3k", "1.5M").
fn format_token_count(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Formats a byte count into a human-readable string (e.g. "798.0 MB").
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Formats a number with comma separators (e.g. 243302 -> "243,302").
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

/// Formats a single table cell with left-aligned label and right-aligned value.
fn format_cell(label: &str, value: &str, width: usize) -> String {
    let content_len = label.len() + value.len();
    let pad = width.saturating_sub(2 + content_len);
    format!(" {}{}{} ", label, " ".repeat(pad), value)
}

/// Builds a horizontal separator line (e.g. ├──┬──┬──┤).
fn table_separator(left: char, mid: char, right: char, cell_width: usize, num_cols: usize) -> String {
    let mut line = String::from(left);
    for i in 0..num_cols {
        line.push_str(&"─".repeat(cell_width));
        line.push(if i < num_cols - 1 { mid } else { right });
    }
    line
}

/// Prints the status output as a compact bordered table.
fn print_status_table(
    stats: &tokensave::types::GraphStats,
    tokens_saved: u64,
    global_tokens_saved: Option<u64>,
    worldwide: Option<u64>,
    country_flags: &[String],
) {
    let num_cols = 3;
    debug_assert!(stats.file_count > 0 || stats.node_count == 0,
        "print_status_table: node_count should be 0 when file_count is 0");
    debug_assert!(stats.node_count >= stats.file_count || stats.file_count == 0,
        "print_status_table: node_count should be >= file_count");

    let mut sorted_kinds: Vec<_> = stats.nodes_by_kind.iter().collect();
    sorted_kinds.sort_by_key(|(k, _)| (*k).clone());
    let num_kind_rows = sorted_kinds.len().div_ceil(num_cols);

    let cell_width = compute_cell_width(&sorted_kinds);
    let inner_width = cell_width * num_cols + (num_cols - 1);

    println!("{}", table_separator('╭', '─', '╮', cell_width, num_cols));
    print_title_row(tokens_saved, global_tokens_saved, worldwide, inner_width);
    print_flags_row(country_flags, inner_width);
    println!("{}", table_separator('├', '┬', '┤', cell_width, num_cols));

    let stats_rows = build_stats_rows(stats, num_cols);
    print_table_rows(&stats_rows, cell_width, num_cols);

    if !sorted_kinds.is_empty() {
        println!("{}", table_separator('├', '┼', '┤', cell_width, num_cols));
        print_kind_rows(&sorted_kinds, num_kind_rows, num_cols, cell_width);
    }

    println!("{}", table_separator('╰', '┴', '╯', cell_width, num_cols));
}

/// Compute cell width from the widest node-kind entry.
fn compute_cell_width(sorted_kinds: &[(&String, &u64)]) -> usize {
    let max_kind_len = sorted_kinds.iter().map(|(k, _)| k.len()).max().unwrap_or(10);
    let max_count_len = sorted_kinds.iter().map(|(_, c)| format_number(**c).len()).max().unwrap_or(5);
    (max_kind_len + max_count_len + 3).max(22)
}

/// Print the title row with version and token counts.
fn print_title_row(
    tokens_saved: u64,
    global_tokens_saved: Option<u64>,
    worldwide: Option<u64>,
    inner_width: usize,
) {
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("TokenSave v{}", version);
    let tokens_text = {
        let mut parts = Vec::new();
        match global_tokens_saved {
            Some(global) => {
                parts.push(format!("Local ~{}", format_token_count(tokens_saved)));
                parts.push(format!("Global ~{}", format_token_count(global)));
            }
            None => {
                parts.push(format!("Saved ~{}", format_token_count(tokens_saved)));
            }
        }
        if let Some(ww) = worldwide {
            parts.push(format!("Worldwide ~{}", format_token_count(ww)));
        }
        parts.join("  ")
    };
    let title_pad = inner_width.saturating_sub(2 + title.len() + tokens_text.len());
    println!(
        "│ {}{}\x1b[32m{}\x1b[0m │",
        title,
        " ".repeat(title_pad),
        tokens_text
    );
}

/// Print centered country flags row if any flags are provided.
fn print_flags_row(country_flags: &[String], inner_width: usize) {
    if country_flags.is_empty() { return; }
    let available = inner_width.saturating_sub(2);
    let mut flags_str = String::new();
    let mut display_width = 0;
    let flag_width = 2;
    for (i, flag) in country_flags.iter().enumerate() {
        let needed = if i == 0 { flag_width } else { 1 + flag_width };
        let reserve = if i + 1 < country_flags.len() { 2 } else { 0 };
        if display_width + needed + reserve > available {
            flags_str.push_str(" …");
            display_width += 2;
            break;
        }
        if i > 0 {
            flags_str.push(' ');
            display_width += 1;
        }
        flags_str.push_str(flag);
        display_width += flag_width;
    }
    let left_pad = (available.saturating_sub(display_width)) / 2;
    let right_pad = available.saturating_sub(display_width + left_pad);
    println!("│ {}{}{} │", " ".repeat(left_pad), flags_str, " ".repeat(right_pad));
}

/// Build the stats rows (files/nodes/edges, DB size, languages).
fn build_stats_rows<'a>(
    stats: &'a tokensave::types::GraphStats,
    num_cols: usize,
) -> Vec<Vec<(&'a str, String)>> {
    let mut sorted_langs: Vec<_> = stats.files_by_language.iter().collect();
    sorted_langs.sort_by(|a, b| b.1.cmp(a.1));

    let mut rows: Vec<Vec<(&str, String)>> = vec![vec![
        ("Files", format_number(stats.file_count)),
        ("Nodes", format_number(stats.node_count)),
        ("Edges", format_number(stats.edge_count)),
    ]];

    let mut second_row: Vec<(&str, String)> = vec![("DB Size", format_bytes(stats.db_size_bytes))];
    if stats.total_source_bytes > 0 {
        second_row.push(("Source", format_bytes(stats.total_source_bytes)));
    }
    let mut lang_idx = 0;
    while second_row.len() < num_cols && lang_idx < sorted_langs.len() {
        let (lang, count) = sorted_langs[lang_idx];
        second_row.push((lang.as_str(), format_number(*count)));
        lang_idx += 1;
    }
    while second_row.len() < num_cols {
        second_row.push(("", String::new()));
    }
    rows.push(second_row);

    while lang_idx < sorted_langs.len() {
        let mut row: Vec<(&str, String)> = Vec::new();
        for _ in 0..num_cols {
            if lang_idx < sorted_langs.len() {
                let (lang, count) = sorted_langs[lang_idx];
                row.push((lang.as_str(), format_number(*count)));
                lang_idx += 1;
            } else {
                row.push(("", String::new()));
            }
        }
        rows.push(row);
    }
    rows
}

/// Print rows of label-value pairs in a bordered table.
fn print_table_rows(rows: &[Vec<(&str, String)>], cell_width: usize, num_cols: usize) {
    for row in rows {
        print!("│");
        for (i, (label, value)) in row.iter().enumerate() {
            if label.is_empty() {
                print!("{}", " ".repeat(cell_width));
            } else {
                print!("{}", format_cell(label, value, cell_width));
            }
            print!("{}", if i < num_cols - 1 { "│" } else { "│\n" });
        }
    }
}

/// Print node kinds in column-major order.
fn print_kind_rows(sorted_kinds: &[(&String, &u64)], num_kind_rows: usize, num_cols: usize, cell_width: usize) {
    for r in 0..num_kind_rows {
        print!("│");
        for c in 0..num_cols {
            let idx = r + c * num_kind_rows;
            if idx < sorted_kinds.len() {
                let (kind, count) = &sorted_kinds[idx];
                print!("{}", format_cell(kind, &format_number(**count), cell_width));
            } else {
                print!("{}", " ".repeat(cell_width));
            }
            print!("{}", if c < num_cols - 1 { "│" } else { "│\n" });
        }
    }
}

/// Runs a comprehensive health check of the tokensave installation.
fn run_doctor(agent_filter: Option<&str>) {
    use tokensave::agents::{self, DoctorCounters, HealthcheckContext};

    debug_assert!(!env!("CARGO_PKG_VERSION").is_empty(), "CARGO_PKG_VERSION must not be empty");
    let mut dc = DoctorCounters::new();

    eprintln!("\n\x1b[1mtokensave doctor v{}\x1b[0m\n", env!("CARGO_PKG_VERSION"));

    doctor_check_binary(&mut dc);

    eprintln!("\n\x1b[1mCurrent project\x1b[0m");
    let project_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if TokenSave::is_initialized(&project_path) {
        dc.pass(&format!("Index found: {}/.tokensave/", project_path.display()));
    } else {
        dc.warn(&format!("No index at {}/.tokensave/ — run `tokensave sync`", project_path.display()));
    }

    doctor_check_global_db(&mut dc);
    doctor_check_user_config(&mut dc);

    // Agent-specific health checks
    if let Some(ref home) = agents::home_dir() {
        let hctx = HealthcheckContext {
            home: home.clone(),
            project_path: project_path.clone(),
        };
        let agents_to_check: Vec<Box<dyn agents::Agent>> = match agent_filter {
            Some(id) => match agents::get_agent(id) {
                Ok(ag) => vec![ag],
                Err(e) => {
                    dc.fail(&format!("{e}"));
                    vec![]
                }
            },
            None => agents::all_agents(),
        };
        for ag in &agents_to_check {
            ag.healthcheck(&mut dc, &hctx);
        }
    } else {
        dc.fail("Could not determine home directory");
    }

    doctor_check_network(&mut dc);
    doctor_print_summary(&dc);
}

/// Doctor: check binary location and version.
fn doctor_check_binary(dc: &mut DoctorCounters) {
    eprintln!("\x1b[1mBinary\x1b[0m");
    if let Ok(exe) = std::env::current_exe() {
        dc.pass(&format!("Binary: {}", exe.display()));
    } else {
        dc.fail("Could not determine binary path");
    }
    dc.pass(&format!("Version: {}", env!("CARGO_PKG_VERSION")));
}

/// Doctor: check global database exists.
fn doctor_check_global_db(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mGlobal database\x1b[0m");
    if let Some(db_path) = tokensave::global_db::global_db_path() {
        if db_path.exists() {
            dc.pass(&format!("Global DB: {}", db_path.display()));
        } else {
            dc.warn("Global DB not yet created (created on first sync)");
        }
    } else {
        dc.fail("Could not determine home directory for global DB");
    }
}

/// Doctor: check user config file.
fn doctor_check_user_config(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mUser config\x1b[0m");
    if let Some(config_path) = tokensave::user_config::config_path() {
        if config_path.exists() {
            let config = tokensave::user_config::UserConfig::load();
            dc.pass(&format!("Config: {}", config_path.display()));
            if config.upload_enabled {
                dc.pass("Upload enabled");
            } else {
                dc.info("Upload disabled (opt-out)");
            }
            if config.pending_upload > 0 {
                dc.info(&format!("Pending upload: {} tokens", config.pending_upload));
            }
        } else {
            dc.warn("Config not yet created (created on first sync)");
        }
    } else {
        dc.fail("Could not determine home directory for config");
    }
}

/// Doctor: check network connectivity.
fn doctor_check_network(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mNetwork\x1b[0m");
    if let Some(total) = tokensave::cloud::fetch_worldwide_total() {
        dc.pass(&format!("Worldwide counter reachable (total: {})", format_token_count(total)));
    } else {
        dc.warn("Worldwide counter unreachable (offline or timeout)");
    }
    if tokensave::cloud::fetch_latest_version().is_some() {
        dc.pass("GitHub releases API reachable");
    } else {
        dc.warn("GitHub releases API unreachable (offline or timeout)");
    }
}

/// Doctor: print final summary.
fn doctor_print_summary(dc: &DoctorCounters) {
    eprintln!();
    if dc.issues == 0 && dc.warnings == 0 {
        eprintln!("\x1b[32mAll checks passed.\x1b[0m");
    } else if dc.issues == 0 {
        eprintln!("\x1b[33m{} warning(s), no issues.\x1b[0m", dc.warnings);
    } else {
        eprintln!("\x1b[31m{} issue(s), {} warning(s).\x1b[0m", dc.issues, dc.warnings);
        eprintln!("Run \x1b[1mtokensave install\x1b[0m to fix most issues.");
    }
    eprintln!();
}

/// PreToolUse hook handler for Claude Code's Agent tool matcher.
///
/// Reads the `TOOL_INPUT` environment variable (JSON), inspects the
/// `subagent_type` and `prompt` fields, and prints a JSON decision to
/// stdout. Blocks Explore agents and exploration-style prompts, directing
/// Claude to use tokensave MCP tools instead.
fn hook_pre_tool_use() {
    let tool_input = std::env::var("TOOL_INPUT").unwrap_or_default();

    let block_msg = serde_json::json!({
        "decision": "block",
        "reason": "STOP: Use tokensave MCP tools (tokensave_context, tokensave_search, \
                   tokensave_callees, tokensave_callers, tokensave_impact, tokensave_files, \
                   tokensave_affected) instead of agents for code research. Tokensave is \
                   faster and more precise for symbol relationships, call paths, and code \
                   structure. Only use agents for code exploration if you have already tried \
                   tokensave and it cannot answer the question."
    });

    let parsed: serde_json::Value =
        serde_json::from_str(&tool_input).unwrap_or_else(|_| serde_json::json!({}));

    // Block Explore agents outright
    if parsed.get("subagent_type").and_then(|v| v.as_str()) == Some("Explore") {
        println!("{}", block_msg);
        return;
    }

    // Check if the prompt is exploration/research work that tokensave can handle
    if let Some(prompt) = parsed.get("prompt").and_then(|v| v.as_str()) {
        let lower = prompt.to_ascii_lowercase();
        let exploration_patterns = [
            "explore", "codebase structure", "codebase architecture", "codebase overview",
            "source files contents", "read every", "full contents", "entire codebase",
            "architecture and structure", "call graph", "call path", "call chain",
            "symbol relat", "symbol lookup", "who calls", "callers of", "callees of",
        ];
        if exploration_patterns.iter().any(|pat| lower.contains(pat)) {
            println!("{}", block_msg);
            return;
        }
    }

    println!(r#"{{"decision": "allow"}}"#);
}

fn resolve_path(path: Option<String>) -> PathBuf {
    match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Returns `true` if the file path looks like a test file.
fn is_test_file(path: &str) -> bool {
    // Common test file naming conventions
    let test_segments = [
        "test/", "tests/", "__tests__/", "spec/", "e2e/",
        ".test.", ".spec.", "_test.", "_spec.",
    ];
    let lower = path.to_ascii_lowercase();
    test_segments.iter().any(|s| lower.contains(s))
}

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
            is_test_file(path)
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
