# SQL query optimization summary

Review date: 2026-10-24. Tracking issue: [#786](https://github.com/aovestdipaperino/tokensave/issues/786).

This document consolidates the related SQL query optimizations, implementation status, benchmark decisions, validation, and remaining limitations. Published PRs are implementations awaiting merge; they are not yet in master.

| Implementation | Issue | PR | Local full-suite result |
| --- | --- | --- | --- |
| Batch file cleanup and dispatch deletion index; schema v19 | [#775](https://github.com/aovestdipaperino/tokensave/issues/775) | [#776](https://github.com/aovestdipaperino/tokensave/pull/776) | 4,406 passed, four ignored |
| Scoped complexity ranking and project/time savings index | [#777](https://github.com/aovestdipaperino/tokensave/issues/777) | [#779](https://github.com/aovestdipaperino/tokensave/pull/779) | 4,408 passed, four ignored |
| Indexed executable bodies and exact ambiguous candidates; schema v20 | [#778](https://github.com/aovestdipaperino/tokensave/issues/778) | [#781](https://github.com/aovestdipaperino/tokensave/pull/781) | 4,412 passed, four ignored |
| Q0121 scoped inheritance seeds, unrestricted ancestors | [#782](https://github.com/aovestdipaperino/tokensave/issues/782) | [#783](https://github.com/aovestdipaperino/tokensave/pull/783) | 4,404 passed, four ignored |
| Q0118/Q0130 ID/count ranking before full hydration | [#784](https://github.com/aovestdipaperino/tokensave/issues/784) | [#785](https://github.com/aovestdipaperino/tokensave/pull/785) | 4,405 passed, four ignored |
| Q0227/Q0228 weighted FTS rank ordering | [#751](https://github.com/aovestdipaperino/tokensave/issues/751) | [#787](https://github.com/aovestdipaperino/tokensave/pull/787) | 4,404 passed, four ignored |
| Benchmark review, consolidated summary, and agent rule | [#786](https://github.com/aovestdipaperino/tokensave/issues/786) | Separate summary PR; see tracking issue | Documentation/benchmark-only; no application change |

The test totals differ because these are separate branches with different dependencies. Each recorded run had zero failures and no filtered tests. Do not add these totals together.

## Dependencies and integration

- [#779](https://github.com/aovestdipaperino/tokensave/pull/779) is stacked on [#776](https://github.com/aovestdipaperino/tokensave/pull/776); [#781](https://github.com/aovestdipaperino/tokensave/pull/781) is stacked on [#779](https://github.com/aovestdipaperino/tokensave/pull/779). Rebase/retarget after their dependencies merge.
- [#783](https://github.com/aovestdipaperino/tokensave/pull/783) and [#785](https://github.com/aovestdipaperino/tokensave/pull/785) target master independently; the FTS and summary follow-ups do too. The summary PR can merge independently: its benchmark runner supports both the original BM25 SQL and the rank-column implementation.
- Reconcile query-report IDs, source links, counts, schema migrations, and changelog entries during integration, then run the full suite on the combined result.
- Schema v19/v20 use the existing full-reindex-on-migration behavior. The independent ranking/FTS changes need no migration.

## Q0119/Q0120 benchmark decisions

Native SQLite 3.46.1, ANALYZE, 12,000 nodes, 96,000 edges, LIMIT 20, two warmups, 15 alternating old/new runs:

- Q0119 deferred hydration: empty-text global median 1.352 → 1.826 ms; 2-KiB-per-text-field global 4.713 → 3.109 ms; corresponding scoped medians 1.410 → 1.429 and 2.439 → 2.366 ms. This is workload-dependent; leave the current query pending a production workload that justifies the tradeoff.
- Q0120 pre-deduplicated file pairs: no consistent benefit. With 96 files, outgoing global median 134.123 → 184.341 ms; with 12,000 files, 94.141 → 132.826 ms. Other directions/scopes range from near-equal to slower, with one small scoped win. Do not ship this candidate rewrite.

## FTS result and limitations

The implementation uses FTS5's rank-ordered cursor, configured per query with the existing BM25 weights. Cursor-local ranking work/state can still scale with the full matching set; LIMIT bounds returned rows. It retains exact scoring and kind exclusion before LIMIT; no approximate candidate truncation, stop-term heuristic, or persistent cache is introduced.

A real 24,163-node tokensave index, 31 alternating native-SQLite runs: bounded `node*` median 3.690 → 2.765 ms, bounded `search*` 0.790 → 0.462 ms, bounded boolean `search* OR node*` 4.096 → 3.141 ms. General search also improves for these frequent terms. Some rare/empty cases and the synthetic all-matching corpus regress; the benchmark report records them. Functional equivalence is also tested using bundled libSQL.

These measurements do not establish production throughput or long-running RSS behavior. They do not reproduce or eliminate every large-corpus tail case from [#751](https://github.com/aovestdipaperino/tokensave/issues/751). Keep that investigation open for broader corpora and production-level percentile measurements.

## Reproducing the measurements

Run `python3 scripts/benchmark-query-review.py --repetitions 15` for synthetic
Q0119/Q0120/FTS comparisons. For an existing production-shaped index, use
`--database /absolute/path/to/tokensave.db --repetitions 31`; that mode opens
the database read-only and benchmarks FTS only. The runner works before or after the independent FTS PR merges. No values from indexed source
text are printed. The script uses the Python runtime's SQLite, not bundled
libSQL. FTS functional equivalence is separately checked through libSQL tests.

- [Benchmark runner](../scripts/benchmark-query-review.py)
- [Synthetic raw results](benchmarks/query-review/synthetic.json)
- [Real-index raw results](benchmarks/query-review/real-index.json)

The real corpus is this repository at `4527d5a1168f66a33ce7afdb2d492d0113203bac`,
indexed in an isolated disposable clone using the production CLI. Timings use
LIMIT 20, two warmups, alternating old/new ordering, and median/nearest-rank p95
samples. Synthetic fixtures explicitly run ANALYZE; real-index mode does not
write statistics. These are warm single-process comparisons on one machine,
not statistically established p99s or application-level concurrency results.
Analytical fixtures compare complete results; FTS checks limited score
sequences because the original queries do not specify an ID tie-breaker.

## Summary rule and completion scope

[AGENTS.md](../AGENTS.md) requires a maintained consolidated summary when a topic spans multiple implementations or related PRs, including status, links, dependencies, validation, limitations, and remaining work.

The agreed implementation batch has been implemented and published, and all recorded local checks passed. The PRs remain unmerged. Q0119 is deferred as workload-sensitive; the tested Q0120 rewrite is rejected. This summary does not claim that every possible query optimization has been exhausted. Issue closure is left to the maintainer.
