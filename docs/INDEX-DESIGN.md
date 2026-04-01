# Index Design

How tokensave builds and maintains a semantic code graph from source files, and how that graph powers diff-aware queries.

## Overview

tokensave indexes a codebase into a directed graph stored in a single SQLite database at `.tokensave/tokensave.db`. Source files are parsed with tree-sitter to extract **nodes** (code entities) and **edges** (relationships between them). Cross-file references that cannot be resolved during single-file extraction are stored as **unresolved refs** and resolved in a second pass once all files have been processed.

The graph is kept up-to-date through incremental sync: only files whose content hash has changed are re-extracted. A file-level lock prevents concurrent sync operations from the CLI and the background daemon.

## Database Schema

Schema version is tracked via `PRAGMA user_version` and advanced through sequential migrations. Current version: **5**.

### Core Tables

#### `nodes`

Every code entity produces one row. The primary key is a deterministic hash.

```sql
CREATE TABLE nodes (
    id              TEXT PRIMARY KEY,       -- "kind:32hexchars"
    kind            TEXT NOT NULL,          -- e.g. "function", "struct", "class"
    name            TEXT NOT NULL,          -- short name: "process_request"
    qualified_name  TEXT NOT NULL,          -- full path: "src/api.rs::handlers::process_request"
    file_path       TEXT NOT NULL,          -- relative to project root
    start_line      INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,
    start_column    INTEGER NOT NULL,
    end_column      INTEGER NOT NULL,
    docstring       TEXT,
    signature       TEXT,                   -- e.g. "pub async fn process_request(req: Request) -> Response"
    visibility      TEXT NOT NULL DEFAULT 'private',  -- public | pub_crate | pub_super | private
    is_async        INTEGER NOT NULL DEFAULT 0,
    -- Complexity metrics (V3)
    branches        INTEGER NOT NULL DEFAULT 0,   -- if/match/switch arms; cyclomatic = branches + 1
    loops           INTEGER NOT NULL DEFAULT 0,   -- for, while, loop
    returns         INTEGER NOT NULL DEFAULT 0,   -- return, break, continue, throw
    max_nesting     INTEGER NOT NULL DEFAULT 0,   -- max brace depth
    -- Safety metrics (V4)
    unsafe_blocks   INTEGER NOT NULL DEFAULT 0,
    unchecked_calls INTEGER NOT NULL DEFAULT 0,   -- .unwrap(), !!, force-get
    assertions      INTEGER NOT NULL DEFAULT 0,
    updated_at      INTEGER NOT NULL
);
```

**Node ID generation.** IDs are deterministic: `SHA-256(file_path + ":" + kind + ":" + name + ":" + start_line)`, truncated to 32 hex characters and prefixed with the kind. Format: `function:a1b2c3d4...`. The same entity on the same line always produces the same ID, which means incremental re-extraction of an unchanged function results in the same row being written.

**Indexes:**

```sql
CREATE INDEX idx_nodes_kind               ON nodes(kind);
CREATE INDEX idx_nodes_name               ON nodes(name);
CREATE INDEX idx_nodes_qualified_name     ON nodes(qualified_name);
CREATE INDEX idx_nodes_file_path          ON nodes(file_path);
CREATE INDEX idx_nodes_file_path_start_line ON nodes(file_path, start_line);
```

#### `edges`

Directed relationships between two nodes. An edge always points from **source** to **target**.

```sql
CREATE TABLE edges (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    source  TEXT NOT NULL,          -- FK → nodes(id)
    target  TEXT NOT NULL,          -- FK → nodes(id)
    kind    TEXT NOT NULL,          -- relationship type
    line    INTEGER,               -- source line where the relationship appears
    FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
);
```

**Edge kinds and their semantics:**

