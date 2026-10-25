# Query Optimizations: Eliminating N+1 Patterns

## Problem

Several graph query and traversal paths issue one SQL query per loop iteration
(the classic N+1 pattern). While each individual query is fast on a
WAL-mode SQLite database, the accumulated round-trip overhead dominates
at scale. This document catalogs each instance, explains the fix, and
provides the replacement SQL.

All affected code lives in two files:

- `src/graph/traversal.rs` — BFS/DFS traversal, callers, callees, path finding
- `src/graph/queries.rs` — dead code, file dependencies, circular dependencies

## Pattern 1: BFS/DFS Graph Traversal

**Files:** `src/graph/traversal.rs` — `traverse_bfs` (line 28), `traverse_dfs` (line 141)

### Current behavior

Each hop in the traversal issues two sequential queries:

```
while queue is not empty:
    edges = SELECT * FROM edges WHERE source = ?    -- 1 query per node
    for each edge:
        node = SELECT * FROM nodes WHERE id = ?     -- 1 query per neighbor
```

A depth-3 traversal with fan-out 5 produces ~310 queries
(5 + 25 + 125 edge queries, plus 5 + 25 + 125 node lookups).

### Fix: batch per BFS level

Process all nodes at the same depth in a single pass:

```sql
-- 1. Fetch all outgoing edges for the current frontier
SELECT source, target, kind, line
FROM edges
WHERE source IN (?1, ?2, ..., ?N)
  AND (?kind_filter = '' OR kind IN (...));

-- 2. Fetch all neighbor nodes in one go
SELECT id, kind, name, qualified_name, file_path,
       start_line, end_line, start_column, end_column,
       docstring, signature, visibility, is_async,
       branches, loops, returns, max_nesting,
       unsafe_blocks, unchecked_calls, assertions, updated_at
FROM nodes
WHERE id IN (?1, ?2, ..., ?M);
```

This reduces the query count to **2 per depth level** — 6 total for depth-3
instead of 310. The `IN (...)` clause is bounded by `opts.limit`.

### Implementation notes

- Build the `IN` placeholders dynamically from the frontier `Vec<String>`.
- libsql supports up to `SQLITE_MAX_VARIABLE_NUMBER` (default 32766) bind
  parameters, which is well above any realistic frontier size.
- For the DFS variant, collect the full stack snapshot before issuing queries
  and process results in stack order.
- The `Both` direction variant in `get_edges_for_direction` should issue a
  single `WHERE source IN (...) OR target IN (...)` instead of two calls.

### Estimated improvement

| Depth | Fan-out | Before (queries) | After (queries) | Reduction |
|-------|---------|-------------------|-----------------|-----------|
| 2     | 5       | ~60               | 4               | 15x       |
| 3     | 5       | ~310              | 6               | ~50x      |
| 3     | 10      | ~2,220            | 6               | ~370x     |

---

## Pattern 2: Dead Code Detection

**File:** `src/graph/queries.rs` — `find_dead_code` (line 44)

### Current behavior

```
all_nodes = SELECT * FROM nodes                          -- 1 query
for each node (7,563 on this repo):
    incoming = SELECT * FROM edges WHERE target = ?      -- 1 query each
    if incoming is empty: mark as dead
```

Total: **~7,564 queries**.

### Fix: single LEFT JOIN

```sql
SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
       n.start_line, n.end_line, n.start_column, n.end_column,
       n.docstring, n.signature, n.visibility, n.is_async,
       n.branches, n.loops, n.returns, n.max_nesting,
       n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at
FROM nodes n
LEFT JOIN edges e ON e.target = n.id
WHERE e.source IS NULL
  AND n.name != 'main'
  AND n.name NOT LIKE 'test%'
  AND n.visibility != 'pub';
```

With an optional kind filter:

```sql
  AND n.kind IN ('function', 'method')   -- when kinds is non-empty
```

One query. The `LEFT JOIN ... WHERE IS NULL` anti-join pattern is well
optimized by SQLite's query planner when an index exists on `edges(target)`.

### Estimated improvement

| Nodes | Before (queries) | After (queries) | Reduction |
|-------|-------------------|-----------------|-----------|
| 1,000 | 1,001            | 1               | 1,000x    |
| 7,563 | 7,564            | 1               | 7,564x    |
| 50,000| 50,001           | 1               | 50,000x   |

---

## Pattern 3: File Dependencies / Dependents

**File:** `src/graph/queries.rs` — `get_file_dependencies` (line 115),
`get_file_dependents` (line 144)

### Current behavior

```
nodes = SELECT * FROM nodes WHERE file_path = ?          -- 1 query
for each node in file (N):
    edges = SELECT * FROM edges WHERE source = ? AND kind IN (...)  -- N queries
    for each edge (M per node):
        target = SELECT * FROM nodes WHERE id = ?        -- M queries each
        collect target.file_path
```

A file with 30 nodes averaging 5 outgoing edges: **1 + 30 + 150 = 181 queries**.

### Fix: single three-way JOIN

For dependencies (outgoing):

