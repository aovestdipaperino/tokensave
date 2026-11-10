# Memory safety and soundness audit

- **Issue:** [#947](https://github.com/aovestdipaperino/tokensave/issues/947)
- **Date:** 2026-11-10
- **Branches audited:** `master` at `251a055` and `beta` at `cf03ec1`. Except
  where a finding is marked *beta only*, the audited code is the same on both
  branches. This file is kept identical on both.
- **Toolchains:** Rust 1.98.1 on `master` and Rust 1.99.0 on `beta`, each
  pinned in `rust-toolchain.toml`. Both use edition 2021 and the default
  `panic = "unwind"` in every profile.
- **Background:** Microsoft FORGE Lab,
  [*Memory Safety Bugs in Rust Standard Library*](https://forge-blog.github.io/posts/rust-std-memory-safety/)

The FORGE research groups standard-library soundness bugs into three patterns:

1. code that assumes an operation cannot panic;
2. code that trusts a user-provided trait implementation or callback to uphold
   an invariant;
3. code that trusts process-wide or OS-managed state to stay stable.

This report checks TokenSave's own code against each pattern. It also checks
whether any Rust issue cited in the article can be reached from TokenSave. The
findings are scoped to the paths listed below. They are not a claim that
TokenSave or its dependencies are sound.

## Summary

| Area | Result |
|---|---|
| `unsafe` in TokenSave code | On both branches: 9 sites in 3 files, made up of 3 grammar FFI declarations and 6 `memmap2` mappings. Beta only: 3 more Win32 console calls in `src/app_identity.rs`. Each site has a `SAFETY:` comment. There is no unchecked indexing, `transmute`, `static mut`, raw-pointer arithmetic or manual initialization. On beta, one console-title buffer needed a NUL-termination hardening, which is made in the beta change. |
| Pattern 1, panic safety | No partially initialized state is visible during unwinding. `safe_extract` is the only `catch_unwind`. No actionable finding. |
| Pattern 2, untrusted trait implementations | No `unsafe` code depends on a trait implementation's result. The one custom `Ord` is a total order. No actionable finding. |
| Pattern 3, OS and process state | **One confirmed test-only defect, fixed on both branches:** `tests/incremental_resolution_test.rs` mutated `TOKENSAVE_FULL_RESOLVE` while sibling tests synced concurrently. Production code never mutates the environment. |
| Upstream Rust issues in the article | None reachable from TokenSave code. #161018 is unfixed in master's pinned 1.98.1, but TokenSave never triggers it. Beta's 1.99.0 includes the fix. Details are in the [table below](#upstream-rust-issues). |

## Inventory of `unsafe` code

Found by searching `src/`, `tests/`, `benches/`, `examples/` and `build.rs`
for `unsafe`. Hits in comments, string literals and complexity-metric code
that only *counts* `unsafe` blocks in indexed source are excluded.

| Location | Operation | Invariant | Assessment |
|---|---|---|---|
| `src/extraction/ts_provider.rs` (`wgsl_grammar`, `actionscript_grammar`, `gdscript_grammar`) | `unsafe extern "C"` declarations and `LanguageFn::from_raw` | The C symbol is the generated tree-sitter entry point: no arguments, returns a static `TSLanguage` pointer. | Sound if the vendored grammar matches its declaration. `build.rs` compiles the grammars only when the feature is on, and the module has the same `cfg`. Grammar C code is not memory-safe. That risk is contained by worker-process isolation, see [FFI and native libraries](#ffi-and-native-libraries). |
| `src/monitor.rs` `write_entry_inner`, `MmapReader::open_at`, `MmapReader::refresh` | `memmap2::MmapMut::map_mut` and `Mmap::map` of `~/.tokensave/monitor.mmap` | The file is never truncated while mapped. Writers hold `lock_exclusive`. | TokenSave only grows the file to `FILE_SIZE`. Readers check bounds against `mmap.len()` and copy bytes out. A concurrent writer can cause at most a torn entry. |
| `src/memstats.rs` writer, `open_at`, `purge_dead_slots` | Same `memmap2` mappings for the memory-stats table | Same as above. | Same as above. |
| `src/app_identity.rs` `ConsoleIdentity` (*beta only*, Windows) | `SetCurrentProcessExplicitAppUserModelID`, `GetConsoleTitleW` and `SetConsoleTitleW`, with the old title restored in `Drop` | Strings passed in are NUL-terminated UTF-16 that outlive the call. The buffer restored in `Drop` is NUL-terminated. | The input strings are built with `.chain(Some(0))`. `GetConsoleTitleW` returns the title length without its NUL. The original code then truncated to `length + 1`, which does nothing when the reported length is the full title and not what fit in the 32,768-unit buffer. The result relied on the API terminating a truncated title. **Hardened on beta:** the last element is now forced to NUL after truncation, so `Drop` can never read past the buffer. No crash was reproduced. This is defense in depth against an unusually long or failed title read. |

**Residual mmap risk (accepted, documented).** `memmap2` mappings are only as
safe as the file behind them. If something outside TokenSave truncates the
file while it is mapped, a write or read through the mapping faults with
`SIGBUS` on Unix or an access violation on Windows. Examples are a user
running `truncate` or a sync tool replacing the file. TokenSave cannot prevent
another program from doing this. The file is per-user telemetry under the home
directory, not a security boundary. The `monitor.rs` writer indexes the
mapping without a length check after `set_len(FILE_SIZE)`. That is correct
under the stated invariant, because the file is never shrunk and writers are
serialized.

## Pattern 1: panic and unwind safety

- **No partially initialized state.** TokenSave has no `MaybeUninit`,
  `set_len` on a `Vec`, `ptr::write` or manual `Drop` that relies on
  initialization progress. A panic therefore cannot expose uninitialized
  memory in TokenSave code, which is the shape of #156501.
- **`catch_unwind`.** The only use is `safe_extract` in
  `src/tokensave/extract.rs`. It wraps `LanguageExtractor::extract`, which
  takes `&self` and returns an owned `ExtractionResult`. No extractor in
  `src/extraction/` holds `RefCell`, `Cell`, `Mutex` or thread-local state, so
  a panic leaves no shared extractor state behind, and `AssertUnwindSafe` is
  justified. The result of a panicking extraction is discarded and the file is
  skipped. Unwinding cannot catch C `abort()` calls from grammars, so worker
  subprocess isolation (`src/extraction_worker.rs`) remains the primary
  defense, as `AGENTS.md` already requires.
- **Database consistency after errors and panics.** The insert phases of a
  sync are not one transaction. An error, panic or cancellation partway
  through can leave some files updated and others not. That is the same state
  a crash leaves. A dirty sentinel covers it: the index stays marked stale and
  the next sync redoes the work, as described for `check_partial` in
  `src/cancel.rs`. This partial state is a logical inconsistency, not a memory
  safety issue: TokenSave has no `unsafe` cleanup in the database layer, and
  SQLite rolls back any transaction left open. This audit did not add
  panic-injection tests for the database layer. Crash recovery is covered by
  [DATABASE-RECOVERY.md](DATABASE-RECOVERY.md).
- **Allocation failure.** TokenSave uses the default global allocator, with no
  `#[global_allocator]` and no `allocator_api`. Allocation failure calls
  `handle_alloc_error`, which aborts instead of unwinding. The
  allocator-unwinding issues (#156490, #159334, #159368, #159370) therefore do
  not apply.

## Pattern 2: untrusted trait implementations and API contracts

- **No `unsafe` code consumes a trait result.** The only `unsafe` operations
  are FFI declarations and mmap creation. Neither uses a length, index or
  pointer obtained from `AsRef`, `Borrow`, `Deref`, `Clone`, `Ord` or an
  iterator.
- **Custom trait implementations.** TokenSave implements no `Borrow`, `Deref`
  or `AsRef` that `std` could call twice and get different answers. The only
  custom `Ord` is `RankedPair` in
  `src/mcp/tools/handlers/redundancy.rs`. It is a total order: `f64::total_cmp`
  on the score, then the `(a, b)` index tuple. An inconsistent `Ord` would
  make `std` sorting panic on Rust 1.81 and later, not cause undefined
  behavior. That cannot happen with this implementation.
- **String and byte handling.** TokenSave has no `from_utf8_unchecked`,
  `get_unchecked` or other `*_unchecked` call. Byte offsets from tree-sitter
  are applied with checked slicing. Code that cuts strings for display walks
  to a `char` boundary first, for example `spinner_tail` in `src/main.rs`.
  A wrong offset therefore panics, and the extractor's panic boundary catches
  it. It cannot create an invalid `str`. The mmap readers decode with
  `String::from_utf8_lossy`.
- **`Pattern` and `str::replace` issues.** Implementing `Pattern` requires the
  unstable `pattern` feature. TokenSave builds on stable and uses only the
  standard pattern types (`&str`, `char`, closures), so #156491, #158265,
  #165076 and #164904 do not apply.

## Pattern 3: OS, FFI and process-wide state

### Environment variables

- **Production code never calls `set_var` or `remove_var`.** Child processes
  get their environment through `Command::env`, which changes only the child's
  environment. This includes the worker token `TOKENSAVE_WORKER_TOKEN` and the
  git and diagnostic subprocesses.
- **Tests.** Every test file that mutates the environment is a separate
  `[[test]]` target, as [CONTRIBUTING.md](../CONTRIBUTING.md#test-layout)
  requires. Inside such a target, `cargo test` still runs the file's tests
  concurrently on threads of one process. Targets with more than one test must
  therefore serialize their environment changes.
  - `autosync_budget_test`, `omp_agent_test`, `opencode_agent_test` and
    `pi_agent_test` already serialize with a lock.
  - `global_git_hooks_*_test`, `rebuild_lock_test`,
    `local_git_hooks_shadow_test` and `tls_root_fallback_test` each contain a
    single test.
  - **Defect (fixed):** `incremental_resolution_test` has four tests on
    `master` and two on `beta`. One toggled `TOKENSAVE_FULL_RESOLVE` around
    `sync()` without a lock, while the others called `init`/`sync` on other
    threads. Every sync reads that
    variable (`incremental_resolution_enabled` in
    `src/tokensave/indexing.rs`), so a sibling test could run in full-resolve
    mode. On `master`,
    `an_untouched_call_loses_its_stale_edge_when_a_new_target_makes_it_ambiguous`
    exists to test the incremental path, so it could then pass without
    exercising it. On glibc, `setenv` racing a `getenv` from native code is
    also a data race. Rust's own `env::var` calls take the standard library's
    environment lock, so this race involves native code only. This change
    holds a file-wide lock in every test of the file and sets the variable
    through a drop guard, so a failing assertion cannot leave it set. The impact is
    limited to test reliability. It does not affect shipped binaries.

### Process exit and signals

- **`process::exit`.** It is called from `main`, from command functions that
  `main` runs directly, from `exit_after_serve`, and from the
  `extract-worker` entry point. All of these run on the main thread:
  `#[tokio::main]` runs the main future on the main thread through
  `block_on`. No spawned thread or task calls `exit`, and no `Drop` or
  thread-local access calls it either.
- **Signals.** On Unix, `install_signal_handlers` in `src/cancel.rs` uses
  `signal_hook::low_level::pipe::register`. The handler only writes a byte to
  a self-pipe, which is async-signal-safe, and a dedicated thread translates
  that byte into the cancellation flag. On Windows, `tokio::signal::ctrl_c` is
  used. Registration runs once, behind a `Once`.
- **`exec`.** TokenSave never replaces its own process image. The only
  `CommandExt` use is `std::os::windows::process::CommandExt` for creation
  flags in `src/upgrade.rs`.

### FFI and native libraries

- **libSQL/SQLite.** The known double-close use-after-free in libsql 0.9.30 is
  already patched in `vendor/libsql` (#367), and
  `tests/vendored_libsql_test.rs` guards that patch against drift. TokenSave
  uses only the safe libsql API.
- **tree-sitter grammars.** These are about 50 C and C++ grammars plus
  external scanners, linked statically. Extraction runs them in authenticated
  worker subprocesses, so a crash or `abort()` in a scanner costs one file,
  not the indexer. `.cargo/config.toml` builds the vendored grammars with
  `-DNDEBUG`. Running the grammar C code under sanitizers is outside this
  audit.
- **File watchers.** TokenSave has none. Staleness is checked on demand.
- **File locks.** `fs2` advisory locks are released when the file is
  dropped, including during unwinding.

## Upstream Rust issues

| Issue | Pattern | Affected API | Fixed in | TokenSave status |
|---|---|---|---|---|
| [#156501](https://github.com/rust-lang/rust/issues/156501) | 1 | `Iterator::map_windows` clone (nightly feature) | closed 2026-06 | **Not applicable.** Nightly-only, not used. |
| [#156490](https://github.com/rust-lang/rust/issues/156490), #159334, #159368, #159370 | 1 | Allocation-dependent collections with an unwinding allocator | open / various | **Not applicable.** Default global allocator, which aborts on failure. No `allocator_api`. |
| [#156491](https://github.com/rust-lang/rust/issues/156491) | 2 | `str::replace` with a custom `Pattern` | 1.97.0 (PR #156556) | **Not applicable.** `Pattern` cannot be implemented on stable. The pinned 1.98.1 includes the fix. |
| #158265, #165076, #164904 | 2 | `Pattern` searchers (`strip_prefix`, `split_once`, `split`) | open | **Not applicable.** Only the standard pattern types are used. |
| [PR #155708](https://github.com/rust-lang/rust/pull/155708) (`Borrow`/`join` family, CVE-2020-36323) | 2 | `[T]::join` with a misbehaving `Borrow` | 1.97.0 | **Not applicable.** No custom `Borrow`. `join` is used on `String` and `&str`. The pinned 1.98.1 includes the fix. |
| [#161018](https://github.com/rust-lang/rust/issues/161018) | 3 | `process::exit` from a non-main thread unmaps the main thread's alternate signal stack (Unix) | 1.99.0 (PR #161037) | **Not reachable.** Master's pinned 1.98.1 does **not** include the fix, but TokenSave calls `exit` only from the main thread. Recheck this if `exit` is ever called from a spawned thread or a blocking task. Beta's pinned 1.99.0 includes the fix. |
| [#156951](https://github.com/rust-lang/rust/issues/156951) | 3 | `CommandExt::exec` writes `environ` under a read lock (Unix) | open (PR #157144) | **Not applicable.** No `exec`. |
| [#164243](https://github.com/rust-lang/rust/issues/164243) | 3 | `process::exit` while a thread-local is in an inconsistent state (double free) | open (PR #165125) | **Not reachable.** It requires calling `exit` from inside a thread-local mutation or a `Drop` that runs during one. TokenSave's thread-locals in `src/profile/` hold plain values and `RefCell` contexts, and nothing in their scope calls `exit`. |

## Tooling and verification

| Check | Result |
|---|---|
| Source search for `unsafe`, `*_unchecked`, `transmute`, `static mut`, `catch_unwind`, `set_var`, `remove_var`, `process::exit`, `exec` and `CommandExt` | Results are in the sections above. |
| `cargo test --locked --no-default-features --features lite --test incremental_resolution_test` (Windows) | `master` (Rust 1.98.1): 4 passed, 0 failed, with the fix. `beta`: see the beta pull request. |
| `rustfmt --check` on the changed test | Clean. |
| `cargo clippy` | **Not run locally.** The change adds no `unsafe` code and touches one test file. CI's Clippy job runs on it. |
| `cargo audit` / RustSec | **Not run.** `cargo-audit` is not installed in the audit environment, and CI has no RustSec job. Recommended follow-up: add a scheduled `cargo audit` or `cargo deny check advisories` job. |
| Miri | **Not run.** TokenSave's `unsafe` code is FFI and mmap, which Miri cannot execute, and the pinned toolchain is stable. Running Miri on the remaining safe code would find only standard-library bugs, which the table above covers. |
| Sanitizers (ASan/TSan) on vendored grammars | **Not run.** They need nightly and a Linux host. Worker isolation contains grammar crashes in the meantime. |

## Follow-up work

- Add a scheduled RustSec advisory check (`cargo audit` or
  `cargo deny check advisories`) to CI. Done in #956: the `Security
  advisories` workflow (`.github/workflows/advisories.yml`) runs `cargo audit`
  daily on `master` and `beta`, with its policy in `.cargo/audit.toml`.
- When master's pinned toolchain moves to 1.99 or later, record that #161018
  is fixed there. Until then, keep `process::exit` on the main thread.
- Consider fuzzing the vendored grammars, or running them under sanitizers on
  a Linux CI job, as a separate evaluation.

The [Rust memory safety and soundness review](../AGENTS.md#rust-memory-safety-and-soundness-review)
section in `AGENTS.md` defines the checks future changes in these areas must
apply.