| Kind | Source | Target | Meaning |
|---|---|---|---|
| `contains` | parent entity | child entity | Structural nesting: module contains struct, struct contains method |
| `calls` | function/method | function/method | Call site at a specific line |
| `uses` | function/method | type/struct/enum | Type reference (parameter, return, field type) |
| `implements` | impl/class | trait/interface | Implementation relationship |
| `extends` | class/struct | class/struct | Inheritance |
| `annotates` | annotation/decorator | target entity | Decorator/annotation applied to a declaration |
| `derives_macro` | struct/enum | macro | Derive macro usage (Rust-specific) |
| `receives` | function/method | parameter type | Parameter type binding |
| `type_of` | entity | type | Type association |
| `returns` | function/method | type | Return type |

**Indexes:**

```sql
CREATE INDEX  idx_edges_source      ON edges(source);
CREATE INDEX  idx_edges_target      ON edges(target);
CREATE INDEX  idx_edges_kind        ON edges(kind);
CREATE INDEX  idx_edges_source_kind ON edges(source, kind);
CREATE INDEX  idx_edges_target_kind ON edges(target, kind);
CREATE UNIQUE INDEX idx_edges_unique ON edges(source, target, kind, COALESCE(line, -1));
```

The unique index (added in V5) prevents duplicate edges from accumulating across incremental syncs. `COALESCE(line, -1)` handles the case where `line` is NULL—SQLite treats two NULLs as distinct in unique constraints, so the sentinel maps them to a single bucket.

Edge insertion uses `INSERT OR IGNORE` so that duplicate edge writes are silently dropped rather than erroring.

**CASCADE behavior.** When a node is deleted (e.g. during re-indexing of its file), all edges where it appears as source or target are automatically removed by the foreign key cascade. This is the primary mechanism that keeps the graph consistent during incremental syncs.

#### `files`

One row per indexed source file. Drives change detection during sync.

```sql
CREATE TABLE files (
    path          TEXT PRIMARY KEY,      -- relative to project root
    content_hash  TEXT NOT NULL,         -- SHA-256 of file content
    size          INTEGER NOT NULL,
    modified_at   INTEGER NOT NULL,
    indexed_at    INTEGER NOT NULL,
    node_count    INTEGER NOT NULL DEFAULT 0
);
```

#### `unresolved_refs`

References discovered during extraction that cannot be resolved to a target node from the same file. Stored here and resolved in a batch pass after all files are extracted.

```sql
CREATE TABLE unresolved_refs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    from_node_id    TEXT NOT NULL,       -- FK → nodes(id): the calling/referencing node
    reference_name  TEXT NOT NULL,       -- symbol name as written in source
    reference_kind  TEXT NOT NULL,       -- edge kind to create if resolved (e.g. "calls")
    line            INTEGER NOT NULL,
    col             INTEGER NOT NULL,
    file_path       TEXT NOT NULL,
    FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
);
```

**Lifecycle.** Unresolved refs are created during extraction and consumed during reference resolution. The CASCADE ensures they are cleaned up when their owning node is deleted during re-indexing.

#### `metadata`

Simple key-value store for persistent counters and timestamps.

