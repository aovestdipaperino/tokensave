# Troubleshooting

Fixes for common problems, and the `tokensave doctor` health check.

## `tokensave doctor`

Run a comprehensive health check of your tokensave installation:

```bash
tokensave doctor
```

Run `tokensave doctor --read-only` to collect local diagnostics without changing project, machine, or agent configuration. The report checks database paths and project database size from file metadata. It skips SQLite opens, compaction, stale-project inspection and cleanup, automatic repairs, and live subprocess and network probes. This avoids creating or updating SQLite journal or shared-memory files. Run plain `tokensave doctor` when you want database maintenance, the stale-project cleanup prompt, or live startup diagnostics. The read-only flag also works with `--agent` and `--caveman`.

Checks: binary location, project index, global DB, user config, where project state lives, agent integration (MCP server, hooks, permissions, prompt rules), and network connectivity. If any tool permissions are missing after an upgrade, it tells you to run `tokensave install`. Use `--agent` to check a specific agent only.
Checks: binary location, project index (including the periodic integrity-check interval and last result), global DB, user config, where project state lives, agent integration (MCP server, hooks, permissions, prompt rules), and network connectivity. If any tool permissions are missing after an upgrade, it tells you to run `tokensave install`. Use `--agent` to check a specific agent only.

Doctor checks an agent integration in full only when tokensave is set up in that agent. The **Agent integrations** section sorts every supported agent into one of these groups:

- **Installed**: the agent is registered in `installed_agents` in `~/.tokensave/config.toml`, or its configuration has the tokensave MCP server. Every problem is counted.
- **Partially installed or incompletely removed**: no MCP registration, but tokensave content remains, such as a rules block, a hook or a configuration file doctor cannot read. Doctor checks it in full and warns once, naming `tokensave install --agent ID` to repair it and `tokensave uninstall --agent ID` to finish removing it.
- **Detected without tokensave**: the agent is on this machine, but tokensave was never installed in it or was removed cleanly. Doctor lists it with the install command and does not count it.
- **Not installed**: neither the agent nor any tokensave content was found. Doctor lists it and does not count it.

Agents that are not installed therefore add no issues or warnings. If tokensave is installed in no agent at all, doctor reports one warning. `tokensave doctor --agent ID` on an agent without tokensave prints one warning instead of a list of missing files.

Doctor also validates that each installed hook uses the correct tokensave subcommand and auto-repairs broken hooks.

## Debug diagnostics

Diagnostic events are off by default. Use `--debug` for one command, set
`TOKENSAVE_LOG=debug` in the environment before starting an agent, or persist
`"log_level": "debug"` in the project's `.tokensave/config.json`, so new
TokenSave MCP servers and hooks inherit it. An explicit `--log-level` takes
precedence over `--debug`, which takes precedence over `TOKENSAVE_LOG`, which
takes precedence over the project configuration. Levels
are `off`, `error`, `warn`, `info`, `debug`, and `trace`. Use `trace` only for
short captures that need per-file extraction outcomes. Unset the variable or
set it to `off`, or remove the project setting, then restart the agent to turn
logging off. An explicit `TOKENSAVE_LOG=off` temporarily disables a persisted
project level.

For example, capture a sync or an MCP session from the project
directory. Choose a file outside the indexed checkout if the capture will
outlive the command.

```bash
tokensave --debug sync 2> /path/to/private/sync.stderr
TOKENSAVE_LOG=debug tokensave serve 2> /path/to/private/mcp.stderr
```

For an agent-managed server, keep its normal stdio connection and use the
agent's MCP stderr log. Stdout still carries the MCP JSON-RPC protocol.

To reproduce a Claude-compatible Bash hook decision, run this from an indexed
project with a known symbol and adjust the example to match the search:

```bash
printf '%s\n' '{"tool_name":"Bash","tool_input":{"command":"rg KnownSymbol src/lib.rs"}}' |
  TOKENSAVE_LOG=debug tokensave hook-pre-tool-use 2> /path/to/private/hook.stderr
```

Look for `grep_check`, `grep_classification`, `grep_index_probe`, and
`grep_decision` events. Together they show the recognized grep program, code
target eligibility, pattern shape, index lookup result, and final redirect
decision. The index probe also identifies failures that trigger the hook's
fail-closed policy.

