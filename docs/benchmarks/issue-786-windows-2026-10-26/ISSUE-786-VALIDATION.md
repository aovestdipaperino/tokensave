# TokenSave Beta — Issue #786 Windows Validation Report

**Environment**

- OS: Microsoft Windows 11 Business, version 10.0.26300, build 26300.
- CPU: 11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz; 8 logical processors.
- Installed physical memory reported by Windows: 16844595200 bytes.
- Storage: SAMSUNG MZVLQ512HBLU-00BH1, SSD.
- Power profile: GUID combinazione risparmio energia: 381b4222-f694-41f0-9685-ff5bb260df2e  (Bilanciato).
- Stable: `tokensave 7.16.0`; SHA-256 `e023d7fd611d16cc6e8e02a25d6d77baaf14219b04d963724f6f9e4ce2ac22cd`.
- Beta: `tokensave 7.16.0`; SHA-256 `13b7b68e725f9708591acbe9369d559a778a8befcf1995f0c4afffd0a8bd52fb`.
- Workload repository: local `e-shock_b-challenger`; commit `b7e8d6468e43fc269082aab0707fd43a5811e602`. Only committed C/C++ headers/sources and Python files were exported; manifest includes source hashes and relocated paths. Added synthetic Python fixtures exercise duplicate method names and inheritance.
- Binary source commits: **unverified**. Filenames identify the supplied channels; identical version strings cannot establish that Beta contains the agreed integration tip.
- Benchmark start: `2026-10-26T12:43:19+0100` (local timestamp). Machine metadata: `environment.json`; binary identity: `binaries.json`.
- Methodology: serial execution, alternating Stable/Beta order per repetition, 5 indexing repetitions, 15 measured CLI repetitions and 100 measured persistent MCP repetitions per query; 2 warmup rounds per query excluded. CLI includes process startup and database open. MCP includes local JSON-RPC transport and excludes process initialization. Both servers stay open during the MCP sequence. CPU profile is `balanced`. OS caches are not flushed. No machine isolation or thermal control was imposed.

## 1. Executive summary

The clearest query improvement is scoped complexity over MCP: 51.075 → 44.887 ms (12.1% faster). Rare-term MCP search is 3.016 → 3.172 ms (5.2% slower); its exploratory interval excludes zero. Most other query differences are inconclusive. Database files are 2.12% smaller in Beta. Sampled memory observations do not establish an overall memory-efficiency advantage. Initial indexing is approximately equivalent within broad uncertainty; no reliable overall speed advantage is established.

This is an external Windows binary comparison, not confirmation of the upstream Rust/SQL test suite. The measurements below describe this exported firmware/synthetic corpus only. Overall superiority and functional equivalence are **not established**. Results are workload-dependent; uncertainty intervals that cross zero do not establish an improvement or regression.

For the 12 query cases, all 24 CLI/MCP exact ordered-output comparisons matched after normalization. This establishes equivalence only for the exercised outputs, not all functionality.

There are 62 automated checks: 62 successful and 0 failed strict comparisons/checks. A failed exact-output comparison is an investigation item, not automatically a semantic defect. Short serial MCP operation does not establish sustained production stability. Memory figures are observations, not proof of a leak or absence of one.

Recommendation: **NEEDS ADDITIONAL VALIDATION** before promotion. Keep #786 open pending provenance, output-difference review, production concurrency/cancellation and the upstream completion requirements.

## 2. Performance comparison

