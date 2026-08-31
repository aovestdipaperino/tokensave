// Rust guideline compliant 2025-10-17
// Updated 2026-03-23: compact bordered table for status output
use clap::Parser;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process;

use tokensave::tokensave::TokenSave;

mod cli;
mod commands;
mod global;
mod serve;
mod tool_command;

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

/// Build the JSON object for one `by_agent` row.
///
/// `cost_usd` is `null` for Droid (a credits-only agent with no USD pricing);
/// it is a numeric value for all other agents.
/// All token fields and `credits` are always present.
fn agent_row_json(a: &tokensave::global_db::AgentCostSummary) -> serde_json::Value {
    let cost_usd: serde_json::Value = if a.agent == "droid" {
        serde_json::Value::Null
    } else {
        serde_json::json!(a.cost_usd)
    };
    serde_json::json!({
        "agent": a.agent,
        "cost_usd": cost_usd,
        "credits": a.credits,
        "input_tokens": a.input_tokens,
        "output_tokens": a.output_tokens,
        "cache_write_tokens": a.cache_write_tokens,
        "cache_read_tokens": a.cache_read_tokens,
        "turns": a.turns,
    })
}

/// Format one row of the `--by-agent` table.
///
/// Columns: agent (left), USD (right), credits (right), raw tokens (right), rows (right).
/// Droid USD is always "n/a"; other agents are formatted as `$X.XX`.
/// Credits are formatted via `format_token_count` when present, or "n/a".
fn format_agent_row(a: &tokensave::global_db::AgentCostSummary) -> String {
    let raw = a
        .input_tokens
        .saturating_add(a.output_tokens)
        .saturating_add(a.cache_write_tokens)
        .saturating_add(a.cache_read_tokens);
    let raw_str = tokensave::display::format_token_count(raw);
    let credits_str = match a.credits {
        Some(c) => tokensave::display::format_token_count(c),
        None => "n/a".to_string(),
    };
    let usd_str = if a.agent == "droid" {
        "n/a".to_string()
    } else {
        format!("${:.2}", a.cost_usd)
    };
    format!(
        "  {:<16} {:>9} {:>9} {:>12} {:>6}",
        a.agent, usd_str, credits_str, raw_str, a.turns
    )
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

    let skip_agent_install_maintenance = should_skip_agent_install_maintenance(&command);

    // First-run notice (check BEFORE any config save creates the file)
    let is_first_run = tokensave::user_config::UserConfig::is_fresh();

    // Best-effort flush of pending worldwide counter tokens. Which command is
    // running no longer affects this: `try_flush` gates on one daily interval
    // shared by every upload path, so `init`/`sync`/`status` no longer force an
    // attempt the other commands would have skipped.
    let mut user_config = tokensave::user_config::UserConfig::load();
    // Skip the worldwide-counter flush on hot startup paths. `try_flush`
    // makes a synchronous HTTP call (#84) which can add seconds to
    // `tokensave serve` startup on slow networks — long enough to blow the
    // MCP client's 30 s `initialize` timeout.
    if !skip_agent_install_maintenance {
        global::try_flush(&mut user_config);
    }
    user_config.save();

    if is_first_run && !skip_agent_install_maintenance {
        eprintln!(
            "note: tokensave uploads anonymous token-saved counts to a worldwide counter.\n\
             \x20     Run `tokensave disable-upload-counter` to opt out."
        );
    }

    // The "beta merged into stable" nudge that lived here through 4.3.x was
    // retired in 4.3.12. The beta channel is open again as of v5.0.0-beta.1
    // and beta users now stay on beta until they explicitly switch off.

    // Best-effort check: warn if install needs re-running.
    if !skip_agent_install_maintenance {
        tokensave::agents::claude::check_install_stale();
    }

    // Silent reinstall: re-run install for every tracked agent so permissions,
    // hooks, and MCP config stay in sync with the new binary.
    //
    // Two signals can trigger this:
    //   (a) `previous_version` (set by `tokensave upgrade` / `channel switch`
    //       just before replacing the binary) differs from the running version
    //       AND the transition is a minor/major bump. Patch bumps are no-ops:
    //       we just advance `previous_version` and skip reinstall.
    //   (b) Fallback for external upgrades (`brew upgrade`, `cargo install`):
    //       the running version is newer than `last_installed_version`.
    if !skip_agent_install_maintenance {
        let running = env!("CARGO_PKG_VERSION");
        let paths = (
            tokensave::agents::home_dir(),
            tokensave::agents::which_tokensave(),
        );
        // Without a home dir or a resolvable binary path there is nothing to
        // write, so leave the markers alone and retry on the next run.
        if let (Some(home), Some(bin)) = paths {
            let wildcard = user_config.wildcard_permissions;
            // This resync is background maintenance, not a user-requested
            // install: keep the per-agent setup banners off stderr so they
            // don't appear on every `init`/`sync` (#255).
            tokensave::agents::set_quiet_install(true);
            // Captured before the resync, which advances both markers.
            let resynced_from = if user_config.last_installed_version.is_empty() {
                user_config.previous_version.clone()
            } else {
                user_config.last_installed_version.clone()
            };
            let agent_count = user_config.installed_agents.len();
            let outcome =
                tokensave::agents::resync_installed_agents(&mut user_config, running, |id| {
                    let Ok(ag) = tokensave::agents::get_integration(id) else {
                        return true;
                    };
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: bin.clone(),
                        tool_permissions: tokensave::agents::install_tool_perms(wildcard),
                        scope: tokensave::agents::InstallScope::Global,
                        // Silent reinstall on upgrade is never an explicit
                        // style request — preserve an existing covering
                        // grant rather than clobbering it.
                        force_permission_style: false,
                    };
                    ag.install(&ctx).is_ok()
                });
            tokensave::agents::set_quiet_install(false);
            if outcome.ran {
                // Say what this was and why (#419). The user did not ask for an
                // install — they may well have run a read-only query — so a
                // write to their agent config has to name itself. Before this,
                // the only output was a bare `✔ Wrote <path>` escaping from the
                // file layer, which read as though the query had done it.
                eprintln!(
                    "\x1b[32m✔\x1b[0m {}",
                    resync_summary(running, &resynced_from, agent_count)
                );
                if let Some(warning) = tokensave::agents::cargo_build_binary_warning(&bin) {
                    eprintln!("{warning}");
                }
            }
            if !outcome.failed.is_empty() {
                eprintln!(
                    "\x1b[33mwarning:\x1b[0m could not refresh tokensave config for: {}.\n  \
                     Run \x1b[1mtokensave install\x1b[0m to see the error.",
                    outcome.failed.join(", ")
                );
            }
            if outcome.changed {
                user_config.save();
            }
        }
    }

    match command {
        Commands::Init {
            path,
            skip_folders,
            git_hook,
            no_git_hook,
        } => {
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
            // Guard against indexing a large non-project tree (e.g. a home dir).
            // Outside a git working tree the Git-supplied exclude sources are
            // unavailable, and with no `.gitignore` to constrain the walk,
            // toolchain trees (site-packages, conda envs, caches) get indexed
            // wholesale and the DB grows unbounded (#174). Confirm before
            // proceeding; skip the prompt when stdin isn't a TTY (CI/scripts).
            if !tokensave::config::is_inside_git_repo(&project_path) {
                eprintln!("{}", tokensave::config::non_git_scan_warning(&project_path));
                if std::io::stdin().is_terminal() {
                    eprint!("Continue initializing here anyway? [y/N] ");
                    io::stderr().flush().ok();
                    let mut answer = String::new();
                    io::stdin().lock().read_line(&mut answer).map_err(|e| {
                        tokensave::errors::TokenSaveError::Config {
                            message: format!("failed to read stdin: {}", e),
                        }
                    })?;
                    if !answer.trim().eq_ignore_ascii_case("y") {
                        eprintln!("Aborted.");
                        std::process::exit(1);
                    }
                }
            }
            // Check for updates in parallel with indexing
            let version_handle = std::thread::spawn(tokensave::cloud::fetch_latest_version_passive);

            // Memory instrumentation for #253: the initial full index is
            // the largest single graph build; record its phases (emitted
            // from inside indexing.rs) against this process.
            tokensave::memstats::init("index", &project_path);
            tokensave::memstats::record("start");

            commands::init_and_index(&project_path, &skip_folders, false).await?;

            // Offer this repository's own git hooks (#455). Local rather than
            // global, because the global path claims `core.hooksPath` — one
            // machine-wide setting that forces the same hook directory on
            // every repository — and `init` is per-project by definition.
            offer_local_git_hooks(&project_path, git_hook, no_git_hook);

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
            // An explicit sync is unbounded by design and can run for minutes
            // on a large tree; Ctrl-C must stop it at the next phase boundary
            // rather than at the end (#450).
            tokensave::cancel::install_signal_handlers();
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
            let version_handle = std::thread::spawn(tokensave::cloud::fetch_latest_version_passive);

            // Memory instrumentation for #253: the sync/indexing path is
            // the suspected transient RSS peak; phases are recorded from
            // inside the indexing code itself.
            tokensave::memstats::init("sync", &project_path);
            tokensave::memstats::record("start");

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
                if let Some(warning) = cg.warn_skipped_hidden_dirs() {
                    eprintln!("{warning}");
                }
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
                } else if !verbose {
                    // Verbose already emitted the full per-extension list mid-run.
                    commands::print_skipped_extension_summary(&result.skipped_extensions);
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
            runtime,
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
            if runtime {
                let snap = tokensave::runtime_telemetry::collect(&cg).await?;
                if json {
                    println!("{}", tokensave::runtime_telemetry::to_pretty_json(&snap));
                } else {
                    print!("{}", tokensave::runtime_telemetry::to_text_report(&snap));
                }
                return Ok(());
            }
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
                let droid_present = if let Some(ref db) = gdb {
                    let stats = tokensave::accounting::parser::ingest(db).await;
                    stats.coverage.iter().any(|coverage| {
                        coverage.agent == "droid"
                            && coverage.state != tokensave::accounting::CoverageState::Absent
                    })
                } else {
                    false
                };
                // Best-effort cost summary for the status header.
                let cost_info = match &gdb {
                    Some(db) => {
                        tokensave::accounting::metrics::quick_cost_summary_with_droid_presence(
                            db,
                            tokens_saved,
                            global_tokens_saved.unwrap_or(0),
                            droid_present,
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
                        Some(cg.project_root()),
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
                        Some(cg.project_root()),
                        details,
                    );
                }

                // Warn if .tokensave is not in .gitignore
                if !tokensave::config::is_in_gitignore(&project_path) {
                    eprintln!(
                        "\n\x1b[33mWarning: .tokensave is not excluded from git — \
                         run `echo '.tokensave/' >> .git/info/exclude` to exclude it locally.\x1b[0m"
                    );
                }

                // Version check (5 min cache, always show for status)
                global::check_for_update(&mut config, false, true);
            }
        }
        Commands::Tool { name, args } => {
            tool_command::run(name, args).await?;
        }
        Commands::Install {
            agent,
            git_hook,
            local,
            wildcard_permissions,
            explicit_permissions,
        } => {
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
            let scope = resolve_install_scope(local)?;
            let mut user_cfg = tokensave::user_config::UserConfig::load();
            tokensave::agents::migrate_installed_agents(&home, &mut user_cfg);

            // --wildcard-permissions / --explicit-permissions (mutually
            // exclusive, enforced by clap) override the persisted default for
            // this run; a flag on a global install also updates the default
            // for future silent reinstalls. A --local install is
            // project-scoped and must not touch the persisted global choice.
            if wildcard_permissions {
                user_cfg.wildcard_permissions = true;
                if !local {
                    user_cfg.save();
                }
            } else if explicit_permissions {
                user_cfg.wildcard_permissions = false;
                if !local {
                    user_cfg.save();
                }
            }
            let want_wildcard = user_cfg.wildcard_permissions;
            // Only an explicit flag on this invocation counts as a style
            // request; a flagless install must preserve an existing covering
            // grant instead of clobbering it (see `install_permissions` in
            // `agents/claude.rs`).
            let force_permission_style = wildcard_permissions || explicit_permissions;

            let mut installed_names: Vec<String> = Vec::new();
            let mut removed_names: Vec<String> = Vec::new();

            if let Some(id) = agent {
                let ag = tokensave::agents::get_integration(&id)?;
                let name = ag.name().to_string();
                if local && !ag.supports_local() {
                    return Err(tokensave::errors::TokenSaveError::Config {
                        message: format!(
                            "--local is not supported for \"{}\" — it has no project-scoped config. \
                             Run a global install instead (omit --local).",
                            ag.id()
                        ),
                    });
                }
                let ctx = tokensave::agents::InstallContext {
                    home: home.clone(),
                    tokensave_bin: tokensave_bin.clone(),
                    tool_permissions: tokensave::agents::install_tool_perms(want_wildcard),
                    scope: scope.clone(),
                    force_permission_style,
                };
                ag.install(&ctx)?;
                // A --local install is project-scoped; it must not touch the
                // global installed-agents registry (which `reinstall` replays
                // as global installs) or persist global user config.
                if local {
                    installed_names.push(name);
                } else {
                    if !user_cfg.installed_agents.contains(&id) {
                        user_cfg.installed_agents.push(id);
                        installed_names.push(name);
                    }
                    user_cfg.save();
                }
            } else {
                let (to_install, to_uninstall) = tokensave::agents::pick_integrations_interactive(
                    &home,
                    &user_cfg.installed_agents,
                )?;

                for id in &to_uninstall {
                    let ag = tokensave::agents::get_integration(id)?;
                    if local && !ag.supports_local() {
                        eprintln!(
                            "  skipping {} (no project-scoped config; --local unsupported)",
                            ag.name()
                        );
                        continue;
                    }
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: tokensave_bin.clone(),
                        tool_permissions: tokensave::agents::expected_tool_perms(),
                        scope: scope.clone(),
                        force_permission_style: false,
                    };
                    ag.uninstall(&ctx)?;
                    removed_names.push(ag.name().to_string());
                    if !local {
                        user_cfg.installed_agents.retain(|a| a != id);
                    }
                }
                for id in &to_install {
                    let ag = tokensave::agents::get_integration(id)?;
                    if local && !ag.supports_local() {
                        eprintln!(
                            "  skipping {} (no project-scoped config; --local unsupported)",
                            ag.name()
                        );
                        continue;
                    }
                    let ctx = tokensave::agents::InstallContext {
                        home: home.clone(),
                        tokensave_bin: tokensave_bin.clone(),
                        tool_permissions: tokensave::agents::install_tool_perms(want_wildcard),
                        scope: scope.clone(),
                        force_permission_style,
                    };
                    ag.install(&ctx)?;
                    installed_names.push(ag.name().to_string());
                    if !local && !user_cfg.installed_agents.contains(id) {
                        user_cfg.installed_agents.push(id.clone());
                    }
                }
                if !local {
                    user_cfg.save();
                }
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

            // Skip global user-config writes for a project-scoped install.
            if !local {
                user_cfg.last_installed_version = env!("CARGO_PKG_VERSION").to_string();
                user_cfg.save();
                if let Some(warning) = tokensave::agents::cargo_build_binary_warning(&tokensave_bin)
                {
                    eprintln!("{warning}");
                }
            }

            // Best-effort during `install`: a hook that could not be written
            // must not fail the whole install. The reason was already printed.
            let _ = tokensave::agents::offer_git_post_commit_hook(&tokensave_bin, git_hook);
        }
        Commands::Reinstall {
            wildcard_permissions,
            explicit_permissions,
        } => {
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

            // See the matching comment on Commands::Install: a flag overrides
            // and persists the default for future silent reinstalls.
            if wildcard_permissions {
                user_cfg.wildcard_permissions = true;
                user_cfg.save();
            } else if explicit_permissions {
                user_cfg.wildcard_permissions = false;
                user_cfg.save();
            }
            let want_wildcard = user_cfg.wildcard_permissions;
            // As in Commands::Install: only an explicit flag on this
            // invocation forces the style; a flagless `reinstall` (including
            // the automatic silent one on upgrade) must preserve an existing
            // covering grant.
            let force_permission_style = wildcard_permissions || explicit_permissions;

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
                        tool_permissions: tokensave::agents::install_tool_perms(want_wildcard),
                        scope: tokensave::agents::InstallScope::Global,
                        force_permission_style,
                    };
                    ag.install(&ctx)?;
                }
                eprintln!("\x1b[32m✔\x1b[0m All agents reinstalled");
                user_cfg.last_installed_version = env!("CARGO_PKG_VERSION").to_string();
                user_cfg.save();
                if let Some(warning) = tokensave::agents::cargo_build_binary_warning(&tokensave_bin)
                {
                    eprintln!("{warning}");
                }
            }
        }
        Commands::Uninstall {
            agent,
            local,
            keep_git_hooks,
        } => {
            let home = tokensave::agents::home_dir().ok_or_else(|| {
                tokensave::errors::TokenSaveError::Config {
                    message: "could not determine home directory".to_string(),
                }
            })?;
            let scope = resolve_install_scope(local)?;
            let mut user_cfg = tokensave::user_config::UserConfig::load();
            tokensave::agents::migrate_installed_agents(&home, &mut user_cfg);

            if let Some(id) = agent {
                let ag = tokensave::agents::get_integration(&id)?;
                if local && !ag.supports_local() {
                    return Err(tokensave::errors::TokenSaveError::Config {
                        message: format!(
                            "--local is not supported for \"{}\" — it has no project-scoped config. \
                             Run a global uninstall instead (omit --local).",
                            ag.id()
                        ),
                    });
                }
                let ctx = tokensave::agents::InstallContext {
                    home,
                    tokensave_bin: String::new(),
                    tool_permissions: tokensave::agents::expected_tool_perms(),
                    scope: scope.clone(),
                    force_permission_style: false,
                };
                ag.uninstall(&ctx)?;
                // A --local uninstall only removes project config; leave the
                // global installed-agents registry untouched.
                if !local {
                    user_cfg.installed_agents.retain(|a| a != &id);
                    user_cfg.save();
                }
            } else {
                for id in user_cfg.installed_agents.clone() {
                    if let Ok(ag) = tokensave::agents::get_integration(&id) {
                        if local && !ag.supports_local() {
                            eprintln!(
                                "  skipping {} (no project-scoped config; --local unsupported)",
                                ag.name()
                            );
                            continue;
                        }
                        let ctx = tokensave::agents::InstallContext {
                            home: home.clone(),
                            tokensave_bin: String::new(),
                            tool_permissions: tokensave::agents::expected_tool_perms(),
                            scope: scope.clone(),
                            force_permission_style: false,
                        };
                        ag.uninstall(&ctx).ok();
                    }
                }
                if local {
                    eprintln!("Project-local agent integrations removed.");
                } else {
                    user_cfg.installed_agents.clear();
                    user_cfg.save();
                    eprintln!("All agent integrations removed.");
                    // #420: the global post-commit hook outlives every agent
                    // integration, so without this a commit in any repo
                    // recreates the index the user just deleted. --local never
                    // touches it: the hooks are global, not project-scoped.
                    if keep_git_hooks {
                        eprintln!(
                            "  Global git hooks left in place (--keep-git-hooks). \
                             Remove them later with `tokensave githooks off`."
                        );
                    } else {
                        report_hook_removal(&tokensave::agents::remove_git_hooks());
                    }
                }
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
        Commands::HookKiroPreToolUse => {
            let code = tokensave::hooks::hook_kiro_pre_tool_use();
            if code != 0 {
                process::exit(code);
            }
        }
        Commands::HookKiroPromptSubmit => {
            let code = tokensave::hooks::hook_kiro_prompt_submit().await;
            if code != 0 {
                process::exit(code);
            }
        }
        Commands::HookKiroPostToolUse => {
            let code = tokensave::hooks::hook_kiro_post_tool_use().await;
            if code != 0 {
                process::exit(code);
            }
        }
        Commands::HookDroidPreToolUse => {
            let code = tokensave::hooks::hook_droid_pre_tool_use();
            if code != 0 {
                process::exit(code);
            }
        }
        Commands::Serve {
            path,
            timings,
            idle_timeout_secs,
        } => {
            let canonical_disable = std::env::var("TOKENSAVE_DISABLE_SERVER").ok();
            let legacy_disable = std::env::var("DISABLE_TOKENSAVE").ok();
            if server_disabled_from_env(canonical_disable.as_deref(), legacy_disable.as_deref()) {
                // Allow users to opt out per-project by setting
                // TOKENSAVE_DISABLE_SERVER=true in their MCP server config.
                // DISABLE_TOKENSAVE=true remains compatible with issue #19.
                // The process exits cleanly so the host does not retry.
                return Ok(());
            }
            let original_cwd = std::env::current_dir().ok();
            // An explicit --path is a deliberate choice of project root
            // (possibly a different repo than the CWD, #201); only
            // CWD-discovered roots get the borrowed-worktree check.
            let explicit_path = path.is_some();
            // What the host actually launched, kept separate from what we
            // resolved: a process lister shows only the former, and the
            // registry has to be readable against both (#421).
            let path_for_registry = path.clone();
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
                                    });
                                }
                            }
                        }
                    }
                }
            };

            // Set the shutdown flag the instant a signal arrives, rather than
            // only when the run loop next looks. The loop's own SIGTERM stream
            // is polled only while it waits for a request, so a signal
            // delivered during a long sync would otherwise not be observed
            // until that sync finished (#450).
            tokensave::cancel::install_signal_handlers();
            watch_for_orphaning();
            // An index whose *scope* is wrong is still a valid index, so this
            // warns and continues rather than refusing: retroactively applying
            // #396's cap would decide which of the user's existing setups stop
            // working. `suppress_scope_warning` opts out (#450).
            tokensave::index_scope::warn_on_serve(cg.project_root());

            // Memory instrumentation for #253: mark this process as a
            // long-lived MCP server and take a baseline RSS sample.
            tokensave::memstats::init("serve", cg.project_root());
            tokensave::memstats::record("start");

            // Record this process in the server registry, now that the index
            // is open and the lock is genuinely held (#421). Nothing else on
            // the machine records which server holds which index: most
            // instances carry no project in argv, because the host supplies it
            // through the global DB or MCP `initialize` roots rather than
            // `--path`.
            tokensave::servers::register(
                cg.project_root(),
                &cg.db_path(),
                path_for_registry.as_deref(),
            );

            // Compute scope prefix: relative path from project root to original cwd
            let scope_prefix = original_cwd.and_then(|cwd| {
                cwd.strip_prefix(cg.project_root())
                    .ok()
                    .filter(|rel| !rel.as_os_str().is_empty())
                    .map(|rel| rel.to_string_lossy().into_owned())
            });

            let server = if explicit_path {
                tokensave::mcp::McpServer::new_explicit_root(cg, scope_prefix).await
            } else {
                tokensave::mcp::McpServer::new(cg, scope_prefix).await
            };
            server.set_timings_enabled(timings);
            let mut transport = tokensave::mcp::StdioTransport::new();
            // If we peeked at stdin to read `initialize` roots, replay that line.
            if let Some(line) = peeked_line {
                server.handle_and_write(&line, &mut transport).await;
            }
            server
                .run_with_idle_timeout(
                    &mut transport,
                    idle_timeout_secs.map(std::time::Duration::from_secs),
                )
                .await?;
            server.shutdown().await;
            // A hard kill skips this; that is what reaping on startup and on
            // read is for.
            tokensave::servers::unregister();
            // Exit explicitly rather than unwinding out of `main` (#450/#436).
            //
            // `tokio::io::stdin()` performs its reads on a blocking thread,
            // and a blocking task cannot be cancelled — so the outstanding
            // read is still parked when the run loop leaves. Dropping the
            // runtime waits for it, and under a supervisor that holds our
            // stdin open it never completes: the server ran its whole
            // graceful shutdown, printed its summary, and then sat there
            // alive and unkillable by anything short of `SIGKILL`. That is
            // the reported "kill did nothing" and the servers that "never
            // exit" under a live parent.
            //
            // Shutdown has already persisted counters and checkpointed the
            // WAL, and is idempotent, so there is nothing left to unwind for.
            // Flush stdout first: a response written just before a signal
            // must still reach the client.
            {
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            std::process::exit(0);
        }
        Commands::Servers { json } => {
            let entries = tokensave::servers::list();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
                );
            } else {
                print!("{}", tokensave::servers::render(&entries));
            }
        }
        Commands::Upgrade { kill } => {
            tokensave::upgrade::run_upgrade(kill)?;
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
        Commands::Githooks {
            action,
            local,
            path,
        } => {
            let repo = tokensave::config::resolve_path(path);
            match (action.as_deref(), local) {
                (Some("off"), true) => {
                    report_hook_removal(&tokensave::agents::remove_local_git_hooks(&repo));
                }
                (Some("off"), false) => {
                    report_hook_removal(&tokensave::agents::remove_git_hooks());
                }
                (Some("on"), true) => {
                    let bin = current_bin_path();
                    match tokensave::agents::install_local_git_hooks(&repo, &bin) {
                        Ok(outcome) => {
                            report_local_hook_install(&outcome);
                            // The specific reason was already printed by
                            // write_global_hook; this only stops an explicit
                            // `githooks on --local` from reporting a partial
                            // install as success.
                            if !outcome.failed.is_empty() {
                                return Err(tokensave::errors::TokenSaveError::Config {
                                    message: format!(
                                        "could not install git hooks: {}",
                                        outcome.failed.join(", ")
                                    ),
                                });
                            }
                        }
                        Err(message) => {
                            return Err(tokensave::errors::TokenSaveError::Config { message })
                        }
                    }
                }
                (Some("on"), false) => {
                    // The specific reason was already printed; this only stops
                    // `githooks on` from reporting a failed install as success.
                    if let Err(message) = tokensave::agents::offer_git_post_commit_hook(
                        &current_bin_path(),
                        tokensave::agents::GitHookMode::Yes,
                    ) {
                        return Err(tokensave::errors::TokenSaveError::Config { message });
                    }
                }
                (Some(other), _) => {
                    return Err(tokensave::errors::TokenSaveError::Config {
                        message: format!("unknown action '{other}': expected 'on' or 'off'"),
                    });
                }
                (None, true) => {
                    for line in tokensave::agents::describe_local_git_hooks(&repo) {
                        eprintln!("{line}");
                    }
                }
                (None, false) => {
                    for line in tokensave::agents::describe_git_hooks() {
                        eprintln!("{line}");
                    }
                }
            }
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
        Commands::Cost {
            range,
            by_agent,
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
            let changed = ingest_stats.turns_inserted + ingest_stats.turns_updated;
            if changed > 0 {
                eprintln!("Ingested or refreshed {} local accounting rows.", changed);
            }

            let since = tokensave::accounting::metrics::parse_range(&range);
            let droid_present = ingest_stats.coverage.iter().any(|coverage| {
                coverage.agent == "droid"
                    && coverage.state != tokensave::accounting::CoverageState::Absent
            });
            let summary = tokensave::accounting::metrics::cost_summary_with_droid_presence(
                &gdb,
                since,
                droid_present,
            )
            .await;

            let Some(s) = summary else {
                println!("No supported local session data found.");
                return Ok(());
            };
            let coverage =
                tokensave::accounting::format_coverage(&ingest_stats.coverage, &s.by_agent);

            if let Some(ref fmt) = export {
                match fmt.as_str() {
                    "json" => {
                        let mut obj = serde_json::json!({
                            "range": range,
                            "total_cost_usd": s.total_cost,
                            "total_input_tokens": s.total_input_tokens,
                            "total_output_tokens": s.total_output_tokens,
                            // Cache reads and writes are priced, and they dominate
                            // agent traffic. Without them the exported tokens
                            // cannot account for the exported cost (#472).
                            "total_cache_read_tokens": s.total_cache_read_tokens,
                            "total_cache_creation_tokens": s.total_cache_write_tokens,
                            "total_tokens": s.total_input_tokens
                                + s.total_output_tokens
                                + s.total_cache_read_tokens
                                + s.total_cache_write_tokens,
                            "tokens_saved": s.tokens_saved,
                            "efficiency_ratio": s.efficiency_ratio,
                            "by_model": s.by_model.iter().map(|(m, c, t)| serde_json::json!({"model": m, "cost": c, "tokens": t})).collect::<Vec<_>>(),
                            "by_category": s.by_category.iter().map(|(cat, c, n)| serde_json::json!({"category": cat, "cost": c, "turns": n})).collect::<Vec<_>>(),
                        });
                        if droid_present {
                            obj["by_agent"] = serde_json::json!(s
                                .by_agent
                                .iter()
                                .map(agent_row_json)
                                .collect::<Vec<_>>());
                            obj["coverage"] = serde_json::json!(&ingest_stats.coverage);
                        }
                        println!("{}", serde_json::to_string_pretty(&obj).unwrap_or_default());
                    }
                    "csv" => {
                        if by_agent {
                            println!("agent,cost_usd,credits,input_tokens,output_tokens,cache_write_tokens,cache_read_tokens,turns");
                            for a in &s.by_agent {
                                let cost_cell = if a.agent == "droid" {
                                    String::new()
                                } else {
                                    format!("{:.4}", a.cost_usd)
                                };
                                let credits_cell = match a.credits {
                                    Some(c) => c.to_string(),
                                    None => String::new(),
                                };
                                println!(
                                    "{},{},{},{},{},{},{},{}",
                                    a.agent,
                                    cost_cell,
                                    credits_cell,
                                    a.input_tokens,
                                    a.output_tokens,
                                    a.cache_write_tokens,
                                    a.cache_read_tokens,
                                    a.turns,
                                );
                            }
                        } else if by_model {
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
            } else if by_agent {
                println!(
                    "  {:<16} {:>9} {:>9} {:>12} {:>6}",
                    "Agent", "USD", "Credits", "Raw tokens", "Rows"
                );
                for a in &s.by_agent {
                    println!("{}", format_agent_row(a));
                }
                if !coverage.is_empty() {
                    println!("{coverage}");
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
                    .unwrap_or((0, 0, 0, 0));

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
                if !coverage.is_empty() {
                    println!();
                    println!("{coverage}");
                }
            }
        }
        Commands::Discover { since, json } => {
            commands::handle_discover(&since, json).await?;
        }
        Commands::Bench {
            queries,
            json,
            path,
            max_nodes,
        } => {
            let project_path = tokensave::config::resolve_path(path);
            let cg = serve::ensure_initialized(&project_path).await?;

            let opts = tokensave::bench::BenchOptions {
                format: if json {
                    tokensave::bench::OutputFormat::Json
                } else {
                    tokensave::bench::OutputFormat::Markdown
                },
                max_nodes,
            };

            let report = match queries {
                Some(p) => tokensave::bench::run_bench(&cg, std::path::Path::new(&p), opts).await?,
                None => {
                    tokensave::bench::run_bench_with_toml(
                        &cg,
                        tokensave::bench::DEFAULT_QUERIES_TOML,
                        opts,
                    )
                    .await?
                }
            };

            if json {
                println!("{}", tokensave::bench::format_report_json(&report));
            } else {
                print!("{}", tokensave::bench::format_report_console(&report));
            }
        }
        Commands::Gain {
            all,
            history,
            range,
            json,
        } => {
            commands::handle_gain(all, history, &range, json).await?;
        }
        Commands::Monitor => {
            if let Err(e) = tokensave::monitor::run() {
                eprintln!("Monitor error: {e}");
                process::exit(1);
            }
        }
        Commands::Memory { clean } => {
            // Diagnostic tool: always exits 0, even when the report or
            // the --clean purge fails — it must never break scripts
            // that poll it while investigating an incident (#253).
            if let Err(e) = tokensave::memstats::run(clean) {
                eprintln!("Memory report error: {e}");
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

fn resolve_install_scope(
    local: bool,
) -> tokensave::errors::Result<tokensave::agents::InstallScope> {
    if local {
        let project_path =
            std::env::current_dir().map_err(|e| tokensave::errors::TokenSaveError::Config {
                message: format!("could not determine current directory: {e}"),
            })?;
        Ok(tokensave::agents::InstallScope::Local { project_path })
    } else {
        Ok(tokensave::agents::InstallScope::Global)
    }
}

fn should_skip_agent_install_maintenance(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Install { .. }
            | Commands::Reinstall { .. }
            | Commands::Uninstall { .. }
            | Commands::Doctor { .. }
            // `Serve` is the hot path used by MCP clients (Claude Code,
            // Codex, etc.). Clients impose a 30 s `initialize` timeout, so
            // every pre-serve startup task — `try_flush` network round-trip,
            // `check_install_stale`, the silent-reinstall loop over every
            // tracked agent — risks pushing us past it on slow networks or
            // big home-dir trees (#84). Skip them; the same maintenance
            // runs on the user's next interactive `tokensave …` invocation.
            | Commands::Serve { .. }
            // `Servers` exists for wrappers to poll while rendering a list UI
            // (#421), so it is a hot path for the same reason `Serve` is: a
            // network flush and a scan over every tracked agent do not belong
            // behind a directory listing that a UI refreshes on a timer.
            | Commands::Servers { .. }
            // Hook handlers are on the per-tool-call hot path (Cursor fires
            // `preToolUse` before every Grep/Shell; Factory Droid fires
            // `hook-droid-pre-tool-use` before every Execute/Grep). They must
            // not run the HTTP flush, install-stale scans, or the
            // silent-reinstall loop, and stdout must stay JSON-only for the
            // permission gate (see hooks.rs `hook_pre_tool_use`).
            | Commands::HookPreToolUse
            | Commands::HookPromptSubmit
            | Commands::HookStop
            | Commands::HookKiroPreToolUse
            | Commands::HookKiroPromptSubmit
            | Commands::HookKiroPostToolUse
            | Commands::HookDroidPreToolUse
    )
}

/// Print what `remove_git_hooks` did. Says so explicitly when it found
/// nothing, so `githooks off` never exits silently on a machine that has no
/// tokensave hooks installed.
fn current_bin_path() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "tokensave".to_string())
}

fn report_local_hook_install(outcome: &tokensave::agents::LocalHookInstall) {
    for name in &outcome.installed {
        eprintln!(
            "\x1b[32m✔\x1b[0m Installed git {name} hook at {}",
            outcome.hooks_dir.join(name).display()
        );
    }
    for name in &outcome.already_present {
        eprintln!("  git {name} hook already contains tokensave, skipping");
    }
    // A hook that will never run is worse than no hook, because nothing else
    // says so. `core.hooksPath` makes git resolve every hook from one
    // directory with no fallback to the repository's own.
    if let Some(path) = &outcome.shadowed_by {
        eprintln!(
            "  \x1b[33m⚠\x1b[0m core.hooksPath is set to {} — git reads hooks from there, so \
             the hooks just written will not run. Unset it (`git config --unset core.hooksPath`), \
             or use the global hooks instead (`tokensave githooks on`).",
            path.display()
        );
    }
}

/// Offer this repository's own git hooks after `init` (#455).
///
/// Local rather than global on purpose: the global path claims
/// `core.hooksPath`, a single machine-wide setting that forces one hook
/// directory on every repository, which is exactly what someone juggling
/// projects with different tooling does not want.
///
/// Prompt defaults to *yes* — the hooks are per-repository, live in `.git/`
/// so they are never committed, and are removable with one command — but a
/// non-TTY skips silently, so scripted and CI installs are unchanged.
fn offer_local_git_hooks(project_path: &std::path::Path, forced: bool, refused: bool) {
    if refused {
        return;
    }
    if tokensave::agents::repo_hooks_dir(project_path).is_none() {
        return;
    }
    // Global hooks already cover this repository; installing local ones too
    // would run a sync twice per commit.
    if !forced && tokensave::agents::global_git_hooks_installed() {
        return;
    }
    if !forced && tokensave::agents::local_git_hooks_present(project_path) {
        return;
    }
    if !forced {
        if !std::io::stdin().is_terminal() {
            return;
        }
        eprintln!();
        eprint!(
            "Install this repository's git \x1b[1mpost-commit\x1b[0m + \x1b[1mpost-checkout\x1b[0m + \x1b[1mpost-merge\x1b[0m hooks to keep the index fresh? [Y/n] "
        );
        io::stderr().flush().ok();
        let mut answer = String::new();
        if io::stdin().lock().read_line(&mut answer).is_err() {
            return;
        }
        let answer = answer.trim();
        if !(answer.is_empty()
            || answer.eq_ignore_ascii_case("y")
            || answer.eq_ignore_ascii_case("yes"))
        {
            eprintln!("  Skipped git hooks — install later with `tokensave githooks on --local`");
            return;
        }
    }
    match tokensave::agents::install_local_git_hooks(project_path, &current_bin_path()) {
        Ok(outcome) => report_local_hook_install(&outcome),
        Err(message) => eprintln!("  \x1b[31m✘\x1b[0m {message}"),
    }
}

fn report_hook_removal(r: &tokensave::agents::HookRemoval) {
    if r.found_nothing() {
        eprintln!(
            "  No tokensave git hooks found in {}",
            r.hooks_dir.display()
        );
        return;
    }
    for p in &r.deleted {
        eprintln!("\x1b[32m✔\x1b[0m Removed git hook {}", p.display());
    }
    for p in &r.cleaned {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave section from {} (your own hook content kept)",
            p.display()
        );
    }
    if r.hooks_path_unset {
        eprintln!("\x1b[32m✔\x1b[0m Unset git core.hooksPath");
    } else if r.dir_kept_for_foreign_files {
        eprintln!(
            "  Left {} and core.hooksPath in place — the directory still holds hooks tokensave did not write",
            r.hooks_dir.display()
        );
    }
}

/// The one line the silent upgrade resync prints when it has written agent
/// config (#419).
///
/// The user did not ask for an install — they may have run a read-only query
/// like `tokensave gitignore` — so a write to their agent config has to name
/// itself and say why. Before this, the only output was a bare `✔ Wrote
/// <path>` escaping from the file layer, which read as though the query had
/// done it.
fn resync_summary(running: &str, previous: &str, agent_count: usize) -> String {
    let from = if previous.is_empty() {
        "version not recorded"
    } else {
        previous
    };
    let agents = if agent_count == 1 {
        "1 agent".to_string()
    } else {
        format!("{agent_count} agents")
    };
    format!("Refreshed agent config for tokensave {running} (was {from}) — {agents}")
}

fn server_disabled_from_env(canonical: Option<&str>, legacy: Option<&str>) -> bool {
    match canonical {
        Some("true") => true,
        Some("false") => false,
        Some(_) | None => legacy == Some("true"),
    }
}

#[cfg(test)]
mod startup_tests {
    use super::{server_disabled_from_env, should_skip_agent_install_maintenance, Commands};

    /// #419: the resync used to announce itself only as `✔ Wrote <path>` from
    /// the file layer, under a command the user had run as a query.
    #[test]
    fn resync_summary_names_the_versions_and_the_scale() {
        let s = super::resync_summary("7.10.0", "7.9.0", 3);
        assert_eq!(
            s,
            "Refreshed agent config for tokensave 7.10.0 (was 7.9.0) — 3 agents"
        );
        // A path, which is what the old message consisted of, must not be the
        // whole story any more.
        assert!(!s.contains(".claude"));
    }

    #[test]
    fn resync_summary_handles_one_agent_and_an_unrecorded_previous_version() {
        assert_eq!(
            super::resync_summary("7.10.0", "7.9.0", 1),
            "Refreshed agent config for tokensave 7.10.0 (was 7.9.0) — 1 agent"
        );
        // `last_installed_version` and `previous_version` are both empty on an
        // install that predates the markers, and the line still has to read.
        assert_eq!(
            super::resync_summary("7.10.0", "", 2),
            "Refreshed agent config for tokensave 7.10.0 (was version not recorded) — 2 agents"
        );
    }

    #[test]
    fn canonical_server_disable_env_controls_serve() {
        assert!(server_disabled_from_env(Some("true"), None));
        assert!(!server_disabled_from_env(Some("false"), None));
        assert!(!server_disabled_from_env(None, None));
    }

    #[test]
    fn legacy_server_disable_env_remains_compatible() {
        assert!(server_disabled_from_env(None, Some("true")));
        assert!(!server_disabled_from_env(None, Some("false")));
        assert!(!server_disabled_from_env(None, Some("1")));
    }

    #[test]
    fn canonical_server_disable_env_has_precedence() {
        assert!(server_disabled_from_env(Some("true"), Some("false")));
        assert!(!server_disabled_from_env(Some("false"), Some("true")));
        assert!(server_disabled_from_env(Some("TRUE"), Some("true")));
        assert!(server_disabled_from_env(Some("1"), Some("true")));
        assert!(server_disabled_from_env(Some(""), Some("true")));
    }

    #[test]
    fn doctor_skips_agent_install_maintenance() {
        let command = Commands::Doctor {
            agent: Some("kiro".to_string()),
        };
        assert!(should_skip_agent_install_maintenance(&command));
    }

    #[test]
    fn explicit_agent_config_commands_skip_agent_install_maintenance() {
        assert!(should_skip_agent_install_maintenance(&Commands::Install {
            agent: Some("kiro".to_string()),
            git_hook: tokensave::agents::GitHookMode::Default,
            local: false,
            wildcard_permissions: false,
            explicit_permissions: false,
        }));
        assert!(should_skip_agent_install_maintenance(
            &Commands::Reinstall {
                wildcard_permissions: false,
                explicit_permissions: false,
            }
        ));
        assert!(should_skip_agent_install_maintenance(
            &Commands::Uninstall {
                agent: Some("kiro".to_string()),
                local: false,
                keep_git_hooks: false,
            }
        ));
    }

    #[test]
    fn normal_commands_keep_agent_install_maintenance() {
        assert!(!should_skip_agent_install_maintenance(&Commands::Status {
            path: None,
            json: false,
            short: false,
            details: false,
            runtime: false,
        }));
    }

    #[test]
    fn serve_skips_agent_install_maintenance() {
        // `tokensave serve` is the MCP hot path with a 30 s client-side
        // `initialize` timeout (#84). Pre-serve maintenance work
        // (worldwide-counter flush, install-stale check, silent reinstall)
        // must NOT run on this path.
        assert!(should_skip_agent_install_maintenance(&Commands::Serve {
            path: None,
            timings: false,
            idle_timeout_secs: None,
        }));
    }

    #[test]
    fn per_tool_call_hooks_skip_agent_install_maintenance() {
        // Every per-tool-call hook must take the fast, side-effect-free path.
        // The Droid hook was previously missing from the skip list, so unlike
        // the Claude and Kiro hooks it ran the pre-dispatch maintenance
        // (network flush, install-stale check, and the silent-reinstall loop
        // that rewrites every tracked agent's config) on each invocation.
        for command in [
            Commands::HookPreToolUse,
            Commands::HookPromptSubmit,
            Commands::HookStop,
            Commands::HookKiroPreToolUse,
            Commands::HookKiroPromptSubmit,
            Commands::HookKiroPostToolUse,
            Commands::HookDroidPreToolUse,
        ] {
            assert!(
                should_skip_agent_install_maintenance(&command),
                "per-tool-call hook must skip agent install maintenance"
            );
        }
    }
}

// handle_branch_action, handle_wipe, handle_list, handle_no_command,
// init_and_index, and print_sync_doctor have been moved to src/commands.rs.
//
// update_global_db, try_flush, check_for_update, gather_target_projects,
// gather_local_projects, gather_local_projects_from, find_descendant_tokensave,
// print_flash_warning, and tokensave_dir_size have been moved to src/global.rs.
// direct test 1774739850

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod cost_tests {
    use super::format_agent_row;
    use tokensave::global_db::AgentCostSummary;

    fn droid_summary(credits: Option<u64>) -> AgentCostSummary {
        AgentCostSummary {
            agent: "droid".to_string(),
            cost_usd: 0.0,
            input_tokens: 1000,
            output_tokens: 500,
            cache_write_tokens: 200,
            cache_read_tokens: 300,
            credits,
            turns: 5,
        }
    }

    #[test]
    fn droid_row_has_na_usd_and_not_zero_dollar() {
        // credits = 2900 → "2.9k"; raw = 1000+500+200+300 = 2000 → "2.0k"
        let a = AgentCostSummary {
            agent: "droid".to_string(),
            cost_usd: 0.0,
            input_tokens: 1000,
            output_tokens: 500,
            cache_write_tokens: 200,
            cache_read_tokens: 300,
            credits: Some(2900),
            turns: 5,
        };
        let row = format_agent_row(&a);
        assert!(row.contains("2.9k"), "credits 2.9k expected in: {row}");
        assert!(row.contains("n/a"), "n/a for USD expected in: {row}");
        assert!(!row.contains("$0.00"), "$0.00 must not appear in: {row}");
    }

    #[test]
    fn droid_row_none_credits_shows_na() {
        let a = droid_summary(None);
        let row = format_agent_row(&a);
        // Both USD and credits should be "n/a"
        assert_eq!(row.matches("n/a").count(), 2, "two n/a expected in: {row}");
        assert!(!row.contains("$0.00"), "$0.00 must not appear in: {row}");
    }

    #[test]
    fn claude_row_formats_usd() {
        let a = AgentCostSummary {
            agent: "claude".to_string(),
            cost_usd: 1.23,
            input_tokens: 100,
            output_tokens: 20,
            cache_write_tokens: 0,
            cache_read_tokens: 0,
            credits: None,
            turns: 3,
        };
        let row = format_agent_row(&a);
        assert!(row.contains("$1.23"), "$1.23 expected in: {row}");
        assert!(row.contains("n/a"), "n/a for credits expected in: {row}");
    }

    #[test]
    fn droid_json_row_cost_usd_is_null() {
        let a = droid_summary(Some(1000));
        let json = super::agent_row_json(&a);
        assert!(
            json["cost_usd"].is_null(),
            "cost_usd must be null for droid: {json}"
        );
        assert_eq!(json["credits"].as_u64(), Some(1000));
        assert_eq!(json["input_tokens"].as_u64(), Some(1000));
    }

    #[test]
    fn claude_json_row_cost_usd_is_numeric() {
        let a = AgentCostSummary {
            agent: "claude".to_string(),
            cost_usd: 2.50,
            input_tokens: 500,
            output_tokens: 100,
            cache_write_tokens: 0,
            cache_read_tokens: 0,
            credits: None,
            turns: 7,
        };
        let json = super::agent_row_json(&a);
        assert!(
            json["cost_usd"].is_number(),
            "cost_usd must be numeric for claude: {json}"
        );
        assert!(
            (json["cost_usd"].as_f64().unwrap() - 2.50).abs() < 1e-9,
            "cost_usd value mismatch: {json}"
        );
        assert!(
            json["credits"].is_null(),
            "claude credits must be null: {json}"
        );
    }
}

/// Exit when the process that launched us is gone (#450, defect 3 of the #396
/// triage).
///
/// A `serve` whose host died keeps its index mapped and keeps answering
/// nothing, because stdin never reaches EOF once the parent's end of the pipe
/// is inherited elsewhere — the reported servers survived their supervisor and
/// had to be found by hand. A reparented process is unambiguous on Unix: its
/// parent becomes PID 1 (or whatever subreaper adopts it), never the PID it
/// started with.
///
/// This is deliberately *not* a fix for #436, where every surplus server has a
/// live parent and there is no dead-parent signal to key on.
#[cfg(unix)]
fn watch_for_orphaning() {
    /// Slow on purpose. An orphan wastes memory until it is noticed, which is
    /// a minute-scale problem, not a second-scale one, and this polls for the
    /// entire life of every server.
    const POLL: std::time::Duration = std::time::Duration::from_secs(30);

    let original = std::os::unix::process::parent_id();
    // A server already started by PID 1 (a launchd/systemd unit) has no parent
    // to lose, and would otherwise exit on its first tick.
    if original <= 1 {
        return;
    }
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL).await;
            if std::os::unix::process::parent_id() != original {
                eprintln!("[tokensave] parent process {original} is gone; shutting down");
                tokensave::cancel::request();
                return;
            }
        }
    });
}

/// No reparenting signal to watch for off Unix.
#[cfg(not(unix))]
fn watch_for_orphaning() {}
