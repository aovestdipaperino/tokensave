# Experimental Graphify interoperability

Tracking: [#791](https://github.com/aovestdipaperino/tokensave/issues/791).
This investigation targets master. Beta is currently reserved for data-critical PRs. It does not establish a supported production
integration or commit to later roadmap phases.

## Compatibility contract

The evaluation target is Graphify **0.9.97**, commit
`0a8cd4e1e342235fe53e881fb5d591bf0460071f` on its `v8` branch.
The older `main` branch is not the current interface reference.

| Surface | Evidence at the pinned revision | Decision |
| --- | --- | --- |
| File interchange | [export.py](https://github.com/Graphify-Labs/graphify/blob/0a8cd4e1e342235fe53e881fb5d591bf0460071f/graphify/export.py) emits NetworkX node-link JSON, schema 1 | Emit a directed multigraph with `nodes` and `links`; retain parallel edges |
| Queries | [cli.py](https://github.com/Graphify-Labs/graphify/blob/0a8cd4e1e342235fe53e881fb5d591bf0460071f/graphify/cli.py) accepts `--graph` for explain/path/query | Test actual CLI reads against an external fixture |
| Visualization | Same CLI accepts `export html --graph PATH` | Optional manual rendering in a disposable directory |
| Validation | [validate.py](https://github.com/Graphify-Labs/graphify/blob/0a8cd4e1e342235fe53e881fb5d591bf0460071f/graphify/validate.py) defines required extraction attributes | Test required fields; this validator alone does not prove reader compatibility |
| Storage | No database integration needed | Never open Graphify's private databases |
| Versions | Graph metadata has schema and producer version fields | Label TokenSave as producer; record target Graphify commit separately |

This is an observed file-reader contract at one revision, not an upstream promise
of compatibility for all future versions. Other releases require rerunning the
compatibility harness before extending the supported matrix.

## Scope and evidence rules

The first candidate exports one explicitly selected indexed source file and
relationships whose endpoints are both in that file. It excludes cross-file
edges, documents, source bodies, signatures, and docstrings. The output must
label this omission; it must never claim a complete dependency or impact graph.
This restricted scope makes an initial integration reviewable without an
unbounded repository export.

IDs are snapshot-scoped, namespaced by canonical worktree, selected source
content, and exported graph content. They are not stable rename identities or
public repository identifiers. The artifact should preserve raw resolver
provenance, edge kinds/directions/multiplicity, and source line locations.
Unknown resolver provenance must not become an `EXTRACTED` fact. Resolved edges
may be presented conservatively as `INFERRED`; unset provenance is `AMBIGUOUS`.
Neither label is a substitute for TokenSave's original provenance field.

The exporter must read a consistent DB snapshot, refuse a source hash mismatch,
refuse an untracked branch fallback, and avoid migrations, automatic sync, agent
installation, or hooks. Source freshness covers the selected file at export
checks, not all resolver inputs or later edits. Exports are immutable snapshots.
A later export replaces the complete selected snapshot rather than patching it.

## Resource and privacy requirements

Bound source bytes, nodes, edges, per-field text, serialized output, and lock
waits. Refuse over-budget input explicitly, without silently dropping in-scope
relationships. Publish only a complete artifact using a same-directory temporary
file and atomic no-clobber publication. Error/cancellation paths must remove
temporary output; the exporter owns no workers or background tasks.

No Graphify installation or model call belongs in the exporter. Exported symbol
names and paths can disclose proprietary information even without source bodies.
Keep files private and explicitly select any later external processing. Check
Graphify query logging and HTML external assets before using sensitive data.
Do not install competing assistant instructions or read-routing hooks.

## License and deployment review

The pinned [LICENSE](https://github.com/Graphify-Labs/graphify/blob/0a8cd4e1e342235fe53e881fb5d591bf0460071f/LICENSE)
is Apache-2.0; upstream also ships LICENSE-MIT and NOTICE. Keep Graphify a separate,
user-installed optional tool. This candidate copies no upstream implementation
or assets. Redistributing Graphify would require reviewing its NOTICE, bundled
components and dependency licenses separately. Do not imply Graphify Labs
endorsement or reuse its logo.

## Independent coexistence baseline

In a disposable checkout, initialize/sync TokenSave with the built TokenSave binary, then
use its scoped search/context tools for a fixed set of code questions. In a
separate environment, run the pinned Graphify CLI on the same source, code-only
with model backends disabled. Record both outputs and duplicate extraction cost.
Do not invoke `graphify install`, hooks, watch mode, or cloud services for this
assessment. A Graphify graph is not an embedding/vector index and does not
resolve shared indexing or server lifecycle concerns in #742/#780.

## Consolidated implementation status

| Work | Implementation / merge status | Validation / remaining work |
| --- | --- | --- |
| [Contract PR #849](https://github.com/aovestdipaperino/tokensave/pull/849) | Open, unmerged; targets master | Pinned source contract; runtime evidence follows separately |
| [Exporter PR #850](https://github.com/aovestdipaperino/tokensave/pull/850) | Draft, unmerged; based on master; depends on #849 | Master-based five-run Graphify fixture passed; see #791 for workspace validation and platform limits |
| [Compatibility harness](../../scripts/graphify/README.md) | Implemented in the dependent harness PR; unmerged | Five fixture runs passed, including dirty linked worktrees; [raw evidence](fixture-results.json). Representative resource/quality assessment remains open |
| Documents and product rollout | Conditional, not implemented | User benefit and resource budgets must be reviewed first |
| #742 shared storage | Separate open evaluation | No storage/backend result asserted here |

Maintain linked PR states and measured results in #791. Do not interpret an
open PR, a synthetic fixture, or a benchmark candidate as merged functionality
or a completed production integration.

## Candidate exporter usage

After initializing and syncing the selected checkout separately:

```bash
tokensave graphify-export --project /path/to/project --file src/main.rs --output /private/directory/snapshot.json
```

The output must not exist. Run another export to a new filename after edits and
sync; delete old snapshots explicitly when no longer needed. Defaults and hard
ceilings are 1,000 nodes and 5,000 internal edges. Lower them with `--max-nodes`
and `--max-edges`. An over-budget file fails instead of yielding a partial graph.
The current candidate accepts one source file, with no neighborhood expansion.

Run the separately installed pinned Graphify in a disposable directory:

```bash
graphify explain 'qualified symbol name' --graph /private/directory/snapshot.json
graphify path 'first symbol' 'second symbol' --directed --graph /private/directory/snapshot.json
graphify export html --graph /private/directory/snapshot.json --node-limit 1000
```

Graphify writes `graph.html` beside the selected JSON, even when invoked from a
different working directory. Keep the JSON in its own disposable export directory
to avoid replacing a curated visualization. The HTML renderer uses externally hosted JavaScript. Rendering success
is not a privacy or offline-browser validation. Query logs may retain questions;
consult the pinned Graphify privacy settings before querying sensitive artifacts.
The exporter itself never launches Graphify, selects a model, or sends data.

The reader uses a dedicated read-only connection and transaction, a 2 MiB SQLite
page-cache target, and a one-second SQLite busy timeout. It avoids the normal
repository-wide trait-dispatch cache and startup maintenance. Application memory
scales with the capped selected graph and two bounded source reads. SQLite query
execution has no hard CPU deadline; row budgets bound retained data, not all DB
work. No daemon, subprocess, worker, retry queue or watcher is created.


## Evaluation result

The [fixture harness](../../scripts/graphify/README.md) verified the pinned CLI's
file reader, explain/query/path and HTML generation without reparsing exported
code. It also verified conflicting dirty linked-worktree isolation after explicit
branch tracking. This establishes a small interoperability candidate, not a
benefit or resource-budget result on representative projects. Keep the exporter
experimental and defer document enrichment and product rollout until those
separate gates pass. All proposed changes remain unmerged unless #791 records
verified merges; no release publication is implied.