Positive improvement means Beta is faster/smaller. Absolute delta is Beta minus Stable. Confidence intervals are paired percentile bootstrap intervals for the difference of medians (3,000 resamples, deterministic seed 786). They are exploratory, unadjusted for multiple comparisons, and assume rounds are exchangeable; system drift can violate that assumption. Indexing intervals with only five repetitions are weak evidence.

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| initial_index | 5 | ms | 3399.770 | 3454.755 | +54.984 | -1.6% | [-977.527, 337.067] |
| forced_index | 5 | ms | 3597.384 | 3261.021 | -336.364 | +9.4% | [-424.529, 1072.773] |
| incremental_index | 5 | ms | 682.981 | 474.029 | -208.952 | +30.6% | [-61.733, 421.855] |
| addition_check | 5 | ms | 203.035 | 232.958 | +29.923 | -14.7% | [-290.704, 91.590] |
| deletion | 5 | ms | 592.993 | 647.566 | +54.572 | -9.2% | [-218.977, 217.923] |
| deletion_check | 5 | ms | 213.169 | 234.088 | +20.919 | -9.8% | [-72.979, 33.109] |
| verify | 5 | ms | 211.475 | 207.000 | -4.475 | +2.1% | [-112.466, 152.666] |
| database_bytes | 5 | bytes | 17793024.000 | 17416192.000 | -376832.000 | +2.1% | [376832.000, 376832.000] |
| cli_fts_common | 15 | ms | 177.268 | 182.198 | +4.930 | -2.8% | [-20.529, 15.285] |
| cli_fts_rare | 15 | ms | 168.255 | 170.360 | +2.105 | -1.3% | [-25.218, 9.586] |
| cli_fts_empty | 15 | ms | 182.155 | 188.020 | +5.865 | -3.2% | [-18.936, 12.034] |
| cli_complexity | 15 | ms | 200.150 | 217.085 | +16.935 | -8.5% | [-30.935, 46.091] |
| cli_complexity_scoped | 15 | ms | 200.741 | 213.268 | +12.527 | -6.2% | [-21.888, 29.989] |
| cli_inheritance | 15 | ms | 164.721 | 176.628 | +11.907 | -7.2% | [-17.604, 4.561] |
| cli_inheritance_scoped | 15 | ms | 179.014 | 170.219 | -8.795 | +4.9% | [-4.622, 22.296] |
| cli_body | 15 | ms | 160.582 | 164.250 | +3.667 | -2.3% | [-22.114, 12.177] |
| cli_ambiguous | 15 | ms | 168.880 | 166.266 | -2.614 | +1.5% | [-14.912, 11.882] |
| cli_context | 15 | ms | 164.344 | 170.257 | +5.913 | -3.6% | [-17.077, 22.941] |
| cli_hotspots | 15 | ms | 173.482 | 187.293 | +13.811 | -8.0% | [-30.266, 20.966] |
| cli_ambiguous_calls | 15 | ms | 188.199 | 187.426 | -0.773 | +0.4% | [-37.032, 15.412] |
| mcp_fts_common | 100 | ms | 6.745 | 6.531 | -0.214 | +3.2% | [-0.213, 0.451] |
| mcp_fts_rare | 100 | ms | 3.016 | 3.172 | +0.156 | -5.2% | [-0.281, -0.065] |
| mcp_fts_empty | 100 | ms | 11.533 | 12.265 | +0.732 | -6.3% | [-1.410, 0.206] |
| mcp_complexity | 100 | ms | 56.372 | 56.892 | +0.520 | -0.9% | [-4.728, 2.388] |
| mcp_complexity_scoped | 100 | ms | 51.075 | 44.887 | -6.188 | +12.1% | [2.468, 10.230] |
| mcp_inheritance | 100 | ms | 10.100 | 10.206 | +0.106 | -1.0% | [-0.504, 0.456] |
| mcp_inheritance_scoped | 100 | ms | 10.418 | 10.497 | +0.079 | -0.8% | [-0.812, 0.307] |
| mcp_body | 100 | ms | 2.285 | 2.331 | +0.046 | -2.0% | [-0.145, 0.027] |
| mcp_ambiguous | 100 | ms | 4.519 | 4.578 | +0.059 | -1.3% | [-0.178, 0.235] |
| mcp_context | 100 | ms | 7.301 | 7.428 | +0.127 | -1.7% | [-0.308, 0.165] |
| mcp_hotspots | 100 | ms | 14.236 | 13.910 | -0.326 | +2.3% | [-0.683, 1.120] |
| mcp_ambiguous_calls | 100 | ms | 37.959 | 38.277 | +0.318 | -0.8% | [-2.809, 2.849] |

