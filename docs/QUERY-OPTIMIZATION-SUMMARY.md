# SQL query optimization summary

Review date: 2026-10-25. Tracking issue: [#786](https://github.com/aovestdipaperino/tokensave/issues/786).

This document consolidates the related SQL query optimizations, implementation status, benchmark decisions, validation, and remaining limitations. The seven PR branches below are integrated into `beta` in dependency order. Their application changes are not in `master`; only the AGENTS.md summary rule was separately imported there in commit `20459af`. The test counts in the table describe the original individual branches, not the combined integration.

| Implementation | Issue | PR | Local full-suite result |
| --- | --- | --- | --- |
| Batch file cleanup and dispatch deletion index; schema v19 | [#775](https://github.com/aovestdipaperino/tokensave/issues/775) | [#776](https://github.com/aovestdipaperino/tokensave/pull/776) | 4,406 passed, four ignored |
| Scoped complexity ranking and project/time savings index | [#777](https://github.com/aovestdipaperino/tokensave/issues/777) | [#779](https://github.com/aovestdipaperino/tokensave/pull/779) | 4,408 passed, four ignored |
| Indexed executable bodies and exact ambiguous candidates; schema v20 | [#778](https://github.com/aovestdipaperino/tokensave/issues/778) | [#781](https://github.com/aovestdipaperino/tokensave/pull/781) | 4,412 passed, four ignored |
| Q0121 scoped inheritance seeds, unrestricted ancestors | [#782](https://github.com/aovestdipaperino/tokensave/issues/782) | [#783](https://github.com/aovestdipaperino/tokensave/pull/783) | 4,404 passed, four ignored |
| Q0118/Q0130 ID/count ranking before full hydration | [#784](https://github.com/aovestdipaperino/tokensave/issues/784) | [#785](https://github.com/aovestdipaperino/tokensave/pull/785) | 4,405 passed, four ignored |
| Q0227/Q0228 weighted FTS rank ordering | [#751](https://github.com/aovestdipaperino/tokensave/issues/751) | [#787](https://github.com/aovestdipaperino/tokensave/pull/787) | 4,404 passed, four ignored |
| Benchmark review, consolidated summary, and agent rule | [#786](https://github.com/aovestdipaperino/tokensave/issues/786) | [#788](https://github.com/aovestdipaperino/tokensave/pull/788) | Documentation/benchmark-only; no application change |

The test totals differ because these are separate branches with different dependencies. Each recorded run had zero failures and no filtered tests. Do not add these totals together.

## Dependencies and integration

- Integration target: upstream [`beta`](https://github.com/aovestdipaperino/tokensave/tree/beta). Refs #786. The pre-existing beta history is preserved; its obsolete hook implementation was superseded by current master before integrating the optimizations. That baseline merge produces exactly the master tree.
- Merge order: #776 → #779 → #781 → #783 → #785 → #787 → #788. The #779/#781 stack is preserved. All seven PRs are retargeted to beta.
- Integration merge commits: #776 `a00e291`, #779 `6b570ae`, #781 `e458586`, #783 `59c77d9`, #785 `a7e0d54`, #787 `c01b966`, #788 `9ff2b49`. These commits record integration into beta, not promotion to master.
- Independent branch conflicts were reconciled by retaining every added regression test and changelog entry; the new inheritance/hydration entries were moved into Unreleased rather than the already published 7.16.0 section. No application source conflicts occurred within the optimization batch.
- The combined SQL report retains stable IDs, adds the scoped inheritance fragment as Q0353, contains 350 entries and 173 construction functions, and refreshes copied functions and source links against the integrated source.
- Schema v19/v20 retain the existing full-reindex-on-migration behavior. The independent ranking/FTS changes need no migration. Beta testing should use a disposable index; promoting these migrations remains a separate master integration decision.
- CI push and pull-request triggers cover both master and beta so the combined tree receives the existing Linux/macOS/Windows, format, MSRV, and clippy checks. The disabled beta release workflow remains disabled; this integration does not publish a release.
- Combined validation and remaining work are recorded below. Original branch test totals must not be added together.

## Combined beta validation and resource review

Combined local validation completed on 2026-10-25:

- Full workspace suite: **4,416 passed, zero failed, four ignored, none filtered** across 16 test-result groups, including doc tests.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked` (shared target directory): passed with warnings.
- `python3 scripts/benchmark-query-review.py --repetitions 5`: all 24 synthetic analytical/FTS cases completed with the runner's equivalence assertions. This was a smoke check under build load, not new performance evidence.
- SQL inventory: 350 unique IDs, per-file totals sum to 350; 349 SQL literals/templates matched integrated source strings after trimming outer whitespace. Q0253 is the unchanged pre-existing Ruby `begin` marker. All 173 copied construction functions were refreshed from integrated source, and changed SQL was reviewed against the report.
- Both conflicted test files retain all original async test/helper names from the six implementation branches. All seven PR heads are ancestors of the integrated tree. `git diff --check` passed.
- GitHub cross-platform CI remains pending until the beta push; local results do not establish Windows/macOS behavior.

The final full-suite run uses `cargo test --target-dir /home/tont/tokensave/target --workspace --locked` outside the filesystem sandbox. Earlier attempts exposed harness constraints: the sandbox injects a synthetic `/tmp/.git` marker that breaks two project-discovery assertions, and exporting a shared `CARGO_TARGET_DIR` makes a nested-Cargo dispatch test contend for the parent build lock. The final command avoids both conditions; the previously affected tests pass there. No application or test changes were made to bypass those failures.

Resource review of the combined source confirms that body insert parameter batches retain the 128-row / 256-KiB limits, with one input-sized oversized document allowed. Candidate membership storage scales with input candidate relationships. SQL ranking materialization and FTS cursor state remain request-scoped and can scale with the candidate or matching set; LIMIT bounds returned rows, not all database work. File deletion and body insertion retain write-lock ownership, transactions, and explicit error rollback. No new background task, polling loop, persistent cache, or concurrency layer is added by integration.

Long-running RSS stabilization, production concurrency and percentile latency, and cancellation during active writes have not been measured on the combined tree. The schema/index write overhead and rare/all-matching FTS regressions recorded below remain beta evaluation risks. Remaining work: observe cross-platform CI and exercise representative beta workflows before proposing promotion to master. Keep the tracking and performance issues open.

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

The agreed implementation batch is integrated into beta for validation; it has not been promoted to master. Original individual-branch checks passed as recorded above. Q0119 is deferred as workload-sensitive; the tested Q0120 rewrite is rejected. This summary does not claim that every possible query optimization has been exhausted. Issue closure is left to the maintainer.
