# CLI Reference

Every `tokensave` command at a glance.

```bash
tokensave init [path]              # Initialize a new project (full index); without a path, the git root
                                   # (a no-op inside an existing project)
tokensave init --cpu-usage low     # Initialize with a fixed CPU profile (skip the benchmark)
tokensave config benchmark         # Run or reuse the machine-level hardware benchmark
tokensave config benchmark --show  # Show the saved benchmark and settings (never measures)
tokensave config benchmark --force # Measure this machine again
tokensave config benchmark --reset # Delete the saved benchmark
tokensave tune [path] [--dry-run]  # Re-benchmark, save globally, update a pinned cpu_usage
tokensave sync [path]              # Incremental sync; without a path, auto_init may initialize a new repo
tokensave sync --force [path]      # Force a full re-index
tokensave sync --doctor [path]     # Sync and list added/modified/removed files
tokensave verify [path]            # Read-only index integrity and source-stat checks
tokensave verify [path] --deep --json # Full SQLite checks and bounded source hashing
tokensave status [path]            # Show statistics + cost summary
tokensave status [path] --json     # Show statistics (JSON output)
tokensave status --details         # Include node-kind breakdown
tokensave cost [range]             # Token cost summary (default: 7d)
tokensave cost --by-agent          # Cost grouped by supported agent
tokensave cost --by-model          # Cost grouped by model
tokensave cost --by-task           # Cost grouped by task category
tokensave cost --export json|csv   # Export cost data
tokensave gain [--all] [--range R] [--json]   # Tokens saved by MCP calls (default: this project, 30d)
tokensave gain --history           # Tokens saved per day
tokensave gain --by-tool           # Calls and tokens saved per tool
tokensave query <search> [path]    # Search symbols
tokensave files [--filter dir] [--pattern glob] [--json]   # List indexed files
tokensave affected <files...> [--stdin] [--depth N]        # Find affected test files
tokensave install [--agent NAME]   # Configure agent integration
tokensave reinstall                # Refresh settings for all installed agents
tokensave uninstall [--agent NAME] # Remove agent integration
tokensave serve [--idle-timeout-secs N] [--watch]   # Start MCP server (N: exit after N idle seconds; --watch: opt-in filesystem watcher)
tokensave servers [--json]         # List running servers and the index each one holds
tokensave monitor                  # Live TUI showing MCP calls across all projects
tokensave memory [--clean]         # Per-instance RSS report for all tokensave processes
tokensave upgrade                  # Self-update to latest version
tokensave channel [stable|beta]    # Show or switch update channel
tokensave doctor [--agent NAME]    # Check installation health
tokensave doctor --caveman [--session-id ID]  # Also check Caveman compatibility; JSON report on stdout
tokensave githooks [on|off] [--local]  # Manage git hooks (--local: this repo only, no core.hooksPath)
tokensave branch add|list|remove|removeall|gc|set-default   # Multi-branch management
tokensave current-counter          # Show per-project token counter
tokensave reset-counter            # Reset per-project token counter
tokensave disable-upload-counter   # Opt out of worldwide counter uploads
tokensave enable-upload-counter    # Re-enable worldwide counter uploads
tokensave state list               # List relocated project state and the project of each
tokensave state prune [--dry-run]  # Delete relocated state whose project directory is gone
tokensave okf validate <dir> [--json] [--strict] [--verbose]  # Check an OKF v0.2 bundle
tokensave okf export --output <dir> [--project P]  # Export the code index as an OKF bundle (no sync)
```

See [Open Knowledge Format](OKF.md) for the `okf` commands, their findings and the export layout.

## Settings for automatic initialization

| Setting | Where | Effect |
|---|---|---|
| `auto_init` | `~/.tokensave/config.toml` | `true` (default): a git repository with no index is initialized by a pathless `sync` (the `post-commit` and `post-merge` hooks) or by `serve`. `false`: run `tokensave init` yourself. |
| `TOKENSAVE_AUTO_INIT` | environment | Overrides `auto_init` for one run. `0`, `false`, `no`, `off` or empty turn it off; any other value turns it on. |

See [Automatic initialization](INDEXING.md#automatic-initialization) for the guards that apply.

## Settings for Caveman cooperation

| Setting | Where | Effect |
|---|---|---|
| `caveman_compatibility` | `~/.tokensave/config.toml` | `false` (default): the MCP instructions never mention Caveman. `true`: when a Caveman skill is installed for a supported agent, the MCP `initialize` instructions ask the agent to keep tokensave results exact. Read when the MCP server starts a session. |
| `TOKENSAVE_CAVEMAN_COMPATIBILITY` | environment | Overrides `caveman_compatibility` for one run. `0`, `false`, `no`, `off` or empty turn it off; any other value turns it on. |

`tokensave init` reports an installed Caveman skill and, while the setting is off, how to turn it on; it never changes the setting. See [Working alongside Caveman](AGENT-INTEGRATIONS.md#working-alongside-caveman).

## Settings for the state location

| Setting | Where | Effect |
|---|---|---|
| `state_dir` | `~/.tokensave/config.toml` | Unset (default): each project keeps its state in `.tokensave/` at its root. An absolute path, or one starting with `~`: state lives in `<state_dir>/projects/<name>-<hash>/` and nothing is written inside the repository. Read when a tokensave process starts. |
| `TOKENSAVE_STATE_DIR` | environment | Overrides `state_dir`. Empty selects the default layout. |

See [Keeping state outside the repository](INDEXING.md#keeping-state-outside-the-repository) for how existing state is kept and how orphans are reported.

## Settings for the filesystem watcher

| Setting | Where | Effect |
|---|---|---|
| `filesystem_watch_enabled` | `~/.tokensave/config.toml` | `false` (default): `serve` creates no filesystem watcher. `true`: `serve` follows file changes through OS notifications and syncs them. |
| `serve --watch` | command line | Turns the watcher on for one run, whatever `filesystem_watch_enabled` says. |
| `watcher_debounce` | `~/.tokensave/config.toml` | Quiet period before a burst of changes is applied: `"500ms"`, `"2s"` (default), `"1m"`. Clamped to 50 ms – 60 s. Read only when the watcher is on. |

See [Opt-in filesystem watcher](INDEXING.md#opt-in-filesystem-watcher) for scope, bounds and platform limits.
