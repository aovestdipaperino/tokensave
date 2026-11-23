# Rust dependency audit

This audit addresses [#1061](https://github.com/aovestdipaperino/tokensave/issues/1061)
against upstream master at `d4a4cc4b`, with Rust 1.100.0 and a declared MSRV of
1.95.0. Cargo's `available:` messages report resolver alternatives. The
Windows `E0463` for `std` or `core` concerns the local toolchain or target.

## Inventory and compatible update

The [published 7.17.0 archive](https://static.crates.io/crates/tokensave/tokensave-7.17.0.crate)
contains a normalized manifest and a lockfile with 51 direct dependency
entries and 463 registry package records. The master lockfile has 53 direct
dependency entries across target conditions and development/build dependencies,
and 448 registry package records. These counts include optional packages and
multiple versions of the same crate. The archive locks Tree-sitter at 0.26.9.
The install transcript selected 0.26.13.

Use these commands from the repository root to regenerate the inventory and
inspect a package's immediate dependents:

```sh
cargo tree --locked --workspace --target all --all-features
cargo tree --locked --target all --all-features --invert equivalent --depth 1
cargo update --dry-run
```

The compatible-update preview proposed 135 package selections in the audit
environment. Several would change native parsers, runtime libraries, or
proc-macro dependencies. This PR selects `equivalent` 1.0.3 (from 1.0.2).
It is pulled in by `indexmap` and `hashbrown`. The published Rust source
diff consists of trait documentation clarifications, and its dependency list is unchanged. Its Rust 1.6 minimum is below
TokenSave's MSRV. Review of `itoa` 1.0.19 found new unchecked buffer writes
in integer formatting, so this audit retains 1.0.18 pending a focused review.
The selected updates leave the parser, database engine, and indexing and
search algorithms unchanged. This audit uses no timing measurements.

## Candidates from the installation output

The candidate releases below were verified against the crates.io sparse index.
All seven packages are direct dependencies of TokenSave on master.

| Crate | Published 7.17.0 lock | Master lock | Candidate | Decision and validation needed |
| --- | --- | --- | --- | --- |
| [bincode](https://docs.rs/crate/bincode/3.0.0/source/src/lib.rs) | 1.3.3 | 1.3.3 | 3.0.0 | Defer. The release contains a deliberate compile error. A codec replacement must update both ends of the extraction worker protocol, preserve bounded framing and error handling, and test malformed and maximum-size frames. Compare encoded size and worker throughput. |
| [crossterm](https://docs.rs/crate/crossterm/0.29.0/source/CHANGELOG.md) | 0.28.1 | 0.28.1 | 0.29.0 | Defer to a monitor-focused change. The release changes key-modifier display and moves to rustix 1.x. Verify key handling, terminal cleanup, and rendering on native Windows and Unix terminals. |
| [gix](https://github.com/aovestdipaperino/tokensave/pull/1085) | 0.81.0 | 0.90.0 | 0.90.0 | Already on master via #1085. The selected release declares Rust 1.88. The existing faster-hex/heapless follow-up is tracked by [#1059](https://github.com/aovestdipaperino/tokensave/issues/1059). |
| [libsql](https://docs.rs/crate/libsql/0.10.0/source/Cargo.toml) | 0.9.30 | 0.9.30 (vendored) | 0.10.0 | Defer. The release also changes native dependency versions. Verify the vendored double-close fix and the core-only feature set first. Check existing database opens, migrations, WAL handling, and transaction behavior. Benchmark indexing and database workloads before replacing the engine. |
| [sysinfo](https://docs.rs/crate/sysinfo/0.40.4/source/CHANGELOG.md) | 0.32.1 | 0.32.1 | 0.40.4 | Defer. The candidate declares Rust 1.95 and changes OS-specific process and memory behavior, including Linux cgroup reporting. Validate parent PID detection, process liveness, hardware profiles, and telemetry on native targets. Measure sampling cost. |
| [tree-sitter](https://docs.rs/crate/tree-sitter/0.27.2/source/include/tree_sitter/api.h) | 0.26.9 | 0.26.9 | 0.27.2 | Defer both 0.26.13 and 0.27.2 to a parser-focused change. 0.27.2 retains the advertised grammar ABI range 13–15, but the grammar bundles and standalone grammars also constrain the Rust crate version. Check unified resolution, run the extraction matrix and native sanitizer evaluation, and compare indexing and search on the same pinned corpus. |
| [dirs](https://docs.rs/crate/dirs/7.0.0/source/src/lib.rs) | 6.0.0 | 6.0.0 | 7.0.0 | Defer. Verify the resolved home, config, and data paths on native Windows, macOS, and Linux, including discovery of existing installations. |

## Security and release verification

[#960](https://github.com/aovestdipaperino/tokensave/issues/960) has already
landed its security changes, including selecting the local-only libsql feature
set and upgrading the Windows zip dependency. The PR's first RustSec run flagged `cookie` 0.18.1, also present in the master
baseline, under [RUSTSEC-2026-0594](https://rustsec.org/advisories/RUSTSEC-2026-0594.html).
The patch updates the locked version to 0.18.3, the first fixed release,
without changing other package selections or the existing audit policy.
`ureq` lists `cookie` as optional. The all-target, all-feature workspace tree
has no enabled path to it.

`cargo install --locked tokensave --version 7.17.3` checks the published
stable package. A published install verifies that release's lockfile. The
candidate branch is covered by the repository's Linux, macOS, and Windows
test matrix, and its eventual release package can be checked with the
[packaged release smoke workflow](RELEASE-SMOKE.md).
