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
tokensave cost --by-model          # Cost grouped by model
tokensave cost --by-task           # Cost grouped by task category
tokensave cost --export json|csv   # Export cost data
tokensave query <search> [path]    # Search symbols
tokensave files [--filter dir] [--pattern glob] [--json]   # List indexed files
tokensave affected <files...> [--stdin] [--depth N]        # Find affected test files
tokensave install [--agent NAME]   # Configure agent integration
tokensave reinstall                # Refresh settings for all installed agents
tokensave uninstall [--agent NAME] # Remove agent integration
tokensave serve [--idle-timeout-secs N]   # Start MCP server (N: exit after N idle seconds)
tokensave servers [--json]         # List running servers and the index each one holds
tokensave monitor                  # Live TUI showing MCP calls across all projects
tokensave memory [--clean]         # Per-instance RSS report for all tokensave processes
tokensave upgrade                  # Self-update to latest version
tokensave channel [stable|beta]    # Show or switch update channel
tokensave doctor [--agent NAME]    # Check installation health
tokensave githooks [on|off] [--local]  # Manage git hooks (--local: this repo only, no core.hooksPath)
tokensave branch add|list|remove|removeall|gc|set-default   # Multi-branch management
tokensave current-counter          # Show per-project token counter
tokensave reset-counter            # Reset per-project token counter
tokensave disable-upload-counter   # Opt out of worldwide counter uploads
tokensave enable-upload-counter    # Re-enable worldwide counter uploads
```

## Settings for automatic initialization

| Setting | Where | Effect |
|---|---|---|
| `auto_init` | `~/.tokensave/config.toml` | `true` (default): a git repository with no index is initialized by a pathless `sync` (the `post-commit` and `post-merge` hooks) or by `serve`. `false`: run `tokensave init` yourself. |
| `TOKENSAVE_AUTO_INIT` | environment | Overrides `auto_init` for one run. `0`, `false`, `no`, `off` or empty turn it off; any other value turns it on. |

See [Automatic initialization](INDEXING.md#automatic-initialization) for the guards that apply.
