# Multi-Branch Support

## Status: Design (not implemented)

Created: 2026-04-02

## The Problem

The tokensave database lives at `<project>/.tokensave/tokensave.db`. Since `.tokensave/` is (and should be) gitignored, there is **one single DB shared across all branches**. When a user switches branches:

1. Files that exist on branch A but not branch B leave **ghost nodes/edges** in the DB.
2. Files that differ between branches have **stale data** until the next sync.
3. Incremental sync uses `content_hash` in the `files` table — it re-indexes changed files but doesn't delete nodes from files that vanished unless those files are explicitly detected as removed by `find_removed_files`.
4. Over time, switching branches back and forth causes the DB to **accumulate garbage** from every branch, growing unboundedly.

The schema has zero concept of "which branch produced this data." There is no `branch` column, no branch metadata key, and no branch-scoped DB path.

### Why it matters beyond correctness

We want to support queries like **"what changed between these two branches in terms of blast radius"** — comparing the impact graphs of two branches side by side. This requires **simultaneous access to two graphs**, which the single-DB architecture cannot provide.

## Rejected Approaches

### Tracking the DB in git (excluded from remotes)

The intuition is solid — let git's branching model give you per-branch DB state "for free." But "tracked locally, excluded from remotes" isn't a first-class git concept:

| Mechanism | Why it fails |
|---|---|
| Pre-push hook | Per-clone setup, easily bypassed, doesn't prevent `git add` |
| `.gitattributes` clean/smudge filter | Fragile, needs regeneration on checkout |
| `--skip-worktree` / `--assume-unchanged` | Prevents tracking, doesn't enable it |
| Branch-local refs | Non-standard, no tooling support |

Even if the exclusion problem were solved:

- **Binary bloat in `.git/objects`**: The DB is ~37MB. Git stores full snapshots of binary files. Ten syncs across five branches = ~1.8GB in `.git`.
- **Merge conflicts are unresolvable**: Two branches modify the DB → binary conflict. The only resolution is "pick one and re-index."
- **WAL files**: SQLite WAL mode creates `-wal` and `-shm` sidecar files that are transient and can't be committed.
- **Checkout latency**: `git checkout` would swap a 37MB binary on every branch switch.

### Single DB with branch column

Add `branch TEXT` to `files` and `nodes`:

```sql
ALTER TABLE files ADD COLUMN branch TEXT NOT NULL DEFAULT 'main';
ALTER TABLE nodes ADD COLUMN branch TEXT NOT NULL DEFAULT 'main';
```

- Every row tagged with source branch; queries filter by `WHERE branch = ?`.
- Cross-branch analysis uses different WHERE clauses in the same DB.

**Rejected because:**
- This is the DB explosion we're trying to avoid — every branch multiplies row count.
- Every query needs a `WHERE branch = ?` filter — easy to forget, hard to retrofit across all tools.
- FTS and vector indexes would need branch scoping.
- Shared nodes (identical across branches) could be deduplicated via `content_hash`, but the complexity is high.

### Nuke-on-branch-switch

Detect branch changes (store current branch in `metadata`), wipe and re-index.

**Rejected because:**
- Full re-index on every `git checkout` — too slow for large projects.
- Loses accumulated data (vectors, token counts, etc.).
- Makes cross-branch comparison impossible (only one graph exists at a time).

## Recommended: Branch-Scoped DBs with Copy-on-Switch

```
.tokensave/
  tokensave.db          → main/master (always exists, the canonical baseline)
  branches/
    feature-foo.db      → snapshot from last sync on feature-foo
    bugfix-bar.db       → snapshot from last sync on bugfix-bar
```

### How it works

**main/master** always lives at `tokensave.db` (the canonical graph). Non-default branches get their own DB under `branches/`.

### Sync flow

```
sync():
  current_branch = git_current_branch()       // e.g. "feature-x"
  stored_branch  = metadata["current_branch"]  // e.g. "main"

  if current_branch != stored_branch:
    // Branch switch detected
    active_db = resolve_db_path(stored_branch)   // .tokensave/tokensave.db
    target_db = resolve_db_path(current_branch)  // .tokensave/branches/feature-x.db

    if !target_db.exists():
      cp(active_db, target_db)   // seed from previous branch's DB

    // Swap to branch DB
    reopen(target_db)

  // Normal incremental sync against working tree
  incremental_sync()
  metadata["current_branch"] = current_branch
```

