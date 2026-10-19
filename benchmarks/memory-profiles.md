# RAM profile measurements

Measured on 2026-10-19 using debug/full-feature builds, original commit `c7e1689` and this change. Linux host: Linux-6.18.33.2-microsoft-standard-WSL2-x86_64-with-glibc2.43. OS affinity: 8 CPUs. No build or test commands ran during measurement.

Deterministic mixed Rust/Python workload: 96 source files, 40 generated functions per file; 16 files edited per incremental sync. Three repeats per case, alternating case order; two competing hash processes and an independent MCP server. `cpu_usage=performance` for all measured builds, subprocess extraction enabled. All 36 operations completed and their semantic graph signatures matched the corresponding original operation.

Reproduce with the same Cargo/toolchain/feature flags for both binaries:

```bash
python3 benchmarks/cpu_profiles.py \
  --baseline /tmp/tokensave-memory-baseline/tokensave \
  --candidate target/debug/tokensave --profile-field memory_usage \
  --files 96 --functions 40 --repeats 3 --output /tmp/memory-results.json
```

Medians across three repeats. RSS and CPU peaks are sampled; MCP p95 is the median of per-run p95 values. 100% CPU represents one core. Files/s normalizes by total indexed files (96), including incremental and unchanged operations; it is not the rate of changed files processed.

| Operation | Profile | Elapsed s | Files/s | Average CPU % | Peak CPU % | Peak RSS MiB | MCP median ms | MCP p95 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| full | original | 2.19 | 43.82 | 169.87 | 602.73 | 321.98 | 3.05 | 13.84 |
| full | low | 3.09 | 31.10 | 123.77 | 592.55 | 80.88 | 3.40 | 9.75 |
| full | balanced | 2.91 | 32.95 | 152.60 | 608.81 | 115.22 | 3.71 | 26.94 |
| full | performance | 2.47 | 38.79 | 178.90 | 604.64 | 319.65 | 3.78 | 18.48 |
| incremental | original | 2.11 | 45.49 | 137.50 | 533.63 | 56.27 | 2.85 | 5.23 |
| incremental | low | 2.51 | 38.29 | 127.32 | 534.15 | 73.34 | 3.56 | 4.18 |
| incremental | balanced | 3.57 | 26.87 | 129.25 | 446.61 | 107.36 | 4.28 | 12.99 |
| incremental | performance | 3.08 | 31.17 | 136.71 | 515.07 | 56.31 | 3.79 | 15.00 |
| unchanged_sync | original | 1.06 | 90.66 | 96.25 | 106.25 | 40.35 | 3.13 | 3.70 |
| unchanged_sync | low | 1.06 | 90.19 | 97.79 | 105.78 | 41.50 | 2.87 | 4.00 |
| unchanged_sync | balanced | 1.48 | 64.91 | 96.74 | 105.72 | 40.70 | 3.93 | 5.45 |
| unchanged_sync | performance | 1.39 | 68.94 | 96.74 | 106.31 | 40.62 | 3.75 | 4.87 |

For this small workload, balanced reduced the median sampled full-index process-tree RSS from 322.0 to 115.2 MiB (64% lower). Completion times are noisy and should not be treated as a general speed prediction.

Limitations: ~100 ms sampling misses short peaks and RSS double-counts shared pages. Concurrent external host load is uncontrolled. This is a small synthetic workload and a few minutes of repeated commands, not a large-repository or long-running server leak test. Each sync command exits; these measurements cannot establish post-warmup RSS stabilization in a persistent indexing process. Same-server request interleaving is not tested. Cancellation/recovery is covered by the workspace tests rather than the resource harness. Completed graph storage remains input-proportional; the policy is not a hard RSS cap.

Raw samples and binary hashes: [memory-profile-results.json](memory-profile-results.json).
