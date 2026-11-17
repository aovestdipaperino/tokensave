# Linux ASan evidence, 2026-11-14

This is selected output from the direct-parser evaluation described in
[the evaluation record](../../GRAMMAR-ASAN-EVALUATION.md), using upstream base
`b1c48fb05bb4ac64c714964d2ab3a57d092419f0` plus this harness.
The synthetic container commit identifies that source snapshot. File hashes
in the [summary](summary.json) identify the evaluated driver, runner, compiler
wrapper, native control and Cargo manifests.

[Publication validation](validation.json) records the ordinary workspace and
runner test results separately from the sanitizer outcome.

The summary preserves the observed completion state, commands, exit codes,
timings, input hashes and per-grammar start/done counts. A timeout can leave
inputs unprocessed; a failed sanitizer run must not be read as clean.

- [C calibration](control-c.txt) and [C++ calibration](control-cxx.txt) contain
  the deliberate heap-overflow diagnostics. Negative-control outcomes appear
  in the summary; their logs contained no ASan overflow diagnostic.
- [Native audit](native-audit.jsonl) records every accepted native compile,
  its command and object hash. Each object had undefined ASan symbols and
  was rehashed before parsing. Compiler probes and repeat builds are included.
- [Linked native definitions](linked-native-symbols.txt) lists only the
  grammar/scanner symbols from the full binary symbol dump. Each had an
  audited definition; Rust and unrelated native symbols are omitted here.
- [Driver regression](driver-tests.txt) records the Groovy inventory test.
- Grammar logs for non-passing processes preserve the last input and any
  sanitizer diagnostic. [Perl empty-input](repro-perl-empty.txt) and
  [SQL single-input](repro-sql-sample.txt) logs repeat both leak findings with
  one input per process; exact commands are in
  [isolated reproductions](isolated-reproductions.json).

Diagnostic text copies trim trailing whitespace and blank lines; the raw logs
and their recorded hashes remain unchanged. The native audit is copied verbatim.

Paths in native commands and stacks refer to the evaluation container, not
portable cache locations. These files are evidence, not build inputs. Use
[the runner guide](../../../scripts/grammar-asan/README.md) to repeat the run
with a new output directory. The full report, build/symbol logs and input
snapshots remain in the local evidence directory; their relevant hashes are
preserved in the summary. No proprietary inputs or credentials were used.
