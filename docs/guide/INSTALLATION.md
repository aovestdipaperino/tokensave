# Installation and Upgrades

How to install tokensave, choose a language tier, upgrade, and build from source.

## Installing

Choose one installation method:

| Platform | Command |
|---|---|
| Any platform (Cargo) | `cargo install tokensave`<br>or, with cargo-binstall, `cargo binstall tokensave` |
| macOS (Homebrew) | `brew install aovestdipaperino/tap/tokensave` |
| Windows (Scoop) | `scoop bucket add tokensave https://github.com/aovestdipaperino/scoop-bucket`<br>`scoop install tokensave` |

You can also download a [prebuilt binary](https://github.com/aovestdipaperino/tokensave/releases/latest) and add it to your `PATH`.

### Installing from upstream master

To install the latest unreleased code directly from the upstream `master` branch instead of the published crates.io release:

```bash
cargo install --git https://github.com/aovestdipaperino/tokensave.git --branch master --locked --force
```

This builds the current `master` branch from source. It may include changes not yet published in a stable release and requires the Rust toolchain.

### Language tiers and release archives

The default Cargo build includes all supported languages. For a smaller build:

```bash
cargo install tokensave --no-default-features --features medium
cargo install tokensave --no-default-features    # lite (smallest binary)
```

Prebuilt release archives:

| Platform | Archive |
|---|---|
| macOS (Apple Silicon) | `tokensave-vX.Y.Z-aarch64-macos.tar.gz` |
| Linux (x86_64) | `tokensave-vX.Y.Z-x86_64-linux.tar.gz` |
| Linux (ARM64) | `tokensave-vX.Y.Z-aarch64-linux.tar.gz` |
| Windows (x86_64) | `tokensave-vX.Y.Z-x86_64-windows.zip` |

See [Supported Languages](LANGUAGES.md) for the languages in each tier and for
installing individual languages.

After installing, connect your agent with `tokensave install` (see
[Agent Integrations](AGENT-INTEGRATIONS.md)) and index a project with
`tokensave init` (see [Indexing and Freshness](INDEXING.md)).

## Trying the beta

The [beta branch](https://github.com/aovestdipaperino/tokensave/tree/beta) has new,
improved queries, including optimized search ranking, graph analysis, and
indexing cleanup. See the [beta changelog](https://github.com/aovestdipaperino/tokensave/blob/beta/CHANGELOG.md)
for details.

Install it from source:

```bash
cargo install --git https://github.com/aovestdipaperino/tokensave.git --branch beta --locked --force
```

For builds with isolated beta storage, follow the
[database compatibility guide](../DATABASE-COMPATIBILITY.md) when initializing beta
or returning to stable. Existing shared databases are preserved; beta uses a fresh
index under `.tokensave/beta/`.

## Upgrading

Run `tokensave upgrade --dry-run` to see the installation evidence and the proposed action. For a managed install, `tokensave upgrade` prints instructions for the original installer. Use Cargo with the original source, features and install root, Homebrew with the TokenSave tap, or Scoop in the original install scope. The command makes no changes to managed binaries or package records.

If you installed a standalone archive from GitHub Releases, run `tokensave upgrade --standalone`. TokenSave verifies the running binary against its published release and checks the replacement archive against the release checksums. Binaries that cannot be verified can be updated manually with their original installer or release channel.

```bash
tokensave channel                        # show the active channel
tokensave channel beta --standalone      # switch a verified standalone installation to beta
tokensave channel stable --standalone    # return a verified standalone installation to stable
```

Standalone upgrades stay on their current channel. To change channels for a managed installation, use a channel supplied by its installer or install the requested channel separately.

## Versioning and upgrades

tokensave version numbers look like SemVer but **do not follow it**: the component that changes encodes the maintenance the update requires, which tokensave performs automatically on the next launch — you never run a reinstall or reindex by hand.

| Bump | Example | Update requires | Automatic action |
| --- | --- | --- | --- |
| **Patch** (`x.y.Z`) | `7.2.0 → 7.2.1` | Nothing | None — no reinstall, no reindex |
| **Minor** (`x.Y.0`) | `7.2.0 → 7.3.0` | A reinstall (new harnesses, new tools, new config) | **Global reinstall** of every installed agent integration (refreshes permissions, hooks, and MCP config) |
| **Major** (`X.0.0`) | `7.2.0 → 8.0.0` | A reinstall + full resync | Global reinstall **and** a per-project forced reindex (`sync -f` equivalent) |

**Global reinstall.** On the first run of a new minor or major build, tokensave silently re-runs `install` for each agent it has registered, so the agent config always points at the current binary and exposes the current tool set. Patch bumps skip this — the running version marker is simply advanced.

The reinstall is genuinely silent: the per-agent setup output you see from an explicit `tokensave install` is suppressed here, so it never appears in front of an ordinary `tokensave init` or `tokensave sync`. If an agent's config can't be refreshed — the app isn't installed, or its config lives somewhere read-only — you get one line naming the agents that failed:

```
warning: could not refresh tokensave config for: copilot.
  Run tokensave install to see the error.
```

Run `tokensave install` to see the underlying error. The version markers advance either way, so a config path that can never be written is reported once per upgrade rather than retried on every subsequent command.

**Per-project forced reindex (major only).** A major bump means project indexes must be rebuilt. tokensave does this lazily and per project: on the **first MCP tool call** in a project after a major upgrade, it spawns a background full reindex (equivalent to `tokensave sync --force`) that never blocks the tool response.

**Brew / cargo fallback.** External upgrades that replace the binary outside of `tokensave upgrade` — `brew upgrade tokensave` or `cargo install tokensave` — are detected the same way: if the running version is newer than the last version that performed an install, the reinstall runs on the next launch just as it would after a self-upgrade.

See [TOKENSAVE-VERSIONING.md](../../TOKENSAVE-VERSIONING.md) for why tokensave diverges from SemVer (encoding maintenance in the version is what makes zero-touch upgrades possible), the marker mechanics, the independent database schema version, and the maintainer rules for cutting releases.

### Upgrading from 5.x

The standalone `tokensave daemon` command and its launchd/systemd/Windows Service autostart were removed in 6.0.0. The embedded OS-level file watcher that replaced the daemon was itself removed in 6.1.1 (it caused runaway CPU and memory on large monorepos with deep `node_modules` or `target` trees). The [on-demand staleness model](INDEXING.md#index-freshness) is the current design; a bounded, [opt-in filesystem watcher](INDEXING.md#opt-in-filesystem-watcher) can be added to it and is off by default.

If you still have a daemon autostart from 5.x, remove it:

- macOS: `launchctl unload ~/Library/LaunchAgents/com.tokensave.daemon.plist && rm ~/Library/LaunchAgents/com.tokensave.daemon.plist`
- Linux: `systemctl --user disable --now tokensave-daemon && rm ~/.config/systemd/user/tokensave-daemon.service`
- Windows: `sc.exe delete tokensave-daemon` (from an elevated terminal)

If you don't recall the exact name: `launchctl list | grep tokensave` / `systemctl --user list-units | grep tokensave` / `sc.exe query state= all | findstr -i tokensave`.

## Uninstalling

Removing tokensave takes up to three steps, and each one leaves the others' data alone:

| Command | Removes | Keeps |
| --- | --- | --- |
| `tokensave uninstall` | Agent integrations (MCP entries, approvals, hooks, rules) and tokensave's global git hooks | Indexes, settings, caches, usage history |
| `tokensave purge` | Everything `uninstall` removes, plus tokensave's settings, caches and indexes in the chosen scope | Usage history (unless `--include-history`) |
| Your installer | The `tokensave` executable | — |

`purge` always prints its plan first: every path it will delete, why it recognizes the path as tokensave's, its category and its approximate size, plus what it keeps or skips and why. `--dry-run` prints that plan and changes nothing. A real run asks you to type `purge` to confirm. When stdin is not a terminal it refuses unless you pass `--yes`. `--yes` skips only the question; the ownership checks below still apply.

```bash
tokensave purge --dry-run                 # global state: integrations, global git hooks, ~/.tokensave settings and caches
tokensave purge --local --dry-run         # the project containing the current directory
tokensave purge --project ~/src/app       # one project (repeat --project for several)
tokensave purge --all-projects            # global state and every project in the registry
tokensave purge --all-projects --include-history   # ...and the token-savings history
```

**Scopes.** Without a scope flag, purge acts on global state only: agent integrations, global git hooks, and the settings and caches in `~/.tokensave` (`config.toml`, `state.toml`, the benchmark and pricing caches, telemetry rings, the server registry and profiler data). Project indexes stay. `--local` and `--project <PATH>` act on projects only: the project's index directory, its project-scoped agent config and tokensave's section of its own git hooks. `--all-projects` acts on global state and on every project in the global registry, listing each one in the plan. Registry paths are checked like paths you type, and a project whose directory no longer exists is skipped (`tokensave state prune` removes relocated state left behind by such projects).

**History.** The global database (`global.db`) holds your token-savings history and cost records, which cannot be rebuilt. Purge keeps it unless you pass `--include-history`, which is only accepted with the global scope.

**What purge never deletes.**

- Project source files and repositories. Only the state directory tokensave creates (`.tokensave/`, or `<base>/projects/<id>` when [state is relocated](INDEXING.md#keeping-state-outside-the-repository)) is removed, and only when it holds a tokensave index.
- Files it does not recognize. A file in `~/.tokensave` that tokensave does not create is listed as skipped and left in place.
- Anything behind a symbolic link or junction. Purge does not follow them, and it checks each path again just before deleting it.
- Other tools' settings in shared agent config files and git hooks tokensave did not write. Integration cleanup is scoped to tokensave entries and hook sections.
- The beta channel's state, from a stable build, and the reverse. Settings in `~/.tokensave` are shared by both channels, so only a stable build deletes them.
- The `tokensave` executable.

Deleting `config.toml` returns every setting to its default, including an opt-out of the worldwide counter. Set `TOKENSAVE_TELEMETRY=0` or run `tokensave disable-upload-counter` afterwards to keep it off. Indexes come back the usual way, with `tokensave init`.

**Failures.** Purge reports each path it deleted, found already gone, or could not delete, and exits with status 1 if anything failed. On Windows a running MCP server keeps its database open, so close the agents that use tokensave (`tokensave servers` lists them) and run the same command again; it acts only on what is left. A refused or aborted run exits with status 2 and changes nothing.

**Previewing integration cleanup.** Run `tokensave uninstall --dry-run` to list the files and operations in the selected scope. Add `--agent NAME` to target one integration or `--local` to target the current directory. Global Git hooks are included only when uninstalling all global integrations. Pass `--keep-git-hooks` to keep them. The command reports removed content, missing files, skipped files and failures. `--verbose` adds paths to routine progress, and `--quiet` keeps errors and the executable removal reminder. After a failure, resolve the reported problem and run the command again. It exits with status 1 when any requested cleanup fails.

**Removing the executable.** Both `uninstall` and `purge` end by saying where the running executable is. They name the removal command when installer records match the running binary. Homebrew detection requires the TokenSave tap's keg receipt, Scoop detection requires a matching app manifest, and Cargo detection uses its installed-package manifest. For unknown installs, use your original installer or remove the executable if you copied it yourself.

A successful global `tokensave install` records verified provenance in `~/.tokensave/installation.json` when available. Schema version 1 stores the method, canonical executable path, file length and modification timestamp. `tokensave --version --verbose` reports the current method and evidence. An outdated record or an unrecognized schema can be replaced by rerunning `tokensave install` after a verified reinstall. With no matching record, detection falls back to current installer receipts.


## Building from source

```bash
cargo build --locked --release                          # full (50+ languages, default)
cargo build --locked --release --features medium        # medium tier
cargo build --locked --release --no-default-features    # lite (smallest binary)

cargo test --workspace --locked                         # run all tests (requires full)
cargo check --locked --no-default-features              # verify lite compiles
cargo clippy --workspace --all-targets --locked
```

Development commands use debug builds; installation uses optimized release output
only (`cargo install --path . --locked`). `target/debug` is never an installation input.

For local development on Linux and macOS, the optional
[`scripts/dev-build` runner](../../CONTRIBUTING.md#optional-blaze-for-local-builds-and-tests)
prefers Blaze when it is installed and usable, with Cargo fallback.
`scripts/dev-build --cargo test --workspace --locked` explicitly selects Cargo;
`TOKENSAVE_BUILD_RUNNER=cargo` selects it for a shell session. Windows keeps
Cargo. CI, releases, installation and final PR validation use the Cargo commands
above. See the [Blaze evaluation](../BLAZE-EVALUATION.md) for setup, command
selection, compatibility coverage and measurements.
