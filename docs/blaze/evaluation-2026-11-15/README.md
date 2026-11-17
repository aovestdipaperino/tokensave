# Blaze evaluation evidence, 2026-11-15

Refs [#984](https://github.com/aovestdipaperino/tokensave/issues/984) and
[PR #990](https://github.com/aovestdipaperino/tokensave/pull/990), open and
unmerged at evaluation time. See [the consolidated evaluation](../../BLAZE-EVALUATION.md)
for implementation status, results, dependencies and remaining checks.

The native Linux run used TokenSave `a1e87d652c7f00d134268fb9fadcb16d56fa7bc7`,
Cargo 1.100.0 and Blaze 0.20261027.0. Tool installation and registry fetches
were outside timed steps. Paths are replaced with `$CHECKOUT`, `$HOME` and
`$BLAZE_HOME` in the retained records; no environment dump is included.

```sh
python3 -B scripts/blaze-bench/blaze_bench.py \
  --source "$PWD" --rev a1e87d652c7f00d134268fb9fadcb16d56fa7bc7 \
  --work /private/disposable-run --edits 2 --test-edits 1 --timeout 1200 \
  --output /private/initial.json --markdown /private/initial.md

# Reuse the same checkout and shared cache to confirm the failed Blaze load.
python3 -B scripts/blaze-bench/blaze_bench.py \
  --source "$PWD" --rev a1e87d652c7f00d134268fb9fadcb16d56fa7bc7 \
  --work /private/disposable-run --tools blaze --timeout 300 \
  --output /private/confirmation.json --markdown /private/confirmation.md
```

Both commands returned 1 because Blaze's build failed. Cargo passed all seven
measured steps, with 84 tests passing on each of the three test runs. The
initial harness lacked accounting for reaped daemon workers; its Blaze CPU
value is incomplete. The confirmation used corrected accounting, but reused
the populated shared store. Neither result is a successful Blaze benchmark.

`initial.json` and `confirmation.json` retain individual measurements and
`blaze status`; `measurements.md` summarizes the initial run. `cargo-*.log`
retain Cargo output, `blaze-confirmation.log` retains the second Blaze command,
and `blaze-worker.log` contains the build-script failure.
`runner-fallback.log` records a real wrapper invocation in the Cargo checkout:
Blaze failed, Cargo succeeded, the wrapper returned 0, and the daemon was
explicitly stopped afterward. The harness stops
the daemon after each attempt. The shared store is outside `BLAZE_HOME` and
its reported size is separate from tool-home growth.

These are single desktop runs with sampled memory, not idle-machine trials,
a full-feature test suite, a leak assessment or macOS validation. No CI or
release workflow uses Blaze.
