# Grammar ASan evaluation

Tracking issue: [#958](https://github.com/aovestdipaperino/tokensave/issues/958).
Reproduction and bounds: [runner guide](../scripts/grammar-asan/README.md).

## Scope

The optional `grammar_asan` example parses byte inputs directly with the full
bundled grammar inventory and TokenSave's vendored/direct overrides. The Python
runner adds malformed seeds to the checked-in fixtures and gives every grammar
the same corpus. It does not run extraction, mutate a production index or use
the production worker subprocesses.

This direct-parser audit complements the extraction-path harness proposed in
[PR #1005](https://github.com/aovestdipaperino/tokensave/pull/1005), which was open
and had no Linux execution result when checked on 2026-11-14. It adds no CI
workflow. Normal production builds, dependency versions, parser flags and
worker containment are unchanged.

## Evaluation environment

- Dates: 2026-11-13 and 2026-11-14 (Asia/Jakarta).
- Source base: `a2b861442dd48e60fddf39fd7e9907051a02b8b6`, with this branch's
  uncommitted harness changes copied into an isolated source snapshot. The
  report revision is that container snapshot's synthetic commit, not an upstream
  commit for this change. The source and lockfile hashes are retained in the
  historical report; this base precedes the publication branch's upstream sync.
- Publication rerun: upstream base `b1c48fb05bb4ac64c714964d2ab3a57d092419f0`
  plus the harness, with a fresh target directory because Cargo.lock changed.
  Its source hashes match the files submitted in this PR.
- Linux aarch64 in a Debian Bookworm container on Colima. Initial resources:
  two CPUs and a 6 GiB memory limit, later reduced to 3 GiB and one build job.
  The shared VM has 8 GiB RAM and was busy
  with other workloads; timings are observations, not a performance baseline.
- Rust `nightly-2026-11-11`: `1.102.0-nightly (9c5648833 2026-11-10)`.
- Clang/Clang++ and compiler-rt 14 from Debian, Python 3.11.

Direct container egress failed. An allowlisted localhost HTTPS relay and SSH
tunnel supplied official Debian/Rust/crates/GitHub downloads. Existing Git
source caches were copied into the container after a Fortran fetch stalled.
No dependency version was changed. The runner's build duration includes
dependency resolution/download work that remained after that cache setup.
The VM subsequently restarted and the initial disposable container disappeared;
the cause was not established. Host copies of the controls and partial native
audit survived. Evaluation 03 uses a persistent container and named volumes,
with evidence copied to a host directory. The recorded container cgroup peak
was about 3 GiB, including page cache, setup and debugging; no OOM or OOM-kill
event was recorded. This is not a standalone build RSS measurement.

## Instrumentation controls

Both Linux C and C++ positive controls produced
`ERROR: AddressSanitizer: heap-buffer-overflow`, identifying the intentional
write at `scripts/grammar-asan/control.c:7`. The corresponding uninstrumented
controls did not satisfy the diagnostic check. These are deliberate harness
calibration failures, not grammar defects.

Every audited native compilation must use `-fsanitize=address` and produce
an object with undefined ASan symbols. The wrapper records its command,
working directory and object SHA-256. The runner additionally requires
records for all five vendored parser/scanner translation units and at least
one bundled parser before it starts the corpus. It rehashes native objects and
requires every linked native grammar/scanner symbol to have an audited object
definition. Resumed builds retain these checks.

## Results

Evaluation 05 attempted all 63 grammars with 155 snapshotted inputs each
(148 fixtures and seven generated seeds), including the OKF fixtures added by
upstream since the first evaluation. Sixty grammar processes passed,
including WGSL, ActionScript, GDScript and the direct Groovy binding. Perl
and SQL completed parsing but exited with LeakSanitizer findings. COBOL
exceeded its 60-second budget on the first input, so its remaining inputs
were not exercised. The runner exited 1 and retained `status: incomplete`;
this is not a clean sanitizer result.

| Measurement | Observed result |
| --- | --- |
| Evaluation 05 elapsed | 1,223.111 s |
| Fresh target build | 968.974 s |
| ASan example regression test | 166.932 s; one test passed |
| Grammar execution, including COBOL timeout | 85.015 s |
| Native audit | 249 C compilation records, 194 distinct object paths |
| Audited definitions covering linked native grammar/scanner symbols | 284 |

No C++ grammar translation unit was compiled in this inventory; the separate
C++ calibration control still detected its deliberate overflow. The native
counts include compiler probes and repeated builds, not 249 distinct grammars. Every recorded object passed its ASan-symbol
check and final hash verification; every linked grammar/scanner definition had
an audited object. All five vendored translation units were present.

The normal macOS workspace suite and Clippy ran concurrently with this Linux
build. Timings include shared-machine contention and are not a baseline.
[Selected machine-readable evidence and diagnostics](grammar-asan/evaluation-2026-11-14/README.md)
are checked in for review. The prior evaluation on the older base attempted
63 grammars with 61 inputs: it had the same three non-passing grammars, took
225.478 seconds with a retained build cache and recorded 498 native compilations
across its repeated builds. Those earlier figures do not describe the fresh run.

### Findings and isolated reproductions

| Grammar | Finding | Single-input reproduction |
| --- | --- | --- |
| Perl, `tree-sitter-perl` 1.1.2 | Latest full corpus: 9,472 bytes leaked in 467 allocations. Allocation stacks include `create_queue`, `src/scanner.c:146`. | Empty input alone: 48 bytes leaked in three allocations, exit 1. The prior 61 isolated input processes all reported a leak; the latest binary reproduces the empty-input leak. |
| SQL, `tree-sitter-sequel` 0.3.11 | 780 bytes leaked in 156 allocations at `tree_sitter_sql_external_scanner_deserialize`, `src/scanner.c:185`. | `tests/fixtures/sample.gw` alone reproduces the same leak, exit 1. The prior evaluation's other 60 isolated inputs passed; the latest binary also reproduces this single-input leak. |
| COBOL, bundled at `tokensave-large-treesitters` commit `694d60b236ff6c397a8df1f3c4f046f94a6df57b` | No sanitizer diagnostic; the process exceeded 60 seconds on the 159-byte `julia_module.jl` fixture. | The same input timed out again at 15 seconds. A one-byte `a` probe also exceeded five seconds and then 60.007 seconds in a separate confirmation; empty, newline and first-line-only probes passed. This establishes a bounded timeout, not an infinite-loop or memory-safety diagnosis. |

Run a single snapshot with the built driver to reproduce a finding:

```sh
ASAN_OPTIONS=detect_leaks=1:halt_on_error=1:abort_on_error=0 \
  /path/to/target/aarch64-unknown-linux-gnu/debug/examples/grammar_asan \
  perl /path/to/evaluation/inputs/seed-empty
```

Use `sql` with the `*-sample.gw` snapshot for the SQL finding. Snapshot indices
change as fixtures are added; match its source path and hash in the summary.
Keep COBOL under an external time limit with the `*-julia_module.jl` snapshot.
Input hashes and exact
commands are in the report and the isolated-reproduction reports. These tests
exercise raw parsers; they do not establish production impact or exploitability.
No dependency patch or public follow-up issue was created during this evaluation.
The selected evidence includes the latest [Perl empty-input diagnostic](grammar-asan/evaluation-2026-11-14/repro-perl-empty.txt)
and [SQL single-input diagnostic](grammar-asan/evaluation-2026-11-14/repro-sql-sample.txt).

### Startup failure and workaround

Evaluation 03 built the full ASan example in 534.366 seconds. Its driver-test
build took 174.878 seconds and then failed with SIGSEGV before `main`;
`--list` and three parser preflights failed at startup too. Those failures were
not grammar findings.

GDB traced the failure through `__asan_init`, an outlined AArch64 atomic in
compiler-builtins and an instrumented `core::sync::atomic` load before shadow
memory initialization. A dependency-free Cargo hello program reproduced the
failure. The same command with
`--config profile.dev.package.compiler_builtins.opt-level=1` built in 11.18
seconds and printed its startup marker with exit 0. The runner applies that
package-specific override in dev and test profiles while retaining
`-Zbuild-std`, Rust ASan and all native instrumentation checks. Evaluation 04
then passed startup, inventory, driver tests and native audit before parsing.

### Verification and retained evidence

All 13 runner tests passed as a non-root user on Linux, including real native
compilation, unreadable corpus directories, bounded traversal, verified resume,
output bounds, process-group timeout cleanup and SIGTERM reporting/cleanup.
On macOS, 12 tests passed and the Linux entry-point test was skipped. Wiki
validation and all 11 wiki tests passed. The publication branch passed `cargo fmt --all -- --check` with Rust 1.100.0.
The full workspace suite with `test-transport` passed (5,074 tests, four ignored),
and Clippy across all targets exited 0 with warnings in unchanged source and
manifest entries; none referenced the new example. No GitHub sanitizer workflow
was run.

A separate read-only reviewer found three harness defects: missing direct
Groovy inventory, silently skipped unreadable directories and a traversal cap
applied too late. Each was reproduced by a failing regression before its fix;
the final Linux suites passed. Focused review found no additional actionable
issue in resume/audit logic or the startup workaround. The reviewer did not
independently compile Rust or run Linux ASan.

Raw setup/build logs, controls, audits, input snapshots, individual grammar
logs and reproduction results are retained under
`local-only/grammar-asan-958/` on the evaluation host. Historical evaluation 04 references
the target and native audit in evaluation 03; evaluation 05 has a fresh target
and its own audit. These local artifacts are ignored
by Git; target directories are excluded from the host evidence copy. Selected
results and diagnostics are included in this PR for review.

## Limits

There is no fuzzing campaign, incremental reparse coverage, Windows/macOS
grammar evaluation or TSan result. Syntax-error nodes from malformed input
are expected. A completed corpus run can establish only that its selected
inputs produced no reported sanitizer failure, not that the grammars are safe.
The full Rust test suite passed on macOS; Windows and normal Linux workspace
tests remain CI checks. The harness is a master-based candidate; beta has no
corresponding files, so its guidance does not advertise this command.

References: [Rust sanitizers](https://doc.rust-lang.org/unstable-book/compiler-flags/sanitizer.html),
[Clang AddressSanitizer](https://clang.llvm.org/docs/AddressSanitizer.html).
