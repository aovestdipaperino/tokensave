# Multi-Branch Manual Testing

Step-by-step manual test plan for the multi-branch indexing feature. Run these
against a real project with a tokensave index. Each test builds on the previous
one, so run them in order.

## Prerequisites

- tokensave built from the branch with multi-branch support
- A git repository with an existing tokensave index (`tokensave sync` has been run)
- You are on the `main` (or `master`) branch

Verify the starting state:

```bash
tokensave status
git branch --show-current
ls .tokensave/
```

**Expected**: status prints normally, current branch is `main`, `.tokensave/`
contains `tokensave.db` and `config.json` but no `branch-meta.json` or
`branches/` directory.

---

## Test 1: Branch list without multi-branch

```bash
tokensave branch list
```

**Expected**: prints `No branch tracking configured. Run 'tokensave branch add' to start.`

---

## Test 2: Track the default branch

```bash
tokensave branch add
```

**Expected**:
- Creates `.tokensave/branch-meta.json`
- Message indicates the branch is already tracked (main's DB is `tokensave.db`,
  so copy + sync produces 0 added / 0 modified / 0 removed) OR it shows
  `branch 'main' tracked` with minimal changes.

Verify:

```bash
cat .tokensave/branch-meta.json
tokensave branch list
```

**Expected**:
- `branch-meta.json` exists with `default_branch: "main"` and a single entry
  for `main` with `db_file: "tokensave.db"`
- `branch list` shows `main *` with a size and sync time

---

## Test 3: Create and track a feature branch

```bash
git checkout -b test/multibranch
echo "// test file" > test_branch_file.rs
tokensave branch add
```

**Expected**:
- Prints `copying DB from 'main'` then `syncing changes`
- Finishes with `branch 'test/multibranch' tracked — N added, N modified, N removed`
- The numbers should be small (only the new file was added)

Verify:

```bash
ls .tokensave/branches/
tokensave branch list
```

**Expected**:
- `.tokensave/branches/test_multibranch.db` exists (slashes replaced with underscores)
- `branch list` shows two entries: `main` and `test/multibranch *` (asterisk on
  the current branch), with `(from main)` for the feature branch

---

## Test 4: Sync on the feature branch

```bash
echo "pub fn new_function() {}" >> test_branch_file.rs
tokensave sync
```

**Expected**:
- Sync reports modifications (at least `test_branch_file.rs` modified)
- Only the feature branch DB is updated, not main's

Verify by checking DB modification times:

```bash
ls -la .tokensave/tokensave.db .tokensave/branches/test_multibranch.db
```

**Expected**: `test_multibranch.db` has a more recent modification time than
`tokensave.db`.

---

## Test 5: Status shows branch info

```bash
tokensave status
tokensave status --short
```

**Expected**: Both outputs include a line like
`Branch: test/multibranch  (from main)` in the status table.

---

## Test 6: Switch back to main

```bash
git checkout main
tokensave sync
```

