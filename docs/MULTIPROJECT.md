# Multiproject Support

## Problem

The global git post-commit hook runs `tokensave sync` in every repo. Before the init/sync separation (see `tokensave init`), this silently created databases in non-enrolled repos. Even with that fix, tokensave currently assumes one project per database. Monorepos and multi-app workspaces (e.g. `Code/App1/`, `Code/App2/`) have no way to scope queries to a single sub-project.

## Goal

A single `.tokensave` database can cover multiple projects. Each node and file belongs to exactly one project. Queries gain an optional `project` filter. New cross-project queries enable comparison (e.g. "which project has the most god classes?").

## Concepts

- **Single-project mode** (default): All nodes belong to `[root]`. Existing behavior, no changes needed.
- **Multiproject mode** (`tokensave init --multiproject`): The indexer creates a project for each direct subfolder of the root. Files in the root itself belong to `[root]`.

Example:
```
Code/              <- root, init --multiproject here
  shared.rs        <- project: [root]
  App1/
    src/main.rs    <- project: App1
  App2/
    src/lib.rs     <- project: App2
```

## Approaches Considered

### Approach A: Project as a column on `nodes` + `files` (recommended)

Add `project TEXT NOT NULL DEFAULT '[root]'` to both `nodes` and `files` tables.

**Schema change**: `ALTER TABLE nodes ADD COLUMN project`; same for `files`. Migration v7 sets all existing rows to `[root]`. Fresh `create_schema` includes the column from the start.

**Query changes**: Every query that accepts `path_prefix` gains an optional `project` parameter. ~15 DB methods + ~15 MCP handlers. The SQL filter is `AND n.project = ?` when provided.

**Config**: `TokenSaveConfig` gains `multiproject: bool`. The indexer reads direct subdirectories to assign project names.

**New MCP tools**:
- `tokensave_projects` -- list available projects in the database
- Cross-project analysis tools (GROUP BY project variants of existing tools)

**Pros**:
- Clean semantic model. Project is a first-class indexed column.
- Enables SQL `GROUP BY project` for cross-project comparison queries.
- Project list is discoverable -- LLM can ask "which projects?" then scope any query.
- Fast filtering via index.

**Cons**:
- Largest diff. Every query method signature changes.
- Migration touches every existing row (sets to `[root]`).
- FTS rebuild not needed (project isn't full-text searched).

### Approach B: Project as a virtual/computed concept (no schema change)

Derive project from `file_path` at query time. Config stores the project-to-prefix mapping. A helper translates `project=App1` into `path_prefix=App1/` and reuses existing path filtering.

**Pros**: Tiny diff. No migration. No query signature changes. Fast to ship.

**Cons**: No indexed column -- can't `GROUP BY project` efficiently. Cross-project comparisons need multiple round-trips or ugly `CASE WHEN` SQL. Not truly first-class. Folder renames require manual config updates.

### Approach C: Hybrid -- column exists, queries stay path-based

Add the column (like A) but don't change existing query signatures. A resolution layer in MCP handlers converts `project` to `path_prefix`. The column is only used for new GROUP BY / cross-project queries.

**Pros**: Column exists for powerful SQL. Existing queries untouched -- smaller handler diff. Incremental migration path.

**Cons**: Two parallel filtering mechanisms (project column vs path_prefix). Must keep column in sync during indexing. Risk of drift if mapping diverges from folder structure.

## Decision

**Approach A** -- make project a first-class column.

The whole point is making project a real concept. Approach B doesn't deliver cross-project queries. Approach C creates redundant filtering that can drift. A is more work up front but the result is clean: one column, one filter, SQL GROUP BY just works.

The query signature churn is mechanical -- every `path_prefix: Option<&str>` method gains `project: Option<&str>`, every handler reads one more optional param.

## Cross-Project Query Ideas

These are new queries enabled by having `project` as a column:

- **Project summary**: node/file/edge counts per project
- **Complexity comparison**: average/max complexity per project
- **God class comparison**: god classes grouped by project
- **Dead code by project**: unreferenced symbols per project
- **Cross-project dependencies**: edges where source.project != target.project
- **Coupling matrix**: how many edges cross between each pair of projects
- **Doc coverage by project**: percentage of public symbols documented, per project

## Migration Strategy

- **v7 migration**: `ALTER TABLE nodes ADD COLUMN project TEXT NOT NULL DEFAULT '[root]'`; same for `files`. All existing rows get `[root]` via the DEFAULT. No data rewrite needed -- SQLite DEFAULT handles it.
- **Fresh databases**: `create_schema` includes the column and index from the start.
- **Index**: `CREATE INDEX idx_nodes_project ON nodes(project)` for fast filtering.

## Open Questions

- Should `project` and `path` filters be combinable? (e.g. "god classes in App1 under src/main/")
- Should `tokensave sync` in multiproject mode re-discover new subdirectories automatically, or require `tokensave init` again?
- Should there be a way to manually map project names to paths (beyond auto-discovery from subdirectories)?
