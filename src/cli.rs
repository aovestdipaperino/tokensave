use clap::{builder::PossibleValuesParser, Parser, Subcommand};

fn agent_value_parser() -> PossibleValuesParser {
    PossibleValuesParser::new(tokensave::agents::available_integrations())
}

/// Whether `tokensave install` should offer to install the global git
/// `post-commit`/`post-checkout`/`post-merge` hooks, and if so, whether to
/// ask interactively or act non-interactively. Re-exported from
/// `tokensave::agents::GitHookMode` so the enum lives in one place and both
/// the CLI parser and the install dispatch see the same definition.
pub use tokensave::agents::GitHookMode;

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
        /// Install this repository's own git hooks without asking (#455)
        #[arg(long, conflicts_with = "no_git_hook")]
        git_hook: bool,
        /// Do not offer this repository's own git hooks
        #[arg(long = "no-git-hook")]
        no_git_hook: bool,
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
        /// Whether to install global git `post-commit` + `post-merge` hooks
        /// that run `tokensave sync` after each commit and after `git pull`
        /// (plus a `post-checkout` hook for fresh clones/branch tracking).
        /// `default` preserves the interactive prompt (or silent skip on
        /// non-TTY). `yes` installs the hooks without asking; `no` skips
        /// them without asking.
        #[arg(long, value_enum, default_value_t = GitHookMode::Default)]
        git_hook: GitHookMode,
        /// Install into the current project's config instead of the user's
        /// global config. Only supported for agents with a project-scoped
        /// config (claude, cursor, droid, gemini, zed, opencode, roo-code, kiro, auggie,
        /// plank).
        #[arg(long)]
        local: bool,
        /// Grant Claude Code tokensave tools via a single compact
        /// "mcp__tokensave__*" entry instead of listing every tool
        /// individually. Persisted to ~/.tokensave/config.toml for global
        /// installs, so future silent reinstalls keep the choice.
        #[arg(long, conflicts_with = "explicit_permissions")]
        wildcard_permissions: bool,
        /// Grant Claude Code tokensave tools via the explicit per-tool list
        /// (the default). Only useful to switch back after having enabled
        /// `--wildcard-permissions`.
        #[arg(long, conflicts_with = "wildcard_permissions")]
        explicit_permissions: bool,
    },
    /// Refresh settings for all already-installed agents
    Reinstall {
        /// Grant Claude Code tokensave tools via a single compact
        /// "mcp__tokensave__*" entry instead of listing every tool
        /// individually. Persisted to ~/.tokensave/config.toml.
        #[arg(long, conflicts_with = "explicit_permissions")]
        wildcard_permissions: bool,
        /// Grant Claude Code tokensave tools via the explicit per-tool list
        /// (the default). Only useful to switch back after having enabled
        /// `--wildcard-permissions`.
        #[arg(long, conflicts_with = "wildcard_permissions")]
        explicit_permissions: bool,
    },
    /// Remove agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "uninstall", visible_alias = "claude-uninstall")]
    Uninstall {
        /// Agent to remove (removes all if omitted)
        #[arg(long, value_parser = agent_value_parser())]
        agent: Option<String>,
        /// Remove the project-scoped install from the current directory
        /// instead of the global config.
        #[arg(long)]
        local: bool,
        /// Leave tokensave's global git hooks in place. Without this, a global
        /// uninstall removes them, so a later commit cannot recreate an index
        /// (#420).
        #[arg(long)]
        keep_git_hooks: bool,
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
    /// Factory Droid PreToolUse hook handler (called by Droid, not by users directly)
    #[command(name = "hook-droid-pre-tool-use", hide = true)]
    HookDroidPreToolUse,
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
        /// Exit after this many seconds with no request (#436)
        ///
        /// Off by default, which keeps today's indefinite lifetime. Turn it on
        /// when a host leaks servers — one per subagent, none of them exiting,
        /// because the host never closes their stdin so the EOF that would
        /// stop them never arrives. Only safe if your host starts a fresh
        /// server when a tool is called after an idle exit; probe that before
        /// relying on it. The deadline is only ever checked while the server
        /// is waiting for a request, so it cannot interrupt one.
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
        idle_timeout_secs: Option<u64>,
    },
    /// List running `tokensave serve` processes and the index each one holds
    ///
    /// `serve` keeps an exclusive handle on its database for the life of the
    /// process, so an indexed directory cannot be deleted while a client has a
    /// server up — on Windows that is a hard block. This answers which process
    /// holds which index. It deliberately does not stop anything: MCP clients
    /// restart their servers, so a stop the host undoes is a trap that looks
    /// like a fix (#421).
    Servers {
        /// Emit JSON, matching the `~/.tokensave/servers/<pid>.json` entries
        #[arg(long)]
        json: bool,
    },
    /// Download and install the latest version from GitHub
    Upgrade {
        /// Kill other running tokensave processes without asking
        #[arg(long)]
        kill: bool,
    },
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
    /// Show or remove the global git hooks tokensave installs
    #[command(name = "githooks")]
    Githooks {
        /// "off" to remove tokensave's git hooks, "on" to install them,
        /// omit to show what is currently installed
        action: Option<String>,
        /// Act on this repository's own hooks instead of the global ones (#455)
        ///
        /// Global hooks work by claiming `core.hooksPath`, which is a single
        /// machine-wide setting: it forces one hook directory on every
        /// repository and makes git ignore each one's `.git/hooks`. Use
        /// `--local` when your projects need different tooling. No git config
        /// is touched, so other repositories are unaffected.
        #[arg(long)]
        local: bool,
        /// Repository to act on with `--local` (default: current directory)
        #[arg(long, value_name = "PATH")]
        path: Option<String>,
    },
    /// Check tokensave installation, configuration, and agent integration
    Doctor {
        /// Check only this agent (default: all agents)
        #[arg(long, value_parser = agent_value_parser())]
        agent: Option<String>,
    },
    /// Token cost summary from supported local agent sessions
    Cost {
        /// Time range: "today", "7d", "30d", "month", or "all"
        #[arg(default_value = "7d")]
        range: String,
        /// Group by agent (Droid, Claude, etc.)
        #[arg(long)]
        by_agent: bool,
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
    /// Find navigation turns a tokensave query could have served more cheaply
    Discover {
        /// Time range: "today", "7d", "30d", "month", or "all"
        #[arg(long, default_value = "30d")]
        since: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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
    /// Report memory usage of all tokensave instances on this machine (#253 diagnostics)
    Memory {
        /// Purge slots left behind by dead processes
        #[arg(long)]
        clean: bool,
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_cost_by_agent() {
        let cli =
            Cli::try_parse_from(["tokensave", "cost", "30d", "--by-agent"]).expect("parse failed");
        match cli.command {
            Some(Commands::Cost {
                range, by_agent, ..
            }) => {
                assert_eq!(range, "30d");
                assert!(by_agent);
            }
            _ => panic!("expected Cost command"),
        }
    }
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
        /// Only track when auto-tracking is enabled, otherwise exit
        /// successfully without doing anything (#397).
        ///
        /// For automated callers — the `post-checkout` hook passes this — so
        /// the `auto_track` config field (or `TOKENSAVE_AUTO_TRACK`) governs
        /// auto-tracking on every entry point rather than only inside
        /// `TokenSave::open`. A human typing `branch add` has asked for it, so
        /// the flag is opt-in rather than the default.
        #[arg(long)]
        if_enabled: bool,
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
