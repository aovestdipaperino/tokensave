# Branch and worktree index consistency

Status: **implemented regression contract for #865**.

TokenSave may keep one index for a checkout or separate databases for tracked
branches. A linked worktree can also have its own project state. The rules in
this document prevent a checkout from silently reading or writing a graph that
belongs to a different branch or worktree.

## Invariants

1. A database is written only from the checkout and tracked branch that own it.
2. A fallback index borrowed by an untracked branch or detached HEAD is
   read-only in a multi-branch project.
3. A linked worktree with its own index returns only that worktree's files and
   graph.
4. A linked worktree nested below another checkout is not indexed as part of
   the outer checkout.
5. Incremental sync and a fresh full index of the same checkout produce the
   same file membership, nodes and edges.
6. Recovery of an interrupted write applies to the database that was being
   changed, not whichever branch opens next.
7. A single-database project deliberately follows the current working tree on
   any branch. The borrowed-index refusal begins only after the project tracks
   another branch.

## Implemented boundaries

| Boundary | Behavior | Regression coverage |
| --- | --- | --- |
| Branch switch, rename, deletion and addition | Incremental sync is compared with a full index after the switch and after returning to the original branch. | `tests/index_consistency_test.rs` |
| Rebase or forced branch update | A hard reset under the same branch name must produce the graph of the new tree. | `tests/index_consistency_test.rs` |
| Separate linked worktrees | Each worktree's index contains only its own uncommitted files. | `tests/index_consistency_test.rs` |
| Untracked branch in multi-branch mode | Explicit sync, automatic sync and stale-file refresh refuse to write into the tracked ancestor's database. | `tests/borrowed_branch_index_test.rs` |
| Detached HEAD in multi-branch mode | Sync refuses to write into the default branch database. | `tests/borrowed_branch_index_test.rs` |
| Single-database compatibility | A project that tracks only its default branch continues to sync on other branches. | `tests/borrowed_branch_index_test.rs` |
| Interrupted multi-branch write | The dirty sentinel names every affected database; recovery clears only the recovered database's entry. | `tests/dirty_sentinel_database_test.rs` and unit tests in `src/tokensave/guard.rs` |
| Nested linked worktree | Both scanner paths skip it, include/force-include cannot admit it, and sync removes rows written by an earlier release. | `tests/nested_worktree_scan_test.rs` |
| Borrowed nested-worktree index in the CLI | `status`, `sync` and `tool` warn, while `init` creates worktree-local state. | `tests/project_root_discovery_test.rs` |
| Incremental reference re-resolution | Resolver-produced edges that no longer match are removed so incremental and full graphs agree. | `tests/incremental_resolution_test.rs` (#630, #943) |

The production changes landed in #943, #995, #996, #997 and #999. The last
four were found during the #865 audit. They are independent protections: a
borrowed database is read-only, recovery state is database-specific, the CLI
surfaces borrowed worktree state, and the scanner does not cross a nested
worktree boundary.

## Operational behavior

In multi-branch mode, opening an untracked branch may use a tracked ancestor's
database for queries. This is an explicit read-only fallback. A write reports
which branch owns the borrowed index and directs the user to run
`tokensave branch add`.

A nested linked worktree without local TokenSave state may be discovered from
inside the outer checkout. The MCP server and relevant CLI entry points warn
about that mismatch. Running `tokensave init` in the worktree creates an index
at the worktree root. The outer checkout's scanner always prunes the nested
worktree, even when an include rule names it.

The dirty sentinel is shared by the project state directory but stores one
`db=` entry per unfinished database write. Opening another branch cannot consume
or hide that recovery record. Legacy sentinels without a database name retain
their earlier behavior: the next successful recovery clears them.

## Limits

- The check that a checkout still matches its database and the following
  filesystem/database operation are not one atomic OS transaction. The sync
  lock serializes TokenSave writers, and the dirty sentinel makes an
  interrupted write recoverable, but an external Git operation can still make
  a running sync fail.
- A worktree can borrow the outer checkout's index only until it is initialized
  locally. Queries during that period are accompanied by the documented
  mismatch warning; they are not presented as a worktree-local graph.
- Indexing deliberately follows an ordinary in-root symlink outside the root
  (#34). That path policy is separate from Git worktree detection.
- Tests use disposable repositories and real Git commands. Platform-specific
  Git and filesystem behavior is additionally exercised by the normal Linux,
  macOS and Windows CI matrix.

## Validation

For focused validation, run:

```text
cargo test --test integration index_consistency_test::
cargo test --test integration borrowed_branch_index_test::
cargo test --test integration dirty_sentinel_database_test::
cargo test --test integration nested_worktree_scan_test::
cargo test --test integration project_root_discovery_test::
```

The repository-wide authoritative command remains
`cargo test --workspace --locked --features test-transport`.