### Latency percentiles

Nearest-rank sample percentiles, not established production tails. CLI n=15 and indexing n=5 are far too small for useful p99 inference; even MCP n=100 makes p99 depend on the top few samples.

| Workload | Stable p95 ms | Beta p95 ms | Stable p99 ms | Beta p99 ms |
|---|---:|---:|---:|---:|
| initial_index | 3907.232 | 3737.196 | 3907.232 | 3737.196 |
| forced_index | 3907.896 | 3546.159 | 3907.896 | 3546.159 |
| incremental_index | 915.370 | 626.588 | 915.370 | 626.588 |
| addition_check | 252.777 | 437.250 | 252.777 | 437.250 |
| deletion | 744.423 | 853.882 | 744.423 | 853.882 |
| deletion_check | 281.209 | 260.601 | 281.209 | 260.601 |
| verify | 338.530 | 287.852 | 338.530 | 287.852 |
| cli_fts_common | 313.474 | 561.023 | 313.474 | 561.023 |
| cli_fts_rare | 198.413 | 223.040 | 198.413 | 223.040 |
| cli_fts_empty | 249.644 | 239.328 | 249.644 | 239.328 |
| cli_complexity | 274.214 | 360.469 | 274.214 | 360.469 |
| cli_complexity_scoped | 439.679 | 288.515 | 439.679 | 288.515 |
| cli_inheritance | 294.096 | 322.632 | 294.096 | 322.632 |
| cli_inheritance_scoped | 332.740 | 212.034 | 332.740 | 212.034 |
| cli_body | 206.667 | 216.222 | 206.667 | 216.222 |
| cli_ambiguous | 449.180 | 342.386 | 449.180 | 342.386 |
| cli_context | 217.743 | 223.833 | 217.743 | 223.833 |
| cli_hotspots | 249.088 | 334.715 | 249.088 | 334.715 |
| cli_ambiguous_calls | 507.713 | 273.324 | 507.713 | 273.324 |
| mcp_fts_common | 10.119 | 9.924 | 37.971 | 15.904 |
| mcp_fts_rare | 4.501 | 4.947 | 7.321 | 16.599 |
| mcp_fts_empty | 16.204 | 14.606 | 20.396 | 19.567 |
| mcp_complexity | 75.032 | 85.329 | 81.044 | 103.645 |
| mcp_complexity_scoped | 75.011 | 61.552 | 109.042 | 75.146 |
| mcp_inheritance | 14.035 | 15.460 | 16.345 | 22.860 |
| mcp_inheritance_scoped | 13.405 | 13.918 | 14.961 | 34.919 |
| mcp_body | 3.691 | 4.191 | 5.931 | 9.541 |
| mcp_ambiguous | 8.666 | 7.794 | 41.119 | 41.717 |
| mcp_context | 10.439 | 13.878 | 14.777 | 24.824 |
| mcp_hotspots | 18.485 | 18.652 | 26.176 | 29.641 |
| mcp_ambiguous_calls | 57.533 | 56.423 | 72.593 | 75.510 |

### CPU, memory and persistent throughput

CPU utilization is process CPU time / measured wall time, with 100% representing one logical processor; divide by eight for machine capacity. CLI process CPU and RSS are polled every approximately 5 ms, so short-lived peaks/final CPU ticks can be missed. MCP RSS is sampled at request boundaries and is **not true peak RAM**. CPU tick quantization makes individual short MCP CPU measurements unreliable. Child processes are excluded. Database size counts `.db` files, excluding WAL/SHM.