```sql
CREATE TABLE metadata (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Used to store `last_sync_at` and `last_full_sync_at` timestamps.

#### `vectors`

Optional embedding storage for semantic search.

```sql
CREATE TABLE vectors (
    node_id     TEXT PRIMARY KEY,
    embedding   BLOB NOT NULL,
    model       TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
);
```

### Full-Text Search

An FTS5 virtual table is maintained in sync with `nodes` via triggers:

```sql
CREATE VIRTUAL TABLE nodes_fts USING fts5(
    name, qualified_name, docstring, signature,
    content='nodes', content_rowid='rowid'
);
```

Three triggers (`nodes_fts_insert`, `nodes_fts_delete`, `nodes_fts_update`) keep the FTS index consistent with the `nodes` table automatically. Node search queries hit this index first with a prefix match; if zero results are returned, a `LIKE` fallback runs against the base table.

### SQLite Configuration

The database runs with these pragmas applied on every connection:

| Pragma | Value | Why |
|---|---|---|
| `journal_mode` | WAL | Concurrent readers during writes; daemon can serve queries while syncing |
| `foreign_keys` | ON | Enforce CASCADE deletes |
| `busy_timeout` | 120000 | 2-minute wait for locks instead of immediate SQLITE_BUSY |
| `synchronous` | NORMAL | Acceptable durability trade-off for a rebuildable index |
| `cache_size` | -65536 | 64 MB page cache |
| `temp_store` | MEMORY | Temp tables in RAM |
| `mmap_size` | 268435456 | 256 MB memory-mapped I/O |

## Extraction Pipeline

### Phase 1: Tree-sitter Parse

Each source file is parsed by a language-specific extractor that wraps a tree-sitter grammar. The `LanguageRegistry` dispatches to the correct extractor based on file extension.

**Always-available languages (Lite build):** Rust, Go, Java, Scala, TypeScript/JavaScript, Python, C, C++, Kotlin, C#, Swift.

**Feature-gated languages (Medium/Full build):** Dart, Pascal, PHP, Ruby, Bash, Protobuf, PowerShell, Nix, VB.NET, Lua, Zig, Objective-C, Perl, Batch, Fortran, COBOL, MS-BASIC, GW-BASIC, QBasic.

### Phase 2: AST Walk

Every extractor follows the same pattern:

1. **Create a file node.** The root of the extraction is always a `File` node representing the source file itself.
2. **Walk the AST.** A recursive visitor traverses the tree-sitter CST (concrete syntax tree). At each recognized node type, the extractor emits:
   - A **Node** (function, struct, class, method, etc.)
   - A **Contains edge** from the parent to the child (structural nesting)
   - **Unresolved refs** for any call sites, type references, or imports that name symbols not defined in the current file.
3. **Compute complexity metrics.** For function-like nodes, the extractor counts branches, loops, early exits, and nesting depth during the walk.

Each extractor maintains an `ExtractionState` struct that holds:
- The growing lists of nodes, edges, and unresolved refs
- A stack of `(name, node_id)` pairs for building qualified names and parent edges
- The file path and source bytes for text extraction

The output is an `ExtractionResult` containing all nodes, edges, and unresolved refs from a single file.

### Phase 3: Reference Resolution

After all files are extracted, a second pass resolves cross-file references.

The `ReferenceResolver` loads all nodes from the database into two in-memory hash maps:
- `name_cache`: nodes keyed by short name
- `qualified_name_cache`: nodes keyed by qualified name

For each `UnresolvedRef`, resolution is attempted in order:

1. **Qualified name match** (confidence 0.95) — if the reference contains `::`, look it up in the qualified name cache. Also tries suffix matching (e.g. `types::Node` matches `crate::types::Node`).
2. **Exact name match** (confidence 0.9 for unique, 0.7 for ambiguous) — look up the short name. If multiple candidates exist, a scoring heuristic picks the best:
   - Same file as reference: +100
   - Public visibility: +10
   - Callable kind for `Calls` references: +25
   - Line proximity within same file: +20 minus distance/10

Successfully resolved refs become edges with the ref's `from_node_id` as source, the resolved target node's ID as target, and the ref's `reference_kind` as the edge kind.

## Full Index (`index_all`)

1. **Clear** — delete all rows from every table.
2. **Scan** — walk the project directory, collecting files with supported extensions. Respects `.gitignore` (via the `ignore` crate) and user-configured exclude patterns.
3. **Extract** — for each file: parse with tree-sitter, walk the AST, insert nodes, intra-file edges, and unresolved refs into the database.
4. **Resolve** — load all unresolved refs, run the resolver, insert the resulting cross-file edges.
5. **Record timestamps** — write `last_full_sync_at` and `last_sync_at` to metadata.

## Incremental Sync (`sync`)

Designed for the common case where only a handful of files changed.

1. **Acquire sync lock** — atomically create `.tokensave/sync.lock` with the current PID. If the lockfile already exists and the owning PID is alive, the sync fails immediately with an error. Stale locks from dead processes are reclaimed automatically.

2. **Scan and hash** — walk the project directory and compute SHA-256 content hashes for every source file on disk.

3. **Detect changes** — compare disk state against the `files` table:
   - **Stale:** file exists in DB, hash differs → needs re-extraction.
   - **New:** file exists on disk but not in DB → needs extraction.
   - **Removed:** file exists in DB but not on disk → needs deletion.

4. **Remove deleted files** — for each removed file, call `delete_file` which removes the `files` row. The nodes for that file are deleted, and CASCADE propagates to edges, unresolved refs, and vectors.

5. **Re-index changed files** — for each stale or new file:
   - `delete_nodes_by_file`: within a single transaction, delete all edges (source OR target), unresolved refs, and vectors for every node in the file, then delete the nodes themselves.
   - Parse the file and insert fresh nodes, intra-file edges, and unresolved refs.
   - Upsert the `files` row with the new content hash.

6. **Resolve references** — if any files were re-indexed, load ALL unresolved refs from the entire database and run the resolver. Resolution is global (not scoped to changed files) because:
   - A newly added file may provide targets for previously unresolvable refs from other files.
   - A deleted file may have been the target of refs that now need re-evaluation.

   The `INSERT OR IGNORE` on edges (backed by the unique index) prevents duplicate edges from accumulating when unchanged files' unresolved refs are re-resolved.

7. **Release sync lock** — the `SyncLockGuard` is dropped, removing the lockfile.

### Change Detection Details

Change detection is content-based, not timestamp-based. The SHA-256 hash of the file's full content is compared against the stored hash. This avoids false positives from `touch` or editor save-without-change operations, and correctly detects changes even when the clock moves backward.

## How `diff_context` Uses the Index

The `tokensave_diff_context` MCP tool answers the question: *"Given these changed files, what is the semantic impact?"*

It accepts a list of file paths (typically from `git diff --name-only`) and a traversal depth, then returns:

### Step 1: Identify Modified Symbols

For each changed file, query all nodes in that file:

```sql
SELECT * FROM nodes WHERE file_path = ?
```

This gives the complete list of functions, structs, classes, etc. that live in the changed files. These are the **modified symbols**.

### Step 2: Compute Impact Radius

For each modified symbol, perform a BFS traversal over **incoming** edges up to the configured depth. This answers: *"what depends on this symbol?"*

The traversal follows all edge kinds (calls, uses, implements, extends, etc.) in the incoming direction. At each step, the traversed node is recorded as an **impacted symbol**. If the impacted symbol lives in a test file, it is flagged as an **affected test**.

The BFS respects a depth limit (default: 2) to avoid traversing the entire graph. Depth 1 gives direct dependents; depth 2 gives dependents-of-dependents.

### Step 3: File-Level Dependent BFS

A second, broader pass runs at the file level to catch test files that might not be reachable through symbol-level edges:

```
queue = [changed files]
for each file in queue (up to depth):
    dependents = get_file_dependents(file)
    for each dependent:
        if is_test_file(dependent): add to affected_tests
        else: enqueue for next depth level
