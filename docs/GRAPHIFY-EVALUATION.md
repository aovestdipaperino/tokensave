# Optional Graphify evaluation

Refs [#791](https://github.com/aovestdipaperino/tokensave/issues/791).
Evaluated on 2026-10-25. Recommendation: **documented, independent coexistence**;
defer an adapter until a lossless interchange contract and a useful retrieval
workflow have been demonstrated. This evaluation adds a reproducible probe,
not a shipped integration or a shared index.

## Evaluated revision and interfaces

Graphify package `graphifyy` **0.9.95**, commit
[`92741d162b2094ffc2fed58cfbd8ee823e9da62c`](https://github.com/Graphify-Labs/graphify/tree/92741d162b2094ffc2fed58cfbd8ee823e9da62c).
Links below are pinned; conclusions do not imply support for later releases.

| Surface | Evidence and limits |
| --- | --- |
| Local CLI | `extract . --code-only`, `explain`, and explicit graph paths are implemented in [cli.py](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/graphify/cli.py). The probe uses subprocesses, not Python implementation imports. |
| Persisted graph | [export.py](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/graphify/export.py) writes `graph.schema_version = 1` and `graph.graphify_version`. Observed JSON has `nodes` and `links`, source paths/locations, relation and confidence fields. |
| Visualization | The CLI exposes `export html --graph PATH --node-limit N`. This is a candidate consumer of a future bounded export, not a verified TokenSave import contract. Rendering was not tested. |
| Ingestion | The CLI has `merge-graphs`, but its normalization to an undirected simple graph prevents assuming lossless directed/multiple-edge interchange. Internal builders are not a supported TokenSave dependency. |
| Identity | [ids.py](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/graphify/ids.py) normalizes names through case folding, Unicode normalization, and punctuation replacement. Graphify IDs must not substitute for TokenSave IDs. |
| Provenance | The [extraction specification](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/graphify/skills/codex/references/extraction-spec.md) distinguishes `EXTRACTED`, `INFERRED`, and `AMBIGUOUS`; confidence alone does not prove a code reference. |

The fixture produced `app_entry` → `app_helper`, relation `calls`, confidence
`EXTRACTED`, source `app.py`, location `L5`. These paths are relative in the
observed AST output. Consumers must carry the corpus root and validate path
containment; they cannot assume every producer uses the same path convention.
The probe records sample nodes and edges alongside measurements.

## Concrete workflow and benefit

Use TokenSave to locate a symbol and inspect callers/impact. Independently use
Graphify to explore its own graph or visualization; optionally, after reviewing
data residency and costs, evaluate links from design documents to implementation.
The concrete question is: “Which design decision explains this call path?”
TokenSave remains the source of code navigation findings; document suggestions
must be checked against the current source before use.

This code-only probe establishes operational coexistence. It does **not**
establish that document retrieval improves this task: no model-assisted document
processing or retrieval-quality experiment was performed. Both tools parse code
and retain separate graphs, so this workflow duplicates indexing work. For a
code-only symbol query, it offers no demonstrated retrieval advantage over
TokenSave alone. A future study should compare task correctness, source citations,
latency, and token/model cost with and without document context on fixed tasks.

## Try coexistence without assistant installation

Install Graphify in an isolated environment from the pinned commit (network is
needed for installation). With `uv` installed, for example:

```bash
uv venv /tmp/graphify-evaluation
uv pip install --python /tmp/graphify-evaluation/bin/python \
  'graphifyy @ git+https://github.com/Graphify-Labs/graphify.git@92741d162b2094ffc2fed58cfbd8ee823e9da62c'
```

From each worktree root, separately:

```bash
tokensave init --no-git-hook --cpu-usage low  # once, if not already initialized
tokensave sync
GRAPHIFY_BACKEND=none GRAPHIFY_QUERY_LOG_DISABLE=1 \
  /tmp/graphify-evaluation/bin/graphify extract . --code-only
tokensave tool search helper
GRAPHIFY_BACKEND=none GRAPHIFY_QUERY_LOG_DISABLE=1 \
  /tmp/graphify-evaluation/bin/graphify explain helper
```

Replace `helper` with a symbol in your project. Keep `graphify-out/` local and
separate in every worktree. Refresh both tools after working-tree edits, branch
changes, deletion, or renaming; do not use the same output directory across
worktrees. No Graphify `install`, hook, watcher, hosted connection, or MCP server
is needed for this workflow. If Graphify is absent, continue using TokenSave;
there is no runtime dependency between them. Remove only the isolated environment
and generated output you created when finished.

Graphify's [privacy documentation](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/README.md#privacy)
distinguishes local AST processing from semantic processing through an assistant
or configured provider. Provider auto-detection makes an unqualified mixed-corpus
run unsuitable as an implicit offline workflow. The commands above explicitly
disable model backends and skip documents. Document/model processing requires a
separate opt-in decision about content, provider, credentials, cost, and residency.
Graphify also documents query logging; the probe disables it. No network trace
was collected, so configuration and source review are not a network-isolation test.

## Reproduce the observations

The Linux-only [probe](../scripts/graphify_probe.py) needs Python 3.10+, GNU
`/usr/bin/time`, git, TokenSave, and the pinned Graphify CLI. It uses a temporary
git repository and worktree, disables hooks for its git commands, avoids assistant
installation, and deletes the generated corpus on success and failure.

```bash
python3 scripts/graphify_probe.py \
  --graphify /tmp/graphify-evaluation/bin/graphify \
  --benchmark-repo "$PWD" \
  --output /tmp/graphify-results.json
```

The optional repository workload copies tracked `.rs` and `.py` working-tree
files, rejecting symlinks and limiting the copy to 5,000 files / 64 MiB. It does
not benchmark the original repository in place. Record whether that input was
dirty: the recorded HEAD alone cannot identify uncommitted contents. Each command
has a 120-second timeout; interruption terminates its process group and reaps
the direct child. Logs and intermediate results are temporary; the JSON report
is written only after every check succeeds. An unavailable CLI fails preflight.

### Validation scope

The synthetic fixture checks code-only document exclusion, schema metadata,
source-linked call provenance, valid edge endpoints, an uncommitted symbol rename,
file rename pruning, deletion, and conflicting contents in a second worktree.
Three no-op runs per tool measure repeated CLI operation. The probe also feeds
a copied graph with schema version 999 to `explain` to observe its behavior;
successful parsing does not establish compatibility with an unknown schema.

### Recorded results

Linux x86_64, Python 3.14.4, installed TokenSave 7.16.0 and Graphify 0.9.95.
The source corpus was clean for tracked `.rs`/`.py` files at TokenSave commit
`ed7ae0f49d4ee4442fceb00770c657789dffdc3e`: 524 files, 10,591,839 bytes.
The first two measurements below include indexing, not just process startup.

| Operation | Wall seconds | CPU seconds | Peak RSS (KiB) |
| --- | ---: | ---: | ---: |
| TokenSave initial index | 8.04 | 8.11 | 191,244 |
| Graphify initial extract | 26.24 | 27.00 | 175,968 |
| TokenSave no-op, three runs | 0.41 / 0.42 / 0.25 | 0.39 / 0.36 / 0.20 | 37,176 / 36,660 / 35,820 |
| Graphify no-op, three runs | 9.51 / 8.01 / 6.77 | 10.26 / 8.45 / 7.53 | 196,984 / 196,756 / 196,456 |

Sequential initial-index time was 34.28 seconds. Generated storage after these
runs was 105,727,222 bytes for `.tokensave` plus 36,661,750 for `graphify-out`
(142,388,972 combined). The isolated Graphify environment occupied 192,588 KiB
on disk, measured separately with `du -sk`; it is not included in graph storage.
The fixture's symbol queries took 0.09 seconds for TokenSave search and 0.30 for
Graphify explain; they return different information and are not retrieval-quality
comparisons. Tiny-fixture rename/update costs and raw measurements are retained
in [the repository run](../benchmarks/graphify-791-repository.json).
The final probe, including provenance and future-schema observations, passed in
[a separate fixture run](../benchmarks/graphify-791-fixture.json). The larger run
predates those two added assertions; both use the same pinned Graphify build.

`explain` accepted schema 999 in that fixture. This is an observation of a
consumer's permissiveness, not evidence that schema 999 is compatible. A future
adapter must reject unknown versions itself. No adapter was built because
lossless interchange and a beneficial document workflow remain unconfirmed.

Repository checks: Python syntax, JSON parsing, missing-executable preflight,
`git diff --check`, and `cargo fmt --all -- --check` passed. Clippy completed
with seven warnings in unchanged Rust files. `cargo test --workspace --locked`
stopped in library tests: 1,118 passed, 2 failed, 4 ignored. The failures were
`config::discovery_tests::global_state_directory_in_home_is_not_a_project` and
`config::discovery_tests::non_git_workspace_project_is_shared_by_the_repositories_inside_it`.
This change modifies no Rust code/tests; the failure causes were not investigated
and subsequent test binaries were not reached.

The two CLIs run sequentially. Peak RSS is per command as reported by GNU time,
not a sampled combined process-tree total; do not add peaks and label the sum a
measured concurrent peak. There is no persistent CLI process after completion,
so idle MCP-server memory is outside this experiment. Disk totals include the
generated directory contents, not installation or shared cache sizes. Dependency
installation and model costs are outside command timings. Results are local
observations without statistical significance or claims of leak freedom.

## Adapter requirements before implementation

An adapter remains a candidate, not an agreed contract. It would need all of:

- A bounded, documented TokenSave export through public interfaces, preserving
  original IDs and directed typed edges. Reject unsupported schema/producer
  versions before loading, rather than trusting successful JSON parsing.
- Repository and worktree identity, HEAD plus dirty-content fingerprints,
  snapshot time and source hashes. Paths and line ranges must resolve within the
  recorded root; a commit hash alone cannot describe an uncommitted view.
- An explicit edge mapping with unmappable relationships reported. Preserve
  extraction/resolution origin separately from semantic confidence. Inferred
  document edges must never enter authoritative code-reference queries.
- Snapshot replacement or explicit tombstones for deletions/renames, atomic
  publication, freshness checks, and isolation across repos with identical names.
- Export node/edge/byte limits, bounded batches/concurrency, cancellation,
  subprocess ownership and cleanup, and release of temporary graphs. A display
  node limit alone does not bound whole-graph parsing memory.
- Tests for unsupported versions, missing dependencies, partial/cancelled export,
  stale source links, conflicting worktrees, deletions, and duplicate identities.

## Maintenance and consolidated status

[pyproject.toml](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/pyproject.toml)
requires Python >=3.10 and a separate Python dependency environment, including
NetworkX, NumPy, and language parsers; extras add model/provider and server
dependencies. Dependency resolutions are not fully pinned by this probe;
environment versions from this run are recorded in
[the environment snapshot](../benchmarks/graphify-791-environment.json). Linux
x86_64 was exercised; Windows and macOS remain untested. The upstream
[LICENSE](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/LICENSE)
and [NOTICE](https://github.com/Graphify-Labs/graphify/blob/92741d162b2094ffc2fed58cfbd8ee823e9da62c/NOTICE)
identify Apache-2.0 and retained earlier MIT contributions. No upstream code is
vendored or redistributed by this change. Revisit license/notice obligations
before any bundling, and re-evaluate interfaces when upgrading the pinned revision.

| Work | Scope | Implementation / merge status | Dependencies and remaining work |
| --- | --- | --- | --- |
| [#780](https://github.com/aovestdipaperino/tokensave/issues/780) | Elasticsearch, embeddings, worktree/server concerns | Issue closed; no implementation result inferred | Graph traversal does not provide a vector index. |
| [#742](https://github.com/aovestdipaperino/tokensave/issues/742) | Shared database backends | Issue open at evaluation time; no backend result asserted here | Independent storage/process work; Graphify coexistence does not consolidate databases, handles, or MCP processes. |
| [#791](https://github.com/aovestdipaperino/tokensave/issues/791), [PR #799](https://github.com/aovestdipaperino/tokensave/pull/799) | Pinned interface review, coexistence workflow and executable probe | Draft PR; published, unmerged | No adapter. Further work: mixed-document retrieval study, representative multi-repository runs, concurrent/idle server profiling, rendering, cancellation recovery and schema rejection contract. |

Keep this table current when related PRs are added or merged. The recommendation
is limited to optional coexistence; the outstanding checks above prevent an
adapter or a retrieval-quality improvement from being described as validated.
