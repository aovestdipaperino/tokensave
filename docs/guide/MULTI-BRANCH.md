# Multi-Branch Indexing

Optional per-branch graphs, cross-branch queries, automatic tracking, and branch quality comparisons.

tokensave can optionally maintain a separate code graph per git branch. When enabled, switching branches never gives you stale results and never re-indexes files you already parsed on another branch. Multi-branch tracking is opt-in -- without it, tokensave uses a single database for all branches.

## How it works

When you track a branch, tokensave copies the nearest ancestor DB and syncs only the files that differ. This means tracking a feature branch off `main` is nearly instant -- it only parses the files you've changed.

## CLI commands

```bash
tokensave branch add              # track the current branch
tokensave branch list             # see tracked branches and DB sizes
tokensave branch remove <name>    # stop tracking a branch
tokensave branch removeall        # remove all tracked branches except default
tokensave branch gc               # clean up branches deleted from git
tokensave branch set-default <name>  # make <name> the default branch, keeping its index
```

## Cross-branch MCP tools

Three MCP tools enable cross-branch queries without switching your checkout:

- **`tokensave_branch_search`** -- search symbols in another branch's graph
- **`tokensave_branch_diff`** -- compare code graphs between two branches: symbols added, removed, and changed (signature differs). Supports file and kind filters.
- **`tokensave_branch_list`** -- list tracked branches with DB sizes, parent branch, and sync times

## Branch fallback

When the MCP server can't find a database for the current branch, it serves from the nearest ancestor branch's DB and includes a warning in every tool response suggesting you run `tokensave branch add`.

`tokensave_status` (and the CLI `tokensave status`) also explain *why* the branch is untracked. The status response gains `auto_track` (the effective setting) and `auto_track_hint`, which says whether automatic tracking is disabled, turned off for this run by `TOKENSAVE_AUTO_TRACK`, failed, or does not apply to a read-only view, and how to track the branch or enable automatic tracking. The hint itself is reported by the status tool only; the one exception is a tracking *failure*, whose reason is appended to the shared fallback warning so that MCP clients, which never see stderr, learn about it too.

## Default branch

`tokensave init` records the repository's default branch in `.tokensave/branch-meta.json`. It uses the branch `refs/remotes/origin/HEAD` points to while that branch still exists locally or under `origin/`, then a local `main` or `master`, then the branch checked out at the time. `tokensave branch add` detects it the same way when it creates the branch metadata.

Renaming the default branch after init (`git branch -m master main`, or a rename on the hosting side followed by a local one) leaves that record naming a branch that no longer exists. Every time tokensave opens the project, it compares the record with the repository:

- **The recorded branch is gone and the detected default is untracked.** tokensave renames the entry in place, keeping its database, and logs one line such as `default branch updated: master → main (master no longer exists)`. Branches copied from the old name follow the rename. No database is copied or deleted.
- **The recorded branch is gone and the detected default is already tracked with its own database.** This happens when `main` was tracked (by `branch add` or `auto_track`) before the rename was noticed. tokensave changes nothing and prints a warning instead. Run `tokensave branch set-default main` to make that entry the default, then `tokensave branch remove master` to delete the old index.
- **Both branches still exist.** Nothing changes; you may track `master` on purpose.

`tokensave branch set-default <name>` sets the default explicitly. If `<name>` is untracked, it takes over the current default's entry and database. If `<name>` is already tracked, its entry becomes the default and the previous default stays as an ordinary branch entry, which `tokensave branch remove` can delete. The command refuses a name that is neither a local nor an `origin/` branch.

A stale default is reported by `tokensave branch list`, by the `default_branch_warning` field of the `tokensave_status` and `tokensave_branch_list` MCP tools, and as a `BRANCH_DEFAULT_STALE` warning by `tokensave verify`.

## Automatic branch tracking (v7.3.0)

Once multi-branch mode is bootstrapped (a first manual `tokensave branch add` created the branch metadata), new branches can be tracked automatically instead of falling back to the ancestor DB. Automatic tracking is **opt-in**: the `auto_track` config field defaults to `false`, and until you enable it neither mechanism below registers a branch — new branches use the ancestor fallback until you run `tokensave branch add`. Enable it per project with `"auto_track": true` in `.tokensave/config.json`, or for one run with the `TOKENSAVE_AUTO_TRACK` environment variable, which overrides the config (any value enables it except `0`, `false`, `no`, `off`, or empty). Projects in single-DB mode are never affected, and neither mechanism ever touches the default branch's database.

