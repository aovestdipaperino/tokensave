# Tokensave User Guide

Thanks for downloading Tokensave!

Tokensave is a code intelligence tool that builds a semantic knowledge graph of your codebase. It gives AI coding agents (like Claude Code) instant, structured access to your code's symbols, relationships, and dependencies — so they spend fewer tokens scanning files and more time writing code.

Everything runs locally. Your code never leaves your machine.

---

## Table of Contents

1. [Installing Tokensave](#installing-tokensave)
2. [Your First Index](#your-first-index)
3. [Connecting to Your Agent](#connecting-to-your-agent)
4. [Exploring Your Codebase from the CLI](#exploring-your-codebase-from-the-cli)
5. [Keeping the Index Fresh](#keeping-the-index-fresh)
6. [How the MCP Server Refreshes the Index](#how-the-mcp-server-refreshes-the-index)
7. [Checking Your Setup with Doctor](#checking-your-setup-with-doctor)
8. [Finding Affected Tests](#finding-affected-tests)
9. [MCP Tools for AI Agents](#mcp-tools-for-ai-agents)
10. [Supported Languages](#supported-languages)
11. [Privacy and Network](#privacy-and-network)
12. [Updating Tokensave](#updating-tokensave)
13. [Configuration Files](#configuration-files)
14. [Troubleshooting](#troubleshooting)

---

## Installing Tokensave

Pick whichever method suits your platform.

**Homebrew (macOS):**

```bash
brew install aovestdipaperino/tap/tokensave
```

**Scoop (Windows):**

```powershell
scoop bucket add tokensave https://github.com/aovestdipaperino/scoop-bucket
scoop install tokensave
```

**Cargo (any platform):**

```bash
cargo install tokensave
```

If you only work with a subset of languages, you can install a smaller binary:

```bash
cargo install tokensave --features medium        # lite + 9 more languages
cargo install tokensave --no-default-features    # lite tier only
```

**Prebuilt binaries:**

Download from the [latest release](https://github.com/aovestdipaperino/tokensave/releases/latest) and place the binary somewhere on your `PATH`. Archives are available for macOS (Apple Silicon), Linux (x86_64 and ARM64), and Windows (x86_64).

---

## Your First Index

Navigate to any project directory and run:

```bash
cd /path/to/your/project
tokensave init
```

Tokensave will scan every supported source file, extract symbols (functions, classes, methods, imports, type relationships, complexity metrics), and store everything in a local database at `.tokensave/tokensave.db`. You'll see a spinner with file-by-file progress and an ETA.

Once it finishes, run `tokensave status` to see what was indexed:

```bash
tokensave status
```

This prints an overview of your project: the number of files, symbols, edges (relationships between symbols), language distribution, and how many tokens the index has saved you so far. If you just want the summary line without the ASCII art, pass `--short`:

```bash
tokensave status --short
```

For machine-readable output, use `--json`.

### Why `init` and `sync` are separate

Initialization (`tokensave init`) and incremental updates (`tokensave sync`) are deliberately separate commands.

Tokensave installs a global git post-commit hook that runs `tokensave sync` after every commit to keep the index fresh. If `sync` were allowed to create a new database when none existed, it would silently bootstrap a `.tokensave/` directory in every git repository on your machine -- even ones you never intended to index. By requiring an explicit `init`, only projects you opt into get a database. The hook runs harmlessly (exits with a non-zero status, output suppressed) in all other repos.

In short:
- **`tokensave init`** -- one-time setup. Creates the database and performs a full index. Errors if already initialized.
- **`tokensave sync`** -- ongoing updates. Requires an existing database. Errors if the project was never initialized.

### Incremental syncs

After the initial full index, every subsequent `tokensave sync` is incremental. It detects which files changed since the last sync (via content hashing) and only re-indexes those files; files deleted from disk are pruned from the graph along with their symbols and edges. On a typical commit-sized change, this takes under a second.

### Force re-index

If you ever need to rebuild the entire index from scratch (for example, after a major Tokensave upgrade), pass `--force`:

```bash
tokensave sync --force
```

### Skipping folders

If there are directories you never want indexed (vendored code, generated output, etc.), pass `--skip-folder`:

```bash
tokensave sync --skip-folder vendor --skip-folder generated
```

### Seeing what changed

The `--doctor` flag lists every file that was added, modified, or removed during the sync, so you can verify exactly what the index updated:

```bash
tokensave sync --doctor
```

### Diagnosing slow syncs

If a sync appears stuck or is taking longer than expected, add `--verbose` (`-v`) to see per-phase diagnostics with file counts and timings:

```bash
tokensave sync --verbose
```

Example output:

```
  [verbose] scanned 10432 files in 2.3s
  [verbose] stat-checked 10432 files in 0.1s
  [verbose] changes: 3 new, 847 stat-changed, 0 removed, 9582 unchanged
  [verbose] hashed 850 files in 1.2s (0 read errors)
  [verbose] content check: 12 modified, 838 mtime-only
  [verbose] indexed 15 files (204 nodes, 189 edges) in 0.3s
  [verbose] resolved 39841 references in 0.5s
✔ sync done — 3 added, 12 modified, 0 removed in 4412ms
```

This also works with `--force` for full re-index diagnostics.

### Respecting .gitignore

By default, tokensave respects your `.gitignore` rules and skips ignored files during indexing. You can check the current setting or toggle it:

```bash
tokensave gitignore              # show current setting
tokensave gitignore on           # enable (default)
tokensave gitignore off          # disable — index everything
```

Don't forget to add `.tokensave` to your `.gitignore` so the database doesn't get committed:

```bash
echo .tokensave >> .gitignore
```

#### Projects without a Git repository

`.gitignore` filtering does **not** require a Git repository. tokensave reads
ignore files directly rather than asking Git, so a `.gitignore` in your project
root is honored even when there is no `.git` directory — useful under TFVC,
Perforce, SVN, or no version control at all.

What a non-repo project does lose is the exclude sources Git itself supplies:
`.git/info/exclude` and your global excludes file. If you need SCM-independent
rules, set `exclude` globs in `.tokensave/config.json`:

```json
{
  "exclude": ["**/build/**", "**/obj/**", "bin/**"]
}
```

`tokensave init --skip-folder <dir>` writes to that same list.

#### Indexing hidden directories

By default, tokensave prunes all hidden directories (dot-prefixed) during the file walk to avoid indexing massive dependency and config folders (like `.venv`, `.cargo`, or `.next`).

If your project contains code in hidden directories (e.g., `.github/scripts/`), you must explicitly opt-in via the `include` array in `.tokensave/config.json`. **Crucially, you must include both the directory and its contents**, because the walker prunes at the directory level before glob matching happens:

```json
{
  "include": [
    ".github",
    ".github/**"
  ]
}
```

---

## Connecting to Your Agent

Tokensave works as an MCP (Model Context Protocol) server. AI coding agents connect to it to query your codebase instead of scanning files directly. The `install` command sets everything up automatically.

### Claude Code

```bash
tokensave install
```

This is the default. It registers the MCP server in `~/.claude/settings.json`, grants tool permissions so Claude doesn't have to ask you every time, installs a `PreToolUse` hook that redirects Claude away from spawning expensive Explore agents and away from symbol-shaped grep/rg searches that a tokensave tool answers more cheaply, and adds prompt rules to `~/.claude/CLAUDE.md` that tell Claude to prefer tokensave tools.

By default the tool grant is an explicit list (one `permissions.allow` entry per tool). Pass `--wildcard-permissions` to grant them via a single compact `mcp__tokensave__*` entry instead — both forms are fully honored by Claude Code, so this is purely a preference. The choice is remembered in `~/.tokensave/config.toml` (`wildcard_permissions`) for global installs; pass `--explicit-permissions` to switch back.

### Other agents

Tokensave supports many agents. Pass `--agent` to install for a specific one:

```bash
tokensave install --agent claude      # Claude Code (default)
tokensave install --agent opencode    # OpenCode
tokensave install --agent codex       # OpenAI Codex CLI
tokensave install --agent gemini      # Gemini CLI
tokensave install --agent qwen        # Qwen Code
tokensave install --agent copilot     # GitHub Copilot (VS Code, JetBrains, CLI)
tokensave install --agent cursor      # Cursor
tokensave install --agent droid       # Factory Droid
tokensave install --agent zed         # Zed
tokensave install --agent cline       # Cline
tokensave install --agent roo-code    # Roo Code
tokensave install --agent antigravity # Antigravity (Windsurf)
tokensave install --agent kilo        # Kilo CLI
tokensave install --agent kiro        # AWS Kiro
tokensave install --agent kimi        # Moonshot Kimi CLI
tokensave install --agent vibe        # Mistral Vibe
tokensave install --agent grok        # Grok Build (xAI)
tokensave install --agent auggie      # AugmentCode
tokensave install --agent pi          # Pi (pi.dev)
tokensave install --agent plank       # Plank (macOS only)
```

You can also pre-decide the global git `post-commit` hook prompt — useful in
bash scripts and onboarding playbooks where a `read_line` would otherwise block:

```bash
tokensave install --git-hook yes   # install the hook without asking
tokensave install --git-hook no    # skip the hook without asking
tokensave install --git-hook default  # preserve the interactive prompt (default when flag is omitted)
```

To remove the hooks again, see [Removing the git hooks](#removing-the-git-hooks).

Each agent gets an appropriate configuration: MCP server registration, tool permissions (where the agent supports them), and prompt rules in the agent's instruction file.

Kiro setup registers tokensave in `~/.kiro/settings/mcp.json`, writes steering to
`~/.kiro/steering/tokensave.md`, and writes a tokensave-managed agent that loads
that steering as a resource while keeping Kiro's default prompt. The managed
agent exposes all configured tools and pre-approves Kiro built-ins plus the
tokensave MCP server, then adds hooks that block research delegation until
tokensave MCP tools have been tried and sync the index after Kiro writes files.
If you already have a different custom default agent or a user-managed
`tokensave` agent, tokensave leaves it alone and prints a warning.

The install is idempotent — safe to run again after upgrading tokensave. You'll also be offered the option to set up a global git post-commit hook (more on that below).

#### Config backups

Whenever tokensave rewrites an agent config file — on `install`, on `uninstall`, or when the `doctor` auto-repairs hooks — it first copies the original to a sibling `.bak` file in the same directory. For example:

- `~/.codex/config.toml` → `~/.codex/config.toml.bak`
- `~/.cursor/mcp.json` → `~/.cursor/mcp.json.bak`
- `~/.claude.json` → `~/.claude.json.bak`

If anything goes wrong (a typo, an unexpected rewrite, an unknown bug), restore with `cp <path>.bak <path>`. The `.bak` is always the **exact bytes** of whatever was on disk just before the write; tokensave never deletes or rotates it, so the most recent backup is the file you want.

### Project-local install

By default `tokensave install` registers the MCP server in your **global** agent config (e.g. `~/.claude.json`). To register tokensave for just the current project instead, add `--local`:

```bash
tokensave install --local --agent claude
```

This writes project-scoped config you can commit and share with your team. For Claude that's `./.mcp.json`, `./.claude/settings.json`, and `./CLAUDE.md`. Supported agents: **claude, cursor, droid, gemini, zed, opencode, roo-code, kiro, auggie, plank** (each writes its own project file, e.g. `.cursor/mcp.json`, `.factory/mcp.json`, `.gemini/settings.json`, `.zed/settings.json`, `opencode.json`, `.roo/mcp.json`, `.kiro/settings/mcp.json`, `.augment/settings.json`, `.mcp.json` for plank). Other agents have no project-scoped config and report an error with `--local`.

Remove a project-local install with `tokensave uninstall --local`.

### Removing an integration

```bash
tokensave uninstall                   # remove Claude Code integration
tokensave uninstall --agent codex     # remove Codex integration
```

---

## Exploring Your Codebase from the CLI

You don't need an AI agent to use tokensave. The CLI has several commands for direct exploration.

### Searching for symbols

```bash
tokensave query "authenticate"
```

This searches the full-text index for symbols matching your query. It returns function names, class names, method names, and their file locations and signatures. Limit results with `-l`:

```bash
tokensave query "authenticate" -l 5
```

### Building task context

```bash
tokensave context "implement user authentication"
```

This is the same context builder that AI agents use. Given a natural language task description, it finds the most relevant entry points, related symbols, and code structure. Output defaults to Markdown; use `--format json` for structured output.

```bash
tokensave context "implement user authentication" --format json -n 30
```

The `-n` flag controls how many symbols are included (default: 20).

### Listing indexed files

```bash
tokensave files                           # all files
tokensave files --filter src/mcp          # only files under src/mcp/
tokensave files --pattern "**/*.rs"       # only Rust files
tokensave files --json                    # machine-readable output
```

### Running the MCP server directly

```bash
tokensave serve
```

This starts the MCP server over stdio. You normally don't need to run this yourself — the agent integration handles it. But it's useful for debugging or connecting custom tools.

### Working from a subdirectory

You can open your AI agent from any subdirectory of an indexed project. Tokensave will walk up the directory tree to find the nearest `.tokensave/` database — similar to how git finds `.git/`.

When the MCP server starts from a subdirectory, listing tools like `tokensave_files`, `tokensave_search`, and `tokensave_context` automatically scope their results to that subdirectory. This is useful in monorepos or large projects where you want to focus on one area.

Graph traversal tools (`tokensave_callers`, `tokensave_callees`, `tokensave_impact`, etc.) remain unscoped so you can still follow connections across directory boundaries.

You can always override the automatic scope by passing an explicit `path` parameter to any tool. `tokensave_status` shows the active scope prefix when one is in effect.

---

## Keeping the Index Fresh

Tokensave gives you three ways to keep the index up to date.

### Manual sync

Run `tokensave sync` whenever you want. It's incremental and fast.

### Post-commit hook

During `tokensave install`, you'll be offered a global git `post-commit` hook. If you accept, tokensave will automatically sync in the background after every git commit across all your repos. The hook is a no-op in repos that don't have a `.tokensave/` directory.

If you're scripting the install (CI, dotfiles bootstrap, onboarding playbook), pass `--git-hook yes` to install the hook without prompting, or `--git-hook no` to skip it. Omitting the flag preserves the interactive prompt.

#### Per-repository hooks instead of global ones

The global hooks work by claiming `core.hooksPath`, which is a **single
machine-wide setting**. It overrides git's default of a separate `.git/hooks`
per checkout, so every repository on the machine is forced to share one hook
directory — which is wrong if your projects need different tooling (#455).

Per-repository hooks avoid that entirely. They go in the repository's own hook
directory and touch no git config at all, so nothing else on the machine
changes:

```bash
tokensave githooks on --local      # install into this repository only
tokensave githooks --local         # show what this repository has
tokensave githooks off --local     # remove them from this repository
```

`tokensave init` offers them for you. Accept and the three hooks are installed;
decline and nothing is written. It only asks on a terminal, so scripted and CI
installs are unaffected, and it stays quiet when tokensave's global hooks are
already installed — a repository covered by both would sync twice per commit.

```bash
tokensave init --git-hook       # install without asking
tokensave init --no-git-hook    # don't ask, don't install
```

Writing is additive: a repository that already has a `post-commit` — husky,
pre-commit, or one you wrote — keeps everything it had and gains a marked
tokensave section. Linked worktrees share the checkout's hook directory, which
is what git itself reads, so installing from inside a worktree does the right
thing.

One thing to watch: if a `core.hooksPath` is set for the repository, from any
config scope, git reads hooks from *there* and never looks at the repository's
own directory. Local hooks would be written but never run, so tokensave says so
rather than reporting a success that is not one.

#### Removing the git hooks

The hooks are global and outlive every agent integration, so removing tokensave from your agents does not stop them on its own (#420). Two ways to remove them:

```bash
tokensave githooks              # show what is installed, and where
tokensave githooks off          # remove tokensave's global hooks
tokensave githooks off --local  # remove this repository's own hooks
tokensave uninstall             # removes the hooks along with all agent integrations
tokensave uninstall --keep-git-hooks   # ...or keep them
```

Removal is deliberately conservative:

- A hook file that contains anything you wrote keeps that content, and loses only tokensave's marked section.
- A hook file that is nothing but tokensave's own content is deleted.
- `core.hooksPath` is unset, and the hooks directory removed, only when it is tokensave's own default (`~/.config/git/hooks`), it is left empty, and this run actually removed a tokensave hook. A path you configured yourself is never touched.

If the directory still holds hooks tokensave did not write, it is left in place and the command says so.

You can also set it up manually:

**Global (all repos):**

```bash
git config --global core.hooksPath ~/.git-hooks
mkdir -p ~/.git-hooks
cp scripts/post-commit ~/.git-hooks/post-commit
chmod +x ~/.git-hooks/post-commit
```

**Per-repo:**

```bash
cp scripts/post-commit .git/hooks/post-commit
chmod +x .git/hooks/post-commit
```

### The MCP server

When you start the tokensave MCP server (e.g. via your agent), it refreshes the index on demand as you use it. See the next section.

---

## How the MCP Server Refreshes the Index

The server keeps the index fresh on demand rather than by watching the
filesystem, in two places:

- **When the server connects**, a catch-up sync reconciles changes made
  while no server was running.
- **At the start of every MCP tool call**, a staleness check walks the
  project tree — the same gitignore-aware walk `tokensave sync` uses — and
  re-indexes what changed. That check is gated by a 30-second cooldown, so
  calls inside the same window cost at most one walk.

So an edit is picked up by the next tool call once the cooldown has
elapsed, not the moment you save. If you need the index current sooner,
run `tokensave sync`.

There is no background watcher and no daemon. `tokensave serve` embedded an
OS-level watcher from 6.0.0, when daemon mode was removed, until `f7f7c9b`
removed it in 6.1.1 (#80); the CHANGELOG entry for that release attributes the
removal to CPU and memory pressure on large monorepos. The watcher is older
than the embedding: `ProjectWatcher` arrived in 3.5.0, driven then by the
daemon and the CLI. The on-demand model is its replacement.

Multiple MCP servers on the same project (e.g. two agents) coordinate via
a per-project sync lock: only one sync runs at a time.

### What an automatic sync will not do

Both refreshes above are *automatic* — you did not ask for them — so they
are bounded, and they decline rather than run in two cases:

- **The project has no indexed files.** Building the first index is
  `tokensave init`'s job: it is explicit, it reports progress, and you chose
  the directory. A background task will not index a project from scratch by
  inference. Without this, a server started in a directory that was never
  initialised would try to index the whole tree — on a home directory that
  reached tens of gigabytes of memory before anyone noticed (#396).
- **More than `max_auto_sync_files` files are stale** (default 2000). The
  30-second cooldown bounds how *often* a sync runs, never what one costs,
  so the file count is capped separately.
- **The working tree has moved to a different branch than the server is
  serving.** A running server resolves its branch once, at startup, so a
  `git checkout` underneath it does not change which database it holds.
  Syncing then would write the new branch's files into the old branch's
  index, which is how `main` ends up holding a file that only ever existed
  on `feature` (#400). While the drift lasts, local graph tools also refuse
  to answer from the old branch's index: the call fails with an error naming
  both branches and the recovery action. Restart the MCP server to serve the
  new branch; `tokensave_status` stays callable so you can see what is being
  served. This applies only to projects using per-branch databases — a
  single-index project is unaffected by a checkout.

Either way the server prints what it skipped and what to run. `tokensave sync`
is deliberately unbounded and is the supported way to index a large change on
purpose. To change the ceiling, set `max_auto_sync_files` in
`.tokensave/config.json`; `0` disables the file-count check (the empty-index
guard always applies).

The `watcher_debounce` setting in `~/.tokensave/config.toml` is left over
from that watcher. It is still accepted, but nothing reads it, and the
30-second cooldown is not configurable.

### Interrupting a sync

`Ctrl-C` and `kill` stop a sync in progress rather than waiting for it to
finish. A sync polls for the request in the phases that dominate its runtime —
the parallel extraction pass and the per-file write loop — so on a large tree
it stops in well under a second instead of minutes.

Stopping is always safe. The index keeps its stale marker, so the next sync
redoes the abandoned work, and the sync lock is released so that next sync can
run at all. What the message tells you is how far it got:

- *"no partial results were committed"* — it stopped before anything was
  written.
- *"partway through writing"* — some files were updated and others were not.
  This is the same state a power cut would leave, and the next `tokensave
  sync` resolves it.

An MCP server is subject to the same thing: a `kill` during its automatic
catch-up sync now takes effect during the sync rather than after it.

### Index scope warnings

A `serve` warns on stderr, once at startup, about an index whose *scope* looks
wrong rather than whose contents do:

- **Your home directory is initialised as a project.** Every server started
  there indexes your whole home tree. This is reported wherever you are
  standing, because the whole problem with this state is that nobody is
  standing in it when it does the damage (#450).
- **The index is larger than 5 GB.** A server maps the whole file.

Both are warnings, not refusals, and the server starts normally. An index that
already exists is a working setup, and refusing it retroactively would decide
for you which of your projects stop working. If you meant it, set
`suppress_scope_warning: true` in `.tokensave/config.json` to stop being told.
`tokensave doctor` reports the same two conditions and ignores the switch.

### Orphaned servers

On Unix, a `serve` whose launching process has died exits on its own within
about 30 seconds. A host that exits without closing the server's stdin used to
leave the server running indefinitely with its whole index mapped, to be found
and killed by hand (#450). This covers a *dead* parent only; duplicate servers
under a still-live host are a host-side problem (#436).

### Bounding a host that leaks servers

Some MCP hosts start a **new** server per subagent and keep every one of them
alive after the subagent that used it has finished. They are all children of
the same still-live supervisor, so nothing ever closes their stdin and the EOF
that would normally stop them never arrives; each one goes on holding its index
open. A four-server pile-up measured on Windows cost roughly 113 MB private
memory and 922 handles.

This is the host's bug — the same hosts retain duplicate fleets of unrelated
MCP servers too — and tokensave cannot detect it: a parent-PID watchdog has
nothing to key on, because the parent is alive. What tokensave can do is stop
waiting:

```bash
tokensave serve --idle-timeout-secs 900
```

The server exits after that many seconds with no request, through the ordinary
graceful shutdown (counters persisted, WAL checkpointed, registry entry
removed).

**Off by default, and check one thing before turning it on.** Whether this is
safe depends on your host starting a fresh server when a tool is called after
an idle exit. Most do; tokensave cannot verify it for yours, and if yours does
not, tools stop working until the host is restarted. Try it on one project
first.

Two things it deliberately does not do:

- **It never interrupts a request.** The deadline is evaluated only while the
  server is parked waiting for the next request, and the timer restarts each
  time it parks — so a request slower than the timeout cannot be cut off, and a
  server busier than its timeout never expires.
- **It never cuts off startup work.** A deadline landing while the startup
  catch-up sync or a version-bump reindex is still running is deferred, and
  waits out another full window.

This is host-integration policy rather than project state, so it lives on the
command line only — there is no config-file equivalent, and `tokensave install`
does not add it to generated host config for you.

### Finding which server holds an index

A `serve` process keeps an exclusive handle on its database for as long as it
runs — file watching is why the process is long-lived, and the handle comes
with it. So an indexed directory cannot be deleted while a client has a server
up. On Linux and macOS the delete succeeds and the space is reclaimed when the
last handle closes; on Windows it fails outright:

```
Remove-Item: The process cannot access the file
'...\.tokensave\tokensave.db' because it is being used by another process.
```

This bites most often on git worktrees: a worktree per task, each one indexed,
each one cleaned up afterwards. `git worktree remove` deregisters the worktree
even when the file delete fails, which leaves git metadata pruned and the
directory still on disk.

`tokensave servers` names the holder:

```bash
tokensave servers
```

```
  PID  VERSION  PROJECT
87904   7.11.0  /Users/you/Code/app/worktrees/feature-x
```

A per-branch database is not derivable from the project root, so it is shown
on its own line when it is not the default `<project>/.tokensave/tokensave.db`.

Most servers carry no project in their command line — the host supplies it
through the global database or MCP `initialize` roots rather than `--path` — so
a process lister cannot answer this and neither can `ps`.

#### Reading the registry directly

Each running server writes `~/.tokensave/servers/<pid>.json`. Wrappers should
read these files rather than shelling out; `tokensave servers --json` emits the
same objects.

```json
{
  "pid": 87904,
  "started_at": 1788099755,
  "project_path": "/Users/you/Code/app",
  "argv_path": "/Users/you/Code/app",
  "db_path": "/Users/you/Code/app/.tokensave/tokensave.db",
  "version": "7.11.0"
}
```

| Field | Meaning |
|---|---|
| `pid` | OS process id; also the filename stem |
| `started_at` | Process start time, Unix epoch seconds, as the OS reports it. With `pid` this survives PID reuse |
| `project_path` | The project root the server resolved and is serving |
| `argv_path` | The root as given on the command line, or `null` when the host supplied it another way |
| `db_path` | The database actually held open — **match on this** for the index → process direction |
| `version` | The tokensave version serving, so a stale binary is visible |

The registry lives in the global directory rather than beside the index on
purpose: a PID file inside `.tokensave/` would sit in the one directory whose
defining problem is that it cannot be deleted, and would vanish with the
checkout being cleaned up.

Entries are removed on clean exit. A hard kill skips that, so stale entries are
also reaped whenever a server starts and whenever the registry is read — an
entry never outlives one listing.

#### There is no `servers --stop`

Stopping is deliberately left to you. MCP clients restart their servers, so
"stop them all" does not converge: new processes appear while old ones are
being killed. Terminating is only safe for someone who knows whether the host
is running and can stop it first, which tokensave cannot know. Identify the
holder here, then stop the client — or kill that one PID, which `serve` now
honours (a `SIGTERM` used to be ignored; fixed in #436/#450).

### Strict mode: refuse worktree content mismatches

Branch drift is now refused by default (see the sync safeguards above), so
`strict_tree` remains relevant only to the separate different-worktree
mismatch described here: the index belongs to a **different git worktree**
than the one you are in. Branch drift, where the server is serving a
**different branch** than your working tree is on, no longer needs the
opt-in (see
[BRANCHING-USER-GUIDE.md](BRANCHING-USER-GUIDE.md#how-syncing-interacts-with-branches)).

For a different git worktree, tokensave answers with a warning by default,
or refuses when `strict_tree` is enabled.

For worktree-heavy or branch-heavy workflows, a warning may not be enough. An
agent rule that says "always check tokensave before reading files" inherits the
wrong-tree answer with no signal anything is off, and an empty result reads as
"no such symbol" rather than "wrong tree". If a wrong answer is worse than no
answer for you, set `strict_tree` in `.tokensave/config.json`:

```json
{
  "strict_tree": true
}
```

Every `tokensave_*` tool then **fails** with an error naming both trees (or both
branches) and the remedy, instead of answering. `tokensave_status` stays
callable so you can still see what is being served and why the refusal
happened.

It stays off by default deliberately: sharing one index across a family of
worktrees is a legitimate setup, and turning that into a hard error without
being asked would be a bad surprise. Nothing new is detected when you enable
it — the same conditions were already detected, this only changes whether they
warn or refuse.


### CLI-Only Workflows

If you don't keep an agent attached, no MCP server is running to refresh the
index. Use a git post-commit hook to refresh it on commit:

```bash
cp scripts/post-commit .git/hooks/post-commit
chmod +x .git/hooks/post-commit
```

Or run `tokensave sync` manually when you need a fresh index.

### Upgrading from 5.x

The standalone `tokensave daemon` command and its system-service autostart
were removed in 6.0.0. If you had a daemon autostart installed under 5.x,
remove it manually.

If you don't remember the exact service/plist name, list them first:

- macOS: `launchctl list | grep tokensave`
- Linux: `systemctl --user list-units | grep tokensave`
- Windows: `sc.exe query state= all | findstr -i tokensave`

Then remove the entry matching your install:

- macOS: `launchctl unload ~/Library/LaunchAgents/com.tokensave.daemon.plist && rm ~/Library/LaunchAgents/com.tokensave.daemon.plist`
- Linux: `systemctl --user disable --now tokensave-daemon && rm ~/.config/systemd/user/tokensave-daemon.service`
- Windows: `sc.exe delete tokensave-daemon` (from an elevated terminal)

Once your agent is attached, the MCP server keeps the index fresh on its own.

---

## Checking Your Setup with Doctor

The `doctor` command runs a comprehensive health check:

```bash
tokensave doctor
```

It verifies:

- **Binary** — location and version
- **Current project** — whether a `.tokensave/` index exists and the database is healthy
- **Global database** — the cross-project database at `~/.tokensave/global.db`
- **User config** — `~/.tokensave/config.toml` (plus machine-local `state.toml`) and upload settings
- **Agent integrations** — MCP server registration, hook installation, tool permissions, prompt rules
- **Network** — connectivity to the worldwide counter and GitHub releases API

If any tool permissions are missing after an upgrade, doctor will tell you to run `tokensave install` again.

To check only a specific agent:

```bash
tokensave doctor --agent claude
tokensave doctor --agent codex
tokensave doctor --agent kiro
```

The accepted agent values are the same values supported by `tokensave install --agent`.

---

## Finding Affected Tests

When you change source files, you often want to know which tests might be affected. The `affected` command traces through the file dependency graph to find them.

```bash
tokensave affected src/main.rs src/db/connection.rs
```

This performs a breadth-first search from the changed files through import/dependency edges to find test files that directly or transitively depend on those files.

### Piping from git

This is especially useful in CI pipelines:

```bash
git diff --name-only HEAD~1 | tokensave affected --stdin
```

### Options

```bash
tokensave affected src/lib.rs --depth 3         # limit traversal depth (default: 5)
tokensave affected src/lib.rs --filter "*_test.rs"  # custom test file pattern
tokensave affected src/lib.rs --json             # JSON output
tokensave affected src/lib.rs --quiet            # just file paths, no decoration
```

---

## MCP Tools for AI Agents

When running as an MCP server, tokensave exposes more than 80 tools that AI agents can call. The most commonly used are grouped below by purpose; run `tokensave tool` for the complete list with one-line descriptions.

### Core exploration

| Tool | What it does |
|------|-------------|
| `tokensave_context` | Given a task description, returns relevant symbols, relationships, and code snippets. This is the go-to starting point for any coding task. |
| `tokensave_search` | Find symbols by name. Supports filtering by kind (function, class, method, etc.), or `literal: true` for an exact-substring scan of file contents. |
| `tokensave_node` | Get full details for a specific symbol: source code, location, complexity metrics, and relationships. |
| `tokensave_files` | List indexed files, optionally filtered by directory or glob pattern. |
| `tokensave_status` | Index statistics: file counts, symbol counts, language distribution, and tokens saved. |
| `tokensave_annotations` | Attribute/annotation/decorator introspection: histogram of all annotations in the project, or per-site listings filtered by name, file, or target kind. |
| `tokensave_doc` | Companion Markdown documentation for a source file: the doc's content, every file it covers, and whether the code changed after the doc was last touched. Often answers the question without reading the file at all. |
| `tokensave_dependencies` | Package-manifest introspection across 17 ecosystems: workspace summary with license surface and version drift, per-package lookup, and per-member listings. |

#### Literal search: finding strings, not symbols

`tokensave_search` with `literal: true` is a different question from the
default: an exact-substring, case-sensitive scan of file *contents*, which
finds text that lives inside function bodies and never appears in a symbol
name -- a runtime error message, a feature-flag key, a route. Each hit is
reported as `file` / `line` / `text`, plus the innermost symbol enclosing it
(`enclosing: null` where there is none).

It reads bytes and needs no parser, but it scans the files the index holds, so
its reach is the index's reach. A file gets indexed when a language extractor
handles its extension **or** when the extension is listed in
`artifact_extensions` (see the artifacts section in `README.md`). A tracked
`.html` template or `.css` stylesheet is usually neither, and templates and
stylesheets are exactly where a flag's user-facing label or a CSS class
actually lives -- so add those extensions to `artifact_extensions` and run
`tokensave sync -f` if you want "where is this string used" answered across
them.

When a literal search could not reach every tracked file, the response carries
an `unscanned` block giving the number of files and a per-extension breakdown,
so a partial answer is never mistaken for a complete one:

```json
{
  "literal": true, "query": "someFlag", "count": 1,
  "matches": [ { "file": "config/index.js", "line": 1, "text": "..." } ],
  "unscanned": {
    "files": 2,
    "extensions": [ { "extension": "html", "files": 1 },
                    { "extension": "css", "files": 1 } ],
    "reason": "The index holds no row for these tracked files, ...",
    "remedy": "... add its extension to `artifact_extensions` ..."
  }
}
```

Files you excluded yourself are not reported there: config `exclude` globs,
project query-ignore rules, and the call's own `path_include`/`path_exclude`
and scope prefix all apply to the report as well as to the scan, so it names
only files you asked about and the index could not answer for.

### Navigating relationships

| Tool | What it does |
|------|-------------|
| `tokensave_callers` | Find what calls a given function or method. Configurable traversal depth. |
| `tokensave_callees` | Find what a function or method calls. |
| `tokensave_impact` | Trace the full blast radius of changing a symbol — everything that could be affected. |
| `tokensave_affected` | Find test files affected by source file changes. |
| `tokensave_similar` | Find symbols with similar names (useful for naming patterns or related code). |
| `tokensave_rename_preview` | Preview all references to a symbol before renaming it. |

### Code quality analysis

| Tool | What it does |
|------|-------------|
| `tokensave_dead_code` | Find unreachable symbols — functions with no callers. Symbols that are a candidate for an ambiguous call are excluded, since "maybe called" is not "uncalled". |
| `tokensave_ambiguous_calls` | List call sites where several targets tied and no edge was created, with each candidate's name, kind, file, and line. |
| `tokensave_unused_imports` | Find import statements that are never referenced. |
| `tokensave_circular` | Detect circular file dependencies. |
| `tokensave_recursion` | Detect recursive and mutually-recursive call cycles. |
| `tokensave_complexity` | Rank functions by composite complexity score, including cyclomatic complexity from the AST. |
| `tokensave_god_class` | Find classes with the most members — candidates for decomposition. |
| `tokensave_hotspots` | Find the most connected symbols (highest call count). These are high-risk areas. |
| `tokensave_doc_coverage` | Find public symbols missing documentation. |
| `tokensave_simplify_scan` | Quality analysis of changed files: duplications, dead code, complexity, coupling. |

### Health & quality signals

| Tool | What it does |
|------|-------------|
| `tokensave_health` | Composite quality signal (0–10000) from five structural dimensions (acyclicity, depth, equality, redundancy, modularity) with a low-weight penalty for `/// skip-test-coverage` overuse. The single number to track over time. |
| `tokensave_gini` | Gini inequality coefficient for any metric (complexity, lines, fan-in, fan-out, members). Finds god files and uneven distributions. |
| `tokensave_dependency_depth` | Longest file-level dependency chains — the critical paths where upstream changes ripple through the most layers. |
| `tokensave_dsm` | Design Structure Matrix showing file dependencies as clusters, density stats, or an NxN grid. Reveals hidden coupling patterns. |
| `tokensave_test_risk` | Risk-weighted test gaps combining complexity, coupling, git churn, and test coverage. Answers "where should the next test go?" |

### Test Coverage Conventions

#### `/// skip-test-coverage`

Mark functions that are genuinely untestable in unit tests (e.g. infrastructure-dependent, framework-invoked, or private helpers tested only transitively):

```rust
/// skip-test-coverage
pub async fn produce(&mut self, topic: &str, batch: Bytes) -> io::Result<i64> { ... }
```

Marked functions are excluded from `tokensave_test_risk` coverage calculations, giving you an accurate picture of testable-code coverage. The `skipped` count appears in the summary so you can track how many functions use the annotation.

**Health penalty:** The `coverage_discipline` dimension (visible in `tokensave_health` and `tokensave_session_start`/`session_end`) penalises overuse. Each skipped function lowers the score proportionally — a few genuine exclusions have negligible impact, but marking 50%+ of your codebase as untestable will visibly reduce your quality signal. This encourages using the annotation for its intended purpose rather than as a way to game coverage numbers.

### Structural analysis

| Tool | What it does |
|------|-------------|
| `tokensave_module_api` | Public API surface of a file or directory. |
| `tokensave_coupling` | Rank files by coupling (fan-in or fan-out). |
| `tokensave_inheritance_depth` | Find the deepest class inheritance hierarchies. |
| `tokensave_type_hierarchy` | Recursive type hierarchy tree for traits, interfaces, and classes. |
| `tokensave_distribution` | Node kind breakdown (classes, methods, fields) per file or directory. |
| `tokensave_rank` | Rank nodes by relationship count (most-implemented interface, most-extended class, etc.). |
| `tokensave_largest` | Rank nodes by size — largest classes, longest methods. |

### Git-aware tools

| Tool | What it does |
|------|-------------|
| `tokensave_diff_context` | Semantic context for changed files: modified symbols, dependencies, and affected tests. |
| `tokensave_changelog` | Semantic diff between two git refs — which symbols were added, removed, or modified. |
| `tokensave_commit_context` | Semantic summary of uncommitted changes, useful for drafting commit messages. |
| `tokensave_pr_context` | Semantic diff between git refs for pull request descriptions. |
| `tokensave_test_map` | Source-to-test mapping at the symbol level, with uncovered symbol detection. |
| `tokensave_test_coverage` | Per-file/symbol/test-fn coverage rollup with transitive call-edge expansion: which tests cover a symbol, what a test exercises, or a whole-file tested/untested summary. |

### Porting tools

| Tool | What it does |
|------|-------------|
| `tokensave_port_status` | Compare symbols between source/target directories to track cross-language porting progress. |
| `tokensave_port_order` | Topological sort of symbols for porting — tells you what to port first based on dependencies. |

### Session management

| Tool | What it does |
|------|-------------|
| `tokensave_session_start` | Save current health metrics as a baseline before starting work. |
| `tokensave_session_end` | Compare current health against the baseline to detect structural degradation during the session. |

Discovery and analysis tools are read-only and safe to call in parallel. Session baseline tools write/remove `.tokensave/session_baseline.json`, memory-recording tools update the project database, and edit tools modify source files.

---

## Supported Languages

Tokensave supports more than 50 languages, organized into three tiers. Each tier includes all the languages from the tier below it. See the README for the full table with file extensions and feature flags.

### Lite

Always compiled. The smallest binary for the most popular languages, plus Svelte and Astro (script-block extraction via the TypeScript extractor).

Rust, Go, Java, Scala, TypeScript, JavaScript, Python, C, C++, Kotlin, C#, Swift, Svelte, Astro

### Medium (Lite + 9)

Adds scripting, config, and additional systems languages.

Dart, Pascal, PHP, Ruby, Bash, Protobuf, PowerShell, Nix, VB.NET

### Full (Medium + everything else, the default)

Everything: legacy, niche, shader, and document languages.

ActionScript, Lua, Zig, Objective-C, Perl, Batch/CMD, Fortran, COBOL, MS BASIC 2.0, GW-BASIC, QBasic, QuickBASIC 4.5, Dockerfile, GLSL, WGSL, HLSL, Metal, Markdown, R, SQL, Julia, Haskell, OCaml, Clojure, Erlang, Elixir, F#, F*, Quint, TOML, Lean

### Mixing individual languages

You can also cherry-pick individual languages without taking a full tier:

```bash
cargo install tokensave --no-default-features --features lang-nix,lang-bash
```

### What gets extracted

For each supported language, tokensave extracts:

- Function and method definitions (with signatures)
- Class, struct, trait, interface, and enum definitions
- Fields and properties
- Import and export statements
- Call relationships and type references
- Docstrings and annotations
- Complexity metrics (branches, loops, returns, max nesting, cyclomatic complexity)
- Cross-file dependency edges

---

## Privacy and Network

Tokensave's core functionality is 100% local. Indexing, search, graph queries, and the MCP server all run on your machine against a local database. No API keys are needed.

There are two optional network calls.

### Worldwide token counter

Tokensave tracks how many tokens it has saved you. During `sync` and `status`, it uploads that count (a single number like `4823`) to an anonymous worldwide counter. No code, file names, project names, or identifying information is sent. The Cloudflare Worker also logs the country derived from your IP for aggregate geographic statistics — your actual IP is not stored.

This powers the "Worldwide" counter shown in `tokensave status`.

**To opt out:**

```bash
tokensave disable-upload-counter
```

When disabled, tokensave never uploads your count but still fetches and displays the worldwide total. Re-enable at any time:

```bash
tokensave enable-upload-counter
```

### Version check

Tokensave checks GitHub for new releases so it can show you an upgrade notice. This is a single GET request to the GitHub API with no identifying information. It has a 1-second timeout and failures are silently ignored. This check cannot be disabled, but it never blocks your workflow.

---

## Updating Tokensave

When a new version is available, tokensave tells you during `sync` and `status`:

```
Update available: v3.3.3 -> v3.4.0
  Run: tokensave upgrade
```

The `upgrade` command downloads the latest release from GitHub and replaces the binary in place:

```bash
tokensave upgrade
```

Beta and stable are separate update channels — a beta build only sees beta releases and vice versa. Any attached MCP servers will continue running with the previous binary until you restart your agent.

If other tokensave processes (usually MCP servers) are running, `upgrade` lists them and asks whether to kill them first. Pass `--kill` to terminate them without being asked:

```bash
tokensave upgrade --kill
```

When nothing else is running, no prompt appears. In non-interactive runs the upgrade continues without killing anything unless `--kill` is given. The launcher that started the current `tokensave` process (a Scoop shim, a shell, or any other ancestor) is never listed as a killable process, even if it's itself named `tokensave`/`tokensave.exe` — only unrelated tokensave processes are candidates. If a requested kill (via `--kill` or answering `y`) cannot signal every listed process, the upgrade aborts instead of proceeding; stop the remaining process(es) manually and retry.

You can also update through your package manager:

```bash
brew upgrade tokensave          # Homebrew
scoop update tokensave          # Scoop
cargo install tokensave         # Cargo
```

Upgrades are zero-touch: you normally do **not** need to re-run `install` or `sync --force` by hand. Tokensave compares the version that last ran against the running one and performs exactly the maintenance that transition requires — refreshing every registered agent's config on a minor or major bump, and rebuilding project indexes on a major one. That refresh is silent; it will not print install output in front of your next `init` or `sync`. See [TOKENSAVE-VERSIONING.md](../TOKENSAVE-VERSIONING.md) for the full rules.

The two cases where you should still step in:

```bash
tokensave install      # after a "could not refresh tokensave config" or stale-install warning
tokensave sync --force # to rebuild an index you suspect is wrong
```

---

## Configuration Files

Tokensave stores data in two places.

### Per-project: `.tokensave/`

Created inside each project you index. Contains:

- `tokensave.db` — the libSQL database with all symbols, edges, files, and vector embeddings

Add `.tokensave` to your `.gitignore`.

#### Production paths named like test directories

If a production route or feature directory has a name such as `test`, add a
`source_path_overrides` glob to the generated `.tokensave/config.json`:

```json
"source_path_overrides": ["components/test/**"]
```

The override affects test/source classification only; the files are indexed
either way. Explicit test markers such as `__tests__/`, `*.test.*`, and
`*.spec.*` still count as tests inside an overridden path. Restart the
tokensave MCP server after editing the config; no re-index is required.

### Optional: `.tokensave/project.json` — explicit index entries

Alongside `config.json` (walker policy: excludes, size limits, gitignore), you can add a `project.json` manifest listing files or globs to index explicitly, each with an optional `language` override that forces a specific extractor:

```json
{
  "version": 1,
  "entries": [
    { "path": "homedir/.bash_profile", "language": "bash" },
    { "path": "homedir/.bashrc.d/*.shrc", "language": "bash" },
    { "path": "~/.bash_aliases", "language": "bash" }
  ]
}
```

This solves two problems that extension-based dispatch cannot:

- **Extensionless or oddly-named files.** `.bash_profile`, `.bashrc`, `*.shrc` and friends are valid Bash but have no `.sh`/`.bash` extension, so they are normally skipped. A manifest entry makes them indexable and the `language` override picks the extractor. This works for any language, not just shell.
- **Files outside the project root.** Absolute and `~/…` paths (glob-capable) opt in external files — for example your real dotfiles under `$HOME` while the git project only documents them. External files are stored in the graph under their resolved absolute path.

Semantics:

- Entries are **additive**: the normal project walk still happens; `project.json` never removes files. `config.json` excludes and `max_file_size` still apply to in-project entries.
- `language` accepts any supported language name (case-insensitive; a registered extension like `sh` also works). An unknown language fails the sync with an error listing the valid names.
- Hidden (dot-prefixed) paths matched by an entry are walked automatically — no separate `include` glob needed.
- External paths are opt-in and project-local; only add paths you trust, since their content is parsed and indexed.

### Companion Markdown documentation

A large file often has a short prose explanation next to it. Tokensave indexes
those explanations so an agent can read the summary instead of the 3000-line
class. Two conventions are discovered, and both work at once:

- **Sidecar** — `BigClass.cs` next to `BigClass.readme.md`. No configuration:
  the doc is matched by filename and travels with the code in review.
- **Docs directory** — `tokensave-docs/` at the project root, where each
  Markdown file declares which files it covers with an `applies_to` glob list
  in YAML front matter. One doc can cover a whole family of files:

  ```markdown
  ---
  applies_to:
    - "**/*.es8.cs"
    - "src/legacy/**/*.cs"
  ---

  These files target the ES8 runtime; prefer the ES7 variants for new work.
  ```

Rename or relocate the directory with `docs_dir` in `.tokensave/config.json`,
or disable docs-directory discovery entirely by setting it to an empty string
(sidecar discovery is unaffected):

```json
"docs_dir": "architecture-notes"
```

Retrieve a file's documentation with `tokensave_doc`, which returns the doc
path, its content, every file that doc covers, and a `doc_stale` signal (true
when the covered code was committed after the doc; `null` when there is no git
history to compare against). `tokensave_entities` also reports `has_doc` and
`doc_path`, so an agent can see that a summary exists *before* deciding to read
the file. Docs whose globs match nothing are dropped rather than indexed, and
unparseable front matter degrades to "covers nothing" instead of failing the
sync. Section-level anchors (line ranges, `m:MethodName`) are not supported
yet — granularity is whole-doc.

### Per-user: `~/.tokensave/`

Created in your home directory. Contains:

- `config.toml` — stable user preferences. This is the only file meant to be
  version-controlled (e.g. checked into a dotfiles repo).
- `state.toml` — machine-local, frequently-changing state: cached version/pricing/flag
  info and check timestamps (harmless to lose), but also `pending_upload`
  (tokens accumulated locally but not yet uploaded — deleting it forfeits
  that count) and `installed_agents` (this machine's install bookkeeping —
  deleting it can cause the next upgrade to treat agents as not installed).
  Regenerated automatically on every run, but don't delete it casually and
  don't track it in a dotfiles repo — it churns on almost every run.
- `global.db` — cross-project database that tracks tokens saved across all your projects

The `config.toml` is plain TOML and fully transparent:

```toml
upload_enabled = true        # set to false to stop uploading
watcher_debounce = "2s"      # inert; left over from the watcher removed in 6.1.1
extraction_timeout_secs = 60 # per-file extraction timeout
wildcard_permissions = false # true = grant Claude Code tools via one "mcp__tokensave__*" entry
```

`state.toml` holds everything else (`pending_upload`, `last_upload_at`,
`cached_latest_version`, `installed_agents`, and similar cached/timestamp
fields). If you're upgrading from a version that only had `config.toml`, the
state fields are read from your existing file once and then migrated into
`state.toml` automatically on the next save — no data is lost.

---

## Troubleshooting

### "tokensave not initialized"

The `.tokensave/` directory doesn't exist in your current project. Run:

```bash
tokensave init
```

### MCP server not connecting

Your AI agent doesn't see tokensave tools.

1. Run `tokensave doctor` to check the integration
2. Verify `tokensave` is on your PATH: `which tokensave`
3. Re-run `tokensave install` and restart your agent completely

### Missing symbols in search

Some symbols aren't showing up.

- Run `tokensave sync` to update the index
- Check that the language is supported (see the tiers above)
- Verify the file isn't being skipped by `.gitignore` (`tokensave gitignore` to check)

### Indexing is slow on first run

The initial full index of a large project can take a few seconds. This is normal. Use `tokensave sync --verbose` to see which phase is taking the longest.

- Subsequent syncs are incremental and much faster
- Use `tokensave sync` (not `--force`) for day-to-day updates
- The post-commit hook syncs in the background so it never blocks you; the MCP server's staleness walk runs inline on a tool call, at most once every 30 seconds

### Stale install warning

If you see a warning about your install being stale after an upgrade, run:

```bash
tokensave install
```

This updates tool permissions, hooks, and prompt rules to match the new version.

### "could not refresh tokensave config for: ..."

After an upgrade you may see:

```
warning: could not refresh tokensave config for: copilot.
  Run tokensave install to see the error.
```

The automatic post-upgrade refresh could not write one agent's config. The usual causes are an agent that is registered but no longer installed, or a config file in a read-only or centrally-managed location. Everything else was refreshed, and tokensave will not retry that path on every subsequent command — run `tokensave install` when convenient to see the specific error, or `tokensave uninstall --agent <name>` to stop tracking an agent you no longer use.

### "agent config references Cargo build output"

Installing from a source build persists the exact binary you invoked, which is
usually what you want — but when that binary lives under `target/debug`,
`target/release`, or `target/<triple>/{debug,release}`, `cargo clean` or
deleting the worktree leaves your hooks and MCP entries pointing at a file that
no longer exists. Tokensave now names the path when this happens:

```
warning: agent config references Cargo build output:
  /repo/target/debug/tokensave
  `cargo clean` or removing its worktree will break tokensave hooks and MCP servers.
  Re-run `tokensave install` from a stable `cargo install`, Homebrew, or release binary.
```

Nothing is rewritten for you — substituting some other binary found on `PATH`
could silently install an older version than the one you chose. Re-run
`tokensave install` from a `cargo install`ed, Homebrew, or downloaded release
binary when you want the configured path to be durable.

### Getting help

If you run into something not covered here, check the [GitHub repository](https://github.com/aovestdipaperino/tokensave) or open an issue.
