# TokenSave Versioning

tokensave version numbers look like SemVer (`MAJOR.MINOR.PATCH`) but **deliberately do not follow it**. SemVer's contract — "major = breaking API change, minor = backwards-compatible feature, patch = fix" — exists for libraries that other code links against. Nothing links against the tokensave binary, so that contract would encode nothing useful.

Instead, each version component encodes **the maintenance the new binary must perform on its first launch after the update**:

| Bump | Example | What the update requires | What tokensave does automatically |
| --- | --- | --- | --- |
| **Patch** (`x.y.Z`) | `7.2.0 → 7.2.1` | Nothing | Advances the recorded version marker; no reinstall, no reindex |
| **Minor** (`x.Y.0`) | `7.2.0 → 7.3.0` | A reinstall (new harnesses, new tools, new hooks, new config) | Silently re-runs `install` for every registered agent integration |
| **Major** (`X.0.0`) | `7.2.0 → 8.0.0` | A reinstall **and** a full resync | Global reinstall, plus a per-project forced reindex (`sync --force` equivalent) |

## Why diverge from SemVer

Because the version delta alone tells the binary what to do after an update, upgrades are **zero-touch**: the user never runs `tokensave reinstall` or `tokensave sync --force` by hand after upgrading. On launch, tokensave compares the last version that ran against the running version, classifies the transition as patch/minor/major, and performs exactly the maintenance that tier implies — no manifest, no changelog parsing, no "post-install steps" in release notes.

The cost is that the components stop meaning what SemVer readers expect. A release that only adds features may still be a **major** release if it changes the database in a way that requires a full resync, and a large feature release that touches neither agent config nor the database ships as a **patch**. The component is chosen by *required maintenance*, not by feature size or API compatibility.

## How it works

Three version markers drive the behavior:

- `previous_version` (machine-local state, `~/.tokensave/state.toml`) — set by `tokensave upgrade` / `tokensave channel` just before replacing the binary. On the next launch, a minor or major transition from it triggers the silent global reinstall (`agents::resync_installed_agents`, driven from `src/main.rs` startup maintenance). A patch transition just advances the marker.
- `last_installed_version` (machine-local state) — the version that last ran `install`/`reinstall`. This is the fallback for **external upgrades** (`brew upgrade tokensave`, `cargo install tokensave`) that bypass `tokensave upgrade`: if the running binary is newer than this marker, the global reinstall runs regardless of bump size (a reinstall is cheap; guessing wrong is not).

  Both markers advance once the resync has run, **even if some agents failed to install**. A config path that can't be written (an app that isn't installed, a read-only or managed location) fails identically on every attempt, so retrying it forever would re-run the resync on every single command instead of once per upgrade. The failures are reported once as a `warning: could not refresh tokensave config for: <agents>` line pointing at `tokensave install` for the underlying error.
- `last_indexed_version` (per-project config) — the version that indexed the project. On the **first MCP tool call** in a project, the transition from it is classified by `bump_kind` (`src/cloud.rs`): a **major** bump spawns a background forced full reindex that never blocks the tool response, then records the running version. A project with no recorded version (created before this mechanism existed) is treated as major so it backfills.

Beta and stable are separate channels: transitions never cross them, and a cross-channel version pair classifies as "no action".

## The database schema has its own version

Independent of the release version, the on-disk schema is versioned by `LATEST_VERSION` in `src/db/migrations.rs`, stored per-database in `PRAGMA user_version`. Reindex detection is **schema-driven, not release-version-driven**: on the first MCP tool call, a project whose stored schema version is older than the running build's `LATEST_VERSION` gets the forced background reindex *regardless of which release component changed*. This is why a migration-safe schema change may ship in a patch or minor release (as schema v7 did in 4.3.9, v9 in 5.1.1, v11 in 6.4.5, and v12/v13 in 7.2.0).

## Maintainer rules for cutting a release

Pick the component by the maintenance the update requires, not by feature size:

- **Patch** — the new binary works with the existing agent config and existing project databases as-is. Bug fixes, performance work, output changes, even sizable features, as long as no harness/config/schema surface moved.
- **Minor** — the update needs the agent integrations refreshed: new MCP tools, new hooks, new permissions, changed harness config. The silent reinstall handles it.
- **Major** — the update needs project databases rebuilt from scratch: a schema change that a forward migration plus background reindex cannot absorb, one that breaks older binaries reading the new database, or one that alters the meaning of existing data.

For any schema change (new table, column, index, trigger, or FTS surface): bump `LATEST_VERSION` by one and add a sequential entry to `run_migration` — that is the mechanism of record. Migrations only run forward; never renumber or edit a shipped migration — repair mistakes with a new version (the way schema v13 recreates the trait-dispatch cache some v12 databases were missing).