**Git hook (on branch checkout).** The `post-checkout` hook that `tokensave install` sets up recognizes a *branch* checkout (as opposed to a file checkout) and, when `auto_track` is enabled, tracks the checked-out branch in the background. With `auto_track` disabled the hook still runs on every checkout but deliberately does nothing and prints nothing, so a quiet checkout does not mean the hook is broken (see [diagnosing automatic tracking](#diagnosing-automatic-tracking)). Tracking is a no-op when the branch is already tracked or is the default branch, so ordinary switching between known branches costs nothing. The initial checkout of a fresh `git clone` and of a new `git worktree add` is a branch checkout too, and it can land on a branch that is not the default one (`git clone -b feature`, `git worktree add -b feature`); there the hook runs `tokensave init` first and tracks the branch after it (when `auto_track` is enabled), in that order. A hook written by an earlier version keeps the body it was installed with — the installer never rewrites an existing one — so on those installs a fresh worktree still needs open-time auto-track below, or a manual `tokensave branch add`.

**Open-time auto-track (opt-in).** When `TokenSave::open` runs — CLI command or MCP server start — and the active branch is untracked, tokensave can track it on the spot by copying the nearest tracked ancestor's DB and recording it in the branch metadata. This is gated by the same `auto_track` setting and `TOKENSAVE_AUTO_TRACK` override. Read-only views of another project (for example a `graph_root` query) never track. The copy is the same near-instant ancestor-DB copy a manual `branch add` performs; no sync runs at that moment — the `post-commit` hook keeps the new branch DB fresh as you commit, or run `tokensave sync` to refresh immediately. Auto-tracking is strictly best-effort: any failure is reported as a warning and `open()` proceeds with the usual ancestor fallback, so it can never break a tool call.

**Automatic cleanup (opt-in).** Set `"auto_gc": true` in `.tokensave/config.json`
(or `TOKENSAVE_AUTO_GC=1` for one run) to collect indexes whose local Git branches
have been deleted. Cleanup runs when a project opens and during the MCP server's
normal freshness checks. It uses the same collector as `tokensave branch gc`,
preserving the default branch, existing branches, branches used by any Git
worktree, and databases served by running TokenSave servers. Failed Git queries
or busy sync locks skip automatic cleanup without failing the command. The
default is `false`, so older projects retain their indexes until explicitly
cleaned. `TOKENSAVE_AUTO_GC=0` overrides an enabled project setting; an explicit
`tokensave branch gc` runs regardless of this setting.

In short: with `auto_track` enabled and the hook installed, checking out a new feature branch — including the branch a fresh clone or worktree starts on — transparently gives it its own per-branch graph, and even a branch created outside a checkout is picked up the first time tokensave opens the project on it. With `auto_track` disabled (the default), track each branch you want its own graph for with `tokensave branch add`.

### Diagnosing automatic tracking

Installed hooks run in the background with their output discarded, so normal checkouts stay silent. To see what the `post-checkout` hook decided, set `TOKENSAVE_HOOK_DEBUG=1` for the git command:

```bash
TOKENSAVE_HOOK_DEBUG=1 git checkout -b feature-x
cat .tokensave/hook-debug.log
```

Each decision — a skipped file checkout, tracking skipped because `auto_track` is disabled, a branch tracked, or a failure — is printed to stderr and appended, with a UNIX timestamp, to `hook-debug.log` in the project's `.tokensave` directory. The log is written only for an initialized project (the hook never creates `.tokensave` in other repositories) and starts afresh once it reaches 256 KiB. `tokensave branch add --if-enabled`, the command older hook scripts call, honours the same variable. For an untracked branch you are already on, `tokensave status` or the `tokensave_status` MCP tool explains the cause without debug logging.

See the [Multi-Branch Indexing Guide](../BRANCHING-USER-GUIDE.md) for the storage layout and
workflows in depth.

## Branch quality regressions

Compare two **existing branch indexes** at their Git tips:

```bash
tokensave quality-diff --base master --target feature/example
tokensave quality-diff --base master --target HEAD --json --fail-on regression
tokensave quality-diff --base origin/master --target feature/example --cyclomatic 15 --min-increase 5
```

Each reference must resolve to a commit and name a tracked Tokensave branch.
`HEAD` resolves to the current symbolic branch; detached HEAD requires an explicit
branch. Full `refs/heads/...` and `refs/remotes/...` names are accepted. Create and
sync each index explicitly using the existing `tokensave branch add` / `sync`
workflow before comparing. Comparison never checks out a branch or rebuilds an
index. Missing indexes and empty analysis data are errors, including comparisons
of a branch against itself.

The baseline is **tip to tip**, not the merge base. On diverged branches, a
reported difference can originate on either branch; this is not proof that a
feature introduced the difference since branching.

Supported diagnostics reuse extracted/indexed data:

| Code | Metric / identity | Default threshold |
|------|-------------------|-------------------|
| `Q_COMPLEXITY_CYCLOMATIC` | Function branches + 1 | >10 |
| `Q_COMPLEXITY_COGNITIVE` | Extracted cognitive complexity | >15 |
| `Q_FUNCTION_LINES` | Inclusive function span | >100 |
| `Q_FILE_FAN_OUT` / `Q_FILE_FAN_IN` | Distinct files reached by resolved calls/uses, or reaching this file | >10 |
| `Q_CYCLIC_DEPENDENCY_EDGE` | Directed file edge inside a cyclic strongly connected component | Present |

Change thresholds with `--cyclomatic`, `--cognitive`, `--function-lines`, and
`--fan-out` (the latter applies to both fan-in and fan-out). Existing above-threshold
issues become `worsened` when their metric increases by at least `--min-increase`
(default 3); smaller increases remain `unchanged` for gating. Crossing a threshold
is `new`; falling below it or deletion is `resolved`; a decrease above it is
`improved`. Pre-existing cyclic edges remain unchanged. An existing edge becoming
cyclic is new, even if the edge itself predates the comparison. Cycles are reported
as edges in cyclic components, not as an enumerated cycle path.

JSON includes deterministic diagnostic order, codes, categories, warning severity,
identities, base/target values, locations (1-based for functions), confidence,
classification counts and limitations. `--fail-on regression` exits **1** for new
or worsened findings, **0** otherwise. Operational errors and explicitly accepted
incomplete results exit **2**, independently of `--fail-on`. Without the flag,
complete comparisons exit 0. JSON mode writes a single JSON document to stdout,
including on operational errors.

The existing MCP tool accepts the same comparison through:

```json
{"base":"master","head":"feature/example","quality":true,"limit":20,"thresholds":{"cyclomatic":10,"min_increase":3}}
```

The MCP `report` shares the CLI schema. Counts cover all findings; details default
to 20 (maximum 500), with `truncated` and `total_findings` outside the report.
Use `limit: 0` for counts only. `thresholds` accepts the option names in the table
(`cyclomatic`, `cognitive`, `function_lines`, `fan_out`), plus `min_increase` and
`allow_stale`; unknown options are rejected. File/kind filters are rejected in
quality mode because cyclicity and coupling require global evidence. No additional
MCP tool is exposed. Quality calls skip automatic per-call sync/version reindex;
other independent activity in an already-running server retains its normal behavior.

Freshness validation hashes indexed files against Git blobs with the same source
decoder as indexing, accepting LF/CRLF line-ending conversion from Git checkout
or commit normalization. A mismatched/deleted file or an unindexed source candidate is
reported as stale/incomplete. Candidate discovery respects configured exclusion,
hidden-file and size rules; branch-specific ignore or manifest policies may explain
a candidate, so it is described as uncertain. `--allow-stale` (MCP
`thresholds.allow_stale`) permits inspection with `complete: false` and exit 2;
it never silently accepts stale evidence as a clean CI result. `complete` means
these index-content checks passed, not that every language or dependency was
analyzed. Configured exclusions and unsupported languages remain outside scope;
extractor metrics left at zero do not prove low complexity. Indexes must use
comparable analysis versions/configuration. Validation does not certify resolver
completeness or detect arbitrary manual edits to graph rows.

Locations do not participate in identity. Git-detected file renames normalize
function and dependency identities, while qualified symbol names identify functions.
Symbol renames, moves not detected by Git, and ambiguous overloads cannot be matched
reliably: unmatched findings explicitly carry `confidence: "unmatched"`, and
ambiguous same-name/kind/signature metrics are excluded from both snapshots and make the result incomplete. Overloads use their signatures to distinguish otherwise identical qualified names. Review unmatched new/resolved
pairs before using them as defects; they can still affect CI. Function-size
findings use source span, not executable statement count. Architecture policy,
dead-code, test reachability/coverage and module-size regressions are deferred.

Snapshots are read sequentially in database read transactions. Symbol bodies are
loaded one indexed file at a time; only compact metric records survive into the
comparison. File adjacency and the existing iterative SCC algorithm require
O(files + resolved file dependencies) space/time for cycle analysis. No individual
cycles are enumerated. Validation uses one Git batch process per snapshot and
one decoded blob at a time (64 MiB maximum); it reads indexed content plus Git tree
metadata, without scanning the working tree. Total cost also scales with indexed
source bytes and symbols; this deliberately trades some I/O for verifiable
freshness without commit provenance. No caches, worker pools or background jobs
are created by the comparison. Large JSON reports can be substantial; use MCP
counts or limited details when token budget matters.