```

`get_file_dependents` works by querying all nodes in a file, then finding nodes in OTHER files that have `calls` or `uses` edges pointing at them:

```sql
-- For each node in the target file:
SELECT source FROM edges WHERE target = ? AND kind IN ('calls', 'uses')
-- Then look up the source node's file_path
```

### Step 4: Return Structured Result

The output is a JSON object containing:
- `changed_files` — the input file list
- `modified_symbols` — nodes in the changed files (id, name, kind, file, line)
- `impacted_symbols` — nodes that depend on modified symbols (same shape)
- `affected_tests` — test files reachable from the changed files

This gives the AI assistant a complete picture of the blast radius of a change, without reading any source files. The assistant can then decide which files to read, which tests to run, and what might break.

## Concurrency Model

- **Single writer.** The sync lock (`.tokensave/sync.lock`) ensures only one sync or full index runs at a time. The lock is a regular file containing the PID; stale locks from crashed processes are detected by checking if the PID is still alive.
- **Concurrent readers.** WAL mode allows the MCP server to serve queries while a sync is in progress. Reads see a consistent snapshot of the database at the time they started.
- **Schema migrations.** Run inside `BEGIN EXCLUSIVE` to prevent multiple processes from migrating simultaneously. After acquiring the lock, the version is re-checked to handle the race where another process migrated between the initial check and the lock acquisition.
