# Automatic RAM safeguard measurements

Measured on 2026-10-19 using matching debug/full-feature builds: profiles-only baseline `0dc709f` versus this safeguard change. Host: Linux-6.18.33.2-microsoft-standard-WSL2-x86_64-with-glibc2.43; 8 CPUs in OS affinity.

Workload: 96 deterministic mixed Rust/Python files, 40 generated functions per file, 16 edited files for incremental sync. Three repetitions, alternating case order, two competing hash processes, independent MCP server. CPU policy fixed at performance. Baseline RAM policy explicitly fixed at performance so the candidate performance case compares the new probe against equivalent profile settings. No builds or test suites ran during measurement. All 36 operations completed with matching semantic graph signatures for each operation.

Reproduce after building the baseline and candidate with matching Cargo/toolchain/feature flags:

```bash
python3 benchmarks/cpu_profiles.py \
  --baseline /tmp/tokensave-profiles-baseline/tokensave \
  --candidate target/debug/tokensave --profile-field memory_usage \
  --baseline-profile performance --files 96 --functions 40 --repeats 3 \
  --output /tmp/safeguards-results.json
```

Medians across three repetitions. CPU 100% means one core. Peaks are sampled at roughly 100 ms; MCP p95 is the median of per-run p95. Files/s normalizes by the 96 indexed files even for incremental/unchanged operations; it is not the rate of changed files processed.

| Operation | Build/profile | Elapsed s | Files/s | Average CPU % | Peak CPU % | Peak RSS MiB | MCP median ms | MCP p95 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| full | baseline/performance | 2.11 | 45.42 | 179.44 | 550.49 | 319.36 | 3.20 | 13.51 |
| full | candidate/low | 2.75 | 34.90 | 131.59 | 605.39 | 80.82 | 3.16 | 4.19 |
| full | candidate/balanced | 2.35 | 40.84 | 144.65 | 584.38 | 115.50 | 3.38 | 8.41 |
| full | candidate/performance | 2.19 | 43.76 | 178.92 | 560.96 | 319.70 | 3.41 | 11.82 |
| incremental | baseline/performance | 2.11 | 45.56 | 135.39 | 541.92 | 55.94 | 3.21 | 9.33 |
| incremental | candidate/low | 2.27 | 42.32 | 126.03 | 568.02 | 73.63 | 3.19 | 4.12 |
| incremental | candidate/balanced | 2.44 | 39.42 | 136.80 | 583.57 | 56.08 | 3.47 | 7.99 |
| incremental | candidate/performance | 2.28 | 42.10 | 139.64 | 587.74 | 55.97 | 3.41 | 11.32 |
| unchanged_sync | baseline/performance | 1.06 | 90.57 | 93.66 | 106.29 | 41.15 | 3.13 | 3.71 |
| unchanged_sync | candidate/low | 0.90 | 106.24 | 95.48 | 106.18 | 41.60 | 3.12 | 4.15 |
| unchanged_sync | candidate/balanced | 0.90 | 106.21 | 97.83 | 106.14 | 40.67 | 2.89 | 3.76 |
| unchanged_sync | candidate/performance | 0.91 | 105.97 | 100.00 | 105.84 | 40.39 | 3.41 | 4.15 |

This is an ample-headroom experiment. Performance-mode results compare the added preflight path against the profiles-only build; differences include ordinary host/sampling noise, so this small workload cannot isolate the probe cost or establish a general regression bound. Low/balanced resource reductions reflect profile limits, not evidence that automatic pressure detection fired.

Pressure validation uses deterministic injected headroom (including zero, unknown and 512 MiB) in the production extraction helper and pure host/container-budget boundary tests. The helper compares all extraction fields except timestamps and durations across all profiles and budgets. Full workspace tests also cover full/incremental graph equality across CPU/RAM combinations, cancellation/recovery and independent MCP queries. No process-global test override or deliberate RAM exhaustion is used.

Limitations: sampled RSS double-counts shared pages and can miss short peaks. External host load is uncontrolled. These are short-lived repeated CLI syncs, not a persistent-server stabilization or leak test. Live pressure, nested cgroup enforcement, mid-extraction pressure, Windows/macOS, large repositories and cancellation-time resource peaks were not measured. One parser remains allowed even with no headroom, and graph/resolution memory is input-proportional. There is no hard RSS limit or leak-freedom claim.

Raw measurements and binary hashes: [memory-safeguards-results.json](memory-safeguards-results.json).
