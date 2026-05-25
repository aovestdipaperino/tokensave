use clap::{builder::PossibleValuesParser, Parser, Subcommand};

fn agent_value_parser() -> PossibleValuesParser {
    PossibleValuesParser::new(tokensave::agents::available_integrations())
}

/// Code intelligence for Rust codebases.
#[derive(Parser)]
#[command(
    name = "tokensave",
    about = "Code intelligence for 34 languages — semantic graph queries instead of file reads",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
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
        /// Capture a runtime telemetry snapshot (PID, RSS, CPU%, DB / WAL
        /// sizes) — useful when reporting unexpected resource use (#80).
        #[arg(long)]
        runtime: bool,
    },
    /// Invoke an MCP tool from the CLI (e.g. `tokensave tool search foo`).
    ///
    /// Run `tokensave tool` (no name) to list every available tool.
    /// Run `tokensave tool <name> --help` to see that tool's parameters.
    //
    // `disable_help_flag = true` lets `-h`/`--help` flow through to our parser
    // so we can print the per-tool schema instead of clap's generic help.
    #[command(disable_help_flag = true)]
    Tool {
        /// MCP tool name (with or without the `tokensave_` prefix). Omit to list all tools.
        name: Option<String>,
        /// Tool arguments as alternating `--key value` flags, plus reserved flags
        /// `--json`, `--project <path>`, `--args <json>`, and `-h`/`--help`.
        /// Any value starting with `@` is read from that file (handy for
        /// multi-line replacement bodies).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Configure agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "install", visible_alias = "claude-install")]
    Install {
        /// Agent to configure (auto-detects if omitted)
        #[arg(long, value_parser = agent_value_parser())]
        agent: Option<String>,
    },
    /// Refresh settings for all already-installed agents
    Reinstall,
    /// Remove agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "uninstall", visible_alias = "claude-uninstall")]
    Uninstall {
        /// Agent to remove (removes all if omitted)
        #[arg(long, value_parser = agent_value_parser())]
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
    /// Kiro PreToolUse hook handler (called by Kiro, not by users directly)
    #[command(name = "hook-kiro-pre-tool-use", hide = true)]
    HookKiroPreToolUse,
    /// Kiro UserPromptSubmit hook handler (called by Kiro, not by users directly)
    #[command(name = "hook-kiro-prompt-submit", hide = true)]
    HookKiroPromptSubmit,
    /// Kiro PostToolUse hook handler for incremental sync
    #[command(name = "hook-kiro-post-tool-use", hide = true)]
    HookKiroPostToolUse,
    /// Start MCP server over stdio
    Serve {
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
        /// Annotate every `tools/call` response with `_meta.duration_us`,
        /// reporting the handler's pure execution time in microseconds.
        /// Useful for profiling index work vs. JSON-RPC / stdio overhead.
        #[arg(long)]
        timings: bool,
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
        #[arg(long, value_parser = agent_value_parser())]
        agent: Option<String>,
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
    /// Run a reproducible retrieval benchmark against the current project.
    Bench {
        /// Path to a TOML query file (defaults to the shipped default set).
        #[arg(long)]
        queries: Option<String>,
        /// Output as JSON instead of the colored console table.
        #[arg(long)]
        json: bool,
        /// Project path (default: current directory).
        #[arg(short, long)]
        path: Option<String>,
        /// Max nodes per query (default: 20).
        #[arg(long, default_value = "20")]
        max_nodes: usize,
    },
    /// Show token savings (and dollar estimates) recorded in the global ledger.
    Gain {
        /// Show all projects (default: only the current project).
        #[arg(short, long)]
        all: bool,
        /// Print per-day history instead of a single total.
        #[arg(long)]
        history: bool,
        /// Time range: "today", "7d", "30d", "month", or "all" (default: "30d").
        #[arg(long, default_value = "30d")]
        range: String,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
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
pub enum BranchAction {
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