Each diagnostic event is a JSON line with a Unix timestamp in milliseconds,
severity, subsystem, operation, process ID, and request ID. Match an MCP request's
events using the process and request IDs together. Process-level and background
events use request ID `0`. Diagnostic fields include counts, flags, and outcomes.
They omit raw commands, query text, branch names, paths, configuration secrets,
source contents, MCP payloads, SQL text, and bound values.

Before attaching logs to a GitHub bug report, extract the relevant diagnostic
lines and review them. The full stderr capture also includes ordinary errors and
status messages, which can contain local paths or input values. Share the
sanitized excerpt, the TokenSave version, and a minimal reproduction with
private inputs replaced.

## "tokensave not initialized"

The `.tokensave/` directory doesn't exist in your project, or, with a base state directory set, the project has no state there yet.

```bash
tokensave init
```

## "Ignoring a stray tokensave.db in the global state directory"

`~/.tokensave/` holds tokensave's global state (`config.toml`, `global.db`). It is a project only when `tokensave init` was run in your home directory, which also writes `config.json` there. A `tokensave.db` without `config.json` in that directory, for example one left by an older version, is ignored: folders under your home directory that have no index of their own are not served from it, so a `serve` never indexes your whole home tree from it. `serve` and `tokensave doctor` print this warning while the file exists. Delete the file to remove the warning:

```bash
rm ~/.tokensave/tokensave.db
```

## State is not where `state_dir` says

