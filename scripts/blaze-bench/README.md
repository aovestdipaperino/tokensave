# Blaze local runner and benchmark (#984)

Two opt-in tools for building and testing TokenSave with
[Blaze](https://github.com/indexable-inc/blaze) on a developer's own Linux or
macOS machine. Neither is used by CI or releases, and neither is required:
Cargo stays the authoritative build, validation and release path. Results,
limits and the cases that need Cargo are in
[docs/BLAZE-EVALUATION.md](../../docs/BLAZE-EVALUATION.md).

## `scripts/dev-build`: Blaze first, Cargo fallback

```sh
scripts/dev-build test --workspace --locked
scripts/dev-build build --locked --no-default-features --features lite
scripts/dev-build --cargo test --workspace --locked   # always Cargo
```

It takes a Cargo subcommand and its arguments. It runs Blaze when Blaze is
installed and usable on Linux or macOS, and Cargo otherwise, so it works the
same with or without Blaze. `--cargo`, or `TOKENSAVE_BUILD_RUNNER=cargo`,
selects Cargo; `--blaze`, or `TOKENSAVE_BUILD_RUNNER=blaze`, requires Blaze and
fails rather than falling back. The guide's
[runner section](../../docs/BLAZE-EVALUATION.md#local-runner) has the full
selection and fallback rules.

## `blaze_bench.py`: Cargo versus Blaze measurements

Linux (or WSL) only, Python 3.9+, standard library only. It needs `git`, `du`,
Cargo (rustup) and Blaze on `~/.cargo/bin`, and a C compiler.

```sh
python3 scripts/blaze-bench/blaze_bench.py \
    --source https://github.com/aovestdipaperino/tokensave.git --rev <commit> \
    --work ~/blaze-eval/run --output ~/blaze-eval/run.json --markdown ~/blaze-eval/run.md
```

For each tool it clones `--source` into `<work>/tree-<tool>` (Blaze has no
`--target-dir`, so the two never share a `target/`), runs `cargo fetch` so no
download is timed, and then times:

| Step | Command |
|---|---|
| `cold-build` | `<tool> build --locked <features>` after removing `target/` (and, for Blaze, stopping its daemon) |
| `noop-build` | the same build again, with nothing changed |
| `edit-build` | the build after a one-line change to a function body (`--edits` times, 5 by default) |
| `test-first` | `<tool> test --locked <features> --test integration <filter>` |
| `edit-test` | the same tests after another body edit (`--test-edits` times, 2 by default) |

Each step records its wall time, CPU time, and peak memory. CPU time covers
the command's process tree and, for Blaze, the change in its daemon's CPU
time. Peak memory is the sampled RSS of that tree plus every Blaze process.
The run also records the `target/` size after the cold build and at the end,
how much `~/.cargo` or `~/.blaze` grew, and `blaze status`, which says how
many builds the daemon served and how many it handed to Cargo. Logs of every
step are in `<work>/logs/`.

Options: `--tools cargo,blaze`, `--features lite|medium|full|default`
(`lite` by default), `--cold-runs N`, `--test-filter NAME`, `--skip-tests`,
`--edit-file` and `--edit-anchor` (the line after which the edit goes),
`--timeout` per step, `--keep-daemon`.

The run removes and rebuilds `target/` in its own clones only. It does not
stop or reset Blaze's shared store in `~/.blaze`, so a "cold" Blaze build
after an earlier run may reuse cached work there; the report records the
store's growth. Stop the daemon afterwards with `blaze stop` if you used
`--keep-daemon`, and delete `--work` when done.

## Tests

```sh
python3 -B -m unittest discover -s scripts/blaze-bench -p 'test_*.py'
TOKENSAVE_DEV_BUILD_TEST_SHELL="bash --posix" \
    python3 -B -m unittest discover -s scripts/blaze-bench -p 'test_*.py'
```

`test_dev_build.py` runs the real `scripts/dev-build` against stub `cargo`,
`blaze`, `cc` and `uname` programs, so neither tool has to be installed. The
second command runs it under bash in POSIX mode, the `/bin/sh` of macOS. The
tests are POSIX only and are not part of CI.
