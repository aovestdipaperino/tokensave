# Cargo vs Blaze measurements

- Revision: `a1e87d652c7f00d134268fb9fadcb16d56fa7bc7`
- Features: `lite`
- Machine: 8 CPUs, 30.22 GiB RAM, 7.0.0-38-generic
- Cargo: `cargo 1.100.0 (3acc79c0c 2026-11-05)`
- Blaze: `blaze 0.20261027.0 (49ca152)`

Median (min-max) over the successful runs of each step. CPU is user+system time of the command and, for Blaze, the change in its daemon's CPU time. Peak memory is the highest sampled RSS of the command's process tree plus Blaze's daemon processes.

| Step | Tool | Runs ok | Wall | CPU | Peak memory |
|---|---|---:|---:|---:|---:|
| cold-build | cargo | 1/1 | 130.38 s | 666.32 s | 2.67 GiB |
| cold-build | blaze | 0/1 | failed | failed | failed |
| noop-build | cargo | 1/1 | 352 ms | 328 ms | 96 MiB |
| edit-build | cargo | 2/2 | 9.02 s (7.82 s-10.22 s) | 13.65 s (9.90 s-17.41 s) | 1.36 GiB (1.31 GiB-1.41 GiB) |
| test-first | cargo | 1/1 | 73.47 s | 177.91 s | 2.16 GiB |
| test-noop | cargo | 1/1 | 3.63 s | 16.88 s | 275 MiB |
| edit-test | cargo | 1/1 | 68.24 s | 132.95 s | 1.90 GiB |

| Tool | Target dir after cold build | Target dir at end | Tool home growth |
|---|---:|---:|---:|
| cargo | 2.61 GiB | 3.43 GiB | 0 MiB |
| blaze | 3.79 GiB | 3.79 GiB | 0 MiB |

## Failed steps

- blaze `cold-build` exit 101: `aze-eval/run/tree-blaze/target/rust-analyzer/compile-time/build/faster-hex-ce44162b3357e015/build-script-build' (exit status: 101) --- stderr thread 'main' (2) panicked at $HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/autocfg-1.5.1/src/lib.rs:160:20: called 'Result::unwrap()' on an 'Err' value: Error { kind: Other("could not execute rustc") } note: run with 'RUST_BACKTRACE=1' environment variable to display a backtrace (what blaze said is in $BLAZE_HOME/workers/acfb68cdd018aba6/log)`

## `blaze status` at the end

```text
plain cargo: not through blaze (`blaze enable` turns it on)
daemon: pid 68102, version 0.20261027.0-f60566aa-bfa6a50-18e84e6021a7f916, exits after 1800s idle, workers hold at most 15470 MB
  worker: pid 68105, $CHECKOUT/.blaze-eval/run/tree-blaze/, log $BLAZE_HOME/workers/acfb68cdd018aba6/log
    $CHECKOUT/.blaze-eval/run/tree-blaze: failed: the daemon could not load the workspace (exit status: 1: panicked at $HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/autocfg-1.5.1/src/lib.rs:160:20:          called `Result::unwrap()` on an `Err` value: Error { kind: Other("could not execute rustc") }); the worker's log is $BLAZE_HOME/workers/acfb68cdd018aba6/log
store: 834 MB, at most 7195 MB (`blaze cache gc` collects it), $HOME/.cache/ra-jit/store
```

Initial Blaze CPU accounting is incomplete; see `initial.json` notes and
`confirmation.json` for corrected accounting. Both Blaze attempts failed.
