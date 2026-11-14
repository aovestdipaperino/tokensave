# Tree-sitter grammar sanitizer evaluation

Issue #958 follows the memory-safety audit in
[MEMORY-SAFETY-AUDIT.md](MEMORY-SAFETY-AUDIT.md). TokenSave links generated C
parsers and external scanners from roughly fifty tree-sitter grammars. Worker
subprocesses contain a native crash during normal indexing, but containment
does not find or repair the native defect.

## Decision

Start with a manual, Linux-only AddressSanitizer evaluation rather than an
automatic CI gate or an unbounded fuzzing campaign. The harness is
`scripts/grammar-sanitizer/grammar_sanitizer.py`; its README pins prerequisites,
commands, outputs and limitations. A `workflow_dispatch` workflow makes the
same fixture run reproducible on GitHub without adding cost to pushes or pull
requests.

This order answers the resource question before considering scheduled CI:

1. Compile every full-tier grammar with Rust, C and C++ ASan instrumentation.
2. Run the existing extraction fixtures.
3. Optionally index representative corpora with the extraction worker disabled
   so a sanitizer finding is visible at the failing process boundary.
4. Record wall/CPU time, child high-water RSS, target disk use, exit status and
   phase logs in JSON.
5. File a separate issue for each confirmed defect with the grammar, minimized
   non-sensitive input, platform, toolchain and complete sanitizer backtrace.

## Current evidence

The harness's platform-independent unit tests run on Windows and validate its
command, instrumentation environment and containment rules. An ASan grammar
run requires Linux and the pinned nightly, so no clean sanitizer result or
performance number is claimed by the implementation PR itself. The first
manual workflow artifact becomes the baseline; record its run link, runner
image, duration, CPU, high-water RSS, target bytes and findings here before
proposing a scheduled or required job.

## Promotion criteria

Consider a scheduled informational workflow only after at least three fixture
runs establish:

- stable completion below the workflow's 90-minute limit;
- acceptable CPU, peak RSS and target disk use on the hosted runner;
- no recurring infrastructure-only failure;
- logs sufficient to identify the grammar and input behind a failure.

Keep it non-blocking until representative corpora have also run and known
findings are triaged. Coverage-guided fuzzing is a later option if ASan fixtures
and corpora do not exercise scanner state deeply enough; it needs separate
corpus retention, crash minimization, time and disk budgets.
