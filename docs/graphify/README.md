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
| Contract (this document) | Proposed PR, targeting master; unmerged | Source inspection only; runtime acceptance belongs to the next PR |
| Bounded single-file exporter | Next dependent PR | Snapshot, provenance, limits, freshness and output tests required |
| Compatibility / measurement harness | Next dependent PR | Real Graphify readers, repeated runs and reproducible evidence required |
| Documents and product rollout | Conditional, not implemented | User benefit and resource budgets must be reviewed first |
| #742 shared storage | Separate open evaluation | No storage/backend result asserted here |

Maintain linked PR states and measured results in #791. Do not interpret an
open PR, a synthetic fixture, or a benchmark candidate as merged functionality
or a completed production integration.
