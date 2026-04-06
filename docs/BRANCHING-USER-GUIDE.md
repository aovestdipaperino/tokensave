# Multi-Branch Indexing Guide

## The problem

Tokensave maintains a code graph in a single SQLite database per project. When you switch
git branches, the files on disk change but the graph still reflects the old branch. The daemon
eventually catches up by re-indexing changed files, but there are two costs:

1. **Stale window.** Between the checkout and the next sync, every MCP query returns results
   from the old branch. A symbol search might surface a function that doesn't exist on the
   current branch, or miss one that was just added.

2. **Redundant re-indexing.** If you alternate between `main` and `feature-x`, every switch
   triggers a differential sync that re-parses the files that differ between the two branches.
   On large projects this adds up to minutes of wasted CPU and disk I/O per day.

Multi-branch indexing solves both problems by keeping a separate database per branch. Each
branch's graph is always accurate, switching is instant, and the daemon syncs only the branch
you're actually working on.

## How it works

Multi-branch is fully opt-in. Without it, tokensave behaves exactly as before: one database,
one graph, the daemon re-indexes whatever is on disk.

When you opt in, tokensave creates a `branch-meta.json` file inside `.tokensave/` that tracks
which branches have their own database. The storage layout looks like this:

```
.tokensave/
  tokensave.db              # default branch (main/master)
  branch-meta.json          # branch tracking metadata
  branches/
    feature_foo.db          # one DB per tracked branch
    release_3_4.db
```

Creating a new branch database is cheap. Tokensave copies the nearest ancestor's database
(usually `main`) and then runs an incremental sync that only re-parses files whose content
hash differs from what's in the copy. If your branch touches 20 files out of 2,000, only
those 20 get re-indexed.

## Getting started

### Track your first branch

From a feature branch:

```
tokensave branch add
```

This detects the current branch name, copies the nearest tracked ancestor's database,
and syncs the diff. If no branch metadata exists yet, it bootstraps it automatically.

You can also track a branch by name without checking it out:

```
tokensave branch add feature/new-parser
```

### See what's tracked

```
tokensave branch list
```

Output:

```
Default branch: main

  main * — 206.3 MB, synced 5m ago
  feature/foo — 207.1 MB (from main), synced 2h 10m ago
  release/3.4 — 205.8 MB (from main), synced 1d ago
```

The `*` marks the currently checked-out branch. Each entry shows the database size, which
branch it was copied from, and when it was last synced.

### Remove a tracked branch

```
tokensave branch remove feature/foo
```

This deletes the branch's database and removes its entry from `branch-meta.json`. The
default branch cannot be removed.

### Clean up stale branches

After you merge and delete branches in git, their databases linger. To remove databases
for branches that no longer exist:

```
tokensave branch gc
```

This checks each tracked branch against `.git/refs/heads/` and `packed-refs`, and deletes
databases for branches that are gone.

## How the daemon handles branches

The daemon's behavior depends on whether multi-branch is active:

**Without multi-branch (default):** The daemon watches for file changes and syncs the single
`tokensave.db`. Switching branches triggers a sync of all changed files.

**With multi-branch:** Before each sync, the daemon checks the current branch. If that branch
is tracked, it syncs that branch's database. If it's not tracked, it syncs the default
branch's database. After syncing, it updates the `last_synced_at` timestamp in the metadata.

You don't need to restart the daemon after adding a branch. It picks up metadata changes on
the next sync cycle.

## How the MCP server selects a database

When the MCP server starts (via `tokensave mcp` or `tokensave serve`), it reads `.git/HEAD`
to determine the current branch and opens the corresponding database.

If the current branch is tracked, queries run against its own database with full accuracy.

If the current branch is not tracked, the server falls back to the nearest tracked ancestor
(determined by `git merge-base`). Every tool response is prepended with a warning:

```
WARNING: branch 'experiment-x' is not tracked — serving from 'main'.
Run `tokensave branch add experiment-x` to track it.
```

This means queries still work, but results may be stale for files that differ between the
branches.

## Cross-branch queries

Two MCP tools let you query across branches without switching your checkout:

### Search in another branch

`tokensave_branch_search` searches for symbols in a different branch's graph:

```json
{
  "branch": "main",
  "query": "parse_config",
  "limit": 5
}
```

This opens `main`'s database, runs the search, and returns results tagged with the branch
name. Useful for checking whether a symbol exists on `main` before you try to use it.

### Compare branches

`tokensave_branch_diff` compares the code graphs of two branches:

```json
{
  "base": "main",
  "head": "feature/foo"
}
```

Returns three lists:

- **added**: symbols present in `head` but not in `base`
- **removed**: symbols present in `base` but not in `head`
- **changed**: symbols present in both but with different signatures

You can filter by file path or symbol kind:

```json
{
  "base": "main",
  "head": "feature/foo",
  "file": "src/parser.rs",
  "kind": "function"
}
```

Both `base` and `head` default to sensible values: `base` defaults to the project's default
branch, `head` defaults to the current branch. So a bare `tokensave_branch_diff {}` with no
arguments compares the current branch against `main`.

## Disk usage

Each branch database is a full copy of the graph (not a delta). For a project with a 200 MB
index, each tracked branch adds roughly 200 MB. Plan accordingly:

| Tracked branches | Approximate disk usage |
|------------------|-----------------------|
| 1 (default only) | 200 MB |
| 3 | 600 MB |
| 5 | 1 GB |
| 10 | 2 GB |

Cleanup is manual. Tokensave never deletes branch databases automatically. Use
`tokensave branch gc` to clean up after merges, or `tokensave branch remove` to
delete specific branches.

## Backward compatibility

Multi-branch is fully backward compatible:

- If `branch-meta.json` doesn't exist, tokensave operates in single-database mode exactly
  as before. No behavior changes, no new files, no extra disk usage.
- Running `tokensave branch add` for the first time creates `branch-meta.json` and the
  `branches/` directory. The existing `tokensave.db` becomes the default branch's database
  with zero migration.
- `tokensave sync` and `tokensave sync --force` continue to work. With multi-branch active,
  they sync the current branch's database.

## FAQ

**Does rebasing a branch break its database?**
No. Tokensave syncs by comparing file content hashes on disk against what's stored in the
database. It doesn't track git commit history. After a rebase, the next sync re-indexes
whatever files actually changed, regardless of how the history was rewritten.

**Can I query a branch I haven't checked out?**
Yes, using `tokensave_branch_search` and `tokensave_branch_diff`. These open the target
branch's database directly without requiring a checkout.

**What happens on detached HEAD?**
The MCP server falls back to the default branch's database with a warning. The daemon syncs
the default branch's database.

**Does this work with worktrees?**
Each worktree has its own `.git/HEAD` pointing to a different branch. As long as each worktree
has been indexed (has a `.tokensave/` directory), multi-branch works independently in each one.

**Can I track branches that only exist on the remote?**
No. The branch must have a local ref in `.git/refs/heads/`. Run `git checkout` or
`git switch` to create a local tracking branch first.
