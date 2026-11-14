# Grammar AddressSanitizer harness

This development-only harness builds TokenSave and its C/C++ tree-sitter
grammars with AddressSanitizer (ASan), runs the extraction fixtures, and can
index one or more real source corpora. It is the initial evaluation for #958;
it does not change the TokenSave binary or normal CI.

The harness is Linux-only. It pins `nightly-2026-11-10`, instruments Rust with
`-Zsanitizer=address`, and passes `-fsanitize=address` to the C and C++
compilers used by grammar crates and `build.rs`. The `full` feature compiles all
bundled grammars, including the vendored WGSL, ActionScript and GDScript
sources.

## Prerequisites

- Linux x86-64, Clang or GCC, Git and Python 3.11 or newer.
- The pinned nightly toolchain and target installed in advance. The harness
  preserves TokenSave's production `-DNDEBUG` setting while adding ASan:

```text
rustup toolchain install nightly-2026-11-10 --profile minimal
rustup target add x86_64-unknown-linux-gnu --toolchain nightly-2026-11-10
```

Installation is the only networked step. A run uses `--locked` and performs no
downloads when Cargo's dependency cache is already populated.

## Run

Use a private work directory outside every Git checkout. The Cargo target,
logs and their disk usage stay there rather than under the repository.

```text
python3 scripts/grammar-sanitizer/grammar_sanitizer.py \
  --work-dir /var/tmp/tokensave-grammar-asan \
  --output /var/tmp/tokensave-grammar-asan/result.json
```

Add `--corpus /path/to/source` (repeatable) to index real repositories after
the fixtures. Corpus runs put project and global state in a temporary external
directory, disable the extraction worker so an ASan failure terminates the
instrumented process directly, and do not write TokenSave state into the
corpus. They still read every included source file, so use only corpora whose
contents may be processed on that machine.

The output JSON is replaced on an explicit repeated path. It records the
TokenSave commit, toolchain, flags, wall and CPU time, child high-water RSS,
target-directory bytes, commands, exit status and one log per phase. Preserve
the JSON and logs together when reporting a finding. Strip proprietary paths
and source from reports before posting them publicly.

Run the harness's platform-independent tests with:

```text
python3 -m unittest discover -s scripts/grammar-sanitizer -v
```

## Interpretation and limits

- A clean run shows only that the exercised fixtures and corpora did not
  trigger an ASan finding. It is not a proof that the grammars are memory-safe.
- The fixture filter covers integration tests whose module names end in
  `extraction_test`; optional corpora broaden inputs but remain finite.
- ASan detects memory safety faults, not data races. ThreadSanitizer and
  coverage-guided fuzzing remain separate evaluations.
- `ru_maxrss` is the cumulative high-water RSS of child processes on Linux,
  not a time series or retained-memory measurement.
- The sanitizer changes timing, allocation and memory use. Do not compare its
  throughput directly with a release build.
- A timeout is reported as incomplete, not healthy. The default phase limit is
  one hour and can be changed with `--timeout-secs`.

The manual GitHub workflow `.github/workflows/grammar-sanitizer.yml` runs the
fixture phase only and uploads the JSON and logs. It is never triggered by a
push, pull request or schedule, so the evaluation adds no cost to normal CI.
