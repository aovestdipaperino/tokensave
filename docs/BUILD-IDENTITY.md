# Compiled application identity

`build/identity.json` defines the stable and beta executable names, product names,
and Windows application IDs. `build/identity.rs` supplies compile-time values to
the application and embeds beta Windows resources. No runtime configuration file
is loaded. Stable builds retain the existing executable name, default Windows
icon/resources, CLI identity, and version.

| Identity | Stable | Beta |
| --- | --- | --- |
| Windows executable | `tokensave.exe` | `TokenSave-Beta.exe` |
| Linux/macOS executable | `tokensave` | `tokensave-beta` |
| Product name | TokenSave | TokenSave Beta |
| Windows application ID | Existing default | `TokenSave.TokenSave.Beta` |
| Version example | `7.16.0` | `7.16.0-beta.1` |

## Build and package

The helper uses the existing Cargo build, features, lockfile, and profiles:

```sh
python3 scripts/build-app.py --channel stable
python3 scripts/build-app.py --channel beta
python3 scripts/build-app.py --channel beta --version 7.16.0-beta.2
python3 scripts/build-app.py --channel beta --target x86_64-pc-windows-msvc
```

Outputs are under `target/<channel>/[<target>/]release/`, using the executable
names above. Separate Cargo build directories preserve independent outputs.
Use `--lite` for fewer language parsers, `--profile dev` for debug builds, or
`--target-dir` to override the isolated build directory deliberately.

`cargo build --features beta` also selects beta identity, while retaining Cargo's
internal `tokensave` target name. Use the helper for the named distributable.
For `cargo install --path . --features beta`, rename the installed executable to
the beta name before placing it alongside stable. The beta Homebrew recipe does
this itself. A plain stable Cargo invocation needs no new flags.

A prerelease package version also selects beta identity without the feature.
The helper rejects `--channel stable` for a prerelease package rather than
packaging a beta build under a stable filename.
Otherwise the beta feature appends the configured `beta.1` suffix to the package
version. `--version` passes `TOKENSAVE_BETA_VERSION` to the compiler and accepts
`<package-base>-beta.N`, where N is a nonnegative integer with no leading zeros.
The helper clears inherited overrides for both channels. Direct Cargo invocations
must unset the override for stable builds; the build rejects an override without
a beta feature or prerelease package version. Compiled version reporting,
maintenance markers, MCP version reporting, and update checks all use that same
compiled version rather than reporting a stable version for a beta executable.

The existing beta release workflow packages these executables in its existing
archive names, bottles, and Scoop manifest. It passes the release tag as the beta
version and keeps the existing checksum workflow. Its disabled trigger and job
guards are preserved. This change does not publish or enable beta releases.

## Icons and operating-system identity

The beta icon preserves the existing treasure chest, gems, and pixel-art colors,
with a high-contrast beta badge. `src/resources/tokensave-beta.png` is its source
asset; the ICO contains 16, 20, 24, 32, 40, 48, 64, 128, and 256 pixel variants.
The original logo and stable resources are unchanged.

The README uses a separate wide beta wordmark, `src/resources/logo-beta.png`,
matching the original logo's 688 × 267 pixel canvas and the same 300 pixel
README display width. It retains the chest, wordmark, and decorative elements
with a beta badge, while the square application icon remains suited to small
operating-system icon sizes. Keep this README branding difference on beta only.

Windows beta executables embed the icon, `TokenSave Beta` product and file
description, complete prerelease file/product version strings, original filename,
and the Windows prerelease flag. Numeric version fields use the package's base
version because Windows requires numeric components there. The build fails if
the resource compiler fails, rather than silently shipping without the identity.

On Windows, beta sets its explicit application ID and temporarily sets an
interactive console title to `TokenSave Beta`, restoring the prior title on
normal completion and handled errors. Piped CLI/MCP operation does not change
the console title or emit terminal escape sequences. Terminal hosts can control
their own tab titles and taskbar icons; an executable's icon and application ID
cannot force a host such as Windows Terminal to display the executable's icon.