**Expected**: Sync runs against `tokensave.db` (main's DB). The new file from
the feature branch does not exist on disk, so sync removes it.

Verify:

```bash
tokensave status
```

**Expected**: Status shows `Branch: main` with no `(from ...)` suffix and no
`[fallback]` marker.

---

## Test 7: MCP fallback on untracked branch

```bash
git checkout -b test/untracked
tokensave serve --path . <<< '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tokensave_status","arguments":{}}}'
```

(Or start the MCP server and issue a `tokensave_status` call through your agent.)

**Expected**: The status response includes:
- `"active_branch": "test/untracked"`
- `"branch_fallback": true`
- `"branch_warning": "branch 'test/untracked' is not tracked — serving from 'main'. Run 'tokensave branch add test/untracked' to track it."`

Any other tool call should also have a `WARNING:` prefix about the fallback.

---

## Test 8: Cross-branch search

Track the untracked branch first, make a change, then search from the other:

```bash
git checkout test/multibranch
```

Issue via MCP (or test harness):

```json
{
  "name": "tokensave_branch_search",
  "arguments": {
    "branch": "main",
    "query": "new_function",
    "limit": 5
  }
}
```

**Expected**: Returns no results (or fewer results) because `new_function` only
exists on `test/multibranch`, not on `main`.

Now search in the current branch's own graph:

```json
{
  "name": "tokensave_search",
  "arguments": {
    "query": "new_function",
    "limit": 5
  }
}
```

**Expected**: Returns results including `new_function` from `test_branch_file.rs`.

---

## Test 9: Branch diff

Issue via MCP:

```json
{
  "name": "tokensave_branch_diff",
  "arguments": {
    "base": "main",
    "head": "test/multibranch"
  }
}
```

**Expected**: Returns a JSON object with:
- `summary.added > 0` (symbols from `test_branch_file.rs` are in head but not base)
- `summary.removed == 0` (we didn't delete anything)
- Each entry in `added` has `file: "test_branch_file.rs"`

Test with file filter:

```json
{
  "name": "tokensave_branch_diff",
  "arguments": {
    "base": "main",
    "head": "test/multibranch",
    "file": "test_branch_file.rs"
  }
}
```

**Expected**: Same results, filtered to only that file.

Test same-branch error:

```json
{
  "name": "tokensave_branch_diff",
  "arguments": {
    "base": "main",
    "head": "main"
  }
}
```

**Expected**: Error: `base and head are the same branch: 'main'`.

---

## Test 10: Branch list tool and resource

Issue via MCP:

```json
{
  "name": "tokensave_branch_list",
  "arguments": {}
}
```

**Expected**: Returns JSON with `branch_count >= 2`, `current_branch` matching
the checked-out branch, and a `branches` array where each entry has `name`,
`parent`, `size_bytes`, `last_synced_at`, `is_current`, `is_default`.

Also test the resource:

```json
{
  "method": "resources/read",
  "params": { "uri": "tokensave://branches" }
}
```

**Expected**: Same data in the resource contents.

---

## Test 11: Remove a tracked branch

```bash
git checkout main
tokensave branch remove test/multibranch
```

**Expected**:
- Prints `Branch 'test/multibranch' removed.`
- `.tokensave/branches/test_multibranch.db` is deleted
- WAL/SHM sidecar files are also cleaned up

Verify:

```bash
ls .tokensave/branches/
tokensave branch list
```

**Expected**: `branches/` directory is empty (or doesn't list the removed file),
`branch list` shows only `main`.

---

## Test 12: Cannot remove default branch

```bash
tokensave branch remove main
```

**Expected**: Error: `cannot remove default branch 'main'`.

---

## Test 13: Garbage collection

```bash
git checkout -b test/gc-target
tokensave branch add
git checkout main
git branch -d test/gc-target
tokensave branch gc
```

**Expected**:
- `branch gc` detects that `test/gc-target` no longer exists in git
- Prints `removed 'test/gc-target'` and `Cleaned up 1 stale branch(es).`
- The corresponding DB file is deleted

Verify:

```bash
tokensave branch list
```

**Expected**: Only `main` is listed.

---

## Test 14: GC with no stale branches

```bash
tokensave branch gc
```

**Expected**: Prints `No stale branches to clean up.`

---

## Test 15: Backward compatibility (no branch-meta.json)

```bash
rm .tokensave/branch-meta.json
rm -rf .tokensave/branches/
tokensave sync
tokensave status
```

**Expected**:
- `sync` works normally (single-DB mode)
- `status` shows no branch info line
- No errors about missing metadata

```bash
tokensave branch list
```

**Expected**: `No branch tracking configured. Run 'tokensave branch add' to start.`

---

## Test 16: Re-enable after removing metadata

```bash
tokensave branch add
```

**Expected**: Bootstraps `branch-meta.json` from scratch, tracks the current
branch. Everything works as in Test 2.

---

## Cleanup

```bash
git checkout main
git branch -D test/multibranch 2>/dev/null
git branch -D test/untracked 2>/dev/null
git branch -D test/gc-target 2>/dev/null
rm -f test_branch_file.rs
rm -f .tokensave/branch-meta.json
rm -rf .tokensave/branches/
tokensave sync
```