| Workload | Stable mean CPU s | Beta mean CPU s | Stable CPU % | Beta CPU % | Stable max observed RSS bytes | Beta max observed RSS bytes |
|---|---:|---:|---:|---:|---:|---:|
| initial_index | 1.115625 | 2.403125 | 36.19 | 71.02 | 44404736 | 47263744 |
| forced_index | 1.346875 | 2.409375 | 38.42 | 75.03 | 61698048 | 64364544 |
| incremental_index | 0.400000 | 0.303125 | 56.54 | 57.15 | 43122688 | 38678528 |
| addition_check | 0.059375 | 0.062500 | 30.15 | 26.37 | 20066304 | 16019456 |
| deletion | 0.293750 | 0.318750 | 48.74 | 52.19 | 27303936 | 22118400 |
| deletion_check | 0.075000 | 0.068750 | 35.89 | 29.67 | 23040000 | 18878464 |
| verify | 0.231250 | 0.187500 | 101.33 | 87.51 | 26337280 | 20180992 |
| cli_fts_common | 0.047917 | 0.059375 | 25.53 | 31.66 | 22306816 | 16334848 |
| cli_fts_rare | 0.054167 | 0.056250 | 31.95 | 32.00 | 20103168 | 16216064 |
| cli_fts_empty | 0.072917 | 0.073958 | 38.61 | 39.47 | 23048192 | 19025920 |
| cli_complexity | 0.085417 | 0.103125 | 39.04 | 44.12 | 27336704 | 23601152 |
| cli_complexity_scoped | 0.094792 | 0.088542 | 42.93 | 41.02 | 27471872 | 23228416 |
| cli_inheritance | 0.060417 | 0.065625 | 34.22 | 35.47 | 20152320 | 15925248 |
| cli_inheritance_scoped | 0.060417 | 0.055208 | 31.14 | 31.43 | 20140032 | 15966208 |
| cli_body | 0.050000 | 0.051042 | 30.20 | 29.21 | 20082688 | 15912960 |
| cli_ambiguous | 0.043750 | 0.058333 | 25.25 | 33.13 | 20131840 | 15855616 |
| cli_context | 0.055208 | 0.062500 | 31.87 | 35.61 | 20987904 | 16846848 |
| cli_hotspots | 0.056250 | 0.071875 | 30.45 | 36.27 | 20254720 | 15958016 |
| cli_ambiguous_calls | 0.056250 | 0.071875 | 28.20 | 36.21 | 22999040 | 19050496 |
| mcp_fts_common | 0.005938 | 0.008281 | 83.60 | 95.41 | 38215680 | 35323904 |
| mcp_fts_rare | 0.002656 | 0.002031 | 91.50 | 55.93 | 38215680 | 35323904 |
| mcp_fts_empty | 0.010000 | 0.011250 | 84.65 | 97.67 | 37539840 | 35323904 |
| mcp_complexity | 0.052187 | 0.052812 | 96.66 | 94.30 | 37670912 | 35356672 |
| mcp_complexity_scoped | 0.048594 | 0.042813 | 95.56 | 95.91 | 38125568 | 35356672 |
| mcp_inheritance | 0.009687 | 0.008438 | 98.09 | 87.69 | 38125568 | 35356672 |
| mcp_inheritance_scoped | 0.010156 | 0.009687 | 87.11 | 83.03 | 38125568 | 35356672 |
| mcp_body | 0.002031 | 0.001719 | 78.37 | 55.09 | 38125568 | 35356672 |
| mcp_ambiguous | 0.005625 | 0.005000 | 85.18 | 72.39 | 38125568 | 35356672 |
| mcp_context | 0.005313 | 0.007031 | 73.90 | 91.98 | 38125568 | 35356672 |
| mcp_hotspots | 0.012344 | 0.012656 | 94.70 | 91.06 | 38125568 | 35356672 |
| mcp_ambiguous_calls | 0.038125 | 0.035937 | 96.76 | 94.85 | 38215680 | 35323904 |

