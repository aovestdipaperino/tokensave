# CLI Reference

Every `tokensave` command at a glance.

Use `tokensave --debug <command>` to emit structured diagnostics on stderr.
Use `--log-level off|error|warn|info|debug|trace` to select a level or
`TOKENSAVE_LOG` to pass a level to an agent-launched process. Set `log_level`
to the same values in `.tokensave/config.json` for a persistent project default.
CLI flags override the environment, which overrides the project setting. See
[Debug diagnostics](TROUBLESHOOTING.md#debug-diagnostics) for capture examples.

`tokensave --version --verbose` adds the source commit, build profile, target
and compiler. A build from a Git checkout reports the checked-out commit; a
crates.io install (`cargo install tokensave`) reports the release commit that
Cargo recorded in the published package, with a `-dirty` suffix if the
package was made from uncommitted changes. When neither is available, as in a
source archive without that record, the commit is `unknown`.

```bash
tokensave --version                # Print the installed version
tokensave --version --verbose      # Include build metadata
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
tokensave grep [--graph=none] COMMAND...   # Run grep, rg or git grep and add index facts to each hit
tokensave files [--filter dir] [--pattern glob] [--json]   # List indexed files
tokensave affected <files...> [--stdin] [--depth N]        # Find affected test files
tokensave install [--agent NAME]   # Configure agent integration
tokensave reinstall                # Refresh settings for all installed agents
tokensave uninstall [--agent NAME] [--local] [--keep-git-hooks] [--dry-run] [--verbose | --quiet]
tokensave purge [--dry-run] [--yes] [--local | --project PATH | --all-projects] [--include-history]   # Uninstall, then delete tokensave's settings, caches and indexes
tokensave serve [--idle-timeout-secs N] [--watch]   # Start MCP server (N: exit after N idle seconds; --watch: opt-in filesystem watcher)
tokensave servers [--json]         # List running servers and the index each one holds
tokensave monitor                  # Live TUI showing MCP calls across all projects
tokensave memory [--clean]         # Per-instance RSS report for all tokensave processes
tokensave upgrade [--dry-run] [--standalone] # Show installer guidance or verify and upgrade a standalone binary
tokensave channel [stable|beta]    # Show or switch update channel
tokensave doctor [--agent NAME]    # Check installation health
tokensave doctor --caveman [--session-id ID]  # Also check Caveman compatibility; JSON report on stdout
tokensave githooks [on|off] [--local]  # Manage git hooks (--local: this repo only, no core.hooksPath)
tokensave branch add|list|remove|removeall|gc|set-default   # Multi-branch management
tokensave current-counter          # Show per-project token counter
tokensave reset-counter            # Reset per-project token counter
tokensave disable-upload-counter   # Opt out of worldwide counter uploads
tokensave enable-upload-counter    # Re-enable worldwide counter uploads
# TOKENSAVE_TELEMETRY=off or DO_NOT_TRACK=1 turns the counter off for one environment
tokensave state list               # List relocated project state and the project of each
tokensave state prune [--dry-run]  # Delete relocated state whose project directory is gone
tokensave okf validate <dir> [--json] [--strict] [--verbose]  # Check an OKF bundle (v0.3; earlier versions with fallbacks)
tokensave okf export --output <dir> [--project P]  # Export the code index as an OKF bundle (no sync)
tokensave okf import <dir> [--name N] [--replace] [--strict] [--json] [--project P]  # Import a bundle for tokensave_knowledge
tokensave okf list [--json] [--project P]          # List imported bundles
tokensave okf remove <name> [--project P]          # Remove an imported bundle
```

Verbose version output includes the build commit, Cargo build profile, target
triple and Rust compiler version. Values unavailable at build time are shown as
`unknown`; source archives without Git metadata may not contain a commit. This
command does not open or initialize a project database.

See [Open Knowledge Format](OKF.md) for the `okf` commands, their findings, the export layout and imported knowledge.

## Index facts on search output

`tokensave grep COMMAND...` runs a `grep`, `rg` or `git grep` command and adds what the index knows to each hit. The search command runs unchanged: the flags, the regular expression dialect, the result set and the exit code are its own. The command reads the index directly and needs no MCP server.

```console
$ tokensave grep git grep -n add_one
src/lib.rs:4:pub fn add_one(value: i32) -> i32 {<TAB>#ts [defines add_one (function), 4-6]
src/lib.rs:14:        self.total = add_one(self.total);<TAB>#ts [in Counter::bump, 13-15] [calls add_one, src/lib.rs:4-6]
src/lib.rs:19:    counter.total = add_one(-1);<TAB>#ts [in reset, 18-20] [calls add_one, src/lib.rs:4]
```

Each `path:line:` hit keeps its bytes. The facts follow one tab character (`<TAB>` above) and the marker `#ts`. Each fact is in square brackets. The facts follow the conventions of the text results of `tokensave_search`: a line span is `a-b`, and a kind is in parentheses after the name.

| Fact | Meaning |
|---|---|
| `[in X, a-b]` | The innermost symbol that contains the line, and its lines. |
| `[defines X (kind), a-b]` | A symbol starts on the line. The kind is the name that the index uses, for example `function` or `enum_variant`. |
| `[calls X, file:a-b]` | A call on the line, and the definition that the index resolved it to. The first hit on a definition gives its lines. A later hit gives its first line: `[calls X, file:a]`. |
| `[ambiguous name, n candidates: A, B, C]` | A call that the index could not resolve to one definition. |
| `[uses X, file:a-b]`, `[type_of X, file:a-b]` and other edge kinds | A reference that is not a call. The first word is the kind of the edge in the index. The location follows the same rule as for a call. |
| `[stale]` | The file changed after it was indexed: its modification time is later, or its size is different. A stale file gets no other fact, and a definition in a stale file is named with no location. |

A line with no fact is printed as the search command printed it. This applies to comments outside each symbol and to file types that the index does not parse.

What the command changes, and what it does not:

- `-n` is added when the search command has no line-number flag, because the facts need line numbers.
- Output that has no `path:line:` hits is not changed: `-l`, `-c`, `-q`, `--files`, `--json`, `--heading`, NUL-separated output, colored output (`--color=always`) and context lines.
- `--graph=none` runs the search command as written, with no facts and no added `-n`.
- The exit code is the exit code of the search command. A command that is not `grep`, `rg` or `git grep` is refused with exit code 2.
- The index is opened read-only. The command starts no sync and no migration. When the project has no index, the search command runs as written. When the project has an index that the command cannot read, for example one that needs a migration, the search command also runs as written, and one line on standard error gives the reason: `tokensave grep: no index facts: ...`.

Limits:

- The search command gives no match positions. A hit gets a `calls` fact or a fact of a different edge kind only for a name that the search pattern contains as a whole word, and at most three of them. A search for `add_one` reports the call to `add_one` on a hit line and not the other calls on that line. A pattern such as `add_\w+` names no symbol, so its hits get only `in` or `defines`.
- A filter after the command sees the facts: in `tokensave grep rg -n Name | grep -v Test`, the filter also drops a hit whose facts contain `Test`.
- On a branch that has no index of its own, the facts come from the index that the MCP server uses for that branch. Each file is checked against its index entry, so a file that differs is `stale`.
- A line longer than 1 MiB is copied with no facts.

Run `tokensave sync` to bring a stale file up to date.

## Settings for automatic initialization

| Setting | Where | Effect |
|---|---|---|
| `auto_init` | `~/.tokensave/config.toml` | `true` (default): a git repository with no index is initialized by a pathless `sync` (the `post-commit` and `post-merge` hooks) or by `serve`. `false`: run `tokensave init` yourself. |
| `TOKENSAVE_AUTO_INIT` | environment | Overrides `auto_init` for one run. `0`, `false`, `no`, `off` or empty turn it off; any other value turns it on. |

See [Automatic initialization](INDEXING.md#automatic-initialization) for the guards that apply.

## Settings for tool approvals

| Setting | Where | Effect |
|---|---|---|
| `manage_permissions` | `~/.tokensave/config.toml` | `true` (default) adds read-only approvals where supported. Use `--approve-write-tool NAME` for exact write-tool grants or `--reset-tool-approvals` to restore prompts. `false` preserves user-managed approvals. |
| `TOKENSAVE_MANAGE_PERMISSIONS` | environment | Overrides `manage_permissions` for one run. `0`, `false`, `no`, `off` or empty turn it off; any other value turns it on. |

See [Tool approvals](AGENT-INTEGRATIONS.md#tool-approvals) for what each agent gets.

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
| `TOKENSAVE_USER_CONFIG_DIR` | environment | Optional absolute directory for machine configuration files and locks. Useful for disposable CLI tests. |

See [Keeping state outside the repository](INDEXING.md#keeping-state-outside-the-repository) for how existing state is kept and how orphans are reported.

## Settings for the filesystem watcher

| Setting | Where | Effect |
|---|---|---|
| `filesystem_watch_enabled` | `~/.tokensave/config.toml` | `false` (default): `serve` creates no filesystem watcher. `true`: `serve` follows file changes through OS notifications and syncs them. |
| `serve --watch` | command line | Turns the watcher on for one run, whatever `filesystem_watch_enabled` says. |
| `watcher_debounce` | `~/.tokensave/config.toml` | Quiet period before a burst of changes is applied: `"500ms"`, `"2s"` (default), `"1m"`. Clamped to 50 ms – 60 s. Read only when the watcher is on. |

See [Opt-in filesystem watcher](INDEXING.md#opt-in-filesystem-watcher) for scope, bounds and platform limits.
