# Graphify compatibility fixture

This compiler-independent driver exercises the **built** experimental exporter
against an explicitly prepared, pinned Graphify installation. It performs no
installation, model call, hook registration, browser launch or cloud request.
It uses disposable repositories, private HOME/config/cache directories, model
backends disabled, and Graphify query logging disabled. Environment values and
authentication material are never printed or saved in the report.

Requires Python 3.10+, Git, the beta TokenSave binary containing `graphify-export`,
and the pinned Graphify checkout installed editable in an isolated virtualenv.
On Unix, for example:

```bash
cargo build --locked --no-default-features --features lite
git clone https://github.com/Graphify-Labs/graphify.git /tmp/graphify-791
git -C /tmp/graphify-791 checkout --detach 0a8cd4e1e342235fe53e881fb5d591bf0460071f
python3 -m venv /tmp/graphify-791-venv
/tmp/graphify-791-venv/bin/pip install --editable /tmp/graphify-791
python3 scripts/graphify/run.py --tokensave "$PWD/target/debug/tokensave" --graphify-python /tmp/graphify-791-venv/bin/python --graphify-source /tmp/graphify-791 --output /tmp/graphify-result.json --runs 5
```

Preparation downloads dependencies. The driver itself operates locally against
prepared tools. It verifies the exact upstream commit, clean tracked source,
package version, and that the virtualenv imports that editable checkout. It
records resolved dependency versions because the upstream dependency ranges
alone do not lock transitive versions. The output report must not already exist.
On Windows use the virtualenv's `Scripts/python.exe` and absolute binary paths;
Windows behavior has not been validated by the recorded run.

## Acceptance exercised

- Real source indexing and bounded JSON export, schema/producer guards, source
  locations, endpoint integrity, raw provenance, and no source bodies/docstrings.
- Actual Graphify validation, NetworkX directed-multigraph loading, explain,
  query, directed path, and HTML generation.
- Five byte-identical repeated exports and repeated reads through separate CLI
  processes; lower run counts of at least two are supported for a quick check.
- Existing output and over-budget refusal, stale source refusal, separate sync,
  full-snapshot replacement after a source change, and conflicting dirty views
  in two linked worktrees. The second branch is explicitly tracked before export;
  the exporter does not auto-track it.

Each command has a 60-second timeout; failures kill and reap the direct process.
On POSIX the driver kills the process group as well. Windows descendant-process
cleanup is unverified. Captured output is read back with a 1 MiB per-stream cap;
temporary logs and repositories are removed on success and exceptions. Sequential
commands avoid worker queues and uncontrolled parallel extraction.

## What the report establishes

[Recorded fixture evidence](../../docs/graphify/fixture-results.json) establishes
compatibility for three nodes and one edge on the reported Linux environment.
The run used a debug lite binary, Python 3.14.4 and Graphify 0.9.97.
Exporter source revision: `af60a1bdb2f6b52740a3f02dd17783b32ddbb931`;
harness source revision: `06ccb039efbdb3e3d6d34edd8bced6fa5cb25d96`. It does not
establish production readiness, retrieval quality, throughput, or a speedup.
Wall timings include process startup, Python's timed-wait/backoff overhead and
harness overhead; they are diagnostic smoke measurements. No p95/p99 conclusion
is valid from five runs. The snapshot hash is local to that temporary worktree.

The harness generates `graph.html` beside the input JSON in its temporary export
directory. It does not load the renderer's remote JavaScript or validate browser
navigation. Unknown schema rejection is a guard in this harness, not a claim
that every upstream CLI command rejects unknown versions.

## Remaining roadmap gates

Before document enrichment or production integration, use reviewed tasks and
representative small/medium/large multilingual repositories to compare TokenSave
alone, independent coexistence, and the adapter. Agree resource and quality
budgets before measuring. Capture total peak/stabilized idle RSS/PSS, CPU, disk
and cache growth, cold/warm latency, update costs, source-link accuracy, token
use, long steady-state/idle periods, failure recovery, and cancellation/shutdown.
Include model costs and explicit data scope if a semantic pass is later tested.

This fixture does not perform that assessment. Recommendation: retain a bounded
experimental export candidate for review and further measurement; defer document
enrichment, shared storage (#742), vector search, and production rollout. Track
actual PR merge states and remaining work in #791.