TokenSave is a CLI with no GUI window, macOS application bundle, or Linux desktop
launcher. Linux and macOS distinguish the executable/process name and CLI version;
this change does not introduce GUI bundles or launchers merely to attach an icon.

## Compatibility

Settings, data directories, graph formats, update asset names, release channels,
and update installation destinations are unchanged. Both binaries can coexist
on PATH with different names, but continue to share settings and project graphs.
Agent registration continues to use existing integration behavior; installing
one channel can replace the shared agent registration. Separate filenames do
not provide isolated settings or simultaneous independent agent registrations.

The updater recognizes the renamed executables and accepts old beta archives
containing `tokensave`/`tokensave.exe`. Stable archive lookup continues to require
the stable filename. Older updaters that only recognize the old beta archive
member name must first update to a build with this support, or install the named
beta executable manually. Channel switching replaces the existing installed path
as before; it does not rename the installation or change update identifiers.

## Validation

```sh
cargo test --locked --no-default-features --features lite --lib identity::tests
cargo test --locked --no-default-features --features lite,beta --lib identity::tests
cargo test --locked --no-default-features --features lite --lib upgrade::tests
python3 scripts/build-app.py --channel beta --profile dev --lite
python3 scripts/check-app-identity.py --channel beta target/beta/debug/tokensave-beta
```

On Windows use `TokenSave-Beta.exe` in the final command. The smoke check also
inspects Windows Properties version fields and icon availability using PowerShell.
CI runs stable and beta identity checks on Linux, macOS, and Windows after the
existing stable test suite. Native taskbar, Explorer small-icon rendering, and
terminal-host title behavior still require visual checks on each supported host.

Resource impact is limited to compile-time parsing/resource generation and a
bounded Windows startup title buffer (64 KiB, released on completion). No title
buffer is allocated for non-console operation. There are no new queues, caches,
background workers, database queries, or polling loops.

## Implementation validation record

Tracking issue: [#801](https://github.com/aovestdipaperino/tokensave/issues/801).
Implementation: [PR #802](https://github.com/aovestdipaperino/tokensave/pull/802).
The pull request records the current merge status and the beta-only merge restriction.

Initial validation on 2026-10-26, before rebasing the implementation onto upstream
`beta` (these full-suite counts describe the former master-based tree):

- `cargo test --workspace --locked` with `TMPDIR=/var/tmp`: 4,431 passed,
  4 ignored across 16 harnesses. The initial run under `/tmp` encountered two
  discovery-test failures caused by an existing `/tmp/.git` directory; all
  discovery tests and the complete suite passed with the alternate fixture root.
- Lite beta identity unit tests: 2 passed.
- Real compiled Linux CLI smoke checks: stable reports `tokensave 7.16.0`;
  staged beta reports `tokensave-beta TokenSave Beta 7.16.0-beta.2`.
- `cargo clippy --workspace --all-targets --locked`: passed with existing
  warnings on unchanged code. Formatting, diff whitespace, Python syntax, and
  the ICO's nine image sizes passed checks.
- A native x86-64 Windows probe compiled the exact shared build identity,
  resources, and console-identity code. Windows version fields, original
  filename, prerelease flag, and icon extraction passed for `7.16.0-beta.1`.
  This probe excludes the application's unrelated parsers and CLI implementation;
  complete Windows/macOS builds remain covered by the updated CI workflow.

After rebasing onto upstream `beta`, the lite beta library suite passed:
1,120 tests passed and 4 were ignored, including identity, archive compatibility,
SQL-wrapper diagnostics, and SQL-debug settings tests. The rebased SQL-debug CLI
regression checks are recorded in the pull request. The former full-suite count
above does not represent a full-workspace run on the rebased beta tree.

Native taskbar/Explorer appearance, interactive title restoration, ARM64 Windows,
and macOS rendering have not been manually validated. No SQL queries changed.

The upstream master `AGENTS.md` baseline was checked; shared instructions and
beta-specific SQL-debugging guidance were retained, with new build guidance added. Shared README and branch-specific installation
rules are preserved; this change does not synchronize or alter other branches.
