use clap::{builder::PossibleValuesParser, ArgAction, Parser, Subcommand};

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
    /// Emit structured debug diagnostics on stderr
    #[arg(long, global = true)]
    pub debug: bool,
    /// Set stderr diagnostic verbosity (overrides --debug and TOKENSAVE_LOG)
    #[arg(long, global = true, value_enum)]
    pub log_level: Option<tokensave::debug_log::Level>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Export one indexed file as an experimental Graphify snapshot (no sync)
    GraphifyExport {
        /// Project root (default: discover from current directory)
        #[arg(long)]
        project: Option<String>,
        /// Repository-relative source file using / separators
        #[arg(long)]
        file: String,
        /// New output JSON path; existing files are refused
        #[arg(long)]
        output: std::path::PathBuf,
        #[arg(long, default_value_t = 1000)]
        max_nodes: usize,
        #[arg(long, default_value_t = 5000)]
        max_edges: usize,
    },
    /// Compare a SCIP index with the graph, read-only (experimental, no sync)
    ScipCompare {
        /// Project root (default: discover from current directory)
        #[arg(long)]
        project: Option<String>,
        /// SCIP index file produced by an external indexer
        #[arg(long)]
        index: std::path::PathBuf,
        /// Accept an index whose project_root names another directory;
        /// documents are still verified against this checkout
        #[arg(long)]
        rebase_root: bool,
        /// Refuse indexes larger than this many MiB
        #[arg(long, default_value_t = 256)]
        max_index_mib: u64,
        /// Examples kept per report category (max 1000)
        #[arg(long, default_value_t = 20)]
        samples: usize,
    },
    /// Validate, export and import Open Knowledge Format (OKF v0.3) bundles
    Okf {
        #[command(subcommand)]
        action: OkfAction,
    },
    /// Check a database file without recovery or migrations; saves sidecar diagnostics
    IntegrityCheck {
        database: std::path::PathBuf,
        /// Cross-check SQLite indexes as well as B-tree structure
        #[arg(long)]
        full: bool,
        /// Show the last saved result without running a check
        #[arg(long)]
        status: bool,
        #[arg(long, hide = true)]
        worker: bool,
    },
    /// Check index integrity without changing it
    Verify {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Check SQLite integrity and source content hashes
        #[arg(long)]
        deep: bool,
        /// Reserved: incremental repair is not yet implemented
        #[arg(long)]
        repair: bool,
        /// Output stable JSON diagnostics
        #[arg(long)]
        json: bool,
    },
    /// Run grep, rg or git grep and add index facts to each hit
    ///
    /// The search command runs unchanged, with its own flags and exit code.
    /// Each `path:line:` hit keeps its bytes and gains a tab and a `#ts`
    /// suffix with what the index knows about that line.
    Grep {
        /// `none` prints the search output with no index facts
        #[arg(long, value_name = "SPEC")]
        graph: Option<String>,
        /// The search command: `grep ...`, `rg ...` or `git grep ...`
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            required = true,
            value_name = "COMMAND"
        )]
        command: Vec<std::ffi::OsString>,
    },
    /// Initialize a new TokenSave project (full index)
    Init {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Folders to skip during indexing (can be repeated)
        #[arg(long = "skip-folder", num_args = 1, action = ArgAction::Append)]
        skip_folders: Vec<String>,
        /// Install this repository's own git hooks without asking (#455)
        #[arg(long, conflicts_with = "no_git_hook")]
        git_hook: bool,
        /// Do not offer this repository's own git hooks
        #[arg(long = "no-git-hook")]
        no_git_hook: bool,
        /// CPU usage profile to pin in `.tokensave/config.json` for this
        /// project (default: follow the global hardware benchmark, measured
        /// once per machine; see `tokensave config benchmark`)
        #[arg(long, value_enum)]
        cpu_usage: Option<tokensave::cpu::CpuUsage>,
    },
    /// Global (machine-level) settings in ~/.tokensave
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Benchmark this machine again and update the project's CPU usage profile
    ///
    /// Measures CPUs, memory and parallel speedup, saves the result as the
    /// global hardware benchmark (see `tokensave config benchmark`), and
    /// updates a `cpu_usage` this project pins in `.tokensave/config.json`.
    Tune {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Print the recommendation without changing the config
        #[arg(long)]
        dry_run: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Incremental sync (project must already be initialized with `tokensave init`)
    Sync {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Force a full re-index
        #[arg(short, long)]
        force: bool,
        /// Folders to skip during indexing (can be repeated)
        #[arg(long = "skip-folder", num_args = 1, action = ArgAction::Append)]
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
    Tool {
        /// Optional MCP tool name (with or without the `tokensave_` prefix),
        /// followed by tool arguments. Omit the name to list all tools.
        /// Tool flags include `--json`, `--project <path>`, `--args <json>`,
        /// and `-h`/`--help`. Values starting with `@` are read from a file.
        // Capturing the name in the trailing positional lets clap handle help
        // before the name and passes everything after it to the tool parser.
        #[arg(trailing_var_arg = true, value_name = "TOOL_AND_ARGS")]
        invocation: Vec<String>,
    },
    /// Configure agent integration (MCP server, permissions, hooks, prompt rules)
    #[command(name = "install", visible_alias = "claude-install")]
    Install {
        /// Agent to configure (auto-detects if omitted, prompts interactively
        /// when several are detected). Repeat the flag to install several
        /// agents in a single run, e.g.
        /// `tokensave install --agent claude --agent cursor` (#640).
        #[arg(long, value_parser = agent_value_parser(), action = ArgAction::Append, num_args = 1)]
        agent: Vec<String>,
        /// Whether to install git `post-commit` + `post-merge` hooks that run
        /// `tokensave sync` after each commit and after `git pull` (plus a
        /// `post-checkout` hook for fresh clones/branch tracking).
        ///
        /// `default` preserves the interactive prompt (or silent skip on
        /// non-TTY). `yes` installs without asking; `no` skips without asking.
        /// Both install into the **current repository** (#506).
        ///
        /// `global` instead claims `core.hooksPath`, a single machine-wide
        /// setting that forces one hook directory on every repository — which
        /// also takes the slot from every other hook installer, so `git lfs
        /// install --local`, husky and pre-commit will fail while it is held.
        #[arg(long, value_enum, default_value_t = GitHookMode::Default)]
        git_hook: GitHookMode,
        /// Install into the current project's config instead of the user's
        /// global config. Only supported for agents with a project-scoped
        /// config (claude, cursor, droid, gemini, zed, opencode, roo-code, kiro, auggie,
        /// plank).
        #[arg(long)]
        local: bool,
        /// Auto-approve an exact write-capable MCP tool. Repeat to select multiple tools.
        #[arg(long = "approve-write-tool", conflicts_with = "explicit_permissions")]
        approve_write_tools: Vec<String>,
        /// Replace existing TokenSave write grants and whole-server trust with safe defaults.
        #[arg(long = "reset-tool-approvals", visible_alias = "explicit-permissions")]
        explicit_permissions: bool,
        /// Removed because a wildcard approves future write tools.
        #[arg(long, hide = true)]
        wildcard_permissions: bool,
    },
    /// Refresh settings for all already-installed agents
    Reinstall {
        /// Auto-approve an exact write-capable MCP tool. Repeat to select multiple tools.
        #[arg(long = "approve-write-tool", conflicts_with = "explicit_permissions")]
        approve_write_tools: Vec<String>,
        /// Replace existing TokenSave write grants and whole-server trust with safe defaults.
        #[arg(long = "reset-tool-approvals", visible_alias = "explicit-permissions")]
        explicit_permissions: bool,
        /// Removed because a wildcard approves future write tools.
        #[arg(long, hide = true)]
        wildcard_permissions: bool,
    },
    /// Disconnect TokenSave integrations while preserving the executable and data
    ///
    /// Use `tokensave purge` to review data deletion, or your original installer
    /// (for example, `cargo uninstall tokensave`) to remove the executable.
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
        /// Preview affected paths and operations without writing anything.
        #[arg(long, conflicts_with = "quiet")]
        dry_run: bool,
        /// Include affected paths in progress output.
        #[arg(short, long, conflicts_with = "quiet")]
        verbose: bool,
        /// Suppress routine progress and keep errors and executable guidance.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Remove agent integrations and delete tokensave's settings, caches and indexes
    ///
    /// Does everything `uninstall` does, then deletes the data tokensave owns
    /// in the chosen scope. Without a scope flag it purges global state:
    /// agent integrations, global git hooks, and the settings and caches in
    /// ~/.tokensave. `--local` and `--project` purge project indexes instead;
    /// `--all-projects` purges global state and every registered project.
    /// The token-savings history is kept unless `--include-history` is given.
    /// The plan is shown before anything changes, and the tokensave
    /// executable itself is never removed.
    Purge {
        /// Show what would be removed and exit without changing anything
        #[arg(long)]
        dry_run: bool,
        /// Do not ask for confirmation (required when stdin is not a terminal)
        #[arg(short, long)]
        yes: bool,
        /// Purge only the project containing the current directory
        #[arg(long, conflicts_with_all = ["project", "all_projects"])]
        local: bool,
        /// Purge only this project (repeat for several)
        #[arg(long, value_name = "PATH", action = ArgAction::Append, num_args = 1, conflicts_with = "all_projects")]
        project: Vec<std::path::PathBuf>,
        /// Purge global state and every project in the global registry
        #[arg(long)]
        all_projects: bool,
        /// Also delete the token-savings history (global.db), which cannot be rebuilt
        #[arg(long, conflicts_with_all = ["local", "project"])]
        include_history: bool,
    },
    /// Extraction worker (spawned by tokensave itself; not for direct use).
    #[command(name = "extract-worker", hide = true)]
    ExtractWorker,
    /// post-checkout git hook handler (called by the installed hook, not by
    /// users directly).
    ///
    /// Takes git's three post-checkout arguments — previous HEAD, new HEAD,
    /// and the branch-checkout flag. Since #342 Q1 the installed hook is a
    /// single line delegating here, so the decision of what a checkout should
    /// trigger lives in the binary and ships with it.
    #[command(name = "hook", hide = true)]
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// PreToolUse hook handler (called by Claude Code, not by users directly)
    #[command(name = "hook-pre-tool-use", hide = true)]
    HookPreToolUse,
    /// Codex PreToolUse hook handler (called by Codex, not by users directly)
    #[command(name = "hook-pre-tool-use-codex", hide = true)]
    HookPreToolUseCodex,
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
    /// Pi tool-call guardrail handler (called by the Pi extension, not by users directly)
    #[command(name = "hook-pi-pre-tool-use", hide = true)]
    HookPiPreToolUse,
    /// Pi session-settled handler (called by the Pi extension, not by users directly)
    #[command(name = "hook-pi-stop", hide = true)]
    HookPiStop,
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
        /// Watch the project for file changes and sync them as they happen (#937)
        ///
        /// Off by default. Turns on the opt-in filesystem watcher for this run,
        /// as `filesystem_watch_enabled = true` in `~/.tokensave/config.toml`
        /// does for every run. The on-demand staleness check keeps running
        /// either way.
        #[arg(long)]
        watch: bool,
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
    /// Review dependency advisories and local call evidence
    VulnReachability {
        /// Initialized project path (default: discover from the current directory)
        path: Option<String>,
        /// Send declared dependency package names and versions to OSV if the cache needs a refresh
        #[arg(long)]
        online: bool,
    },
    /// Show installer-specific update instructions or upgrade a verified standalone release
    Upgrade {
        /// Verify and upgrade a standalone binary downloaded from GitHub
        #[arg(long)]
        standalone: bool,
        /// Show installer evidence and the proposed action without making changes
        #[arg(long)]
        dry_run: bool,
        /// Kill other running tokensave processes without asking
        #[arg(long)]
        kill: bool,
    },
    /// Show or switch the update channel (stable or beta)
    Channel {
        /// Verify and switch a standalone binary downloaded from GitHub
        #[arg(long, requires = "channel")]
        standalone: bool,
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
    /// Audit bare-name resolution quality over the built index
    ///
    /// Counts cross-file edges resolved through the bare-name path whose
    /// target is the sole symbol of that name — the population the
    /// reachability gate governs. Read it comparatively: index a tree at two
    /// commits and diff the counts. Unlike a production-to-`tests/` count it
    /// also sees phantoms landing inside production, and needs no
    /// test/production classification, so it is not sensitive to layout.
    AuditEdges {
        /// How many of the most-collided targets to list
        #[arg(long, default_value_t = 10)]
        top: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Check tokensave installation, configuration, and agent integration
    Doctor {
        /// Check only this agent (default: all agents)
        #[arg(long, value_parser = agent_value_parser())]
        agent: Option<String>,
        /// Inspect local state without repairs, database opens, or live probes
        #[arg(long)]
        read_only: bool,
        /// Also check Caveman compatibility (read-only) and print the
        /// per-agent report as JSON on stdout
        #[arg(long)]
        caveman: bool,
        /// Claude Code session whose Caveman mode to report; never inferred
        #[arg(long, requires = "caveman")]
        session_id: Option<String>,
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
        /// Print calls and tokens saved per tool instead of a single total.
        #[arg(long, conflicts_with = "history")]
        by_tool: bool,
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
    /// List or prune project state kept under a base state directory
    /// (`TOKENSAVE_STATE_DIR` or `state_dir`, #919)
    State {
        #[command(subcommand)]
        action: StateAction,
    },
    /// Opt-in performance profiling: status, reports and sanitized export (#754)
    Profile {
        /// Apply settings and reports only to the current repository
        #[arg(long, global = true, hide = true)]
        local: bool,
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Compare code quality at two indexed branch tips without changing files.
    QualityDiff {
        #[arg(long)]
        base: String,
        #[arg(long, default_value = "HEAD")]
        target: String,
        #[arg(long)]
        json: bool,
        #[arg(long, value_parser = ["regression"])]
        fail_on: Option<String>,
        #[arg(long, default_value_t = 10)]
        cyclomatic: u32,
        #[arg(long, default_value_t = 15)]
        cognitive: u32,
        #[arg(long, default_value_t = 100)]
        function_lines: u32,
        #[arg(long, default_value_t = 10)]
        fan_out: u32,
        #[arg(long, default_value_t = 3)]
        min_increase: u32,
        #[arg(long)]
        allow_stale: bool,
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
    fn parse_init_skip_folder_before_path() {
        let cli = Cli::try_parse_from(["tokensave", "init", "--skip-folder", "build", "project"])
            .expect("parse failed");

        match cli.command {
            Some(Commands::Init {
                path, skip_folders, ..
            }) => {
                assert_eq!(path.as_deref(), Some("project"));
                assert_eq!(skip_folders, ["build"]);
            }
            _ => panic!("expected Init command"),
        }
    }

    #[test]
    fn parse_sync_repeated_skip_folders_before_path() {
        let cli = Cli::try_parse_from([
            "tokensave",
            "sync",
            "--skip-folder",
            "build",
            "--skip-folder",
            "generated",
            "project",
        ])
        .expect("parse failed");

        match cli.command {
            Some(Commands::Sync {
                path, skip_folders, ..
            }) => {
                assert_eq!(path.as_deref(), Some("project"));
                assert_eq!(skip_folders, ["build", "generated"]);
            }
            _ => panic!("expected Sync command"),
        }
    }

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

    /// `#640`: `tokensave install --agent claude --agent cursor` must parse
    /// into a single Install command holding both agents, in the order the
    /// user typed them. Single-agent and flagless parses must still work.
    #[test]
    fn parse_install_multiple_agents() {
        let cli = Cli::try_parse_from([
            "tokensave",
            "install",
            "--agent",
            "claude",
            "--agent",
            "cursor",
        ])
        .expect("parse failed");
        match cli.command {
            Some(Commands::Install {
                agent,
                local,
                git_hook,
                wildcard_permissions,
                explicit_permissions,
                ..
            }) => {
                assert_eq!(agent, vec!["claude".to_string(), "cursor".to_string()]);
                assert!(!local);
                assert_eq!(git_hook, tokensave::agents::GitHookMode::Default);
                assert!(!wildcard_permissions);
                assert!(!explicit_permissions);
            }
            _ => panic!("expected Install command"),
        }
    }

    #[test]
    fn parse_install_single_agent() {
        let cli = Cli::try_parse_from(["tokensave", "install", "--agent", "claude"])
            .expect("parse failed");
        match cli.command {
            Some(Commands::Install { agent, .. }) => {
                assert_eq!(agent, vec!["claude".to_string()]);
            }
            _ => panic!("expected Install command"),
        }
    }

    #[test]
    fn parse_install_no_agent_still_parses() {
        let cli = Cli::try_parse_from(["tokensave", "install"]).expect("parse failed");
        match cli.command {
            Some(Commands::Install { agent, .. }) => {
                assert!(agent.is_empty(), "omitted --agent must yield empty Vec");
            }
            _ => panic!("expected Install command"),
        }
    }

    /// A bare `--agent` with no value must be rejected rather than silently
    /// falling back to auto-detection.
    #[test]
    fn parse_install_bare_agent_flag_is_rejected() {
        let err_kind = match Cli::try_parse_from(["tokensave", "install", "--agent"]) {
            Ok(_) => panic!("parse should fail for --agent without a value"),
            Err(e) => e.kind(),
        };
        assert!(
            err_kind == clap::error::ErrorKind::InvalidValue
                || err_kind == clap::error::ErrorKind::WrongNumberOfValues
                || err_kind == clap::error::ErrorKind::TooFewValues,
            "expected a missing-value error, got {:?}",
            err_kind
        );
    }

    /// Each `--agent` value is still validated against the known integration
    /// id list, so a typo or an unknown slug is rejected at parse time rather
    /// than mid-install.
    #[test]
    fn parse_install_unknown_agent_is_rejected() {
        let err_kind =
            match Cli::try_parse_from(["tokensave", "install", "--agent", "not-a-real-agent"]) {
                Ok(_) => panic!("parse should fail for an unknown agent id"),
                Err(e) => e.kind(),
            };
        assert!(
            err_kind == clap::error::ErrorKind::ValueValidation
                || err_kind == clap::error::ErrorKind::InvalidValue,
            "expected a value-validation error, got {:?}",
            err_kind
        );
    }
}

#[derive(Subcommand)]
pub enum OkfAction {
    /// Check a bundle against the OKF v0.3 conformance rules
    ///
    /// Exits 0 when the bundle is conformant, 1 when it is not (or, with
    /// `--strict`, when there are warnings). Reads only the bundle.
    Validate {
        /// Bundle directory
        path: std::path::PathBuf,
        /// Output the report as JSON
        #[arg(long)]
        json: bool,
        /// Treat warnings as failures
        #[arg(long)]
        strict: bool,
        /// Also list info findings such as broken links (always in JSON)
        #[arg(long)]
        verbose: bool,
    },
    /// Export the code index as an OKF bundle (read-only; no sync)
    Export {
        /// Project root (default: discover from current directory)
        #[arg(long)]
        project: Option<String>,
        /// New bundle directory; an existing path is refused
        #[arg(long)]
        output: std::path::PathBuf,
        /// Refuse indexes with more source files than this
        #[arg(long, default_value_t = 20_000)]
        max_files: usize,
        /// Symbols listed per file; the rest are counted only
        #[arg(long, default_value_t = 500)]
        max_symbols: usize,
        /// Refuse indexes with more file-to-file dependencies than this
        #[arg(long, default_value_t = 200_000)]
        max_links: usize,
    },
    /// Import a bundle so the MCP server can search it
    ///
    /// Copies the bundle into the project's state directory and validates
    /// the copy. Bundles that are not conformant are imported best-effort
    /// unless `--strict` is given. The code index is not changed.
    Import {
        /// Bundle directory
        path: std::path::PathBuf,
        /// Project root (default: discover from current directory)
        #[arg(long)]
        project: Option<String>,
        /// Name for the imported bundle (default: the directory name)
        #[arg(long)]
        name: Option<String>,
        /// Replace an imported bundle with the same name
        #[arg(long)]
        replace: bool,
        /// Refuse bundles that are not conformant
        #[arg(long)]
        strict: bool,
        /// Output the result as JSON
        #[arg(long)]
        json: bool,
    },
    /// List the imported bundles of a project
    List {
        /// Project root (default: discover from current directory)
        #[arg(long)]
        project: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Remove an imported bundle
    Remove {
        /// Name of the imported bundle
        name: String,
        /// Project root (default: discover from current directory)
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// Manage separate consent for sanitized performance data sharing
    Share {
        #[command(subcommand)]
        action: ProfileShareAction,
    },
    /// Show the profiling mode, storage use, recorded samples and overhead
    Status,
    /// Enable local profiling for new tokensave processes (sharing has separate consent)
    Enable {
        /// Collect detailed data; falls back to light after `--for`
        #[arg(long)]
        detailed: bool,
        /// How long detailed mode lasts, e.g. 2h or 1d (default: 24h)
        #[arg(long = "for", value_name = "DURATION", requires = "detailed")]
        duration: Option<String>,
    },
    /// Disable profiling (recorded data is kept until `profile reset`)
    Disable,
    /// Report recorded latency by operation, ranked by cumulative time
    Report {
        /// Only MCP phases and per-tool invocations
        #[arg(long, group = "profile_section")]
        mcp: bool,
        /// Only database operations
        #[arg(long, group = "profile_section")]
        database: bool,
        /// Only index, sync and run-size series
        #[arg(long, group = "profile_section")]
        indexing: bool,
        /// Window to include, e.g. 30m, 24h or 7d
        #[arg(long, default_value = "24h")]
        since: String,
        /// Only this engine (e.g. libsql)
        #[arg(long)]
        engine: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Rank bottlenecks by cumulative impact, with evidence and N+1 candidates
    Analyze {
        /// Window to include, e.g. 24h or 7d
        #[arg(long, default_value = "7d")]
        since: String,
        /// Number of findings to show per engine
        #[arg(long, default_value_t = 10)]
        top: usize,
        /// Output as JSON (all findings)
        #[arg(long)]
        json: bool,
    },
    /// List operations with executions at or above the slow threshold
    SlowQueries {
        /// Window to include, e.g. 30m, 24h or 7d
        #[arg(long, default_value = "24h")]
        since: String,
    },
    /// Print the sanitized export (static operation names and aggregates
    /// only), e.g. to attach to a GitHub issue
    Export {
        /// Window to include, e.g. 24h or 7d
        #[arg(long, default_value = "7d")]
        since: String,
        /// Write to this file instead of stdout
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
    /// Delete all recorded profile data (asks first unless `--force`)
    Reset {
        /// Delete without asking; required when stdin is not a terminal
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum ProfileShareAction {
    /// Show consent, destination availability, last upload and next due time
    Status,
    /// Explicitly consent after reviewing the destination, policy and payload
    Enable,
    /// Revoke consent for subsequent uploads
    Disable,
    /// Print exactly the sanitized JSON payload (the same as profile export)
    Preview,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show, run, or reset the machine-level hardware benchmark
    ///
    /// The benchmark measures CPUs, memory and parallel speedup once per
    /// machine and saves the recommended CPU usage profile in
    /// ~/.tokensave/benchmark.toml. Every project without its own
    /// `cpu_usage` uses it. Without a flag, a valid saved result is reused
    /// and the benchmark runs only when none exists.
    Benchmark {
        /// Measure again even when a valid result is saved
        #[arg(long, group = "benchmark_action")]
        force: bool,
        /// Print the saved result and settings without measuring
        #[arg(long, group = "benchmark_action")]
        show: bool,
        /// Delete the saved result; the next `tokensave init` measures again
        #[arg(long, group = "benchmark_action")]
        reset: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum StateAction {
    /// List relocated project state directories and the project each belongs to
    List,
    /// Delete relocated state whose project directory no longer exists
    ///
    /// Only directories with a recorded origin whose project root is gone
    /// are removed. State under a project root (`.tokensave/`) is never
    /// touched.
    Prune {
        /// List what would be deleted without deleting anything
        #[arg(long)]
        dry_run: bool,
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
    /// Make a branch the default, keeping the data already indexed
    ///
    /// An untracked branch takes over the current default's entry and
    /// database. A tracked branch becomes the default with its own
    /// database; the previous default stays as an ordinary branch entry.
    SetDefault {
        /// Branch to make the default
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

/// Git hook handlers invoked by the installed hook scripts.
#[derive(clap::Subcommand, Debug)]
pub enum HookAction {
    /// Handle a git `post-checkout` event.
    PostCheckout {
        /// Previous HEAD (git's `$1`). The all-zeros sentinel marks the
        /// initial checkout of a fresh clone or a new worktree.
        prev_head: Option<String>,
        /// New HEAD (git's `$2`). Unused today; accepted so the hook can pass
        /// `"$@"` through verbatim and the signature does not have to change
        /// when it is needed.
        new_head: Option<String>,
        /// Branch-checkout flag (git's `$3`): `1` for a branch checkout, `0`
        /// for a file checkout.
        branch_flag: Option<String>,
    },
}
