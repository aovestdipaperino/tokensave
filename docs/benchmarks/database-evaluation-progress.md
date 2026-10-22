# Database evaluation progress

Tracks the multi-PR investigation for #739 (Turso), #741 (embedded backends)
and #742 (client/server backends). Status changes here only when GitHub
confirms them. A merged PR does not complete an issue; issue closure is left to
the maintainer.

Statuses: Planned, In progress, Draft PR, Ready for review, Merged, Blocked,
Skipped.

## Roadmap

| # | Work item | Issues | Depends on | Status |
| --- | --- | --- | --- | --- |
| 1 | Workload and compatibility inventory ([database-workload-inventory.md](database-workload-inventory.md)) | #739, #741, #742 | — | Draft PR #745 |
| 2 | Benchmark harness: isolated bench target, workload definitions, p50/p95/p99, JSON export, correctness checks that fail on errors | #741, #742 | — | Planned |
| 3 | Deterministic synthetic graph generator (seeded; small/medium/large tiers; nothing large committed) | #741, #742 | 2 | Planned |
| 4 | Current libSQL baseline (W1–W7 from the inventory), hardware and versions recorded | #741 | 2, 3 | Planned |
| 5 | Turso compatibility probes against the inventory's gates; `turso-compatibility.md` | #739 | 1 | Planned |
| 6 | SQLite/rusqlite comparison arm | #741 | 4 | Planned |
| 7 | Optimized libSQL experiments, each change measured separately | #741 | 4 | Planned |
| 8 | Turso benchmark arm, only for workloads that pass item 5 | #739 | 4, 5 | Planned |
| 9 | Native graph database evaluation (one or two candidates) | #741 | 4 | Planned |
| 10 | Client/server evaluation (PostgreSQL first) | #742 | 4 | Planned |
| 11 | Startup and concurrency comparison (1–16 clients) | #741, #742 | 4 | Planned |
| 12 | Consolidated report and Proposed ADR | #739, #741, #742 | all above | Planned |

Items 2 and 5 are independent of each other and can proceed in parallel.

## Findings so far

- No benchmark measurements exist yet.
- Source inventory (item 1) confirms the engine needs FTS5 with porter
  stemming and weighted `bm25`, two recursive CTEs, one named-window query,
  joined triggers, FK cascades, expression indexes and `VACUUM INTO`. These are
  the compatibility gates for items 5, 6, 8, 9 and 10.
- The existing Criterion bench (`benches/large_repos.rs`) times failed tool
  calls as successes (`Value::Null`) and covers only query latency on large
  cloned repositories. Item 2 should not reuse that error handling.

## Blockers

None.

## Next recommended task

Item 2 (benchmark harness) or item 5 (Turso compatibility probes).
