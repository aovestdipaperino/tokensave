# Branch and worktree index consistency contract

Status: **test plan; does not change branch/index routing**.

## Existing implementation to build upon

`src/worktree.rs` detects borrowed indexes using `git rev-parse --show-toplevel`; `src/branch/gc.rs` examines linked worktrees. Existing incremental-sync behavior has a known edge-correctness issue (#630). Tests must distinguish a deliberate shared index from accidentally borrowing the main checkout's index.

## Invariants

1. Results must identify the source checkout/branch/index generation they represent.
2. A linked worktree with local uncommitted changes must never quietly return the main checkout's graph as if it were local.
3. Switching branches cannot reuse an incompatible or stale snapshot without clear detection.
4. Incremental sync and a fresh full index of the same checkout should produce equivalent nodes, references, edges and file membership.
5. Worktree removal and branch garbage collection must not delete an index still used by another worktree.

## Integration matrix

| Scenario | Validation |
|---|---|
| Main checkout branch A to B and back | Verify selected index and graph identity |
| Nested linked worktree | Detect borrowed main index and warn/fail according to entrypoint contract |
| Sibling linked worktree | Ensure index discovery is explicit and no stale source attribution |
| Two worktrees on separate branches | Concurrent queries return only the expected checkout's symbols |
| Dirty worktree | No unexpected sharing of uncommitted symbol graph |
| Rename/delete/re-resolve symbol | Compare incremental graph to full rebuild; regress #630 |
| Rebase / force-update | Invalidate or verify graph provenance before returning results |
| Branch prune / worktree remove | Protect live referenced snapshots |
| Symlinked checkout | Resolve paths consistently and prevent cross-checkout confusion |

## Implementation sequence

1. Add disposable git fixture creator covering nested/sibling worktrees, commits and dirty edits.
2. Establish a normalized graph comparison that ignores unstable row IDs/timestamps.
3. Collect selected branch, index path and generation in test diagnostics.
4. Add regression tests for #630 and race-oriented worktree/GC tests.
5. Change production routing only after tests establish the intended behavior.

## Acceptance criteria

- Each matrix case is exercised automatically on supported platforms with Git available.
- Index results never silently cross worktree boundaries.
- Incremental-vs-full graph mismatches are treated as failures.
- Tests have cleanup guarantees and never modify the developer's checkout.
