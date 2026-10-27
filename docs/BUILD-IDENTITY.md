# Compiled application identity

`build/identity.json` defines the stable and beta executable names, product names,
and Windows application IDs. `build/identity.rs` supplies compile-time values to
the application and embeds beta Windows resources. No runtime configuration file
is loaded. Stable builds retain the existing executable name, default Windows
icon/resources, CLI identity, and version.

| Identity | Stable | Beta |
| --- | --- | --- |
| Windows executable | `tokensave.exe` | `tokensave.exe` |
| Linux/macOS executable | `tokensave` | `tokensave` |
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

Both channels build the single Cargo binary target, named `tokensave`. The helper
prints Cargo's executable path without copying or renaming it. Separate build
directories allow both outputs to coexist; installing either channel into the
same bin directory replaces the other. A plain stable Cargo invocation needs
no new flags.

The beta branch keeps a prerelease package version in `Cargo.toml` and
`Cargo.lock`. Plain `cargo install --git https://github.com/aovestdipaperino/tokensave.git --branch beta --locked --force`
therefore reports a beta package version and selects beta identity without the
feature. Cargo still installs its internal `tokensave` target name; verify the
compiled identity with `tokensave --version` and `tokensave channel`.

A prerelease package version also selects beta identity without the feature.
The helper rejects `--channel stable` for a prerelease package rather than
packaging beta metadata as a stable build.
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
launcher. Linux and macOS distinguish the channels through the compiled CLI version;
this change does not introduce GUI bundles or launchers merely to attach an icon.

## Compatibility

Settings, data directories, graph formats, update asset names, release channels,
and update installation destinations are unchanged. Both channels use the same
executable filename and share settings, project graphs, and agent registration.
Use separate installation directories if both builds must be retained.

The updater accepts canonical archive members and legacy beta members
(`tokensave-beta` / `TokenSave-Beta.exe`). Channel switching replaces the existing
installed path without renaming it. Beta archive asset names, Homebrew formula
names, and Scoop package identifiers retain their channel suffix; their contained
executable is always `tokensave` / `tokensave.exe`.

## Validation

```sh
python3 scripts/test-build-app.py
cargo test --locked --no-default-features --features lite --lib identity::tests
cargo test --locked --no-default-features --features lite,beta --lib identity::tests
cargo test --locked --no-default-features --features lite --lib upgrade::tests
python3 scripts/build-app.py --channel beta --profile dev --lite
python3 scripts/check-app-identity.py --channel beta target/beta/debug/tokensave
```

On Windows use `tokensave.exe` in the final command. The smoke check also
inspects Windows Properties version fields and icon availability using PowerShell.
CI checks the full test build on Linux, macOS, and Windows. Prerelease packages
reuse that beta executable for resource checks; stable packages additionally
build the beta feature with the same full feature set. Native taskbar, Explorer small-icon rendering, and
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

### Plain beta source installation labeling ([#810](https://github.com/aovestdipaperino/tokensave/issues/810))

The follow-up to PR #802 sets the beta package and lockfile version to
`7.16.0-beta.1`. No new identity implementation or feature flag is needed:
the existing prerelease detection now applies to ordinary beta source installs.
This is a direct beta commit; no follow-up PR or stable backport is intended.
Validation results are recorded below. Runtime resource use and SQL are unchanged.

Upstream master guidance was compared at `f82ce87`. Beta retains its executable
identity and SQL-debugging instructions. Master's Pi test commands are absent
because those test scripts are not present on beta. README installation and
branding differences remain intentional.

Validation on 2026-10-26:

- Locked Cargo metadata reports `7.16.0-beta.1`; no beta feature was enabled.
- Plain lite Cargo build and both identity unit tests passed.
- The compiled CLI reports `tokensave-beta TokenSave Beta 7.16.0-beta.1`;
  `channel` reports `v7.16.0-beta.1 (beta)`. The new CI smoke check passed locally.