Stable persistent MCP: 1200 measured requests, 22.103 s summed request wall time, 54.29 requests/s (serial active-service rate, excludes harness gaps and other binary's requests; not concurrent end-to-end throughput).

Beta persistent MCP: 1200 measured requests, 22.005 s summed request wall time, 54.53 requests/s (serial active-service rate, excludes harness gaps and other binary's requests; not concurrent end-to-end throughput).

## 3. Optimization-by-optimization validation

All measurements are external CLI/MCP behavior. Attribution to an individual PR is inconclusive because these are combined binaries with unverified source provenance. Rows below refer directly to the full measurements in Section 2 and the CSV, including Stable/Beta medians, intervals and repetitions.

### #775 / #776

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| initial_index | 5 | ms | 3399.770 | 3454.755 | +54.984 | -1.6% | [-977.527, 337.067] |
| forced_index | 5 | ms | 3597.384 | 3261.021 | -336.364 | +9.4% | [-424.529, 1072.773] |
| incremental_index | 5 | ms | 682.981 | 474.029 | -208.952 | +30.6% | [-61.733, 421.855] |
| deletion | 5 | ms | 592.993 | 647.566 | +54.572 | -9.2% | [-218.977, 217.923] |
| database_bytes | 5 | bytes | 17793024.000 | 17416192.000 | -376832.000 | +2.1% | [376832.000, 376832.000] |

Tests/correctness and limitations: File addition/deletion search checks, CLI verify and SQLite integrity; no rollback/cancellation or dispatch-index SQL-plan assertion. Exact-output results are in `correctness.json`; failures are listed below. Improvements/regressions are the signed percentages and confidence intervals above. **Result: INCONCLUSIVE** for complete implementation validation.

### #777 / #779

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| cli_complexity | 15 | ms | 200.150 | 217.085 | +16.935 | -8.5% | [-30.935, 46.091] |
| cli_complexity_scoped | 15 | ms | 200.741 | 213.268 | +12.527 | -6.2% | [-21.888, 29.989] |
| mcp_complexity | 100 | ms | 56.372 | 56.892 | +0.520 | -0.9% | [-4.728, 2.388] |
| mcp_complexity_scoped | 100 | ms | 51.075 | 44.887 | -6.188 | +12.1% | [2.468, 10.230] |

Tests/correctness and limitations: Exact ranked-output comparisons; project/time savings index is not directly exercised. Exact-output results are in `correctness.json`; failures are listed below. Improvements/regressions are the signed percentages and confidence intervals above. **Result: INCONCLUSIVE** for complete implementation validation.

### #778 / #781

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| cli_body | 15 | ms | 160.582 | 164.250 | +3.667 | -2.3% | [-22.114, 12.177] |
| cli_ambiguous | 15 | ms | 168.880 | 166.266 | -2.614 | +1.5% | [-14.912, 11.882] |
| cli_ambiguous_calls | 15 | ms | 188.199 | 187.426 | -0.773 | +0.4% | [-37.032, 15.412] |
| mcp_body | 100 | ms | 2.285 | 2.331 | +0.046 | -2.0% | [-0.145, 0.027] |
| mcp_ambiguous | 100 | ms | 4.519 | 4.578 | +0.059 | -1.3% | [-0.178, 0.235] |
| mcp_ambiguous_calls | 100 | ms | 37.959 | 38.277 | +0.318 | -0.8% | [-2.809, 2.849] |

Tests/correctness and limitations: Unique body and duplicate method candidates; no direct executable-body SQL or candidate-membership oracle. Exact-output results are in `correctness.json`; failures are listed below. Improvements/regressions are the signed percentages and confidence intervals above. **Result: INCONCLUSIVE** for complete implementation validation.

### #782 / #783

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| cli_inheritance | 15 | ms | 164.721 | 176.628 | +11.907 | -7.2% | [-17.604, 4.561] |
| cli_inheritance_scoped | 15 | ms | 179.014 | 170.219 | -8.795 | +4.9% | [-4.622, 22.296] |
| mcp_inheritance | 100 | ms | 10.100 | 10.206 | +0.106 | -1.0% | [-0.504, 0.456] |
| mcp_inheritance_scoped | 100 | ms | 10.418 | 10.497 | +0.079 | -0.8% | [-0.812, 0.307] |

Tests/correctness and limitations: Synthetic same-file inheritance; out-of-scope ancestors and deep/cyclic inheritance are not independently asserted. Exact-output results are in `correctness.json`; failures are listed below. Improvements/regressions are the signed percentages and confidence intervals above. **Result: INCONCLUSIVE** for complete implementation validation.

### #784 / #785

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| cli_hotspots | 15 | ms | 173.482 | 187.293 | +13.811 | -8.0% | [-30.266, 20.966] |
| mcp_hotspots | 100 | ms | 14.236 | 13.910 | -0.326 | +2.3% | [-0.683, 1.120] |

Tests/correctness and limitations: Connectivity ranking surface exercised; individual Q0118/Q0130 SQL paths and hydration equivalence not proven. Exact-output results are in `correctness.json`; failures are listed below. Improvements/regressions are the signed percentages and confidence intervals above. **Result: INCONCLUSIVE** for complete implementation validation.

### #751 / #787

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| cli_fts_common | 15 | ms | 177.268 | 182.198 | +4.930 | -2.8% | [-20.529, 15.285] |
| cli_fts_rare | 15 | ms | 168.255 | 170.360 | +2.105 | -1.3% | [-25.218, 9.586] |
| cli_fts_empty | 15 | ms | 182.155 | 188.020 | +5.865 | -3.2% | [-18.936, 12.034] |
| mcp_fts_common | 100 | ms | 6.745 | 6.531 | -0.214 | +3.2% | [-0.213, 0.451] |
| mcp_fts_rare | 100 | ms | 3.016 | 3.172 | +0.156 | -5.2% | [-0.281, -0.065] |
| mcp_fts_empty | 100 | ms | 11.533 | 12.265 | +0.732 | -6.3% | [-1.410, 0.206] |

Tests/correctness and limitations: Common, rare and absent-name search; CLI search is not a direct isolated FTS SQL benchmark. All-matching/adversarial production-scale corpus not covered. Exact-output results are in `correctness.json`; failures are listed below. Improvements/regressions are the signed percentages and confidence intervals above. **Result: INCONCLUSIVE** for complete implementation validation.

## 4. Correctness and regression testing

Executed 62 checks, with 62 successful and 0 unsuccessful. Every individual result and detail is in `correctness.json`. Query success/error status is additionally recorded per measured request in `benchmark-results.csv`.

- Exact comparisons preserve result order. Only JSON-RPC IDs, timing `_meta`, and known project roots are normalized. Timing/warning/counter text may still cause differences. No failed equality check is treated as a proven missing/duplicate result without investigation.
- Addition checks require the new function name and count 1; deletion checks require its absence and count 0. These assertions were also re-evaluated against every saved addition/deletion response. These are smoke assertions, not an exhaustive graph cleanup oracle.
- CLI `verify` success and SQLite `PRAGMA integrity_check` are recorded. These do not prove semantic graph completeness.
- Schema definitions were captured independently. Cross-version migration and downgrade compatibility: **NOT TESTED**; disposable indexes were created separately.
- Missing/duplicated results, ranking semantics and tie ordering: exact-output mismatches require review; no complete ground-truth oracle.
- Cancellation during active writes: **NOT TESTED**. Concurrent readers: bounded checks below; concurrent writers: **NOT TESTED**. Long-running RSS stabilization: **NOT TESTED**. Upstream Rust regression tests: **NOT TESTED**.



### Additional bounded stability checks

- Stable: 4212 requests over 60.052 s, 0 response errors. RSS across sampled request boundaries: first 30437376, last 33243136, maximum 38600704 bytes. Four concurrent reader processes each issued 30 requests against the same index; errors: 0.
- Stable forced-termination probe: termination after 0.05 s; process was still running: True; exit code 1. Subsequent forced reindex and verify outcomes appear in `correctness.json` and raw CSV. The active database-write phase was not confirmed, and this is not graceful MCP cancellation.
- Beta: 4168 requests over 60.068 s, 0 response errors. RSS across sampled request boundaries: first 28622848, last 36737024, maximum 37011456 bytes. Four concurrent reader processes each issued 30 requests against the same index; errors: 0.
- Beta forced-termination probe: termination after 0.05 s; process was still running: True; exit code 1. Subsequent forced reindex and verify outcomes appear in `correctness.json` and raw CSV. The active database-write phase was not confirmed, and this is not graceful MCP cancellation.

Source: `extended-results.json`, `extended-run.log` and cancellation logs. One minute is a bounded smoke soak, insufficient to establish long-run RSS stabilization. Concurrency covers multiple read-only server processes, not concurrent requests on one server or simultaneous writers.

## 5. Performance regressions

Exploratory supported regressions below require a slowdown exceeding 5% and a bootstrap interval entirely below zero. This is not a multiple-comparison-adjusted confirmatory result. Reproducibility is limited to the repetitions on this machine; an independent rerun has not been performed. Impact is slower response/index completion for the named workload.

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| mcp_fts_rare | 100 | ms | 3.016 | 3.172 | +0.156 | -5.2% | [-0.281, -0.065] |

Suspected slowdowns exceeding 5% with intervals including zero:

| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |
|---|---:|---|---:|---:|---:|---:|---:|
| addition_check | 5 | ms | 203.035 | 232.958 | +29.923 | -14.7% | [-290.704, 91.590] |
| deletion | 5 | ms | 592.993 | 647.566 | +54.572 | -9.2% | [-218.977, 217.923] |
| deletion_check | 5 | ms | 213.169 | 234.088 | +20.919 | -9.8% | [-72.979, 33.109] |
| cli_complexity | 15 | ms | 200.150 | 217.085 | +16.935 | -8.5% | [-30.935, 46.091] |
| cli_complexity_scoped | 15 | ms | 200.741 | 213.268 | +12.527 | -6.2% | [-21.888, 29.989] |
| cli_inheritance | 15 | ms | 164.721 | 176.628 | +11.907 | -7.2% | [-17.604, 4.561] |
| cli_hotspots | 15 | ms | 173.482 | 187.293 | +13.811 | -8.0% | [-30.266, 20.966] |
| mcp_fts_empty | 100 | ms | 11.533 | 12.265 | +0.732 | -6.3% | [-1.410, 0.206] |

## 6. Known limitations

- Both executables report the same release version. SHA-256 distinguishes them but does not establish their upstream commit or build options. No promotion decision can rely on an assumed Beta source revision.
- Source files were relocated under short hashed directories to avoid Windows path limits. File contents are unchanged, but include/import path resolution and project discovery can differ from the original repository. This is a hybrid exported corpus, not a production checkout.
- Five index repetitions and fifteen CLI repetitions provide limited statistical power. Concurrent system activity, power management, filesystem cache, antivirus, thermal state and fixed workload order are uncontrolled. No across-machine replication.
- CLI/MCP timing includes application behavior beyond SQL. Internal SQL plans, bundled-libSQL equivalence, write-lock/rollback behavior and per-optimization attribution cannot be established from these measurements.
- Public issue #786's reported upstream tests are background information, not tests executed here. No source build or cross-platform CI verification was performed.
- The paired serial benchmark and four-reader smoke check are not a production concurrency stress test. Request-boundary RSS and sampled CLI RSS do not establish true peak memory or a stable long-run plateau.
- No large all-matching FTS stress corpus, confirmed active-write cancellation, controlled migration/downgrade or source-level regression suite was executed. Forced-termination recovery is covered only by the bounded checks above.

## 7. Final recommendation

**NEEDS ADDITIONAL VALIDATION**

Do not promote on this evidence alone. Confirm the binary build commits, investigate any measured slowdowns and broaden correctness coverage, then run representative full-path production corpora, active-write cancellation, concurrent workloads and a long RSS soak. Timing improvements on selected rows cannot compensate for these unresolved correctness and coverage gaps.

## 8. Issue closure recommendation

**RECOMMEND KEEPING #786 OPEN**

The [public tracking issue](https://github.com/aovestdipaperino/tokensave/issues/786) requires broader Beta evaluation and cross-platform CI observation before proposing promotion; it explicitly keeps tracking/performance work open. This run does not verify those completion requirements. The unavailable source-level validation, unverified build provenance and production stability gaps prevent a defensible closure recommendation. Benchmark execution made no GitHub changes; subsequent publication was explicitly authorized by the user. The issue remains open.

## 9. Reproduction instructions

Run from the delivered directory in PowerShell. Python 3 with psutil is required (the executed environment used Python 3.14.7 and psutil 7.2.2). No GitHub CLI, authentication, SDK, source build or network download is required for the runner. Standard Git reads the configured local commit. Review/edit the two binary paths and repository path in the JSON if moving the package.

```powershell
Set-Location 'C:\Users\AndreaTontini\Desktop\e-shock_b-challenger\tokensave-validation-786'
python -c "import psutil; print(psutil.__version__)"
Get-Content .\benchmark-config.json
powershell -NoProfile -ExecutionPolicy Bypass -File .\benchmark.ps1 -Config .\benchmark-config.json
python .\summarize.py
Get-Content -Raw .\ISSUE-786-VALIDATION.md
Start-Process .\benchmark-report.html
```

The runner recreates only its named Stable/Beta/cancellation directories under `%TEMP%/tokensave-issue786-work/`, outside the repository. It overwrites the output CSV and numbered raw logs; archive this completed evidence package before a separate historical run. It reuses `corpus/` only when the corpus manifest exists. Each run records binary hashes. Query arguments, limits, repetitions, process timeout and CPU profile are explicit in the runner/config. Warmups are omitted from raw measurements. `summarize.py` deterministically regenerates statistics and reports from the CSV/check logs and metadata.

## 10. Evidence and attachments

- `benchmark-results.csv`: each measured operation, timing, sampled CPU/RSS, exit/error status and raw-log path. The `database_bytes` row uses bytes in the historical `elapsed_ms` field; the summary's unit column disambiguates it.
- `benchmark-summary.csv`: medians, differences, percentage improvements, bootstrap intervals, p95/p99 and resources.
- `benchmark-config.json`, `benchmark.ps1`, `benchmark.py`, `extended.py`, `summarize.py`: exact reproducible configuration, runner and report generator.
- `binaries.json`: versions, SHA-256 and start timestamp.
- `environment.json`: hardware, OS and power metadata.
- `corpus-manifest.json`, `corpus/`: committed source mapping/hashes and actual synthetic fixture corpus.
- `correctness.json`: individual executed checks, including unsuccessful comparisons.
- `logs/`: complete request/command outputs and errors, schema captures and server stderr. Logs/corpus may contain proprietary firmware source; review before publicly attaching them.
- `issue-786.json`: unauthenticated public API snapshot of issue requirements.
- `run.log`, `extended-run.log`, `extended-results.json`: execution progress and bounded stability evidence.
- `excluded-pilot/`: invalid nested-workspace pilot, excluded entirely from all statistics. Beta discovered the parent index and migrated it to schema 20. `parent-index-recovery.log` records a fresh Stable rebuild, verification and restoration at the original database path; the pre-recovery database is preserved. Tracked firmware files were not modified. The shortened-path corpus also follows an earlier Git path-length export failure.
- `repair-parent-index.py`: one-time recovery script, not part of benchmark reproduction.
- `benchmark-report.html`: standalone visual summary.
- `README.md`: setup/reproduction and interpretation notes.

All new timing/resource counts come from the raw CSV or its deterministic summary. No unexecuted test is reported as PASS. Upstream measurements were not mixed into this dataset.

### Public upload scope

Public upload scope: this package contains the report, numeric measurements, summaries, checks, environment metadata and reproduction scripts. The private firmware corpus, source-bearing request logs, corpus manifest and database backups are retained locally and are not included. Reproducing this exact corpus requires access to the private repository; the CSV log paths refer to retained local evidence. Benchmark execution used no GitHub authentication; this subsequent upload was explicitly requested by the user and used existing WSL GitHub authentication.

