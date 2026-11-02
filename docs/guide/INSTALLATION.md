# Installation and Upgrades

How to install tokensave, choose a language tier, upgrade, and build from source.

## Installing

This is the `beta` branch. Beta builds are not published to Homebrew, Scoop,
crates.io or the releases page, so `brew install`, `cargo install tokensave`
and the prebuilt binaries install the latest stable release instead. Build the
beta from source with the Rust toolchain:

```bash
cargo install --git https://github.com/aovestdipaperino/tokensave.git --branch beta --locked --force
```

Or clone the branch and build it yourself (the repository's pinned Rust
toolchain is picked up automatically):

```bash
git clone --branch beta --single-branch https://github.com/aovestdipaperino/tokensave.git tokensave-beta
cd tokensave-beta
cargo build --release --locked
```

The binary is `target/release/tokensave` (`tokensave.exe` on Windows); put it
on your `PATH`.

The beta includes database schema migrations that trigger a full reindex on
first use, so try it on a disposable checkout first. Builds with isolated
storage require a fresh beta initialization under `.tokensave/beta/`; see the
[database compatibility guide](../DATABASE-COMPATIBILITY.md) before
transitioning existing indexes. To go back to stable, reinstall with one of the
commands in the [`master` installation guide](https://github.com/aovestdipaperino/tokensave/blob/master/docs/guide/INSTALLATION.md#installing).

### Installing from upstream master

To install the latest unreleased code directly from the upstream `master` branch instead of the published crates.io release:

```bash
cargo install --git https://github.com/aovestdipaperino/tokensave.git --branch master --locked --force
```

This builds the current `master` branch from source. It may include changes not yet published in a stable release and requires the Rust toolchain.

### Language tiers and release archives

The default Cargo build includes all supported languages. For a smaller beta
build:

```bash
BETA=https://github.com/aovestdipaperino/tokensave.git
cargo install --git $BETA --branch beta --locked --force --no-default-features --features medium
cargo install --git $BETA --branch beta --locked --force --no-default-features    # lite (smallest binary)
```

No prebuilt beta archives are published. Prebuilt binaries on the
[releases page](https://github.com/aovestdipaperino/tokensave/releases/latest)
are stable releases only.

See [Supported Languages](LANGUAGES.md) for the languages in each tier and for
installing individual languages.

After installing, connect your agent with `tokensave install` (see
[Agent Integrations](AGENT-INTEGRATIONS.md)) and index a project with
`tokensave init` (see [Indexing and Freshness](INDEXING.md)).

## Trying the beta

You are reading the beta documentation. The beta's additions over the stable
release are listed in the [beta changelog](https://github.com/aovestdipaperino/tokensave/blob/beta/CHANGELOG.md);
install it as described in [Installing](#installing).

## Self-upgrade

```bash
tokensave upgrade                  # upgrade to latest in current channel
tokensave channel                  # show current channel (stable/beta)
tokensave channel beta             # switch to beta channel
tokensave channel stable           # switch back to stable
```

`tokensave upgrade` downloads the correct platform binary from GitHub releases and replaces the running binary in place. Supports stable and beta channels independently.

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

The standalone `tokensave daemon` command and its launchd/systemd/Windows Service autostart were removed in 6.0.0. The embedded OS-level file watcher that replaced the daemon was itself removed in 6.1.1 (it caused runaway CPU and memory on large monorepos with deep `node_modules` or `target` trees). The [on-demand staleness model](INDEXING.md#index-freshness) is the current design.

If you still have a daemon autostart from 5.x, remove it:

- macOS: `launchctl unload ~/Library/LaunchAgents/com.tokensave.daemon.plist && rm ~/Library/LaunchAgents/com.tokensave.daemon.plist`
- Linux: `systemctl --user disable --now tokensave-daemon && rm ~/.config/systemd/user/tokensave-daemon.service`
- Windows: `sc.exe delete tokensave-daemon` (from an elevated terminal)

If you don't recall the exact name: `launchctl list | grep tokensave` / `systemctl --user list-units | grep tokensave` / `sc.exe query state= all | findstr -i tokensave`.

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