`tokensave doctor` warns that a project's state "is not at" the configured location when the project already had state before you set or changed `TOKENSAVE_STATE_DIR` or `state_dir`. tokensave keeps using the existing state, so nothing is lost. To follow the setting, stop the project's MCP servers and move the directory doctor names to the location it gives. Doctor also lists relocated state whose project directory was deleted; `tokensave state prune --dry-run` shows what `tokensave state prune` would delete. See [Keeping state outside the repository](INDEXING.md#keeping-state-outside-the-repository).

## MCP server not connecting

The AI agent doesn't see tokensave tools.

1. Ensure the agent config includes the tokensave MCP server (run `tokensave doctor`)
2. Restart the agent completely
3. Check that `tokensave` is in your PATH: `which tokensave`

### Codex CLI

Codex starts MCP servers when a session starts and drops any server that has not listed its tools within its startup timeout, which is 10 seconds unless configured. The session then has no `tokensave_*` tools and may show no error. Before `serve` answers, it can build a first index or migrate the index schema, and on a large project that can take longer than 10 seconds.

`tokensave install --agent codex` sets `startup_timeout_sec = 60` in `[mcp_servers.tokensave]` in `~/.codex/config.toml`, unless the table already has a `startup_timeout_sec` or `startup_timeout_ms`. Run `tokensave doctor --agent codex` from an indexed project. Besides the registration and approvals, it checks:

- that the server is not disabled with `enabled = false`
- that the configured `command` can be found and its `args` include `serve`
- the startup timeout, with a warning when it is not set
- that `enabled_tools` or `disabled_tools` do not hide tokensave's read-only tools
- a live startup: doctor checks the configured MCP `cwd` and uses it for the probe. Without a `cwd`, doctor uses the current project. It launches the configured command with its `env` table, sends `initialize` and `tools/list`, and compares the time with the startup timeout. The probe runs only for a `tokensave` executable and only when the working directory is indexed. It stops the server afterwards.

If the probe is slower than the timeout, raise `startup_timeout_sec`. If it fails, run the configured command with `serve` from its working directory to see its error output. Then start a new Codex session, because Codex reads MCP servers only at startup.

## Missing symbols in search

- Run `tokensave sync` to update the index
- Check that the language is supported (see [Supported Languages](LANGUAGES.md))
- Verify the file isn't excluded by `.gitignore`
- Check the `exclude` globs in `.tokensave/config.json`: the defaults skip directories such as `build/`, `out/` and `bin/`, which some projects use for source (see [Default excludes](INDEXING.md#default-excludes))

## GitHub Copilot clients in `doctor`

`tokensave install --agent copilot` configures VS Code and the Copilot CLI, and VS Code Insiders and JetBrains IDEs when they are present. Doctor reports each of them separately:

- **not detected**: the client is not on this machine
- **detected, tokensave not configured there**: for example a Copilot CLI-only setup on a machine that also has VS Code. This is not counted as a problem.
- **configured**: the MCP registration and the instructions are both checked
- **partially configured**: one of the two is missing, so doctor reports an issue. When the VS Code instructions are current but `User/mcp.json` has no `tokensave` server, the registration was removed after installation, for example by a manual edit, a reset or Settings Sync replacing `mcp.json` from another machine. Run `tokensave install --agent copilot` to restore it.

VS Code profiles that do not share the default profile's settings read their own `mcp.json` under `User/profiles/<id>/`. Doctor lists how many profiles exist and how many of them register tokensave. Add the server to a profile from that profile with **MCP: Add Server**.

## "branch '…' is not tracked" after a checkout

Automatic branch tracking is on by default, but a project can turn it off. With `auto_track` disabled, the `post-checkout` hook runs on every checkout but intentionally does not register new branches, and tokensave serves them from the nearest tracked ancestor's index. Projects initialized before tracking became the default have `"auto_track": false` written in their config and keep it after an upgrade.

- Run `tokensave status` (or call `tokensave_status`) to see whether `auto_track` is enabled and why the branch was not tracked
- Track the branch now with `tokensave branch add`
- Track new branches automatically by setting `"auto_track": true` in `.tokensave/config.json`, or `TOKENSAVE_AUTO_TRACK=1` for one run; check that `TOKENSAVE_AUTO_TRACK` is not set to `0`, `false`, `no`, `off`, or empty in your environment
- Run `TOKENSAVE_HOOK_DEBUG=1 git checkout <branch>` and read `.tokensave/hook-debug.log` to see what the hook decided

See [Automatic branch tracking](MULTI-BRANCH.md#automatic-branch-tracking-v730) for details.

## "refusing to open …: the WAL holds …-byte pages"

tokensave found a `-wal` file beside the database whose page size differs from the database's. SQLite never writes such a file, so the database or its sidecar is corrupt or was crafted, for example a `.tokensave/` directory committed to a repository. Opening it could return uninitialized memory into tool output (CVE-2026-119375), so tokensave refuses it and leaves the files untouched.

- If you don't trust where the `.tokensave/` directory came from, delete it and run `tokensave init`
- Otherwise delete the `-wal` and `-shm` files named in the message and run `tokensave sync --force`

See [Database channel compatibility](../DATABASE-COMPATIBILITY.md#wal-sidecar-page-size-check) for details.

## "Access denied (os error 5)" on Windows

If `sync`, `init`, `install` or `doctor` fail with "Access denied" on lockfiles, `.tokensave/`, the SQLite database or `.git\hooks`, while your user account has full control over those folders, tokensave is probably running at low integrity level. Windows starts a program at the integrity level of its executable's mandatory label, and a low-integrity process cannot write to ordinary (medium-integrity) folders, whatever their ACLs say. `cargo install` builds in `%TEMP%`, so if `%TEMP%` carries a Low mandatory label (some sandboxing tools apply one), the installed `tokensave.exe` inherits it.

tokensave detects this case: a failed command prints a `Hint:` line, and `tokensave doctor` reports it under **Binary**.

- Check the executable: `icacls "%USERPROFILE%\.cargo\bin\tokensave.exe"`. A `Mandatory Label\Low Mandatory Level` entry confirms the cause
- Reset it: `icacls "%USERPROFILE%\.cargo\bin\tokensave.exe" /setintegritylevel medium`
- Check `%TEMP%` with `icacls "%TEMP%"`. While it carries the Low label, every later `cargo install` reapplies it to the new binary, so reset the binary after each upgrade or remove the label from `%TEMP%`

## Indexing is slow

Large projects take longer on the first full index.

- Subsequent runs use incremental sync and are much faster
- Use `tokensave sync` (not `--force`) for day-to-day updates
- Staleness is checked automatically on every MCP tool call while an agent is connected

## Disabling tokensave for specific projects

If a project is too large and tokensave uses too much RAM, you can disable the MCP server per-project by setting `TOKENSAVE_DISABLE_SERVER=true` in its environment. The server exits cleanly without initializing.

**Claude Code** — add to your project's `.claude/settings.json`:

```json
{
  "mcpServers": {
    "tokensave": {
      "command": "tokensave",
      "args": ["serve"],
      "env": {
        "TOKENSAVE_DISABLE_SERVER": "true"
      }
    }
  }
}
```

**Other agents** — set the environment variable in whatever config your agent uses to launch MCP servers.

You can also set it globally via the shell (`TOKENSAVE_DISABLE_SERVER=true claude`), but this disables the tokensave MCP server for every project in the session.

`DISABLE_TOKENSAVE=true` remains supported as a deprecated compatibility alias for configurations created before this variable was namespaced.