```sql
SELECT DISTINCT n2.file_path
FROM nodes n1
JOIN edges e ON e.source = n1.id AND e.kind IN ('uses', 'calls')
JOIN nodes n2 ON n2.id = e.target
WHERE n1.file_path = ?1
  AND n2.file_path != ?1
ORDER BY n2.file_path;
```

For dependents (incoming):

```sql
SELECT DISTINCT n2.file_path
FROM nodes n1
JOIN edges e ON e.target = n1.id AND e.kind IN ('uses', 'calls')
JOIN nodes n2 ON n2.id = e.source
WHERE n1.file_path = ?1
  AND n2.file_path != ?1
ORDER BY n2.file_path;
```

### Estimated improvement

| Nodes in file | Edges per node | Before | After | Reduction |
|---------------|----------------|--------|-------|-----------|
| 10            | 3              | 41     | 1     | 41x       |
| 30            | 5              | 181    | 1     | 181x      |
| 100           | 10             | 1,101  | 1     | 1,101x    |

---

## Pattern 4: Circular Dependency Detection

**File:** `src/graph/queries.rs` — `find_circular_dependencies` (line 172)

### Current behavior

```
all_files = SELECT * FROM files                      -- 1 query
for each file (188):
    deps = get_file_dependencies(file)               -- pattern #3 per file
build adjacency list in Rust
run DFS cycle detection
```

This compounds pattern #3 across every file. With 188 files, the query
count reaches into the thousands.

### Fix: build adjacency list in one query

```sql
SELECT DISTINCT n1.file_path AS source_file, n2.file_path AS target_file
FROM nodes n1
JOIN edges e ON e.source = n1.id AND e.kind IN ('uses', 'calls')
JOIN nodes n2 ON n2.id = e.target
WHERE n1.file_path != n2.file_path;
```

Returns the entire file-level directed edge list. Build the
`HashMap<String, HashSet<String>>` adjacency list in Rust from the
result set, then run the existing `dfs_cycle_detect` unchanged.

### Estimated improvement

| Files | Before (queries) | After (queries) | Reduction          |
|-------|-------------------|-----------------|--------------------|
| 50    | hundreds          | 1               | orders of magnitude|
| 188   | thousands         | 1               | orders of magnitude|
| 1,000 | tens of thousands | 1               | orders of magnitude|

---

## Pattern 5: Callers / Callees

**File:** `src/graph/traversal.rs` — `get_callers` (line 219), `get_callees` (line 259)

### Current behavior

Identical BFS N+1 as pattern 1, but restricted to `Calls` edges:

```
while queue is not empty:
    edges = SELECT * FROM edges WHERE target = ? AND kind = 'calls'  -- 1 query
    for each edge:
        node = SELECT * FROM nodes WHERE id = ?                      -- 1 query
```

### Fix

Same batching strategy as pattern 1. Collect the frontier, issue one
`IN (...)` edge query and one `IN (...)` node query per depth level.

---

## Pattern 6: Path Finding

**File:** `src/graph/traversal.rs` — `find_path` (line 414)

### Current behavior

Each BFS step issues both outgoing and incoming edge queries, plus
node lookups during path reconstruction:

```
while queue is not empty:
    outgoing = SELECT * FROM edges WHERE source = ?     -- 1 query
    incoming = SELECT * FROM edges WHERE target = ?     -- 1 query
for each node in path:
    node = SELECT * FROM nodes WHERE id = ?             -- 1 query
```

### Fix

- Batch the BFS frontier as in pattern 1.
- For bidirectional traversal, use a single query:
  `WHERE source IN (...) OR target IN (...)`
- Batch the final path node lookups into one `WHERE id IN (...)`.

---

## Index Requirements

The optimized queries rely on these indexes existing (verify in
`src/db/migrations.rs`):

| Index                  | Columns            | Used by              |
|------------------------|--------------------|----------------------|
| `idx_edges_source`     | `edges(source)`    | Patterns 1, 3, 5, 6 |
| `idx_edges_target`     | `edges(target)`    | Patterns 2, 3, 5, 6 |
| `idx_nodes_file_path`  | `nodes(file_path)` | Patterns 3, 4        |
| `idx_nodes_kind`       | `nodes(kind)`      | Pattern 2            |

A composite index `edges(source, kind)` and `edges(target, kind)` would
further benefit queries that filter by edge kind, avoiding a filter step
after the index lookup.

## Migration Path

These optimizations can be applied incrementally since each pattern is
independent. Suggested order by impact:

1. **Pattern 2** (dead code) — simplest change, highest multiplier
2. **Pattern 4** (circular deps) — depends on pattern 3, dramatic improvement
3. **Pattern 3** (file deps/dependents) — single JOIN replaces nested loops
4. **Pattern 1** (BFS/DFS) — most invasive change, affects the core traversal
5. **Pattern 5** (callers/callees) — follows naturally from pattern 1
6. **Pattern 6** (path finding) — same technique, lower priority

For each pattern, the existing test suite (`tests/graph_test.rs`,
`tests/db_query_test.rs`) provides coverage — the optimizations change
query strategy, not behavior.