### DB path resolution

```
resolve_db_path(branch):
  "main" | "master" → .tokensave/tokensave.db
  other             → .tokensave/branches/<branch>.db
```

### The copy-on-switch seed

When tokensave first encounters a new branch, it copies the **currently active** DB as a seed. This is almost always a good starting point:

- Branch created from main → main.db is copied, incremental sync re-indexes the delta.
- Branch created from feature-a → feature-a.db is copied, minimal work needed.

The key insight: **the copy doesn't need to be from the parent branch** — git doesn't store branch parentage anyway. It just needs a good enough starting point so incremental sync does minimal work. Whatever was active before the switch is the best seed available.

### Detecting branch changes

Tokensave doesn't control `git checkout`. It only runs at two moments:

1. **Sync time** (post-commit hook or explicit `tokensave sync`)
2. **MCP tool call** (when any tool is invoked via the MCP server)

So there's no "branch create" event. Tokensave discovers the branch changed **after the fact** at next sync. The stored `current_branch` in `metadata` is compared to `git rev-parse --abbrev-ref HEAD`.

### Branch from non-main

```
git checkout main
git checkout -b feature-a    ← tokensave copies main.db → feature-a.db, syncs
# ... work, several syncs on feature-a ...
git checkout -b feature-b    ← branched from feature-a, not main
```

At next sync, tokensave sees "feature-b" for the first time. It copies from whatever was active before the switch (feature-a.db). Since feature-b was just created from feature-a, the seed is nearly identical — incremental sync only re-indexes actual changes.

### Cross-branch blast radius

With branch-scoped DBs, comparing branches is straightforward:

```rust
// Open both databases
let main_db = Database::open(".tokensave/tokensave.db")?;
let branch_db = Database::open(".tokensave/branches/feature-x.db")?;

// Diff node sets, run impact analysis on the delta
let changed_nodes = diff_graphs(&main_db, &branch_db);
let blast_radius = compute_impact(&main_db, &changed_nodes);
```

A new `tokensave_branch_impact` MCP tool would take `base` (defaults to main) and `head` (defaults to current branch), open both DBs, diff their graphs, and run impact analysis on the delta.

### Git worktree support

Git worktrees already have separate working trees, and each gets its own `.tokensave/` directory. This means worktree scenarios "just work" — each worktree is an independent tokensave project with its own DB. No special handling needed.

### Size management

Each branch DB is a full copy (~37MB in a typical project). Mitigation strategies:

- **Prune on branch deletion**: `git fetch --prune` hook or periodic cleanup in `tokensave doctor`.
- **TTL-based expiry**: Only keep branch DBs synced within the last N days.
- **`tokensave doctor` reporting**: List stale branch DBs with sizes, offer cleanup.
- **Manual cleanup**: `tokensave branch --prune` or similar.

### Advantages

- Perfect isolation — each branch's graph is always accurate.
- Cross-branch comparison is trivial (two DB handles).
- main DB is always warm — no re-index when switching back.
- Works with `git worktree` out of the box.
- No schema changes needed — same tables, same queries, different files.

### Costs

- Disk usage: ~37MB per active branch. Mitigated by pruning.
- First sync on a new branch requires a file copy (fast) + incremental sync.
- DB connection management becomes slightly more complex (resolve path per branch).

## Implementation Sequence

1. **Store current branch in metadata** — add `current_branch` key, write it on every sync.
2. **Branch-aware DB path resolution** — `resolve_db_path()` function, create `branches/` directory.
3. **Copy-on-switch** — detect branch change, copy active DB to new branch path, reopen.
4. **Prune stale branch DBs** — integrate into `tokensave doctor`, add TTL config.
5. **Cross-branch impact tool** — `tokensave_branch_impact` MCP tool, open two DBs, diff graphs.
6. **Daemon awareness** — the daemon watcher needs to detect branch switches (via HEAD change) and handle DB swapping.

## Open Questions

- Should the MCP server detect branch switches mid-session and swap DBs automatically, or only at sync time?
- Should `tokensave status` show which branch the DB represents?
- How should `tokensave sync --force` behave — re-index current branch DB only, or all branch DBs?
- Should there be a config option for max branch DB count or max total disk usage?
