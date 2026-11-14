# Grammar ASan evaluation

This opt-in Linux harness builds the full grammar set and parses raw bytes
directly, outside TokenSave's production worker isolation. It covers the
bundled grammars, the vendored WGSL/ActionScript/GDScript overrides and the
direct HCL, SystemVerilog, HTML, CSS, VHDL and Groovy dependencies. Groovy is
the production Gradle parser's binding.

## Run

Requirements: Linux GNU on x86-64 or aarch64, Python 3.11+, Clang/Clang++,
compiler-rt, `nm`, Git, Cargo and the pinned nightly with Rust sources.
On Debian/Ubuntu, install `clang llvm libclang-rt-dev` and the normal TokenSave
build prerequisites. Dependency downloads need network access.

```sh
rustup toolchain install nightly-2026-11-11 --profile minimal --component rust-src
python3 -B -m unittest discover -s scripts/grammar-asan -v
python3 -B scripts/grammar-asan/run.py --output /tmp/grammar-asan-new
```

The output directory must not exist and must be outside the input corpus.
Add local inputs with `--corpus /path/to/corpus` (repeatable). Inputs are
snapshotted and hashed; symlink files are excluded. A corpus must contain
1 to 1000 regular files in total, each at most 1 MiB. Discovery stops after
2000 filesystem entries, including directories and excluded symlinks, and
unreadable directories cause an incomplete run. The runner adds seven
deterministic seeds: empty, unmatched delimiters, all byte values, NUL bytes,
Unicode, nested parentheses and a long string.

Every grammar receives every selected input, including inputs that are invalid
for that grammar. Syntax-error nodes are expected and are not sanitizer findings.
The driver records the current input before parsing so a crash can be reproduced
with the grammar and that snapshot alone.

## Evidence and bounds

Before building, C and C++ controls intentionally write beyond a heap buffer.
Both must exit unsuccessfully with ASan's `heap-buffer-overflow` diagnostic.
The same controls compiled without ASan must not satisfy that check.

The build uses a new target directory, nightly `-Zsanitizer=address` and
`-Zbuild-std`, with an explicit target to avoid instrumenting host build scripts.
Its Cargo command sets compiler-builtins to optimization level 1 in dev/test
profiles: the pinned nightly's unoptimized AArch64 atomic helper otherwise calls
instrumented core before ASan initializes its shadow memory. This harness-only
override preserves the instrumented standard library and grammar builds.
A compiler wrapper requires `-fsanitize=address` for native C/C++ compilation
and checks each resulting object's undefined symbols for ASan references.
`native-audit.jsonl` retains commands, working directories and object hashes.
The evaluation refuses missing vendored objects or absent bundled parser evidence.
The example's regression tests run under the same sanitizer configuration.

`report.json` records toolchain identity, source hashes, build and execution
durations, inputs, grammar inventory, exit codes and log locations. Only
`status: completed` with exit 0 means every selected grammar process passed.
Missing tools, failed builds, timeouts and log limits leave `status: incomplete`
and exit 1. Inspect the individual logs before classifying a failure as a
confirmed memory defect.

Defaults: two build jobs (`--build-jobs 1` on constrained machines), one grammar
process at a time, 2400 seconds for each Cargo command, 60 seconds for each
grammar's entire corpus and 64 MiB per log. Change
the time budgets with `--build-timeout` and `--timeout`. Timeouts, output limits
and interrupts kill and reap the child process group. Build artifacts remain
in the explicitly selected output directory for inspection; remove that
directory yourself when no longer needed.

After an interrupted build, use a new output directory and retain its isolated
build cache and audit with `--resume-from /path/to/previous-output`. The target,
nightly and Cargo.lock must match. Cargo rebuilds changed sources; the runner
rehashes audited native objects and checks that every linked native grammar
or scanner symbol has an audited definition before parsing. The new report
identifies the shared build directory and audit. Keep all referenced evaluation
directories until inspection is complete and use only one evaluator per cache.

This runner is manual and adds no CI workflow. The extraction-path harness in
[PR #1005](https://github.com/aovestdipaperino/tokensave/pull/1005) tests TokenSave's
extractors and optional real corpora. This audit complements that scope by
parsing raw bytes directly with every grammar, calibrating native ASan and
checking the objects linked into its driver. Neither run substitutes for the
other. Local runs do not upload anything. Review custom input contents before
sharing diagnostic artifacts.

## Limits

This is a bounded fixture/corpus evaluation, not a fuzzing campaign. It does not
test production extraction traversal, subprocess containment, incremental edits,
TSan, every scanner state or every possible input. The optional example requires
the `full` feature; normal production build behavior is unchanged.

See [the evaluation record](../../docs/GRAMMAR-ASAN-EVALUATION.md) for actual
platforms, measurements and remaining work.