- Lite library suite with `TMPDIR=/var/tmp`: 1,120 passed, 4 ignored. The first
  run under `/tmp` had three discovery failures from existing parent repository
  state; the clean fixture-root run passed.
- Formatting and diff whitespace checks passed. Full workspace, release-profile,
  Windows, and macOS checks were not rerun for this metadata change. CI retains
  its full workspace test matrix and now verifies plain Cargo identity against
  the package version, in addition to packaged beta identity.


### Canonical executable filename ([#836](https://github.com/aovestdipaperino/tokensave/issues/836))

This local follow-up supersedes the executable naming behavior of PR #802 and
the CLI prefix shown in the historical validation records above. Both channels
now report the command name `tokensave`; beta retains its product name, version,
icon, application ID and Windows prerelease resources. The helper performs one
Cargo build with no executable copy. Release archives and package manifests use
the canonical member name while retaining beta asset/package identifiers.
Legacy archive extraction and process-name recognition remain supported.

The beta CI job reuses the full test executable, removing the separate lite beta
compilation without removing identity or PE resource checks. Stable-package CI
retains an explicit beta-feature build. No SQL or runtime resource lifecycles
changed. No release is published by this change.

Upstream master guidance was checked on 2026-10-27. Beta-only identity and SQL
debugging guidance remain here; master's Pi test commands refer to files absent
on beta and are not copied. README content is unchanged.

Validation on 2026-10-27:

- Full locked workspace suite with `test-transport`: 4,543 passed,
  4 ignored across 16 harnesses. Fixtures used `TMPDIR=/var/tmp`:
  the initial default-root run had two unrelated discovery failures caused by
  the existing `/tmp/.git` directory.
- Full workspace/all-target Clippy with `test-transport` passed with warnings
  in unchanged code. Formatting, diff whitespace, Python syntax and workflow
  YAML parsing passed.
- Compiler-free build-helper tests passed for both channels and Windows,
  Linux and macOS target paths: one Cargo application build, one canonical
  executable, no duplicate output. Cargo still defines one application target.
- The real Linux executable passed the identity smoke check and reported
  `tokensave TokenSave Beta 7.16.0-beta.1`; `channel` reported
  `v7.16.0-beta.1 (beta)`.
- Canonical/legacy tar extraction passed locally. A corresponding Windows ZIP
  regression is included for CI. Native Windows PE checks, native macOS builds,
  release-profile archives and package-manager installation were not executed
  locally; the existing cross-platform checks remain in CI.
- Resource review: no new runtime allocations, retained collections, workers,
  queries or polling. The build helper removes an executable copy and beta CI
  avoids an additional compilation. No RSS or long-running runtime workload was
  measured because runtime resource ownership is unchanged.

Implementation commit: [db4813c](https://github.com/aovestdipaperino/tokensave/commit/db4813c),
placed directly on `beta` at the maintainer's request. No follow-up PR was used;
there is no PR merge commit for this change. The temporary implementation branch
is removed. Issue #836 holds the completion summary and closure record. This
change does not publish a release.

### Canonical-output CI follow-up (Refs #836)

The first CI run after direct beta delivery passed Rust tests on Linux, macOS
and Windows, but the Python output check failed on macOS and Windows. The test
kept unresolved temporary roots while the helper resolved paths: macOS expanded
`/var` to `/private/var`, and Windows expanded the runner's short username.
Resolve the test root before constructing metadata and expected outputs.

Local validation on 2026-10-27: both Python tests passed with ordinary temporary
paths and with a symlinked temporary root that reproduced all six failing
subcases before the fix. Diff whitespace checks passed. Native macOS/Windows
confirmation remains with CI; no Rust or runtime behavior changed. This
follow-up is delivered directly to beta without a PR or release.

Latest upstream master guidance was compared before this follow-up. Remaining
differences are beta identity/query-debugging policy and master's Pi test
commands for files absent on beta; shared instructions need no changes.
