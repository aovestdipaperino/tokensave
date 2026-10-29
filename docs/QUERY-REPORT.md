# tokensave SQL query inventory

Snapshot: existing branch inventory with #835 compatibility changes inspected on 2026-10-27. Unaffected entries retain their prior coverage.

## Scope and reading guide

This report interprets “queries” as SQL used by tokensave’s own Rust application. It inventories SQL statement literals and templates throughout `src/`, including reads, writes, schema creation and migrations, triggers, verification, maintenance, transaction control, and inline test/example SQL. Vendored database implementation code, the `codegraph` submodule, external dependencies, standalone test fixtures, and benchmark workloads are outside this application inventory. Tree-sitter extraction patterns and natural-language search inputs are different query languages and are not catalogued here.

Entries are grouped by source file and retain separate occurrences, even when SQL is repeated. Each entry identifies its source line and enclosing function when available. Module-level constants and documentation/example strings are labelled as such; inline test functions are retained, so this is a source inventory rather than a runtime execution trace. A fenced block may contain an entire SQL batch or trigger definition.

Compatibility construction: Q0343–Q0349 run through `scalar` (one integer row,
no bound parameters) or an integer-only `PRAGMA application_id` template. The
origin constants and release-channel selection are in `compatibility.rs`.
Existing connection, migration, global-open, branch-copy, hook, and verification
contexts below include their compatibility checks and channel-aware paths.

Recovery context (#864): on an abandoned dirty sentinel, `TokenSave::open_with`
holds the writer lock across recovery and reindexing, and runs Q0029
(`quick_check`), then Q0030 (`needs_bulk_load_finalization`) and,
when needed, Q0034 (`end_bulk_load`) before a complete source reindex. Errors
propagate; detected corruption refuses automatic deletion. The SQL templates
and bindings are unchanged, so inventory counts remain unchanged. Regression
SQL in `tests/corruption_test.rs` remains outside the documented scope.

`?`, `?1`, and similar placeholders are bound by the caller. Rust format fields such as `{placeholders}`, `{where_clause}`, `{limit}`, and `{}` are preserved as templates; they are not executable SQL until constructed. Rust string escapes and backslash-newline continuations are decoded for readability. Dynamic SQL has potentially unlimited concrete instances, so the report includes its templates and complete builder functions in the construction appendix, including conditional clauses and parameter bindings. No SQL was executed to produce this report.

## Inventory summary

- SQL-bearing source files: **38**.
- SQL literal/template occurrences: **346**.
- Functions retained for construction/binding context: **172**.

## Source file index

| Source file | SQL occurrences |
| --- | ---: |
| [src/agents/hooks.rs](../src/agents/hooks.rs) | 4 |
| [src/agents/mod.rs](../src/agents/mod.rs) | 1 |
| [src/blame_engine.rs](../src/blame_engine.rs) | 16 |
| [src/branch.rs](../src/branch.rs) | 2 |
| [src/db/compatibility.rs](../src/db/compatibility.rs) | 7 |
| [src/db/connection.rs](../src/db/connection.rs) | 11 |
| [src/db/migrations.rs](../src/db/migrations.rs) | 53 |
| [src/db/queries/clear.rs](../src/db/queries/clear.rs) | 1 |
| [src/db/queries/edges.rs](../src/db/queries/edges.rs) | 51 |
| [src/db/queries/files.rs](../src/db/queries/files.rs) | 8 |
| [src/db/queries/fingerprints.rs](../src/db/queries/fingerprints.rs) | 3 |
| [src/db/queries/metadata.rs](../src/db/queries/metadata.rs) | 5 |
| [src/db/queries/mod.rs](../src/db/queries/mod.rs) | 3 |
| [src/db/queries/nodes.rs](../src/db/queries/nodes.rs) | 37 |
| [src/db/queries/routes.rs](../src/db/queries/routes.rs) | 18 |
| [src/db/queries/search.rs](../src/db/queries/search.rs) | 9 |
| [src/db/queries/stats.rs](../src/db/queries/stats.rs) | 5 |
| [src/db/queries/unresolved.rs](../src/db/queries/unresolved.rs) | 10 |
| [src/doctor.rs](../src/doctor.rs) | 1 |
| [src/edge_audit.rs](../src/edge_audit.rs) | 4 |
| [src/extraction/fstar_extractor.rs](../src/extraction/fstar_extractor.rs) | 2 |
| [src/extraction/ruby_extractor.rs](../src/extraction/ruby_extractor.rs) | 1 |
| [src/extraction/ruby_template_extractor.rs](../src/extraction/ruby_template_extractor.rs) | 1 |
| [src/graphify.rs](../src/graphify.rs) | 5 |
| [src/global_db.rs](../src/global_db.rs) | 41 |
| [src/graph/imports.rs](../src/graph/imports.rs) | 1 |
| [src/graph/queries.rs](../src/graph/queries.rs) | 5 |
| [src/hooks.rs](../src/hooks.rs) | 1 |
| [src/mcp/tools/definitions.rs](../src/mcp/tools/definitions.rs) | 4 |
| [src/mcp/tools/handlers/workflow.rs](../src/mcp/tools/handlers/workflow.rs) | 1 |
| [src/quality_diff.rs](../src/quality_diff.rs) | 2 |
| [src/runtime_telemetry.rs](../src/runtime_telemetry.rs) | 4 |
| [src/tokensave/memory.rs](../src/tokensave/memory.rs) | 7 |
| [src/tokensave/rename.rs](../src/tokensave/rename.rs) | 3 |
| [src/tokensave/runtime.rs](../src/tokensave/runtime.rs) | 2 |
| [src/upgrade.rs](../src/upgrade.rs) | 3 |
| [src/verify.rs](../src/verify.rs) | 12 |
| [src/worktree.rs](../src/worktree.rs) | 2 |

## SQL statements and templates

### src/agents/hooks.rs

#### Q0001 — `module constant / top-level string`

Source: [src/agents/hooks.rs:504](../src/agents/hooks.rs#L504).

```sql
commit-msg
```

#### Q0002 — `global_forwarder_runs_repository_hook_from_linked_worktree`

Source: [src/agents/hooks.rs:2381](../src/agents/hooks.rs#L2381).

```sql
commit
```

#### Q0003 — `global_forwarder_runs_repository_hook_from_linked_worktree`

Source: [src/agents/hooks.rs:2418](../src/agents/hooks.rs#L2418).

```sql
commit
```

#### Q0004 — `forwarded_hooks_cover_common_types_but_not_tokensave_owned`

Source: [src/agents/hooks.rs:2523](../src/agents/hooks.rs#L2523).

```sql
commit-msg
```

### src/agents/mod.rs

#### Q0005 — `cargo_build_binary_warning`

Source: [src/agents/mod.rs:216](../src/agents/mod.rs#L216).

```sql
release
```

### src/blame_engine.rs

#### Q0006 — `log_returns_events_when_function_body_changes`

Source: [src/blame_engine.rs:859](../src/blame_engine.rs#L859).

```sql
commit.gpgsign
```

#### Q0007 — `log_returns_events_when_function_body_changes`

Source: [src/blame_engine.rs:868](../src/blame_engine.rs#L868).

```sql
commit
```

#### Q0008 — `log_returns_events_when_function_body_changes`

Source: [src/blame_engine.rs:876](../src/blame_engine.rs#L876).

```sql
commit
```

#### Q0009 — `log_returns_events_when_function_body_changes`

Source: [src/blame_engine.rs:884](../src/blame_engine.rs#L884).

```sql
commit
```

#### Q0010 — `log_follows_function_renamed_across_files`

Source: [src/blame_engine.rs:916](../src/blame_engine.rs#L916).

```sql
commit.gpgsign
```

#### Q0011 — `log_follows_function_renamed_across_files`

Source: [src/blame_engine.rs:921](../src/blame_engine.rs#L921).

```sql
commit
```

#### Q0012 — `log_follows_function_renamed_across_files`

Source: [src/blame_engine.rs:927](../src/blame_engine.rs#L927).

```sql
commit
```

#### Q0013 — `log_follows_function_renamed_across_files`

Source: [src/blame_engine.rs:931](../src/blame_engine.rs#L931).

```sql
commit
```

#### Q0014 — `max_commits_boundary_classifies_correctly`

Source: [src/blame_engine.rs:962](../src/blame_engine.rs#L962).

```sql
commit.gpgsign
```

#### Q0015 — `max_commits_boundary_classifies_correctly`

Source: [src/blame_engine.rs:974](../src/blame_engine.rs#L974).

```sql
commit
```

#### Q0016 — `blob_size_cap_records_skipped`

Source: [src/blame_engine.rs:1003](../src/blame_engine.rs#L1003).

```sql
commit.gpgsign
```

#### Q0017 — `blob_size_cap_records_skipped`

Source: [src/blame_engine.rs:1012](../src/blame_engine.rs#L1012).

```sql
commit
```

#### Q0018 — `walk_history_yields_commits_in_reverse_chrono_order`

Source: [src/blame_engine.rs:1042](../src/blame_engine.rs#L1042).

```sql
commit.gpgsign
```

#### Q0019 — `walk_history_yields_commits_in_reverse_chrono_order`

Source: [src/blame_engine.rs:1046](../src/blame_engine.rs#L1046).

```sql
commit
```

#### Q0020 — `walk_history_yields_commits_in_reverse_chrono_order`

Source: [src/blame_engine.rs:1049](../src/blame_engine.rs#L1049).

```sql
commit
```

#### Q0021 — `walk_history_yields_commits_in_reverse_chrono_order`

Source: [src/blame_engine.rs:1052](../src/blame_engine.rs#L1052).

```sql
commit
```

### src/branch.rs

#### Q0022 — `vacuum_into`

Source: [src/branch.rs:39](../src/branch.rs#L39).

```sql
VACUUM INTO ?1
```

#### Q0023 — `copy_branch_db`

Source: [src/branch.rs:102](../src/branch.rs#L102).

```sql
PRAGMA wal_checkpoint(TRUNCATE);
```

### src/db/connection.rs

#### Q0024 — `try_open_read_only`

Source: [src/db/connection.rs:235](../src/db/connection.rs#L235).

```sql
PRAGMA query_only = 1; PRAGMA busy_timeout = 120000;
```

#### Q0025 — `try_open_read_only`

Source: [src/db/connection.rs:243](../src/db/connection.rs#L243).

```sql
PRAGMA user_version
```

#### Q0026 — `checkpoint`

Source: [src/db/connection.rs:310](../src/db/connection.rs#L310).

```sql
PRAGMA wal_checkpoint(TRUNCATE);
```

#### Q0027 — `optimize`

Source: [src/db/connection.rs:322](../src/db/connection.rs#L322).

```sql
VACUUM; ANALYZE;
```

#### Q0028 — `size`

Source: [src/db/connection.rs:336](../src/db/connection.rs#L336).

```sql
SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()
```

#### Q0029 — `quick_check`

Source: [src/db/connection.rs:372](../src/db/connection.rs#L372).

```sql
PRAGMA quick_check
```

#### Q0030 — `needs_bulk_load_finalization`

Source: [src/db/connection.rs:403](../src/db/connection.rs#L403).

```sql
SELECT name FROM sqlite_master WHERE type IN ('table', 'index')
```

#### Q0031 — `rebuild_fts`

Source: [src/db/connection.rs:432](../src/db/connection.rs#L432).

```sql
INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')
```

#### Q0032 — `apply_pragmas`

Source: [src/db/connection.rs:449](../src/db/connection.rs#L449).

```sql
PRAGMA page_size = 8192;
             PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = {busy_ms};
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -{cache_kb};
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = {mmap};
```

#### Q0033 — `begin_bulk_load`

Source: [src/db/connection.rs:473](../src/db/connection.rs#L473).

```sql
PRAGMA foreign_keys = OFF;
             DROP INDEX IF EXISTS idx_nodes_kind;
             DROP INDEX IF EXISTS idx_nodes_name;
             DROP INDEX IF EXISTS idx_nodes_qualified_name;
             DROP INDEX IF EXISTS idx_nodes_file_path;
             DROP INDEX IF EXISTS idx_nodes_file_path_start_line;
             DROP INDEX IF EXISTS idx_edges_source;
             DROP INDEX IF EXISTS idx_edges_target;
             DROP INDEX IF EXISTS idx_edges_kind;
             DROP INDEX IF EXISTS idx_edges_source_kind;
             DROP INDEX IF EXISTS idx_edges_target_kind;
             DROP INDEX IF EXISTS idx_edges_unique;
             DROP INDEX IF EXISTS idx_unresolved_refs_from_node_id;
             DROP INDEX IF EXISTS idx_unresolved_refs_reference_name;
             DROP INDEX IF EXISTS idx_unresolved_refs_file_path;
             DROP TRIGGER IF EXISTS nodes_fts_insert;
             DROP TRIGGER IF EXISTS nodes_fts_delete;
             DROP TRIGGER IF EXISTS nodes_fts_update;
             DROP TRIGGER IF EXISTS trait_dispatch_call_insert;
             DROP TRIGGER IF EXISTS trait_dispatch_implements_insert;
             DROP TRIGGER IF EXISTS trait_dispatch_call_delete;
             DROP TRIGGER IF EXISTS trait_dispatch_implements_delete;
             DELETE FROM nodes_fts;
```

#### Q0034 — `end_bulk_load`

Source: [src/db/connection.rs:516](../src/db/connection.rs#L516).

```sql
DELETE FROM edges
             WHERE rowid NOT IN (
                 SELECT MIN(rowid) FROM edges
                 GROUP BY source, target, kind, COALESCE(line, -1)
             );
             CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
             CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
             CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
             CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
             CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);
             CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
             CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
             CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique ON edges(source, target, kind, COALESCE(line, -1));
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);
             CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
                 INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
             END;
             CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
                 INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
             END;
             CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
                 INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
                 INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
             END;
             INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
                 SELECT rowid, name, qualified_name, docstring, signature, search_terms FROM nodes;
             PRAGMA foreign_keys = ON;
```

### src/db/migrations.rs

#### Q0035 — `module constant / top-level string`

Source: [src/db/migrations.rs:22](../src/db/migrations.rs#L22).

```sql
CREATE TABLE IF NOT EXISTS ambiguous_calls (
    from_node_id TEXT NOT NULL,
    reference_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line INTEGER NOT NULL,
    candidate_node_ids TEXT NOT NULL,
    PRIMARY KEY (from_node_id, reference_name, line)
);
CREATE INDEX IF NOT EXISTS idx_ambiguous_calls_file ON ambiguous_calls(file_path);
```

#### Q0036 — `module constant / top-level string`

Source: [src/db/migrations.rs:32](../src/db/migrations.rs#L32).

```sql
CREATE TRIGGER IF NOT EXISTS trait_dispatch_call_insert
AFTER INSERT ON edges WHEN NEW.kind = 'calls' BEGIN
    INSERT OR IGNORE INTO trait_dispatch_callers
        (concrete_method_id, trait_method_id, caller_id, line)
    SELECT concrete.id, trait_method.id, NEW.source, COALESCE(NEW.line, -1)
      FROM nodes trait_method
      JOIN edges dispatch
        ON dispatch.target = trait_method.parent_id
       AND dispatch.kind = 'implements'
      JOIN nodes concrete
        ON concrete.parent_id = dispatch.source
       AND concrete.name = trait_method.name
       AND concrete.kind IN ('method', 'function')
     WHERE trait_method.id = NEW.target
       AND trait_method.kind IN ('method', 'function');
END;

CREATE TRIGGER IF NOT EXISTS trait_dispatch_implements_insert
AFTER INSERT ON edges WHEN NEW.kind = 'implements' BEGIN
    INSERT OR IGNORE INTO trait_dispatch_callers
        (concrete_method_id, trait_method_id, caller_id, line)
    SELECT concrete.id, trait_method.id, call.source, COALESCE(call.line, -1)
      FROM nodes trait_method
      JOIN nodes concrete
        ON concrete.parent_id = NEW.source
       AND concrete.name = trait_method.name
       AND concrete.kind IN ('method', 'function')
      JOIN edges call
        ON call.target = trait_method.id
       AND call.kind = 'calls'
     WHERE trait_method.parent_id = NEW.target
       AND trait_method.kind IN ('method', 'function');
END;

CREATE TRIGGER IF NOT EXISTS trait_dispatch_call_delete
AFTER DELETE ON edges WHEN OLD.kind = 'calls' BEGIN
    DELETE FROM trait_dispatch_callers
     WHERE caller_id = OLD.source
       AND trait_method_id = OLD.target
       AND line = COALESCE(OLD.line, -1);
END;

CREATE TRIGGER IF NOT EXISTS trait_dispatch_implements_delete
AFTER DELETE ON edges WHEN OLD.kind = 'implements' BEGIN
    DELETE FROM trait_dispatch_callers
     WHERE concrete_method_id IN (SELECT id FROM nodes WHERE parent_id = OLD.source)
       AND trait_method_id IN (SELECT id FROM nodes WHERE parent_id = OLD.target);
END;
```

#### Q0037 — `get_version`

Source: [src/db/migrations.rs:100](../src/db/migrations.rs#L100).

```sql
PRAGMA user_version
```

#### Q0038 — `set_version`

Source: [src/db/migrations.rs:127](../src/db/migrations.rs#L127).

```sql
PRAGMA user_version = {version}
```

#### Q0039 — `create_schema`

Source: [src/db/migrations.rs:147](../src/db/migrations.rs#L147).

```sql
CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            start_column INTEGER NOT NULL,
            end_column INTEGER NOT NULL,
            docstring TEXT,
            signature TEXT,
            visibility TEXT NOT NULL DEFAULT 'private',
            is_async INTEGER NOT NULL DEFAULT 0,
            branches INTEGER NOT NULL DEFAULT 0,
            loops INTEGER NOT NULL DEFAULT 0,
            returns INTEGER NOT NULL DEFAULT 0,
            max_nesting INTEGER NOT NULL DEFAULT 0,
            unsafe_blocks INTEGER NOT NULL DEFAULT 0,
            unchecked_calls INTEGER NOT NULL DEFAULT 0,
            assertions INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL,
            attrs_start_line INTEGER NOT NULL DEFAULT 0,
            parent_id TEXT,
            cognitive_complexity INTEGER NOT NULL DEFAULT 0,
            distinct_operators INTEGER NOT NULL DEFAULT 0,
            distinct_operands INTEGER NOT NULL DEFAULT 0,
            total_operators INTEGER NOT NULL DEFAULT 0,
            total_operands INTEGER NOT NULL DEFAULT 0,
            search_terms TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            resolved_by INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL,
            node_count INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'code'
        );

        CREATE TABLE IF NOT EXISTS unresolved_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_node_id TEXT NOT NULL,
            reference_name TEXT NOT NULL,
            reference_kind TEXT NOT NULL,
            line INTEGER NOT NULL,
            col INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS vectors (
            node_id TEXT PRIMARY KEY,
            embedding BLOB NOT NULL,
            model TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name, qualified_name, docstring, signature, search_terms,
            content='nodes', content_rowid='rowid',
            tokenize='porter unicode61'
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS executable_body_fts USING fts5(
            node_id UNINDEXED,
            file_path UNINDEXED,
            body,
            tokenize='unicode61'
        );

        CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );

        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
        CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);

        CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique
            ON edges(source, target, kind, COALESCE(line, -1));

        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);

        CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name));
        CREATE INDEX IF NOT EXISTS idx_nodes_parent_id ON nodes(parent_id);

        CREATE TABLE IF NOT EXISTS node_fingerprints (
            node_id TEXT PRIMARY KEY,
            ast_hash TEXT NOT NULL,
            cfg_hash TEXT NOT NULL,
            call_seq_hash TEXT NOT NULL,
            shingles TEXT NOT NULL,
            body_tokens INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_ast ON node_fingerprints(ast_hash);
        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_size ON node_fingerprints(body_tokens);

        CREATE TABLE IF NOT EXISTS memory_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reason TEXT,
            created_at INTEGER NOT NULL,
            files TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS memory_code_areas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            description TEXT,
            last_touched_at INTEGER NOT NULL,
            touch_count INTEGER NOT NULL DEFAULT 1
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_code_areas_path ON memory_code_areas(path);
        CREATE INDEX IF NOT EXISTS idx_memory_decisions_created_at ON memory_decisions(created_at);

        CREATE TABLE IF NOT EXISTS read_cache (
            project_id   TEXT NOT NULL,
            session_id   TEXT NOT NULL,
            file_path    TEXT NOT NULL,
            mtime_ns     INTEGER NOT NULL,
            mode         TEXT NOT NULL,
            args_hash    TEXT NOT NULL,
            digest       TEXT NOT NULL,
            body         BLOB NOT NULL,
            token_count  INTEGER NOT NULL,
            created_at   INTEGER NOT NULL,
            PRIMARY KEY (project_id, session_id, file_path, mode, args_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_read_cache_session
            ON read_cache(session_id, created_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_decisions_fts USING fts5(
            text, reason,
            content='memory_decisions', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_insert
            AFTER INSERT ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_delete
            AFTER DELETE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_update
            AFTER UPDATE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;
```

#### Q0040 — `migrate`

Source: [src/db/migrations.rs:405](../src/db/migrations.rs#L405).

```sql
BEGIN EXCLUSIVE
```

#### Q0041 — `migrate`

Source: [src/db/migrations.rs:431](../src/db/migrations.rs#L431).

```sql
COMMIT
```

#### Q0042 — `migrate`

Source: [src/db/migrations.rs:440](../src/db/migrations.rs#L440).

```sql
ROLLBACK
```

#### Q0043 — `migrate_v1`

Source: [src/db/migrations.rs:555](../src/db/migrations.rs#L555).

```sql
CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            start_column INTEGER NOT NULL,
            end_column INTEGER NOT NULL,
            docstring TEXT,
            signature TEXT,
            visibility TEXT NOT NULL DEFAULT 'private',
            is_async INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL,
            node_count INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'code'
        );

        CREATE TABLE IF NOT EXISTS unresolved_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_node_id TEXT NOT NULL,
            reference_name TEXT NOT NULL,
            reference_kind TEXT NOT NULL,
            line INTEGER NOT NULL,
            col INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS vectors (
            node_id TEXT PRIMARY KEY,
            embedding BLOB NOT NULL,
            model TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );
```

#### Q0044 — `migrate_v1`

Source: [src/db/migrations.rs:619](../src/db/migrations.rs#L619).

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name,
            qualified_name,
            docstring,
            signature,
            content='nodes',
            content_rowid='rowid'
        );

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;
```

#### Q0045 — `migrate_v1`

Source: [src/db/migrations.rs:653](../src/db/migrations.rs#L653).

```sql
CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
        CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);

        CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
        CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
        CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
        CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);

        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);
```

#### Q0046 — `migrate_v2`

Source: [src/db/migrations.rs:685](../src/db/migrations.rs#L685).

```sql
CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )
```

#### Q0047 — `migrate_v2`

Source: [src/db/migrations.rs:698](../src/db/migrations.rs#L698).

```sql
DROP TABLE IF EXISTS schema_versions
```

#### Q0048 — `migrate_v3`

Source: [src/db/migrations.rs:715](../src/db/migrations.rs#L715).

```sql
ALTER TABLE nodes ADD COLUMN branches INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN loops INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN returns INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN max_nesting INTEGER NOT NULL DEFAULT 0;
```

#### Q0049 — `migrate_v4`

Source: [src/db/migrations.rs:736](../src/db/migrations.rs#L736).

```sql
ALTER TABLE nodes ADD COLUMN unsafe_blocks INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN unchecked_calls INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN assertions INTEGER NOT NULL DEFAULT 0;
```

#### Q0050 — `migrate_v5`

Source: [src/db/migrations.rs:761](../src/db/migrations.rs#L761).

```sql
CREATE TABLE edges_dedup (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        INSERT INTO edges_dedup (source, target, kind, line)
        SELECT DISTINCT source, target, kind, line FROM edges;

        DROP TABLE edges;
        ALTER TABLE edges_dedup RENAME TO edges;

        CREATE INDEX idx_edges_source ON edges(source);
        CREATE INDEX idx_edges_target ON edges(target);
        CREATE INDEX idx_edges_kind ON edges(kind);
        CREATE INDEX idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX idx_edges_target_kind ON edges(target, kind);
        CREATE UNIQUE INDEX idx_edges_unique
            ON edges(source, target, kind, COALESCE(line, -1));
```

#### Q0051 — `migrate_v6`

Source: [src/db/migrations.rs:802](../src/db/migrations.rs#L802).

```sql
CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name))
```

#### Q0052 — `migrate_v7`

Source: [src/db/migrations.rs:827](../src/db/migrations.rs#L827).

```sql
ALTER TABLE nodes ADD COLUMN attrs_start_line INTEGER NOT NULL DEFAULT 0
```

#### Q0053 — `migrate_v7`

Source: [src/db/migrations.rs:837](../src/db/migrations.rs#L837).

```sql
UPDATE nodes SET attrs_start_line = start_line WHERE attrs_start_line = 0
```

#### Q0054 — `migrate_v8`

Source: [src/db/migrations.rs:860](../src/db/migrations.rs#L860).

```sql
CREATE TABLE IF NOT EXISTS memory_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reason TEXT,
            created_at INTEGER NOT NULL,
            files TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS memory_code_areas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            description TEXT,
            last_touched_at INTEGER NOT NULL,
            touch_count INTEGER NOT NULL DEFAULT 1
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_code_areas_path
            ON memory_code_areas(path);
        CREATE INDEX IF NOT EXISTS idx_memory_decisions_created_at
            ON memory_decisions(created_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_decisions_fts USING fts5(
            text, reason,
            content='memory_decisions', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_insert
            AFTER INSERT ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_delete
            AFTER DELETE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_update
            AFTER UPDATE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;
```

#### Q0055 — `migrate_v9`

Source: [src/db/migrations.rs:931](../src/db/migrations.rs#L931).

```sql
CREATE TABLE IF NOT EXISTS read_cache (
            project_id   TEXT NOT NULL,
            session_id   TEXT NOT NULL,
            file_path    TEXT NOT NULL,
            mtime_ns     INTEGER NOT NULL,
            mode         TEXT NOT NULL,
            args_hash    TEXT NOT NULL,
            digest       TEXT NOT NULL,
            body         BLOB NOT NULL,
            token_count  INTEGER NOT NULL,
            created_at   INTEGER NOT NULL,
            PRIMARY KEY (project_id, session_id, file_path, mode, args_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_read_cache_session
            ON read_cache(session_id, created_at);
```

#### Q0056 — `migrate_v9`

Source: [src/db/migrations.rs:960](../src/db/migrations.rs#L960).

```sql
PRAGMA table_info(nodes)
```

#### Q0057 — `migrate_v9`

Source: [src/db/migrations.rs:982](../src/db/migrations.rs#L982).

```sql
ALTER TABLE nodes ADD COLUMN parent_id TEXT
```

#### Q0058 — `migrate_v9`

Source: [src/db/migrations.rs:996](../src/db/migrations.rs#L996).

```sql
SELECT 1 FROM sqlite_master WHERE type='table' AND name='edges'
```

#### Q0059 — `migrate_v9`

Source: [src/db/migrations.rs:1018](../src/db/migrations.rs#L1018).

```sql
UPDATE nodes SET parent_id = (
                SELECT source FROM edges
                WHERE edges.target = nodes.id AND edges.kind = 'contains'
                LIMIT 1
            )
```

#### Q0060 — `migrate_v9`

Source: [src/db/migrations.rs:1031](../src/db/migrations.rs#L1031).

```sql
DELETE FROM edges WHERE kind = 'contains'
```

#### Q0061 — `migrate_v9`

Source: [src/db/migrations.rs:1040](../src/db/migrations.rs#L1040).

```sql
CREATE INDEX IF NOT EXISTS idx_nodes_parent_id ON nodes(parent_id)
```

#### Q0062 — `migrate_v10`

Source: [src/db/migrations.rs:1062](../src/db/migrations.rs#L1062).

```sql
CREATE TABLE IF NOT EXISTS node_fingerprints (
            node_id TEXT PRIMARY KEY,
            ast_hash TEXT NOT NULL,
            cfg_hash TEXT NOT NULL,
            call_seq_hash TEXT NOT NULL,
            shingles TEXT NOT NULL,
            body_tokens INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_ast ON node_fingerprints(ast_hash);
        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_size ON node_fingerprints(body_tokens);
```

#### Q0063 — `migrate_v11`

Source: [src/db/migrations.rs:1115](../src/db/migrations.rs#L1115).

```sql
ALTER TABLE nodes ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0
```

#### Q0064 — `migrate_v12`

Source: [src/db/migrations.rs:1138](../src/db/migrations.rs#L1138).

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS executable_body_fts USING fts5(
            node_id UNINDEXED,
            file_path UNINDEXED,
            body,
            tokenize='unicode61'
        );
        CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );
        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);
```

#### Q0065 — `migrate_v12`

Source: [src/db/migrations.rs:1161](../src/db/migrations.rs#L1161).

```sql
SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'
```

#### Q0066 — `migrate_v13`

Source: [src/db/migrations.rs:1198](../src/db/migrations.rs#L1198).

```sql
CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );
        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);
```

#### Q0067 — `migrate_v13`

Source: [src/db/migrations.rs:1216](../src/db/migrations.rs#L1216).

```sql
SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'
```

#### Q0068 — `migrate_v14`

Source: [src/db/migrations.rs:1292](../src/db/migrations.rs#L1292).

```sql
SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'
```

#### Q0069 — `migrate_v14`

Source: [src/db/migrations.rs:1312](../src/db/migrations.rs#L1312).

```sql
DELETE FROM edges
             WHERE kind = 'annotates'
               AND target IN (
                   SELECT id FROM nodes WHERE kind IN ('annotation_usage', 'decorator')
               )
```

#### Q0070 — `migrate_v14`

Source: [src/db/migrations.rs:1329](../src/db/migrations.rs#L1329).

```sql
ALTER TABLE nodes ADD COLUMN search_terms TEXT NOT NULL DEFAULT ''
```

#### Q0071 — `migrate_v14`

Source: [src/db/migrations.rs:1341](../src/db/migrations.rs#L1341).

```sql
DROP TRIGGER IF EXISTS nodes_fts_insert;
         DROP TRIGGER IF EXISTS nodes_fts_delete;
         DROP TRIGGER IF EXISTS nodes_fts_update;
         DROP TABLE IF EXISTS nodes_fts;
```

#### Q0072 — `migrate_v14`

Source: [src/db/migrations.rs:1356](../src/db/migrations.rs#L1356).

```sql
SELECT rowid, name, qualified_name FROM nodes
```

#### Q0073 — `migrate_v14`

Source: [src/db/migrations.rs:1387](../src/db/migrations.rs#L1387).

```sql
UPDATE nodes SET search_terms = ?1 WHERE rowid = ?2
```

#### Q0074 — `migrate_v14`

Source: [src/db/migrations.rs:1400](../src/db/migrations.rs#L1400).

```sql
CREATE VIRTUAL TABLE nodes_fts USING fts5(
            name, qualified_name, docstring, signature, search_terms,
            content='nodes', content_rowid='rowid',
            tokenize='porter unicode61'
        );

        CREATE TRIGGER nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');
```

#### Q0075 — `table_exists`

Source: [src/db/migrations.rs:1459](../src/db/migrations.rs#L1459).

```sql
SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
```

#### Q0076 — `migrate_v15`

Source: [src/db/migrations.rs:1497](../src/db/migrations.rs#L1497).

```sql
CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
            CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
            CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
            CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
            CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);
```

#### Q0077 — `migrate_v15`

Source: [src/db/migrations.rs:1513](../src/db/migrations.rs#L1513).

```sql
CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
            CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
            CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique
                ON edges(source, target, kind, COALESCE(line, -1));
```

#### Q0078 — `migrate_v15`

Source: [src/db/migrations.rs:1529](../src/db/migrations.rs#L1529).

```sql
CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
            CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
            CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);
```

#### Q0079 — `migrate_v15`

Source: [src/db/migrations.rs:1548](../src/db/migrations.rs#L1548).

```sql
DROP TRIGGER IF EXISTS nodes_fts_insert;
        DROP TRIGGER IF EXISTS nodes_fts_delete;
        DROP TRIGGER IF EXISTS nodes_fts_update;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;
```

#### Q0080 — `node_columns`

Source: [src/db/migrations.rs:1593](../src/db/migrations.rs#L1593).

```sql
PRAGMA table_info(nodes)
```

#### Q0081 — `migrate_v16`

Source: [src/db/migrations.rs:1640](../src/db/migrations.rs#L1640).

```sql
ALTER TABLE files ADD COLUMN kind TEXT NOT NULL DEFAULT 'code';
```

#### Q0082 — `indexes_naming`

Source: [src/db/migrations.rs:1677](../src/db/migrations.rs#L1677).

```sql
SELECT name FROM sqlite_master
             WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL AND instr(sql, ?2) > 0
```

#### Q0083 — `table_columns`

Source: [src/db/migrations.rs:1701](../src/db/migrations.rs#L1701).

```sql
PRAGMA table_info({table})
```

#### Q0084 — `migrate_v18`

Source: [src/db/migrations.rs:1751](../src/db/migrations.rs#L1751).

```sql
PRAGMA table_info(edges)
```

#### Q0085 — `migrate_v18`

Source: [src/db/migrations.rs:1780](../src/db/migrations.rs#L1780).

```sql
DROP INDEX IF EXISTS "{index}";
```

#### Q0086 — `migrate_v18`

Source: [src/db/migrations.rs:1787](../src/db/migrations.rs#L1787).

```sql
ALTER TABLE edges DROP COLUMN resolved_by;
```

#### Q0087 — `migrate_v18`

Source: [src/db/migrations.rs:1794](../src/db/migrations.rs#L1794).

```sql
ALTER TABLE edges ADD COLUMN resolved_by INTEGER;
```

### src/db/queries/clear.rs

#### Q0088 — `clear`

Source: [src/db/queries/clear.rs:13](../src/db/queries/clear.rs#L13).

```sql
DELETE FROM vectors;
                 DELETE FROM executable_body_fts;
                 DELETE FROM trait_dispatch_callers;
                 DELETE FROM unresolved_refs;
                 DELETE FROM edges;
                 DELETE FROM nodes;
                 DELETE FROM files;
```

### src/db/queries/edges.rs

#### Q0089 — `rebuild_ruby_reopenings`

Source: [src/db/queries/edges.rs:15](../src/db/queries/edges.rs#L15).

```sql
BEGIN
```

#### Q0090 — `rebuild_ruby_reopenings`

Source: [src/db/queries/edges.rs:25](../src/db/queries/edges.rs#L25).

```sql
DELETE FROM edges WHERE kind = 'reopens'
```

#### Q0091 — `rebuild_ruby_reopenings`

Source: [src/db/queries/edges.rs:29](../src/db/queries/edges.rs#L29).

```sql
INSERT INTO edges (source, target, kind, line)
                     SELECT id, canonical_id, 'reopens', start_line
                     FROM (
                         SELECT id, start_line,
                                FIRST_VALUE(id) OVER declaration AS canonical_id,
                                ROW_NUMBER() OVER declaration AS declaration_number
                         FROM (
                             SELECT id, kind, file_path, start_line, start_column,
                                    ltrim(CASE
                                        WHEN substr(qualified_name, 1, length(file_path) * 2 + 4) = file_path || '::' || file_path || '::'
                                        THEN substr(qualified_name, length(file_path) * 2 + 5)
                                        ELSE substr(qualified_name, length(file_path) + 3)
                                    END, ':') AS constant_name
                             FROM nodes
                             WHERE kind IN ('class', 'module')
                               AND (file_path LIKE '%.rb' OR file_path LIKE '%.rake' OR file_path LIKE '%.erb' OR file_path LIKE '%.slim')
                               AND qualified_name NOT LIKE '%<anonymous>%'
                               AND substr(qualified_name, 1, length(file_path) + 2) = file_path || '::'
                         )
                         WINDOW declaration AS (
                             PARTITION BY kind, constant_name
                             ORDER BY file_path, start_line, start_column, id
                         )
                     )
                     WHERE declaration_number > 1
```

#### Q0092 — `rebuild_ruby_reopenings`

Source: [src/db/queries/edges.rs:64](../src/db/queries/edges.rs#L64).

```sql
COMMIT
```

#### Q0093 — `rebuild_ruby_reopenings`

Source: [src/db/queries/edges.rs:72](../src/db/queries/edges.rs#L72).

```sql
ROLLBACK
```

#### Q0094 — `insert_edge`

Source: [src/db/queries/edges.rs:88](../src/db/queries/edges.rs#L88).

```sql
UPDATE nodes SET parent_id = ?1 WHERE id = ?2
```

#### Q0095 — `insert_edge`

Source: [src/db/queries/edges.rs:101](../src/db/queries/edges.rs#L101).

```sql
INSERT INTO edges (source, target, kind, line, resolved_by) SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS (SELECT 1 FROM nodes WHERE id = ?1) AND EXISTS (SELECT 1 FROM nodes WHERE id = ?2){EDGE_UPSERT_CLAUSE}
```

#### Q0096 — `insert_edges`

Source: [src/db/queries/edges.rs:135](../src/db/queries/edges.rs#L135).

```sql
BEGIN
```

#### Q0097 — `insert_edges`

Source: [src/db/queries/edges.rs:148](../src/db/queries/edges.rs#L148).

```sql
INSERT INTO edges (source, target, kind, line, resolved_by) SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS (SELECT 1 FROM nodes WHERE id = ?1) AND EXISTS (SELECT 1 FROM nodes WHERE id = ?2){EDGE_UPSERT_CLAUSE}
```

#### Q0098 — `insert_edges`

Source: [src/db/queries/edges.rs:161](../src/db/queries/edges.rs#L161).

```sql
UPDATE nodes SET parent_id = ?1 WHERE id = ?2
```

#### Q0099 — `insert_edges`

Source: [src/db/queries/edges.rs:196](../src/db/queries/edges.rs#L196).

```sql
COMMIT
```

#### Q0100 — `get_outgoing_edges`

Source: [src/db/queries/edges.rs:218](../src/db/queries/edges.rs#L218).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE source = ?1
```

#### Q0101 — `get_outgoing_edges`

Source: [src/db/queries/edges.rs:235](../src/db/queries/edges.rs#L235).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE source = ?1 AND kind IN ({})
```

#### Q0102 — `get_incoming_edges`

Source: [src/db/queries/edges.rs:270](../src/db/queries/edges.rs#L270).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE target = ?1
```

#### Q0103 — `get_incoming_edges`

Source: [src/db/queries/edges.rs:287](../src/db/queries/edges.rs#L287).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE target = ?1 AND kind IN ({})
```

#### Q0104 — `rebuild_trait_dispatch_callers`

Source: [src/db/queries/edges.rs:316](../src/db/queries/edges.rs#L316).

```sql
DELETE FROM trait_dispatch_callers;
                 INSERT OR IGNORE INTO trait_dispatch_callers
                     (concrete_method_id, trait_method_id, caller_id, line)
                 SELECT concrete.id, trait_method.id, call.source, COALESCE(call.line, -1)
                   FROM edges dispatch
                   JOIN nodes trait_method
                     ON trait_method.parent_id = dispatch.target
                    AND trait_method.kind IN ('method', 'function')
                   JOIN nodes concrete
                     ON concrete.parent_id = dispatch.source
                    AND concrete.name = trait_method.name
                    AND concrete.kind IN ('method', 'function')
                   JOIN edges call
                     ON call.target = trait_method.id
                    AND call.kind = 'calls'
                  WHERE dispatch.kind = 'implements';
```

#### Q0105 — `refresh_trait_dispatch_callers`

Source: [src/db/queries/edges.rs:346](../src/db/queries/edges.rs#L346).

```sql
SELECT caller.id, caller.kind, caller.name, caller.qualified_name, caller.file_path,
                        caller.start_line, caller.end_line, caller.start_column, caller.end_column,
                        caller.docstring, caller.signature, caller.visibility, caller.is_async,
                        caller.branches, caller.loops, caller.returns, caller.max_nesting,
                        caller.unsafe_blocks, caller.unchecked_calls, caller.assertions,
                        caller.updated_at, caller.attrs_start_line, caller.parent_id,
                        caller.cognitive_complexity, caller.distinct_operators,
                        caller.distinct_operands, caller.total_operators, caller.total_operands,
                        dispatch.concrete_method_id, dispatch.trait_method_id, dispatch.line,
                        EXISTS(
                            SELECT 1 FROM edges upstream
                             WHERE upstream.target = caller.id
                               AND upstream.kind = 'calls'
                        )
                   FROM trait_dispatch_callers dispatch
                   JOIN nodes caller ON caller.id = dispatch.caller_id
                  ORDER BY dispatch.concrete_method_id, caller.file_path, caller.start_line
```

#### Q0106 — `get_incoming_edges_bulk`

Source: [src/db/queries/edges.rs:461](../src/db/queries/edges.rs#L461).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE target IN ({})
```

#### Q0107 — `get_incoming_edges_bulk`

Source: [src/db/queries/edges.rs:472](../src/db/queries/edges.rs#L472).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE target IN ({}) AND kind IN ({})
```

#### Q0108 — `get_test_annotated_node_ids`

Source: [src/db/queries/edges.rs:504](../src/db/queries/edges.rs#L504).

```sql
SELECT DISTINCT e.target FROM edges e JOIN nodes n ON e.source = n.id WHERE n.kind = 'annotation_usage' AND n.name = 'test' AND e.kind = 'annotates' AND e.target IN ({})
```

#### Q0109 — `get_annotation_histogram`

Source: [src/db/queries/edges.rs:552](../src/db/queries/edges.rs#L552).

```sql
SELECT name, COUNT(*) AS n FROM nodes WHERE kind = 'annotation_usage' AND
```

#### Q0110 — `get_annotation_histogram`

Source: [src/db/queries/edges.rs:561](../src/db/queries/edges.rs#L561).

```sql
SELECT name, COUNT(*) AS n FROM nodes WHERE kind = 'annotation_usage' GROUP BY name ORDER BY n DESC, name ASC
```

#### Q0111 — `get_annotation_sites`

Source: [src/db/queries/edges.rs:612](../src/db/queries/edges.rs#L612).

```sql
SELECT a.name AS annotation, a.file_path AS a_file, a.start_line AS a_line, t.id AS target_id, t.name AS target_name, t.kind AS target_kind, t.file_path AS target_file, t.start_line AS target_line, t.qualified_name AS target_qname FROM edges e JOIN nodes a ON e.source = a.id JOIN nodes t ON e.target = t.id WHERE a.kind = 'annotation_usage' AND e.kind = 'annotates'
```

#### Q0112 — `get_files_with_test_annotations`

Source: [src/db/queries/edges.rs:679](../src/db/queries/edges.rs#L679).

```sql
SELECT DISTINCT t.file_path FROM edges e JOIN nodes n ON e.source = n.id JOIN nodes t ON e.target = t.id WHERE n.kind = 'annotation_usage' AND n.name = 'test' AND e.kind = 'annotates' AND t.kind IN ('function', 'method', 'singleton_method')
```

#### Q0113 — `get_skip_test_coverage_node_ids`

Source: [src/db/queries/edges.rs:709](../src/db/queries/edges.rs#L709).

```sql
SELECT id FROM nodes WHERE docstring LIKE '%skip-test-coverage%'
```

#### Q0114 — `get_nodes_by_name`

Source: [src/db/queries/edges.rs:738](../src/db/queries/edges.rs#L738).

```sql
SELECT id, kind, name, qualified_name, file_path,
                          start_line, end_line, start_column, end_column,
                          docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                   FROM nodes
                   WHERE name = ?1
                   LIMIT 200
```

#### Q0115 — `get_nodes_by_qualified_name`

Source: [src/db/queries/edges.rs:763](../src/db/queries/edges.rs#L763).

```sql
SELECT id, kind, name, qualified_name, file_path,
                          start_line, end_line, start_column, end_column,
                          docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                   FROM nodes
                   WHERE qualified_name = ?1
```

#### Q0116 — `get_nodes_by_qualified_name`

Source: [src/db/queries/edges.rs:798](../src/db/queries/edges.rs#L798).

```sql
SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
                 WHERE qualified_name LIKE ?1 ESCAPE '\'
                 LIMIT 50
```

#### Q0117 — `get_nodes_by_qualified_name`

Source: [src/db/queries/edges.rs:808](../src/db/queries/edges.rs#L808).

```sql
SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
                 WHERE name = ?1
                 LIMIT 50
```

#### Q0118 — `get_ranked_nodes_by_edge_kind`

Source: [src/db/queries/edges.rs:882](../src/db/queries/edges.rs#L882).

```sql
SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    COUNT(*) AS cnt
             FROM edges e
             JOIN nodes n ON {join_col} = n.id
             WHERE {where_clause}
             GROUP BY {group_col}
             ORDER BY cnt DESC
             LIMIT ?{param_idx}
```

#### Q0119 — `get_largest_nodes`

Source: [src/db/queries/edges.rs:954](../src/db/queries/edges.rs#L954).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands,
                    (end_line - start_line + 1) AS lines
             FROM nodes
             {where_clause}
             ORDER BY lines DESC
             LIMIT ?{param_idx}
```

#### Q0120 — `get_file_coupling`

Source: [src/db/queries/edges.rs:1021](../src/db/queries/edges.rs#L1021).

```sql
SELECT {group_alias}.file_path, COUNT(DISTINCT {count_alias}.file_path) AS coupling
             FROM edges e
             JOIN nodes n_src ON e.source = n_src.id
             JOIN nodes n_tgt ON e.target = n_tgt.id
             WHERE e.kind IN ('calls', 'uses', 'implements', 'extends')
               AND n_src.file_path != n_tgt.file_path
               {path_filter}
             GROUP BY {group_alias}.file_path
             ORDER BY coupling DESC
             LIMIT ?1
```

#### Q0121 — `get_inheritance_depth`

Source: [src/db/queries/edges.rs:1096](../src/db/queries/edges.rs#L1096).

```sql
WITH RECURSIVE hierarchy(leaf_id, current_id, depth, path) AS (
                 SELECT e.source, e.target, 1,
                        ',' || e.source || ',' || e.target || ','
                 FROM edges e
                 WHERE e.kind = 'extends'
                 UNION ALL
                 SELECT h.leaf_id, e.target, h.depth + 1,
                        h.path || e.target || ','
                 FROM hierarchy h
                 JOIN edges e ON e.source = h.current_id AND e.kind = 'extends'
                 WHERE h.depth < 50
                   AND instr(h.path, ',' || e.target || ',') = 0
             ),
             leaf_depths AS (
                 SELECT leaf_id, MAX(depth) AS max_depth
                 FROM hierarchy
                 GROUP BY leaf_id
             )
             SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    ld.max_depth
             FROM leaf_depths ld
             JOIN nodes n ON ld.leaf_id = n.id
             {path_filter}
             ORDER BY ld.max_depth DESC
             LIMIT ?1
```

#### Q0122 — `get_node_distribution`

Source: [src/db/queries/edges.rs:1162](../src/db/queries/edges.rs#L1162).

```sql
SELECT file_path, kind, COUNT(*) AS cnt FROM nodes
```

#### Q0123 — `get_call_edges`

Source: [src/db/queries/edges.rs:1210](../src/db/queries/edges.rs#L1210).

```sql
SELECT e.source, e.target FROM edges e
                 JOIN nodes n ON e.source = n.id
                 WHERE e.kind = 'calls' AND
```

#### Q0124 — `get_call_edges`

Source: [src/db/queries/edges.rs:1218](../src/db/queries/edges.rs#L1218).

```sql
SELECT source, target FROM edges WHERE kind = 'calls'
```

#### Q0125 — `get_call_edges_with_lines`

Source: [src/db/queries/edges.rs:1261](../src/db/queries/edges.rs#L1261).

```sql
SELECT e.source, e.target, e.line FROM edges e
                 JOIN nodes n ON e.source = n.id
                 WHERE e.kind = 'calls' AND
```

#### Q0126 — `get_call_edges_with_lines`

Source: [src/db/queries/edges.rs:1269](../src/db/queries/edges.rs#L1269).

```sql
SELECT source, target, line FROM edges WHERE kind = 'calls'
```

#### Q0127 — `get_complexity_ranked`

Source: [src/db/queries/edges.rs:1336](../src/db/queries/edges.rs#L1336).

```sql
SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    (n.end_line - n.start_line + 1) AS lines,
                    COALESCE(out_calls.cnt, 0) AS fan_out,
                    COALESCE(in_calls.cnt, 0) AS fan_in,
                    ((n.end_line - n.start_line + 1) + COALESCE(out_calls.cnt, 0) * 3 + COALESCE(in_calls.cnt, 0)) AS score
             FROM nodes n
             LEFT JOIN (SELECT source, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY source) out_calls ON out_calls.source = n.id
             LEFT JOIN (SELECT target, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY target) in_calls ON in_calls.target = n.id
             WHERE {where_clause}
             ORDER BY score DESC
             LIMIT ?{param_idx}
```

#### Q0128 — `get_undocumented_public_symbols`

Source: [src/db/queries/edges.rs:1420](../src/db/queries/edges.rs#L1420).

```sql
SELECT id, kind, name, qualified_name, file_path,
                            start_line, end_line, start_column, end_column,
                            docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                     FROM nodes
                     WHERE visibility = 'public'
                       AND (docstring IS NULL OR docstring = '')
                       AND kind IN ({DOC_COVERAGE_KINDS})
                       AND {path_filter}
                     ORDER BY file_path, start_line
                     LIMIT ?1
```

#### Q0129 — `get_undocumented_public_symbols`

Source: [src/db/queries/edges.rs:1435](../src/db/queries/edges.rs#L1435).

```sql
SELECT id, kind, name, qualified_name, file_path,
                            start_line, end_line, start_column, end_column,
                            docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                     FROM nodes
                     WHERE visibility = 'public'
                       AND (docstring IS NULL OR docstring = '')
                       AND kind IN ({DOC_COVERAGE_KINDS})
                     ORDER BY file_path, start_line
                     LIMIT ?1
```

#### Q0130 — `get_god_classes`

Source: [src/db/queries/edges.rs:1482](../src/db/queries/edges.rs#L1482).

```sql
SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    SUM(CASE WHEN c.kind IN ('method', 'singleton_method', 'abstract_method', 'constructor') THEN 1 ELSE 0 END) AS methods,
                    SUM(CASE WHEN c.kind = 'field' THEN 1 ELSE 0 END) AS fields,
                    COUNT(*) AS total
             FROM nodes n
             JOIN nodes c ON c.parent_id = n.id
             WHERE n.kind IN ('class', 'struct', 'inner_class', 'object')
               {path_filter}
             GROUP BY n.id
             ORDER BY total DESC
             LIMIT ?1
```

#### Q0131 — `get_all_edges`

Source: [src/db/queries/edges.rs:1539](../src/db/queries/edges.rs#L1539).

```sql
SELECT source, target, kind, line, resolved_by FROM edges
```

#### Q0132 — `get_edges_by_kind`

Source: [src/db/queries/edges.rs:1560](../src/db/queries/edges.rs#L1560).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE kind = ?1
```

#### Q0133 — `get_edges_by_kinds`

Source: [src/db/queries/edges.rs:1583](../src/db/queries/edges.rs#L1583).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE kind IN ({})
```

#### Q0134 — `variant_group_candidates`

Source: [src/db/queries/edges.rs:1635](../src/db/queries/edges.rs#L1635).

```sql
SELECT 'rs' || char(1) || n.qualified_name, n.id FROM nodes n JOIN edges e ON e.target = n.id AND e.kind = 'annotates' JOIN nodes a ON a.id = e.source WHERE a.kind = 'annotation_usage' AND a.name IN ('cfg', 'cfg_attr') AND lower(n.file_path) LIKE '%.rs' AND n.kind IN ({kinds})
```

#### Q0135 — `go_variant_candidates`

Source: [src/db/queries/edges.rs:1667](../src/db/queries/edges.rs#L1667).

```sql
SELECT file_path, name, id FROM nodes                    WHERE kind = 'function' AND lower(file_path) LIKE '%.go'                      AND name IN (                        SELECT name FROM nodes                        WHERE kind = 'function' AND lower(file_path) LIKE '%.go'                        GROUP BY name HAVING COUNT(*) >= 2                      )
```

#### Q0136 — `calls_edges_into`

Source: [src/db/queries/edges.rs:1700](../src/db/queries/edges.rs#L1700).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE kind = 'calls' AND target IN ({})
```

#### Q0137 — `get_top_degree_nodes`

Source: [src/db/queries/edges.rs:1740](../src/db/queries/edges.rs#L1740).

```sql
SELECT id, SUM(inc) AS in_degree, SUM(outg) AS out_degree FROM ( SELECT target AS id, 1 AS inc, 0 AS outg FROM edges WHERE source <> target UNION ALL SELECT source AS id, 0 AS inc, 1 AS outg FROM edges WHERE source <> target ) GROUP BY id ORDER BY (in_degree + out_degree) DESC, id ASC LIMIT ?1
```

#### Q0138 — `delete_edges_by_source`

Source: [src/db/queries/edges.rs:1787](../src/db/queries/edges.rs#L1787).

```sql
DELETE FROM edges WHERE source = ?1
```

#### Q0139 — `delete_call_edges_at_sites`

Source: [src/db/queries/edges.rs:1806](../src/db/queries/edges.rs#L1806).

```sql
DELETE FROM edges
                     WHERE kind = 'calls' AND source = ?1 AND line = ?2
                       AND target IN (SELECT id FROM nodes WHERE name = ?3)
```

### src/db/queries/files.rs

#### Q0140 — `upsert_files`

Source: [src/db/queries/files.rs:17](../src/db/queries/files.rs#L17).

```sql
BEGIN
```

#### Q0141 — `upsert_files`

Source: [src/db/queries/files.rs:25](../src/db/queries/files.rs#L25).

```sql
INSERT OR REPLACE INTO files (path,content_hash,size,modified_at,indexed_at,node_count,kind) VALUES (?1,?2,?3,?4,?5,?6,?7)
```

#### Q0142 — `upsert_files`

Source: [src/db/queries/files.rs:51](../src/db/queries/files.rs#L51).

```sql
COMMIT
```

#### Q0143 — `upsert_file`

Source: [src/db/queries/files.rs:63](../src/db/queries/files.rs#L63).

```sql
INSERT OR REPLACE INTO files
                (path, content_hash, size, modified_at, indexed_at, node_count, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
```

#### Q0144 — `get_file`

Source: [src/db/queries/files.rs:89](../src/db/queries/files.rs#L89).

```sql
SELECT path, content_hash, size, modified_at, indexed_at, node_count, kind
                 FROM files WHERE path = ?1
```

#### Q0145 — `get_all_files`

Source: [src/db/queries/files.rs:119](../src/db/queries/files.rs#L119).

```sql
SELECT path, content_hash, size, modified_at, indexed_at, node_count, kind FROM files
```

#### Q0146 — `get_code_files`

Source: [src/db/queries/files.rs:140](../src/db/queries/files.rs#L140).

```sql
SELECT path, content_hash, size, modified_at, indexed_at, node_count, kind
                 FROM files WHERE kind = 'code'
```

#### Q0147 — `delete_file`

Source: [src/db/queries/files.rs:157](../src/db/queries/files.rs#L157).

```sql
DELETE FROM files WHERE path = ?1
```

### src/db/queries/fingerprints.rs

#### Q0148 — `upsert_fingerprint`

Source: [src/db/queries/fingerprints.rs:29](../src/db/queries/fingerprints.rs#L29).

```sql
INSERT OR REPLACE INTO node_fingerprints
                 (node_id, ast_hash, cfg_hash, call_seq_hash, shingles, body_tokens, source_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
```

#### Q0149 — `get_fingerprint`

Source: [src/db/queries/fingerprints.rs:55](../src/db/queries/fingerprints.rs#L55).

```sql
SELECT node_id, ast_hash, cfg_hash, call_seq_hash, shingles, body_tokens, source_hash
                   FROM node_fingerprints WHERE node_id = ?1
```

#### Q0150 — `get_all_fingerprints`

Source: [src/db/queries/fingerprints.rs:80](../src/db/queries/fingerprints.rs#L80).

```sql
SELECT node_id, ast_hash, cfg_hash, call_seq_hash, shingles, body_tokens, source_hash
                   FROM node_fingerprints
```

### src/db/queries/metadata.rs

#### Q0151 — `get_metadata`

Source: [src/db/queries/metadata.rs:13](../src/db/queries/metadata.rs#L13).

```sql
SELECT value FROM metadata WHERE key = ?1
```

#### Q0152 — `set_metadata`

Source: [src/db/queries/metadata.rs:39](../src/db/queries/metadata.rs#L39).

```sql
INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)
```

#### Q0153 — `get_nodes_by_dir`

Source: [src/db/queries/metadata.rs:66](../src/db/queries/metadata.rs#L66).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async,
                    branches, loops, returns, max_nesting,
                    unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes
             WHERE {path_filter} AND kind IN ({})
             ORDER BY file_path, start_line
```

#### Q0154 — `get_internal_edges`

Source: [src/db/queries/metadata.rs:119](../src/db/queries/metadata.rs#L119).

```sql
SELECT source, target, kind, line, resolved_by FROM edges WHERE source IN ({})
```

#### Q0155 — `collect_test_marker_ids`

Source: [src/db/queries/metadata.rs:167](../src/db/queries/metadata.rs#L167).

```sql
SELECT id FROM nodes
                   WHERE kind = 'annotation_usage'
                     AND (
                         name = 'test'
                         OR name LIKE '%::test'
                         OR name = 'wasm_bindgen_test'
                         OR name LIKE '%::wasm_bindgen_test'
                     )
```

### src/db/queries/mod.rs

#### Q0164 — `query_kind_counts_coalesces_null_kind`

Source: [src/db/queries/mod.rs:498](../src/db/queries/mod.rs#L498).

```sql
CREATE TABLE t (kind TEXT)
```

#### Q0165 — `query_kind_counts_coalesces_null_kind`

Source: [src/db/queries/mod.rs:501](../src/db/queries/mod.rs#L501).

```sql
INSERT INTO t (kind) VALUES ('function'), (NULL)
```

#### Q0166 — `query_kind_counts_coalesces_null_kind`

Source: [src/db/queries/mod.rs:505](../src/db/queries/mod.rs#L505).

```sql
SELECT kind, COUNT(*) FROM t GROUP BY kind
```

### src/db/queries/nodes.rs

#### Q0167 — `insert_node`

Source: [src/db/queries/nodes.rs:141](../src/db/queries/nodes.rs#L141).

```sql
INSERT OR REPLACE INTO nodes
                (id, kind, name, qualified_name, file_path,
                 start_line, end_line, start_column, end_column,
                 docstring, signature, visibility, is_async,
                 branches, loops, returns, max_nesting,
                 unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands, search_terms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)
```

#### Q0168 — `insert_all`

Source: [src/db/queries/nodes.rs:241](../src/db/queries/nodes.rs#L241).

```sql
BEGIN;
```

#### Q0169 — `insert_all`

Source: [src/db/queries/nodes.rs:246](../src/db/queries/nodes.rs#L246).

```sql
INSERT OR REPLACE INTO nodes (id,kind,name,qualified_name,file_path,start_line,end_line,start_column,end_column,docstring,signature,visibility,is_async,branches,loops,returns,max_nesting,unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands,search_terms) VALUES
```

#### Q0170 — `insert_all`

Source: [src/db/queries/nodes.rs:325](../src/db/queries/nodes.rs#L325).

```sql
INSERT INTO edges (source,target,kind,line,resolved_by) VALUES
```

#### Q0171 — `insert_all`

Source: [src/db/queries/nodes.rs:355](../src/db/queries/nodes.rs#L355).

```sql
INSERT OR REPLACE INTO files (path,content_hash,size,modified_at,indexed_at,node_count) VALUES
```

#### Q0172 — `insert_all`

Source: [src/db/queries/nodes.rs:379](../src/db/queries/nodes.rs#L379).

```sql
COMMIT;
```

#### Q0173 — `insert_nodes`

Source: [src/db/queries/nodes.rs:399](../src/db/queries/nodes.rs#L399).

```sql
BEGIN
```

#### Q0174 — `insert_nodes`

Source: [src/db/queries/nodes.rs:408](../src/db/queries/nodes.rs#L408).

```sql
INSERT OR REPLACE INTO nodes (id,kind,name,qualified_name,file_path,start_line,end_line,start_column,end_column,docstring,signature,visibility,is_async,branches,loops,returns,max_nesting,unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands,search_terms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29)
```

#### Q0175 — `insert_nodes`

Source: [src/db/queries/nodes.rs:463](../src/db/queries/nodes.rs#L463).

```sql
COMMIT
```

#### Q0176 — `get_node_by_id`

Source: [src/db/queries/nodes.rs:477](../src/db/queries/nodes.rs#L477).

```sql
SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE id = ?1
```

#### Q0177 — `get_nodes_by_ids`

Source: [src/db/queries/nodes.rs:518](../src/db/queries/nodes.rs#L518).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes WHERE id IN ({placeholders})
```

#### Q0178 — `get_nodes_by_file`

Source: [src/db/queries/nodes.rs:543](../src/db/queries/nodes.rs#L543).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE file_path = ?1 ORDER BY start_line
```

#### Q0179 — `get_children_of`

Source: [src/db/queries/nodes.rs:565](../src/db/queries/nodes.rs#L565).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE parent_id = ?1 ORDER BY start_line
```

#### Q0180 — `get_children_of_many`

Source: [src/db/queries/nodes.rs:587](../src/db/queries/nodes.rs#L587).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes WHERE parent_id IN ({placeholders}) ORDER BY file_path, start_line
```

#### Q0181 — `get_trait_methods_for_impl_method`

Source: [src/db/queries/nodes.rs:618](../src/db/queries/nodes.rs#L618).

```sql
SELECT method.id, method.kind, method.name, method.qualified_name, method.file_path,
                        method.start_line, method.end_line, method.start_column, method.end_column,
                        method.docstring, method.signature, method.visibility, method.is_async,
                        method.branches, method.loops, method.returns, method.max_nesting,
                        method.unsafe_blocks, method.unchecked_calls, method.assertions,
                        method.updated_at, method.attrs_start_line, method.parent_id,
                        method.cognitive_complexity, method.distinct_operators,
                        method.distinct_operands, method.total_operators, method.total_operands
                 FROM edges dispatch
                 JOIN nodes trait ON trait.id = dispatch.target AND trait.kind = 'trait'
                 JOIN nodes method ON method.parent_id = trait.id
                 WHERE dispatch.source = ?1
                   AND dispatch.kind = 'implements'
                   AND method.name = ?2
                   AND method.kind IN ('method', 'function')
                 ORDER BY method.file_path, method.start_line
```

#### Q0182 — `get_nodes_by_kind`

Source: [src/db/queries/nodes.rs:649](../src/db/queries/nodes.rs#L649).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE kind = ?1
```

#### Q0183 — `get_all_nodes`

Source: [src/db/queries/nodes.rs:669](../src/db/queries/nodes.rs#L669).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
```

#### Q0184 — `replace_ambiguous_calls`

Source: [src/db/queries/nodes.rs:699](../src/db/queries/nodes.rs#L699).

```sql
DELETE FROM ambiguous_calls WHERE file_path = ?1
```

#### Q0185 — `replace_ambiguous_calls_for_refs`

Source: [src/db/queries/nodes.rs:742](../src/db/queries/nodes.rs#L742).

```sql
BEGIN
```

#### Q0186 — `replace_ambiguous_calls_for_refs`

Source: [src/db/queries/nodes.rs:744](../src/db/queries/nodes.rs#L744).

```sql
begin
```

#### Q0187 — `replace_ambiguous_calls_for_refs`

Source: [src/db/queries/nodes.rs:748](../src/db/queries/nodes.rs#L748).

```sql
DELETE FROM ambiguous_calls
                 WHERE from_node_id = ?1 AND reference_name = ?2
                   AND file_path = ?3 AND line = ?4
```

#### Q0188 — `replace_ambiguous_calls_for_refs`

Source: [src/db/queries/nodes.rs:766](../src/db/queries/nodes.rs#L766).

```sql
COMMIT
```

#### Q0189 — `replace_ambiguous_calls_for_refs`

Source: [src/db/queries/nodes.rs:768](../src/db/queries/nodes.rs#L768).

```sql
commit
```

#### Q0190 — `insert_ambiguous_calls`

Source: [src/db/queries/nodes.rs:783](../src/db/queries/nodes.rs#L783).

```sql
INSERT OR REPLACE INTO ambiguous_calls
                     (from_node_id, reference_name, file_path, line, candidate_node_ids)
                     VALUES (?1, ?2, ?3, ?4, ?5)
```

#### Q0191 — `get_ambiguous_calls`

Source: [src/db/queries/nodes.rs:814](../src/db/queries/nodes.rs#L814).

```sql
SELECT from_node_id, reference_name, file_path, line, candidate_node_ids
             FROM ambiguous_calls
```

#### Q0192 — `ambiguous_candidate_ids`

Source: [src/db/queries/nodes.rs:872](../src/db/queries/nodes.rs#L872).

```sql
SELECT candidate_node_ids FROM ambiguous_calls
```

#### Q0193 — `get_ambiguous_calls_naming`

Source: [src/db/queries/nodes.rs:905](../src/db/queries/nodes.rs#L905).

```sql
SELECT from_node_id, reference_name, file_path, line, candidate_node_ids
                 FROM ambiguous_calls
                 WHERE candidate_node_ids LIKE ?1 ESCAPE '\'
                 ORDER BY file_path, line
```

#### Q0194 — `get_node_paths`

Source: [src/db/queries/nodes.rs:954](../src/db/queries/nodes.rs#L954).

```sql
SELECT id, file_path FROM nodes
```

#### Q0195 — `get_nodes_filtered`

Source: [src/db/queries/nodes.rs:994](../src/db/queries/nodes.rs#L994).

```sql
SELECT id, kind, name, qualified_name, file_path,
                start_line, end_line, start_column, end_column,
                docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes{}
```

#### Q0196 — `get_all_nodes_for_resolution`

Source: [src/db/queries/nodes.rs:1045](../src/db/queries/nodes.rs#L1045).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    NULL AS docstring,
                    CASE WHEN file_path LIKE '%.gd'
                              AND kind IN ('class', 'inner_class', 'function', 'method', 'field')
                         THEN signature
                         WHEN file_path LIKE '%.cs'
                              AND kind IN ('class', 'inner_class', 'struct', 'interface', 'record',
                                           'function', 'method', 'field', 'csharp_property')
                         THEN signature END AS signature,
                    visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
```

#### Q0197 — `touched_nodes_by_file`

Source: [src/db/queries/nodes.rs:1080](../src/db/queries/nodes.rs#L1080).

```sql
SELECT kind, name, qualified_name FROM nodes WHERE file_path = ?1
```

#### Q0198 — `delete_nodes_by_file`

Source: [src/db/queries/nodes.rs:1120](../src/db/queries/nodes.rs#L1120).

```sql
DELETE FROM executable_body_fts WHERE file_path = ?1
```

#### Q0199 — `delete_nodes_by_file`

Source: [src/db/queries/nodes.rs:1134](../src/db/queries/nodes.rs#L1134).

```sql
SELECT id FROM nodes WHERE file_path = ?1
```

#### Q0200 — `delete_nodes_by_file`

Source: [src/db/queries/nodes.rs:1171](../src/db/queries/nodes.rs#L1171).

```sql
DELETE FROM edges WHERE source = ?1 OR target = ?1
```

#### Q0201 — `delete_nodes_by_file`

Source: [src/db/queries/nodes.rs:1181](../src/db/queries/nodes.rs#L1181).

```sql
DELETE FROM unresolved_refs WHERE from_node_id = ?1
```

#### Q0202 — `delete_nodes_by_file`

Source: [src/db/queries/nodes.rs:1191](../src/db/queries/nodes.rs#L1191).

```sql
DELETE FROM vectors WHERE node_id = ?1
```

#### Q0203 — `delete_nodes_by_file`

Source: [src/db/queries/nodes.rs:1201](../src/db/queries/nodes.rs#L1201).

```sql
DELETE FROM nodes WHERE file_path = ?1
```

### src/db/queries/routes.rs

#### Q0204 — `invalidate_rails_routes`

Source: [src/db/queries/routes.rs:203](../src/db/queries/routes.rs#L203).

```sql
SELECT EXISTS (SELECT 1 FROM nodes WHERE kind = 'route')
                    OR EXISTS (SELECT 1 FROM unresolved_refs WHERE reference_name >= ?1 AND reference_name < ?2)
```

#### Q0205 — `rebuild_rails_routes`

Source: [src/db/queries/routes.rs:236](../src/db/queries/routes.rs#L236).

```sql
BEGIN
```

#### Q0206 — `rebuild_rails_routes`

Source: [src/db/queries/routes.rs:242](../src/db/queries/routes.rs#L242).

```sql
COMMIT
```

#### Q0207 — `rebuild_rails_routes`

Source: [src/db/queries/routes.rs:243](../src/db/queries/routes.rs#L243).

```sql
ROLLBACK
```

#### Q0208 — `rebuild_rails_routes`

Source: [src/db/queries/routes.rs:249](../src/db/queries/routes.rs#L249).

```sql
ROLLBACK
```

#### Q0209 — `rebuild_rails_routes_inner`

Source: [src/db/queries/routes.rs:263](../src/db/queries/routes.rs#L263).

```sql
DELETE FROM edges WHERE resolved_by = ?1
                    AND source IN (SELECT id FROM nodes WHERE kind = 'route')
```

#### Q0210 — `rebuild_rails_routes_inner`

Source: [src/db/queries/routes.rs:345](../src/db/queries/routes.rs#L345).

```sql
INSERT INTO edges(source,target,kind,line,resolved_by) VALUES (?1,?2,'calls',?3,?4)
```

#### Q0211 — `rebuild_rails_routes_inner`

Source: [src/db/queries/routes.rs:359](../src/db/queries/routes.rs#L359).

```sql
INSERT OR REPLACE INTO metadata(key,value) VALUES ('rails_routes_pending','0')
```

#### Q0212 — `route_rows`

Source: [src/db/queries/routes.rs:472](../src/db/queries/routes.rs#L472).

```sql
SELECT n.id, n.file_path, n.start_line, r.reference_name
                 FROM nodes n JOIN unresolved_refs r ON r.from_node_id = n.id
                 WHERE n.kind = 'route' AND r.reference_kind = 'calls'
                 ORDER BY n.id
```

#### Q0213 — `rails_evidence`

Source: [src/db/queries/routes.rs:496](../src/db/queries/routes.rs#L496).

```sql
SELECT file_path, from_node_id, reference_name, line FROM unresolved_refs
                 WHERE reference_name >= ?1 AND reference_name < ?2 AND reference_kind = 'uses'
```

#### Q0214 — `rails_evidence`

Source: [src/db/queries/routes.rs:502](../src/db/queries/routes.rs#L502).

```sql
SELECT file_path, from_node_id, reference_name, line FROM unresolved_refs
                 WHERE reference_name = ?1 AND reference_kind = 'uses'
```

#### Q0215 — `ruby_owners`

Source: [src/db/queries/routes.rs:620](../src/db/queries/routes.rs#L620).

```sql
SELECT id, kind, qualified_name, file_path FROM nodes
                 WHERE name IN ({}) AND kind IN ('class','module')
```

#### Q0216 — `owner_methods`

Source: [src/db/queries/routes.rs:661](../src/db/queries/routes.rs#L661).

```sql
SELECT parent_id, id, name, kind, visibility FROM nodes
                 WHERE parent_id IN ({}) AND kind IN ('method','singleton_method')
```

#### Q0217 — `owner_visibility_evidence`

Source: [src/db/queries/routes.rs:697](../src/db/queries/routes.rs#L697).

```sql
SELECT from_node_id, reference_name FROM unresolved_refs
                 WHERE from_node_id IN ({}) AND reference_kind = 'uses'
                    AND reference_name >= ? AND reference_name < ?
```

#### Q0218 — `route_invalidation_waits_for_concurrent_transaction`

Source: [src/db/queries/routes.rs:739](../src/db/queries/routes.rs#L739).

```sql
BEGIN
```

#### Q0219 — `route_invalidation_waits_for_concurrent_transaction`

Source: [src/db/queries/routes.rs:742](../src/db/queries/routes.rs#L742).

```sql
ROLLBACK
```

#### Q0220 — `route_rebuild_checks_pending_after_waiting_for_write_transaction`

Source: [src/db/queries/routes.rs:768](../src/db/queries/routes.rs#L768).

```sql
BEGIN
```

#### Q0221 — `route_rebuild_checks_pending_after_waiting_for_write_transaction`

Source: [src/db/queries/routes.rs:772](../src/db/queries/routes.rs#L772).

```sql
ROLLBACK
```

### src/db/queries/search.rs

#### Q0222 — `insert_executable_body_documents`

Source: [src/db/queries/search.rs:59](../src/db/queries/search.rs#L59).

```sql
BEGIN
```

#### Q0223 — `insert_executable_body_documents`

Source: [src/db/queries/search.rs:68](../src/db/queries/search.rs#L68).

```sql
INSERT INTO executable_body_fts (node_id, file_path, body) VALUES (?1, ?2, ?3)
```

#### Q0224 — `insert_executable_body_documents`

Source: [src/db/queries/search.rs:89](../src/db/queries/search.rs#L89).

```sql
COMMIT
```

#### Q0225 — `search_executable_bodies`

Source: [src/db/queries/search.rs:143](../src/db/queries/search.rs#L143).

```sql
SELECT node_id, body FROM executable_body_fts
                 WHERE executable_body_fts MATCH ?1
                 LIMIT ?2
```

#### Q0226 — `search_nodes`

Source: [src/db/queries/search.rs:273](../src/db/queries/search.rs#L273).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
                 WHERE name LIKE ?1 ESCAPE '\' OR qualified_name LIKE ?1 ESCAPE '\' OR docstring LIKE ?1 ESCAPE '\' OR signature LIKE ?1 ESCAPE '\'
                 LIMIT ?2
```

#### Q0227 — `search_nodes_bounded_fts`

Source: [src/db/queries/search.rs:351](../src/db/queries/search.rs#L351).

```sql
SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0) AS rank
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.rowid
                 WHERE nodes_fts MATCH ?1
                   AND n.kind NOT IN ('file', 'doc')
                 ORDER BY rank
                 LIMIT ?2
```

#### Q0228 — `search_nodes_fts`

Source: [src/db/queries/search.rs:396](../src/db/queries/search.rs#L396).

```sql
SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0) AS rank
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.rowid
                 WHERE nodes_fts MATCH ?1
                 ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0)
                 LIMIT ?2
```

#### Q0229 — `batch_incoming_call_counts`

Source: [src/db/queries/search.rs:443](../src/db/queries/search.rs#L443).

```sql
SELECT target, COUNT(*) AS cnt FROM edges WHERE target IN ({placeholders}) AND kind = 'calls' GROUP BY target
```

#### Q0230 — `search_nodes_by_exact_name`

Source: [src/db/queries/search.rs:481](../src/db/queries/search.rs#L481).

```sql
SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async,
                    branches, loops, returns, max_nesting,
                    unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes
             WHERE LOWER(name) IN ({placeholders})
             LIMIT ?
```

### src/db/queries/stats.rs

#### Q0231 — `get_stats_with_languages`

Source: [src/db/queries/stats.rs:23](../src/db/queries/stats.rs#L23).

```sql
SELECT (SELECT COUNT(*) FROM nodes), (SELECT COUNT(*) FROM edges), (SELECT COUNT(*) FROM files), (SELECT COALESCE(MAX(indexed_at), 0) FROM files), (SELECT COALESCE(SUM(size), 0) FROM files)
```

#### Q0232 — `get_stats_with_languages`

Source: [src/db/queries/stats.rs:59](../src/db/queries/stats.rs#L59).

```sql
SELECT kind, COUNT(*) FROM nodes GROUP BY kind
```

#### Q0233 — `get_stats_with_languages`

Source: [src/db/queries/stats.rs:66](../src/db/queries/stats.rs#L66).

```sql
SELECT kind, COUNT(*) FROM edges GROUP BY kind
```

#### Q0234 — `get_stats_with_languages`

Source: [src/db/queries/stats.rs:78](../src/db/queries/stats.rs#L78).

```sql
SELECT path FROM files
```

#### Q0235 — `last_index_time`

Source: [src/db/queries/stats.rs:144](../src/db/queries/stats.rs#L144).

```sql
SELECT COALESCE(MAX(indexed_at), 0) FROM files
```

### src/db/queries/unresolved.rs

#### Q0236 — `get_gdscript_typed_refs`

Source: [src/db/queries/unresolved.rs:17](../src/db/queries/unresolved.rs#L17).

```sql
SELECT from_node_id, reference_name, line, col, file_path
                 FROM unresolved_refs
                 WHERE reference_kind = 'calls'
                   AND (file_path LIKE '%.gd' OR file_path LIKE '%.cs')
                   AND instr(reference_name, '::') > 0
```

#### Q0237 — `insert_unresolved_ref`

Source: [src/db/queries/unresolved.rs:47](../src/db/queries/unresolved.rs#L47).

```sql
INSERT INTO unresolved_refs
                (from_node_id, reference_name, reference_kind, line, col, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
```

#### Q0238 — `insert_unresolved_refs`

Source: [src/db/queries/unresolved.rs:74](../src/db/queries/unresolved.rs#L74).

```sql
BEGIN
```

#### Q0239 — `insert_unresolved_refs`

Source: [src/db/queries/unresolved.rs:82](../src/db/queries/unresolved.rs#L82).

```sql
INSERT INTO unresolved_refs (from_node_id,reference_name,reference_kind,line,col,file_path) VALUES (?1,?2,?3,?4,?5,?6)
```

#### Q0240 — `insert_unresolved_refs`

Source: [src/db/queries/unresolved.rs:107](../src/db/queries/unresolved.rs#L107).

```sql
COMMIT
```

#### Q0241 — `get_unresolved_refs`

Source: [src/db/queries/unresolved.rs:121](../src/db/queries/unresolved.rs#L121).

```sql
SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs
```

#### Q0242 — `count_unresolved_refs`

Source: [src/db/queries/unresolved.rs:141](../src/db/queries/unresolved.rs#L141).

```sql
SELECT COUNT(*) FROM unresolved_refs
```

#### Q0243 — `get_unresolved_refs_after`

Source: [src/db/queries/unresolved.rs:179](../src/db/queries/unresolved.rs#L179).

```sql
SELECT id, from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs
                 WHERE id > ?1
                 ORDER BY id
                 LIMIT ?2
```

#### Q0244 — `get_unresolved_refs_for_sources`

Source: [src/db/queries/unresolved.rs:228](../src/db/queries/unresolved.rs#L228).

```sql
SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs WHERE from_node_id IN ({})
```

#### Q0245 — `clear_unresolved_refs`

Source: [src/db/queries/unresolved.rs:259](../src/db/queries/unresolved.rs#L259).

```sql
DELETE FROM unresolved_refs
```

### src/doctor.rs

#### Q0246 — `check_database`

Source: [src/doctor.rs:108](../src/doctor.rs#L108).

```sql
VACUUM failed: {e}
```

### src/edge_audit.rs

#### Q0247 — `audit`

Source: [src/edge_audit.rs:111](../src/edge_audit.rs#L111).

```sql
SELECT file_path, name FROM nodes WHERE kind = 'use'
```

#### Q0248 — `audit`

Source: [src/edge_audit.rs:125](../src/edge_audit.rs#L125).

```sql
SELECT name, COUNT(*) FROM nodes WHERE kind <> 'file' AND kind <> 'use' GROUP BY name
```

#### Q0249 — `audit`

Source: [src/edge_audit.rs:141](../src/edge_audit.rs#L141).

```sql
SELECT id, name FROM nodes
```

#### Q0250 — `audit`

Source: [src/edge_audit.rs:161](../src/edge_audit.rs#L161).

```sql
SELECT s.file_path, n.id, n.name, n.file_path, n.kind, n.start_line, n.parent_id FROM edges e JOIN nodes n ON n.id = e.target JOIN nodes s ON s.id = e.source
```

### src/extraction/fstar_extractor.rs

#### Q0251 — `apply_word`

Source: [src/extraction/fstar_extractor.rs:1173](../src/extraction/fstar_extractor.rs#L1173).

```sql
begin
```

#### Q0252 — `ends_with_continuation`

Source: [src/extraction/fstar_extractor.rs:1202](../src/extraction/fstar_extractor.rs#L1202).

```sql
begin
```

### src/extraction/ruby_extractor.rs

#### Q0253 — `visit_node`

Source: [src/extraction/ruby_extractor.rs:281](../src/extraction/ruby_extractor.rs#L281).

```sql
begin
```

### src/extraction/ruby_template_extractor.rs

#### Q0254 — `opens_block`

Source: [src/extraction/ruby_template_extractor.rs:337](../src/extraction/ruby_template_extractor.rs#L337).

```sql
begin
```

### src/global_db.rs

#### Q0255 — `open_at`

Source: [src/global_db.rs:228](../src/global_db.rs#L228).

```sql
PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
```

#### Q0256 — `open_at`

Source: [src/global_db.rs:236](../src/global_db.rs#L236).

```sql
CREATE TABLE IF NOT EXISTS projects (
                path TEXT PRIMARY KEY,
                tokens_saved INTEGER NOT NULL DEFAULT 0
            )
```

#### Q0257 — `open_at`

Source: [src/global_db.rs:245](../src/global_db.rs#L245).

```sql
CREATE TABLE IF NOT EXISTS turns (
                message_id TEXT PRIMARY KEY,
                project_hash TEXT NOT NULL,
                session_id TEXT NOT NULL,
                model TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL,
                category TEXT NOT NULL,
                tool_names TEXT NOT NULL DEFAULT '',
                agent TEXT NOT NULL DEFAULT 'claude',
                credits INTEGER,
                tool_result_tokens INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_turns_timestamp ON turns(timestamp);
            CREATE INDEX IF NOT EXISTS idx_turns_project ON turns(project_hash);
            CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model);
            CREATE TABLE IF NOT EXISTS parse_offsets (
                file_path TEXT PRIMARY KEY,
                byte_offset INTEGER NOT NULL,
                mtime INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS savings_ledger (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                project_path TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                before_tokens INTEGER NOT NULL,
                after_tokens INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_savings_ledger_ts ON savings_ledger(ts);
            CREATE INDEX IF NOT EXISTS idx_savings_ledger_project ON savings_ledger(project_path)
```

#### Q0258 — `open_at`

Source: [src/global_db.rs:285](../src/global_db.rs#L285).

```sql
CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
```

#### Q0259 — `migrate_turns_columns`

Source: [src/global_db.rs:311](../src/global_db.rs#L311).

```sql
PRAGMA table_info(turns)
```

#### Q0260 — `migrate_turns_columns`

Source: [src/global_db.rs:330](../src/global_db.rs#L330).

```sql
ALTER TABLE turns ADD COLUMN agent TEXT NOT NULL DEFAULT 'claude'
```

#### Q0261 — `migrate_turns_columns`

Source: [src/global_db.rs:337](../src/global_db.rs#L337).

```sql
ALTER TABLE turns ADD COLUMN credits INTEGER
```

#### Q0262 — `migrate_turns_columns`

Source: [src/global_db.rs:347](../src/global_db.rs#L347).

```sql
ALTER TABLE turns ADD COLUMN tool_result_tokens INTEGER NOT NULL DEFAULT 0
```

#### Q0263 — `migrate_project_paths`

Source: [src/global_db.rs:366](../src/global_db.rs#L366).

```sql
SELECT 1 FROM meta WHERE key = 'projects_path_normalized'
```

#### Q0264 — `migrate_project_paths`

Source: [src/global_db.rs:377](../src/global_db.rs#L377).

```sql
SELECT path, tokens_saved FROM projects
```

#### Q0265 — `migrate_project_paths`

Source: [src/global_db.rs:406](../src/global_db.rs#L406).

```sql
DELETE FROM projects
```

#### Q0266 — `migrate_project_paths`

Source: [src/global_db.rs:412](../src/global_db.rs#L412).

```sql
INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)
```

#### Q0267 — `migrate_project_paths`

Source: [src/global_db.rs:422](../src/global_db.rs#L422).

```sql
UPDATE savings_ledger SET project_path = ?1 WHERE project_path = ?2
```

#### Q0268 — `migrate_project_paths`

Source: [src/global_db.rs:431](../src/global_db.rs#L431).

```sql
INSERT OR REPLACE INTO meta (key, value) VALUES ('projects_path_normalized', '1')
```

#### Q0269 — `upsert`

Source: [src/global_db.rs:446](../src/global_db.rs#L446).

```sql
INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET tokens_saved = ?2
```

#### Q0270 — `get_project_tokens`

Source: [src/global_db.rs:459](../src/global_db.rs#L459).

```sql
SELECT tokens_saved FROM projects WHERE path = ?1
```

#### Q0271 — `global_tokens_saved`

Source: [src/global_db.rs:476](../src/global_db.rs#L476).

```sql
SELECT COALESCE(SUM(tokens_saved), 0) FROM projects
```

#### Q0272 — `record_savings`

Source: [src/global_db.rs:498](../src/global_db.rs#L498).

```sql
INSERT INTO savings_ledger (ts, project_path, tool_name, before_tokens, after_tokens) VALUES (?1, ?2, ?3, ?4, ?5)
```

#### Q0273 — `sum_savings`

Source: [src/global_db.rs:512](../src/global_db.rs#L512).

```sql
SELECT COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), COUNT(*) FROM savings_ledger WHERE project_path = ?1 AND ts >= ?2
```

#### Q0274 — `sum_savings`

Source: [src/global_db.rs:515](../src/global_db.rs#L515).

```sql
SELECT COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), COUNT(*) FROM savings_ledger WHERE ts >= ?1
```

#### Q0275 — `savings_history`

Source: [src/global_db.rs:543](../src/global_db.rs#L543).

```sql
SELECT (ts/86400)*86400 AS day, COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), COUNT(*) FROM savings_ledger WHERE project_path = ?1 AND ts >= ?2 GROUP BY day ORDER BY day DESC
```

#### Q0276 — `savings_history`

Source: [src/global_db.rs:549](../src/global_db.rs#L549).

```sql
SELECT (ts/86400)*86400 AS day, COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), COUNT(*) FROM savings_ledger WHERE ts >= ?1 GROUP BY day ORDER BY day DESC
```

#### Q0277 — `delete_project`

Source: [src/global_db.rs:578](../src/global_db.rs#L578).

```sql
DELETE FROM projects WHERE path = ?1
```

#### Q0278 — `delete_projects`

Source: [src/global_db.rs:597](../src/global_db.rs#L597).

```sql
DELETE FROM projects WHERE path IN ({})
```

#### Q0279 — `list_project_paths`

Source: [src/global_db.rs:613](../src/global_db.rs#L613).

```sql
SELECT path FROM projects
```

#### Q0280 — `insert_turn`

Source: [src/global_db.rs:631](../src/global_db.rs#L631).

```sql
INSERT OR IGNORE INTO turns
                 (message_id, project_hash, session_id, model, timestamp,
                  input_tokens, output_tokens, cache_write_tokens, cache_read_tokens,
                  cost_usd, category, tool_names, agent, credits)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
```

#### Q0281 — `add_tool_result_tokens`

Source: [src/global_db.rs:675](../src/global_db.rs#L675).

```sql
UPDATE turns SET tool_result_tokens = tool_result_tokens + ?2 WHERE message_id = ?1
```

#### Q0282 — `upsert_droid_turn`

Source: [src/global_db.rs:694](../src/global_db.rs#L694).

```sql
INSERT INTO turns
                 (message_id, project_hash, session_id, model, timestamp,
                  input_tokens, output_tokens, cache_write_tokens, cache_read_tokens,
                  cost_usd, category, tool_names, agent, credits)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(message_id) DO UPDATE SET
                     project_hash        = excluded.project_hash,
                     session_id          = excluded.session_id,
                     model               = excluded.model,
                     input_tokens        = MAX(excluded.input_tokens,        turns.input_tokens),
                     output_tokens       = MAX(excluded.output_tokens,       turns.output_tokens),
                     cache_write_tokens  = MAX(excluded.cache_write_tokens,  turns.cache_write_tokens),
                     cache_read_tokens   = MAX(excluded.cache_read_tokens,   turns.cache_read_tokens),
                     cost_usd            = excluded.cost_usd,
                     category            = excluded.category,
                     tool_names          = excluded.tool_names,
                     agent               = excluded.agent,
                     credits             = COALESCE(excluded.credits, turns.credits)
                 WHERE
                     excluded.input_tokens        >= turns.input_tokens
                     AND excluded.output_tokens       >= turns.output_tokens
                     AND excluded.cache_write_tokens  >= turns.cache_write_tokens
                     AND excluded.cache_read_tokens   >= turns.cache_read_tokens
                     AND (excluded.credits IS NULL
                          OR turns.credits IS NULL
                          OR excluded.credits >= turns.credits)
                     AND (
                         excluded.input_tokens        > turns.input_tokens
                         OR excluded.output_tokens       > turns.output_tokens
                         OR excluded.cache_write_tokens  > turns.cache_write_tokens
                         OR excluded.cache_read_tokens   > turns.cache_read_tokens
                         OR (excluded.credits IS NOT NULL
                             AND (turns.credits IS NULL
                                  OR excluded.credits > turns.credits))
                     )
```

#### Q0283 — `total_cost_since`

Source: [src/global_db.rs:755](../src/global_db.rs#L755).

```sql
SELECT COALESCE(SUM(cost_usd), 0.0) FROM turns WHERE timestamp >= ?1
```

#### Q0284 — `total_tokens_since`

Source: [src/global_db.rs:769](../src/global_db.rs#L769).

```sql
SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM turns WHERE timestamp >= ?1
```

#### Q0285 — `token_breakdown_since`

Source: [src/global_db.rs:784](../src/global_db.rs#L784).

```sql
SELECT COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        COALESCE(SUM(cache_write_tokens), 0)
                 FROM turns WHERE timestamp >= ?1 AND agent = 'claude'
```

#### Q0286 — `cost_by_model_since`

Source: [src/global_db.rs:814](../src/global_db.rs#L814).

```sql
SELECT model, SUM(cost_usd),
                        SUM(input_tokens + output_tokens + cache_read_tokens + cache_write_tokens)
                 FROM turns WHERE timestamp >= ?1 AND agent = 'claude'
                 GROUP BY model ORDER BY SUM(cost_usd) DESC
```

#### Q0287 — `cost_by_category_since`

Source: [src/global_db.rs:840](../src/global_db.rs#L840).

```sql
SELECT category, SUM(cost_usd), COUNT(*)
                 FROM turns WHERE timestamp >= ?1 AND agent = 'claude'
                 GROUP BY category ORDER BY SUM(cost_usd) DESC
```

#### Q0288 — `nav_turns_since`

Source: [src/global_db.rs:877](../src/global_db.rs#L877).

```sql
SELECT tool_names, tool_result_tokens FROM turns WHERE timestamp >= ?1 AND agent = 'claude'
```

#### Q0289 — `cost_by_agent_since`

Source: [src/global_db.rs:901](../src/global_db.rs#L901).

```sql
SELECT agent,
                        COALESCE(SUM(cost_usd), 0.0),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_write_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        CASE WHEN COUNT(credits) = COUNT(*) THEN SUM(credits) ELSE NULL END,
                        COUNT(*)
                 FROM turns WHERE timestamp >= ?1
                 GROUP BY agent ORDER BY agent
```

#### Q0290 — `get_parse_offset`

Source: [src/global_db.rs:949](../src/global_db.rs#L949).

```sql
SELECT byte_offset, mtime FROM parse_offsets WHERE file_path = ?1
```

#### Q0291 — `set_parse_offset`

Source: [src/global_db.rs:965](../src/global_db.rs#L965).

```sql
INSERT INTO parse_offsets (file_path, byte_offset, mtime) VALUES (?1, ?2, ?3)
                 ON CONFLICT(file_path) DO UPDATE SET byte_offset = ?2, mtime = ?3
```

#### Q0292 — `checkpoint`

Source: [src/global_db.rs:976](../src/global_db.rs#L976).

```sql
PRAGMA wal_checkpoint(TRUNCATE);
```

#### Q0293 — `migration_merges_duplicates_and_drops_relative_rows`

Source: [src/global_db.rs:1076](../src/global_db.rs#L1076).

```sql
DELETE FROM meta WHERE key = 'projects_path_normalized';
                     DELETE FROM projects;
```

#### Q0294 — `migration_merges_duplicates_and_drops_relative_rows`

Source: [src/global_db.rs:1088](../src/global_db.rs#L1088).

```sql
INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)
```

#### Q0295 — `migration_merges_duplicates_and_drops_relative_rows`

Source: [src/global_db.rs:1097](../src/global_db.rs#L1097).

```sql
INSERT INTO savings_ledger (ts, project_path, tool_name, before_tokens, after_tokens) VALUES (42, ?1, 'tokensave_context', 1000, 100)
```

### src/graph/imports.rs

#### Q0296 — `build_module_import_graph`

Source: [src/graph/imports.rs:189](../src/graph/imports.rs#L189).

```sql
SELECT DISTINCT n1.file_path, n1.start_line, n1.name, n1.signature, n2.file_path, COALESCE(parent.kind, 'file') FROM edges e JOIN nodes n1 ON e.source = n1.id JOIN nodes n2 ON e.target = n2.id LEFT JOIN nodes parent ON parent.id = n1.parent_id WHERE e.kind = 'uses' AND n1.kind = 'use' AND n1.file_path != n2.file_path
```

### src/graph/queries.rs

#### Q0297 — `find_dead_code`

Source: [src/graph/queries.rs:306](../src/graph/queries.rs#L306).

```sql
SELECT id, kind, name, qualified_name, file_path, start_line, end_line,
                    start_column, end_column, docstring, signature, visibility,
                    is_async, branches, loops, returns, max_nesting, unsafe_blocks,
                    unchecked_calls, assertions, updated_at, attrs_start_line
             FROM nodes
             WHERE kind != 'route'
             AND name != 'main'
             AND name != '_bind_methods'
             AND name NOT LIKE 'test%'
             -- Go package-level `init` is invoked implicitly by the runtime at
             -- package load and can never be called explicitly, so it carries
             -- no incoming `calls` edge — the same reason `main`, `test*`, and
             -- Rust trait-impl methods are exempt. Scoped to `.go` so a callable
             -- function named `init` in another language is still checked (#346).
             AND NOT (name = 'init' AND file_path LIKE '%.go')
             -- Godot engine virtuals (`_ready`, `_process`, ...) are called by the
             -- engine, never by project code, so they have no incoming edge either.
             -- Scoped to `.gd` for the same reason as Go `init` (#598).
             AND NOT (file_path LIKE '%.gd' AND name IN ({godot_virtuals}))
             {visibility_filter}
             {kind_filter}
             {trait_impl_filter}
             AND NOT EXISTS (
                 SELECT 1 FROM edges
                 WHERE target = nodes.id
                 AND kind IN ('calls', 'implements', 'extends', 'type_of', 'returns', 'receives', 'uses')
             )
             AND id NOT IN (
                 SELECT target FROM edges
                 WHERE kind = 'annotates'
                 AND source IN (SELECT value FROM json_each(?1))
             )
```

#### Q0298 — `gdscript_live_overrides`

Source: [src/graph/queries.rs:385](../src/graph/queries.rs#L385).

```sql
WITH RECURSIVE ancestry(cls, base) AS (
                 SELECT e.source, e.target FROM edges e
                 WHERE e.kind = 'extends'
                   AND e.source IN (SELECT id FROM nodes
                                    WHERE file_path LIKE '%.gd'
                                      AND kind IN ('class', 'inner_class'))
                 UNION
                 SELECT a.cls, e.target FROM ancestry a
                 JOIN edges e ON e.source = a.base AND e.kind = 'extends'
             )
             SELECT DISTINCT m.id FROM ancestry a
             JOIN nodes m ON m.parent_id = a.cls AND m.kind IN ('function', 'method')
             JOIN nodes b ON b.parent_id = a.base AND b.name = m.name
                         AND b.kind IN ('function', 'method')
             WHERE EXISTS (SELECT 1 FROM edges c
                           WHERE c.target = b.id AND c.kind IN ('calls', 'uses'))
```

#### Q0299 — `get_file_dependencies`

Source: [src/graph/queries.rs:468](../src/graph/queries.rs#L468).

```sql
SELECT DISTINCT e.target FROM edges e WHERE e.source IN ({}) AND e.kind IN {kind_filter}
```

#### Q0300 — `get_file_dependents`

Source: [src/graph/queries.rs:530](../src/graph/queries.rs#L530).

```sql
SELECT DISTINCT e.source FROM edges e WHERE e.target IN ({}) AND e.kind IN {kind_filter}
```

#### Q0301 — `build_file_adjacency`

Source: [src/graph/queries.rs:623](../src/graph/queries.rs#L623).

```sql
SELECT DISTINCT n1.file_path AS src_file, n2.file_path AS tgt_file FROM edges e JOIN nodes n1 ON e.source = n1.id JOIN nodes n2 ON e.target = n2.id WHERE e.kind IN ('calls', 'uses') AND n1.file_path != n2.file_path
```

### src/hooks.rs

#### Q0302 — `any_symbol_indexed`

Source: [src/hooks.rs:412](../src/hooks.rs#L412).

```sql
SELECT 1 FROM nodes WHERE LOWER(name) IN ({placeholders}) LIMIT 1
```

### src/mcp/tools/definitions.rs

#### Q0303 — `module constant / top-level string`

Source: [src/mcp/tools/definitions.rs:448](../src/mcp/tools/definitions.rs#L448).

```sql
commit, PR, diff, blame, log and changelog context, affected tests, tracked branches
```

#### Q0304 — `def_commit_context`

Source: [src/mcp/tools/definitions.rs:1719](../src/mcp/tools/definitions.rs#L1719).

```sql
Commit Context
```

#### Q0305 — `def_insert_at`

Source: [src/mcp/tools/definitions.rs:2009](../src/mcp/tools/definitions.rs#L2009).

```sql
Insert Into File
```

#### Q0306 — `def_run_affected_tests`

Source: [src/mcp/tools/definitions.rs:2430](../src/mcp/tools/definitions.rs#L2430).

```sql
release
```

### src/mcp/tools/handlers/workflow.rs

#### Q0307 — `handle_run_affected_tests`

Source: [src/mcp/tools/handlers/workflow.rs:281](../src/mcp/tools/handlers/workflow.rs#L281).

```sql
release
```

### src/quality_diff.rs

#### Q0308 — `compare`

Source: [src/quality_diff.rs:556](../src/quality_diff.rs#L556).

```sql
BEGIN
```

#### Q0309 — `compare`

Source: [src/quality_diff.rs:586](../src/quality_diff.rs#L586).

```sql
BEGIN
```

### src/runtime_telemetry.rs

#### Q0310 — `read_journal_mode`

Source: [src/runtime_telemetry.rs:242](../src/runtime_telemetry.rs#L242).

```sql
PRAGMA journal_mode
```

#### Q0311 — `read_source_total_bytes`

Source: [src/runtime_telemetry.rs:269](../src/runtime_telemetry.rs#L269).

```sql
SELECT COALESCE(SUM(size), 0) FROM files
```

#### Q0312 — `read_graph_counts`

Source: [src/runtime_telemetry.rs:294](../src/runtime_telemetry.rs#L294).

```sql
SELECT COUNT(*) FROM nodes
```

#### Q0313 — `read_graph_counts`

Source: [src/runtime_telemetry.rs:295](../src/runtime_telemetry.rs#L295).

```sql
SELECT COUNT(*) FROM edges
```

### src/tokensave/memory.rs

#### Q0314 — `record_decision`

Source: [src/tokensave/memory.rs:100](../src/tokensave/memory.rs#L100).

```sql
INSERT INTO memory_decisions (text, reason, created_at, files, tags) VALUES (?1, ?2, ?3, ?4, ?5)
```

#### Q0315 — `session_recall`

Source: [src/tokensave/memory.rs:140](../src/tokensave/memory.rs#L140).

```sql
SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags FROM memory_decisions d JOIN memory_decisions_fts f ON f.rowid = d.id WHERE memory_decisions_fts MATCH ?1 AND d.created_at >= ?2 ORDER BY d.created_at DESC LIMIT ?3
```

#### Q0316 — `session_recall`

Source: [src/tokensave/memory.rs:151](../src/tokensave/memory.rs#L151).

```sql
SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags FROM memory_decisions d JOIN memory_decisions_fts f ON f.rowid = d.id WHERE memory_decisions_fts MATCH ?1 ORDER BY d.created_at DESC LIMIT ?2
```

#### Q0317 — `session_recall`

Source: [src/tokensave/memory.rs:162](../src/tokensave/memory.rs#L162).

```sql
SELECT id, text, reason, created_at, files, tags FROM memory_decisions WHERE created_at >= ?1 ORDER BY created_at DESC LIMIT ?2
```

#### Q0318 — `session_recall`

Source: [src/tokensave/memory.rs:171](../src/tokensave/memory.rs#L171).

```sql
SELECT id, text, reason, created_at, files, tags FROM memory_decisions ORDER BY created_at DESC LIMIT ?1
```

#### Q0319 — `record_code_area`

Source: [src/tokensave/memory.rs:274](../src/tokensave/memory.rs#L274).

```sql
INSERT INTO memory_code_areas (path, description, last_touched_at, touch_count) VALUES (?1, ?2, ?3, 1) ON CONFLICT(path) DO UPDATE SET description = COALESCE(excluded.description, memory_code_areas.description), last_touched_at = excluded.last_touched_at, touch_count = memory_code_areas.touch_count + 1
```

#### Q0320 — `list_code_areas`

Source: [src/tokensave/memory.rs:299](../src/tokensave/memory.rs#L299).

```sql
SELECT id, path, description, last_touched_at, touch_count FROM memory_code_areas ORDER BY last_touched_at DESC LIMIT ?1
```

### src/tokensave/rename.rs

#### Q0321 — `reserved_words`

Source: [src/tokensave/rename.rs:237](../src/tokensave/rename.rs#L237).

```sql
select
```

#### Q0322 — `reserved_words`

Source: [src/tokensave/rename.rs:589](../src/tokensave/rename.rs#L589).

```sql
BEGIN
```

#### Q0323 — `reserved_words`

Source: [src/tokensave/rename.rs:589](../src/tokensave/rename.rs#L589).

```sql
begin
```

### src/tokensave/runtime.rs

#### Q0324 — `project`

Source: [src/tokensave/runtime.rs:102](../src/tokensave/runtime.rs#L102).

```sql
commit
```

#### Q0325 — `round_trip_tracks_and_syncs_without_polluting_parent`

Source: [src/tokensave/runtime.rs:120](../src/tokensave/runtime.rs#L120).

```sql
commit
```

### src/upgrade.rs

#### Q0326 — `locate_verified_asset`

Source: [src/upgrade.rs:135](../src/upgrade.rs#L135).

```sql
release {tag} exists but asset '{expected_asset}' is not yet available.
  CI build may still be in progress — try again in a few minutes.
  https://github.com/{GITHUB_REPO}/releases/tag/{tag}
```

#### Q0327 — `locate_verified_asset`

Source: [src/upgrade.rs:147](../src/upgrade.rs#L147).

```sql
release {tag} publishes no {SUMS_ASSET}, so the download cannot be verified — refusing to install.
  If CI is still finishing, try again in a few minutes:
  https://github.com/{GITHUB_REPO}/releases/tag/{tag}
```

#### Q0328 — `locate_verified_asset`

Source: [src/upgrade.rs:158](../src/upgrade.rs#L158).

```sql
release {tag} publishes {SUMS_ASSET} but it lists no hash for '{expected_asset}' — refusing to install.
  https://github.com/{GITHUB_REPO}/releases/tag/{tag}
```

### src/verify.rs

#### Q0329 — `inspect`

Source: [src/verify.rs:209](../src/verify.rs#L209).

```sql
PRAGMA query_only=1; PRAGMA busy_timeout=1000; PRAGMA cache_size=-8192; PRAGMA mmap_size=0;
```

#### Q0330 — `inspect`

Source: [src/verify.rs:213](../src/verify.rs#L213).

```sql
PRAGMA user_version
```

#### Q0331 — `inspect`

Source: [src/verify.rs:232](../src/verify.rs#L232).

```sql
PRAGMA integrity_check
```

#### Q0332 — `inspect`

Source: [src/verify.rs:234](../src/verify.rs#L234).

```sql
PRAGMA quick_check
```

#### Q0333 — `inspect`

Source: [src/verify.rs:255](../src/verify.rs#L255).

```sql
SELECT COUNT(*) FROM edges e WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=e.source) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=e.target)
```

#### Q0334 — `inspect`

Source: [src/verify.rs:256](../src/verify.rs#L256).

```sql
SELECT COUNT(*) FROM nodes n WHERE NOT EXISTS (SELECT 1 FROM files f WHERE f.path=n.file_path)
```

#### Q0335 — `inspect`

Source: [src/verify.rs:257](../src/verify.rs#L257).

```sql
SELECT COUNT(*) FROM nodes n WHERE n.parent_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM nodes p WHERE p.id=n.parent_id)
```

#### Q0336 — `inspect`

Source: [src/verify.rs:258](../src/verify.rs#L258).

```sql
SELECT COUNT(*) FROM unresolved_refs r WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=r.from_node_id)
```

#### Q0337 — `inspect`

Source: [src/verify.rs:259](../src/verify.rs#L259).

```sql
SELECT COUNT(*) FROM vectors v WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=v.node_id)
```

#### Q0338 — `inspect`

Source: [src/verify.rs:260](../src/verify.rs#L260).

```sql
SELECT COUNT(*) FROM ambiguous_calls a WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=a.from_node_id)
```

#### Q0339 — `inspect`

Source: [src/verify.rs:261](../src/verify.rs#L261).

```sql
SELECT COUNT(*) FROM trait_dispatch_callers d WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.concrete_method_id) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.trait_method_id) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.caller_id)
```

#### Q0340 — `inspect`

Source: [src/verify.rs:274](../src/verify.rs#L274).

```sql
SELECT path, content_hash, modified_at FROM files ORDER BY path
```

### src/worktree.rs

#### Q0341 — `flags_mismatch_when_started_from_linked_worktree`

Source: [src/worktree.rs:224](../src/worktree.rs#L224).

```sql
commit
```

#### Q0342 — `main_with_sibling_worktree`

Source: [src/worktree.rs:273](../src/worktree.rs#L273).

```sql
commit
```

### `src/db/compatibility.rs`

#### Q0343 — `check_channel`

Source: [src/db/compatibility.rs:42](../src/db/compatibility.rs#L42).

```sql
PRAGMA application_id
```

#### Q0344 — `check_channel`

Source: [src/db/compatibility.rs:54](../src/db/compatibility.rs#L54).

```sql
PRAGMA user_version
```

#### Q0345 — `mark_empty`

Source: [src/db/compatibility.rs:91](../src/db/compatibility.rs#L91).

```sql
SELECT count(*) FROM sqlite_schema
```

#### Q0346 — `mark_empty`

Source: [src/db/compatibility.rs:92](../src/db/compatibility.rs#L92).

```sql
PRAGMA application_id
```

#### Q0347 — `mark_empty`

Source: [src/db/compatibility.rs:99](../src/db/compatibility.rs#L99).

```sql
PRAGMA application_id = {origin}
```

#### Q0348 — `channel_policy_preserves_origin`

Source: [src/db/compatibility.rs:120](../src/db/compatibility.rs#L120).

```sql
PRAGMA application_id = {origin}
```

#### Q0349 — `channel_policy_preserves_origin`

Source: [src/db/compatibility.rs:126](../src/db/compatibility.rs#L126).

```sql
PRAGMA application_id
```

## Dynamic construction and binding context

The following source functions contain SQL plus formatting, clause assembly, or shared SQL fragments. They preserve the branching logic, escaping, placeholder generation, value binding, and batch assembly needed to understand the templates above. Some formatting is only error-message construction; those functions are retained conservatively to avoid omitting relevant context. Helper-only builders are included separately below.

### `global_forwarder_runs_repository_hook_from_linked_worktree` — src/agents/hooks.rs:2342

Source: [src/agents/hooks.rs:2342](../src/agents/hooks.rs#L2342).

```rust
    fn global_forwarder_runs_repository_hook_from_linked_worktree() {
        use std::os::unix::fs::PermissionsExt;

        let repo = tempfile::tempdir().unwrap();
        let run_git = |dir: &Path, args: &[&str], hooks_path: Option<&Path>| {
            let mut command = Command::new("git");
            command
                .args(args)
                .current_dir(dir)
                .env("HOME", repo.path())
                .env("XDG_CONFIG_HOME", repo.path().join(".config"));
            if let Some(hooks_path) = hooks_path {
                command
                    .env("GIT_CONFIG_COUNT", "1")
                    .env("GIT_CONFIG_KEY_0", "core.hooksPath")
                    .env("GIT_CONFIG_VALUE_0", hooks_path);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };

        run_git(repo.path(), &["init", "-q", "-b", "main"], None);
        run_git(
            repo.path(),
            &["config", "user.name", "TokenSave Test"],
            None,
        );
        run_git(
            repo.path(),
            &["config", "user.email", "tokensave@example.com"],
            None,
        );
        std::fs::write(repo.path().join("README"), "main\n").unwrap();
        run_git(repo.path(), &["add", "README"], None);
        run_git(repo.path(), &["commit", "-qm", "initial"], None);

        let sentinel = repo.path().join("hook-fired");
        let repo_hook = repo.path().join(".git").join("hooks").join("pre-commit");
        std::fs::write(
            &repo_hook,
            format!("#!/bin/sh\nprintf fired > '{}'\n", sentinel.display()),
        )
        .unwrap();
        std::fs::set_permissions(&repo_hook, std::fs::Permissions::from_mode(0o755)).unwrap();

        let global_hooks = tempfile::tempdir().unwrap();
        let global_hook = global_hooks.path().join("pre-commit");
        std::fs::write(
            &global_hook,
            format!("#!/bin/sh\n{}", chain_repo_hook_snippet("pre-commit")),
        )
        .unwrap();
        std::fs::set_permissions(&global_hook, std::fs::Permissions::from_mode(0o755)).unwrap();

        let worktree = repo.path().join("worktree");
        run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature",
                worktree.to_str().unwrap(),
            ],
            None,
        );
        std::fs::write(worktree.join("feature"), "feature\n").unwrap();
        run_git(&worktree, &["add", "feature"], Some(global_hooks.path()));
        run_git(
            &worktree,
            &["commit", "-qm", "feature"],
            Some(global_hooks.path()),
        );

        assert_eq!(
            std::fs::read_to_string(&sentinel).unwrap(),
            "fired",
            "the global forwarder must reach the shared repository hook from a linked worktree"
        );
    }
```

### `cargo_build_binary_warning` — src/agents/mod.rs:207

Source: [src/agents/mod.rs:207](../src/agents/mod.rs#L207).

```rust
pub fn cargo_build_binary_warning(path: &str) -> Option<String> {
    let components: Vec<_> = Path::new(path)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();

    let is_profile = |value: &str| matches!(value, "debug" | "release");
    let cargo_build = components
        .windows(2)
        .any(|parts| parts[0] == "target" && is_profile(parts[1]))
        || components
            .windows(3)
            .any(|parts| parts[0] == "target" && is_profile(parts[2]));

    cargo_build.then(|| {
        format!(
            "\x1b[33mwarning:\x1b[0m agent config references Cargo build output:\n  \
             {path}\n  `cargo clean` or removing its worktree will break tokensave hooks and MCP \
             servers.\n  Re-run `tokensave install` from a stable `cargo install`, Homebrew, or \
             release binary."
        )
    })
}
```

### `log_returns_events_when_function_body_changes` — src/blame_engine.rs:844

Source: [src/blame_engine.rs:844](../src/blame_engine.rs#L844).

```rust
    fn log_returns_events_when_function_body_changes() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            let st = Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap();
            assert!(st.success());
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // c1: original body
        std::fs::write(
            root.join("foo.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .unwrap();
        git(&["add", "foo.rs"]);
        git(&["commit", "-q", "-m", "c1: initial"]);

        // c2: change comment only — should NOT yield an event (fingerprint unchanged)
        std::fs::write(
            root.join("foo.rs"),
            "// trivial helper\npub fn add(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .unwrap();
        git(&["commit", "-q", "-am", "c2: comment"]);

        // c3: real mutation
        std::fs::write(
            root.join("foo.rs"),
            "// trivial helper\npub fn add(a: i32, b: i32) -> i32 { let s = a + b; s }\n",
        )
        .unwrap();
        git(&["commit", "-q", "-am", "c3: rebody"]);

        let source = std::fs::read_to_string(root.join("foo.rs")).unwrap();
        // c3 leaves the `pub fn add` body starting on line 2 (0-indexed: 1).
        let fp = compute_target_fingerprint(&source, "rust", 1, 1).expect("fp");
        let result = log(root, "foo.rs", 1, 1, "rust", &fp, &BlameOptions::default()).expect("log");

        // Expect two distinct mutation events (c1 introduction + c3 rebody).
        // c2 (comment-only) should be filtered because the fingerprint matches c3.
        assert_eq!(result.events.len(), 2, "got: {:#?}", result.events);
        // Oldest first
        assert!(result.events[0].summary.contains("c1"));
        assert!(result.events[1].summary.contains("c3"));
        assert_eq!(result.boundary_reason, BoundaryReason::Introduced);
    }
```

### `log_follows_function_renamed_across_files` — src/blame_engine.rs:901

Source: [src/blame_engine.rs:901](../src/blame_engine.rs#L901).

```rust
    fn log_follows_function_renamed_across_files() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            let st = Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap();
            assert!(st.success());
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // c1: function in a.rs
        std::fs::write(root.join("a.rs"), "pub fn helper() -> i32 { 42 }\n").unwrap();
        git(&["add", "a.rs"]);
        git(&["commit", "-q", "-m", "c1: born in a.rs"]);

        // c2: function moves to b.rs (a.rs deleted, b.rs created)
        std::fs::remove_file(root.join("a.rs")).unwrap();
        std::fs::write(root.join("b.rs"), "pub fn helper() -> i32 { 42 }\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "c2: moved to b.rs"]);

        // c3: change body in b.rs
        std::fs::write(root.join("b.rs"), "pub fn helper() -> i32 { 43 }\n").unwrap();
        git(&["commit", "-q", "-am", "c3: edit in b.rs"]);

        let source = std::fs::read_to_string(root.join("b.rs")).unwrap();
        let fp = compute_target_fingerprint(&source, "rust", 0, 0).expect("fp");
        let result = log(root, "b.rs", 0, 0, "rust", &fp, &BlameOptions::default()).expect("log");

        // Should have at least one event from each side of the rename, and the
        // boundary should be RenamedFrom (with the prior file recorded).
        assert!(
            result.events.iter().any(|e| e.file_at_commit == "a.rs"),
            "expected an event from a.rs in {:#?}",
            result.events
        );
        assert_eq!(result.boundary_reason, BoundaryReason::RenamedFrom);
    }
```

### `max_commits_boundary_classifies_correctly` — src/blame_engine.rs:948

Source: [src/blame_engine.rs:948](../src/blame_engine.rs#L948).

```rust
    fn max_commits_boundary_classifies_correctly() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap();
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // 6 commits, all changing foo.rs
        for i in 0..6 {
            std::fs::write(
                root.join("foo.rs"),
                format!("pub fn add() -> i32 {{ {i} }}\n"),
            )
            .unwrap();
            if i == 0 {
                git(&["add", "foo.rs"]);
            }
            git(&["commit", "-q", "-am", &format!("c{i}")]);
        }

        let opts = BlameOptions {
            max_commits: 3,
            ..BlameOptions::default()
        };
        let source = std::fs::read_to_string(root.join("foo.rs")).unwrap();
        let fp = compute_target_fingerprint(&source, "rust", 0, 0).expect("fp");
        let result = log(root, "foo.rs", 0, 0, "rust", &fp, &opts).expect("log");
        assert_eq!(result.commits_walked, 3);
        assert_eq!(result.boundary_reason, BoundaryReason::MaxCommitsReached);
    }
```

### `blob_size_cap_records_skipped` — src/blame_engine.rs:989

Source: [src/blame_engine.rs:989](../src/blame_engine.rs#L989).

```rust
    fn blob_size_cap_records_skipped() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap();
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // Write a large file (3 MB of comments + a tiny function)
        let huge = format!(
            "// {}\npub fn tiny() -> i32 {{ 1 }}\n",
            "x".repeat(3 * 1024 * 1024)
        );
        std::fs::write(root.join("foo.rs"), &huge).unwrap();
        git(&["add", "foo.rs"]);
        git(&["commit", "-q", "-m", "huge"]);

        let opts = BlameOptions {
            max_blob_bytes: 1024, // tiny cap to force skip
            ..BlameOptions::default()
        };
        let small_src = "pub fn tiny() -> i32 { 1 }\n";
        let fp = compute_target_fingerprint(small_src, "rust", 0, 0).expect("fp");
        let result = log(root, "foo.rs", 0, 0, "rust", &fp, &opts).expect("log");
        assert_eq!(result.skipped_large.len(), 1);
    }
```

### `walk_history_yields_commits_in_reverse_chrono_order` — src/blame_engine.rs:1025

Source: [src/blame_engine.rs:1025](../src/blame_engine.rs#L1025).

```rust
    fn walk_history_yields_commits_in_reverse_chrono_order() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        // Bootstrap a tiny repo with three commits touching foo.rs.
        let run = |args: &[&str]| {
            let st = Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap();
            assert!(st.success(), "git {args:?} failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@t"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "commit.gpgsign", "false"]);

        std::fs::write(root.join("foo.rs"), "fn a() {}\n").unwrap();
        run(&["add", "foo.rs"]);
        run(&["commit", "-q", "-m", "c1"]);

        std::fs::write(root.join("foo.rs"), "fn a() { let _ = 1; }\n").unwrap();
        run(&["commit", "-q", "-am", "c2"]);

        std::fs::write(root.join("foo.rs"), "fn a() { let _ = 2; }\n").unwrap();
        run(&["commit", "-q", "-am", "c3"]);

        let walk = walk_file_history(root, "foo.rs", 10).expect("walk");
        let commits = &walk.visits;
        assert_eq!(commits.len(), 3, "expected 3 commits, got {commits:?}");
        // Reverse chrono: c3 first, c1 last.
        assert!(commits[0].summary.contains("c3"));
        assert!(commits[2].summary.contains("c1"));
    }
```

### `copy_branch_db` — src/branch.rs:68

Source: [src/branch.rs:68](../src/branch.rs#L68).

```rust
pub async fn copy_branch_db(parent_db: &Path, new_db_path: &Path) -> crate::errors::Result<()> {
    if let Err(error) =
        crate::db::compatibility::preflight(parent_db, crate::db::migrations::latest_version())
            .await
    {
        if crate::db::compatibility::is_refusal(&error) {
            return Err(error);
        }
    }
    let file_name = new_db_path.file_name().map_or_else(
        || "branch.db".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp =
        new_db_path.with_file_name(format!(".{file_name}.{}.{nanos}.tmp", std::process::id()));
    if vacuum_into(parent_db, &tmp).await.is_ok() {
        if let Err(e) = std::fs::rename(&tmp, new_db_path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        return Ok(());
    }
    let _ = std::fs::remove_file(&tmp);

    let parent_wal = wal_sidecar(parent_db);
    if file_len(&parent_wal) > 0 {
        // Errors are deliberately ignored: the non-empty-WAL check below
        // catches every failure mode uniformly by falling back to a sidecar
        // copy.
        if let Ok(db) = libsql::Builder::new_local(parent_db).build().await {
            if let Ok(conn) = db.connect() {
                let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").await;
            }
        }
    }
    std::fs::copy(parent_db, new_db_path)?;
    if file_len(&parent_wal) > 0 {
        std::fs::copy(&parent_wal, wal_sidecar(new_db_path))?;
    }
    Ok(())
}
```

### `try_open_read_only` — src/db/connection.rs:220

Source: [src/db/connection.rs:220](../src/db/connection.rs#L220).

```rust
    async fn try_open_read_only(db_path: &Path) -> Result<Self> {
        super::compatibility::preflight(db_path, migrations::latest_version()).await?;
        let db = libsql::Builder::new_local(db_path)
            .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .build()
            .await
            .map_err(|error| TokenSaveError::Database {
                message: format!("failed to open database read-only: {error}"),
                operation: "open_read_only".to_string(),
            })?;
        let conn = db.connect().map_err(|error| TokenSaveError::Database {
            message: format!("failed to connect to database read-only: {error}"),
            operation: "open_read_only".to_string(),
        })?;

        conn.execute_batch("PRAGMA query_only = 1; PRAGMA busy_timeout = 120000;")
            .await
            .map_err(|error| TokenSaveError::Database {
                message: format!("failed to apply read-only pragmas: {error}"),
                operation: "open_read_only".to_string(),
            })?;

        let mut rows = conn
            .query("PRAGMA user_version", ())
            .await
            .map_err(|error| TokenSaveError::Database {
                message: format!("failed to read database schema version: {error}"),
                operation: "open_read_only".to_string(),
            })?;
        let row = rows
            .next()
            .await
            .map_err(|error| TokenSaveError::Database {
                message: format!("failed to read database schema version: {error}"),
                operation: "open_read_only".to_string(),
            })?
            .ok_or_else(|| TokenSaveError::Database {
                message: "database did not report a schema version".to_string(),
                operation: "open_read_only".to_string(),
            })?;
        let version: i64 = row.get(0).map_err(|error| TokenSaveError::Database {
            message: format!("failed to decode database schema version: {error}"),
            operation: "open_read_only".to_string(),
        })?;
        let latest = migrations::latest_version();
        if version != i64::from(latest) {
            let remedy = if version < i64::from(latest) {
                "run `tokensave sync` in the selected project to migrate it"
            } else {
                "upgrade Tokensave to a version compatible with the selected project"
            };
            return Err(TokenSaveError::Config {
                message: format!(
                    "database schema version {version} does not match required version {latest}; {remedy}"
                ),
            });
        }

        let database = Self {
            conn,
            _db: db,
            read_only: true,
            trait_dispatch_callers: RwLock::new(HashMap::new()),
            write_lock: tokio::sync::Mutex::new(()),
        };
        database.refresh_trait_dispatch_callers().await?;
        Ok(database)
    }
```

### `checkpoint` — src/db/connection.rs:308

Source: [src/db/connection.rs:308](../src/db/connection.rs#L308).

```rust
    pub async fn checkpoint(&self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to checkpoint WAL: {e}"),
                operation: "checkpoint".to_string(),
            })?;
        Ok(())
    }
```

### `optimize` — src/db/connection.rs:320

Source: [src/db/connection.rs:320](../src/db/connection.rs#L320).

```rust
    pub async fn optimize(&self) -> Result<()> {
        self.conn
            .execute_batch("VACUUM; ANALYZE;")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to optimize database: {e}"),
                operation: "optimize".to_string(),
            })?;
        Ok(())
    }
```

### `size` — src/db/connection.rs:332

Source: [src/db/connection.rs:332](../src/db/connection.rs#L332).

```rust
    pub async fn size(&self) -> Result<u64> {
        let mut rows = self
            .conn
            .query(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to get database size: {e}"),
                operation: "size".to_string(),
            })?;

        let row = rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read database size row: {e}"),
                operation: "size".to_string(),
            })?
            .ok_or_else(|| TokenSaveError::Database {
                message: "no result from page size query".to_string(),
                operation: "size".to_string(),
            })?;

        let size = row.get::<i64>(0).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read size value: {e}"),
            operation: "size".to_string(),
        })?;

        Ok(size as u64)
    }
```

### `quick_check` — src/db/connection.rs:369

Source: [src/db/connection.rs:369](../src/db/connection.rs#L369).

```rust
    pub async fn quick_check(&self) -> Result<bool> {
        let mut rows = self
            .conn
            .query("PRAGMA quick_check", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to run quick_check: {e}"),
                operation: "quick_check".to_string(),
            })?;

        if let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read quick_check result: {e}"),
            operation: "quick_check".to_string(),
        })? {
            let result: String = row.get::<String>(0).unwrap_or_default();
            Ok(result == "ok")
        } else {
            Ok(false)
        }
    }
```

### `needs_bulk_load_finalization` — src/db/connection.rs:399

Source: [src/db/connection.rs:399](../src/db/connection.rs#L399).

```rust
    pub async fn needs_bulk_load_finalization(&self) -> Result<bool> {
        let mut names = self
            .conn
            .query(
                "SELECT name FROM sqlite_master WHERE type IN ('table', 'index')",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to inspect schema for bulk-load state: {e}"),
                operation: "needs_bulk_load_finalization".to_string(),
            })?;
        let mut has_edges = false;
        let mut has_unique_index = false;
        while let Some(row) = names.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read schema row: {e}"),
            operation: "needs_bulk_load_finalization".to_string(),
        })? {
            match row.get::<String>(0).unwrap_or_default().as_str() {
                "edges" => has_edges = true,
                "idx_edges_unique" => has_unique_index = true,
                _ => {}
            }
        }
        Ok(has_edges && !has_unique_index)
    }
```

### `rebuild_fts` — src/db/connection.rs:430

Source: [src/db/connection.rs:430](../src/db/connection.rs#L430).

```rust
    pub async fn rebuild_fts(&self) -> Result<()> {
        self.conn
            .execute("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to rebuild FTS index: {e}"),
                operation: "rebuild_fts".to_string(),
            })?;
        Ok(())
    }
```

### `apply_pragmas` — src/db/connection.rs:445

Source: [src/db/connection.rs:445](../src/db/connection.rs#L445).

```rust
    async fn apply_pragmas(conn: &Connection, db_file_size: u64) -> Result<()> {
        let (cache_kb, mmap) = adaptive_cache_sizes(db_file_size);
        let busy_ms = busy_timeout_ms();
        conn.execute_batch(&format!(
            "PRAGMA page_size = 8192;
             PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = {busy_ms};
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -{cache_kb};
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = {mmap};",
        ))
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to apply pragmas: {e}"),
            operation: "apply_pragmas".to_string(),
        })?;
        Ok(())
    }
```

### `begin_bulk_load` — src/db/connection.rs:470

Source: [src/db/connection.rs:470](../src/db/connection.rs#L470).

```rust
    pub async fn begin_bulk_load(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
             DROP INDEX IF EXISTS idx_nodes_kind;
             DROP INDEX IF EXISTS idx_nodes_name;
             DROP INDEX IF EXISTS idx_nodes_qualified_name;
             DROP INDEX IF EXISTS idx_nodes_file_path;
             DROP INDEX IF EXISTS idx_nodes_file_path_start_line;
             DROP INDEX IF EXISTS idx_edges_source;
             DROP INDEX IF EXISTS idx_edges_target;
             DROP INDEX IF EXISTS idx_edges_kind;
             DROP INDEX IF EXISTS idx_edges_source_kind;
             DROP INDEX IF EXISTS idx_edges_target_kind;
             DROP INDEX IF EXISTS idx_edges_unique;
             DROP INDEX IF EXISTS idx_unresolved_refs_from_node_id;
             DROP INDEX IF EXISTS idx_unresolved_refs_reference_name;
             DROP INDEX IF EXISTS idx_unresolved_refs_file_path;
             DROP TRIGGER IF EXISTS nodes_fts_insert;
             DROP TRIGGER IF EXISTS nodes_fts_delete;
             DROP TRIGGER IF EXISTS nodes_fts_update;
             DROP TRIGGER IF EXISTS trait_dispatch_call_insert;
             DROP TRIGGER IF EXISTS trait_dispatch_implements_insert;
             DROP TRIGGER IF EXISTS trait_dispatch_call_delete;
             DROP TRIGGER IF EXISTS trait_dispatch_implements_delete;
             DELETE FROM nodes_fts;",
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin bulk load: {e}"),
                operation: "begin_bulk_load".to_string(),
            })?;
        Ok(())
    }
```

### `end_bulk_load` — src/db/connection.rs:514

Source: [src/db/connection.rs:514](../src/db/connection.rs#L514).

```rust
    pub async fn end_bulk_load(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM edges
             WHERE rowid NOT IN (
                 SELECT MIN(rowid) FROM edges
                 GROUP BY source, target, kind, COALESCE(line, -1)
             );
             CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
             CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
             CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
             CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
             CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);
             CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
             CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
             CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique ON edges(source, target, kind, COALESCE(line, -1));
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);
             CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
                 INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
             END;
             CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
                 INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
             END;
             CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
                 INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
                 INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
                 VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
             END;
             INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
                 SELECT rowid, name, qualified_name, docstring, signature, search_terms FROM nodes;
             PRAGMA foreign_keys = ON;",
        ).await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to end bulk load: {e}"),
            operation: "end_bulk_load".to_string(),
        })?;
        self.conn
            .execute_batch(crate::db::migrations::TRAIT_DISPATCH_TRIGGERS_SQL)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to restore trait dispatch triggers: {e}"),
                operation: "end_bulk_load".to_string(),
            })?;
        Ok(())
    }
```

### `get_version` — src/db/migrations.rs:98

Source: [src/db/migrations.rs:98](../src/db/migrations.rs#L98).

```rust
async fn get_version(conn: &Connection) -> Result<u32> {
    let mut rows =
        conn.query("PRAGMA user_version", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read user_version: {e}"),
                operation: "get_version".to_string(),
            })?;
    let row = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read user_version row: {e}"),
        operation: "get_version".to_string(),
    })?;
    match row {
        Some(r) => {
            let v: i64 = r.get(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read user_version value: {e}"),
                operation: "get_version".to_string(),
            })?;
            Ok(v as u32)
        }
        None => Ok(0),
    }
}
```

### `set_version` — src/db/migrations.rs:126

Source: [src/db/migrations.rs:126](../src/db/migrations.rs#L126).

```rust
async fn set_version(conn: &Connection, version: u32) -> Result<()> {
    conn.execute(&format!("PRAGMA user_version = {version}"), ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to set user_version: {e}"),
            operation: "set_version".to_string(),
        })?;
    Ok(())
}
```

### `create_schema` — src/db/migrations.rs:138

Source: [src/db/migrations.rs:138](../src/db/migrations.rs#L138).

```rust
pub async fn create_schema(conn: &Connection) -> Result<()> {
    super::compatibility::check(conn, LATEST_VERSION).await?;
    // Do not stamp an existing schema as current without running migrations.
    if get_version(conn).await? != 0 {
        migrate(conn).await?;
        return Ok(());
    }
    super::compatibility::mark_empty(conn).await?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            start_column INTEGER NOT NULL,
            end_column INTEGER NOT NULL,
            docstring TEXT,
            signature TEXT,
            visibility TEXT NOT NULL DEFAULT 'private',
            is_async INTEGER NOT NULL DEFAULT 0,
            branches INTEGER NOT NULL DEFAULT 0,
            loops INTEGER NOT NULL DEFAULT 0,
            returns INTEGER NOT NULL DEFAULT 0,
            max_nesting INTEGER NOT NULL DEFAULT 0,
            unsafe_blocks INTEGER NOT NULL DEFAULT 0,
            unchecked_calls INTEGER NOT NULL DEFAULT 0,
            assertions INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL,
            attrs_start_line INTEGER NOT NULL DEFAULT 0,
            parent_id TEXT,
            cognitive_complexity INTEGER NOT NULL DEFAULT 0,
            distinct_operators INTEGER NOT NULL DEFAULT 0,
            distinct_operands INTEGER NOT NULL DEFAULT 0,
            total_operators INTEGER NOT NULL DEFAULT 0,
            total_operands INTEGER NOT NULL DEFAULT 0,
            search_terms TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            resolved_by INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL,
            node_count INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'code'
        );

        CREATE TABLE IF NOT EXISTS unresolved_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_node_id TEXT NOT NULL,
            reference_name TEXT NOT NULL,
            reference_kind TEXT NOT NULL,
            line INTEGER NOT NULL,
            col INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS vectors (
            node_id TEXT PRIMARY KEY,
            embedding BLOB NOT NULL,
            model TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name, qualified_name, docstring, signature, search_terms,
            content='nodes', content_rowid='rowid',
            tokenize='porter unicode61'
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS executable_body_fts USING fts5(
            node_id UNINDEXED,
            file_path UNINDEXED,
            body,
            tokenize='unicode61'
        );

        CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );

        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
        CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);

        CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique
            ON edges(source, target, kind, COALESCE(line, -1));

        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);

        CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name));
        CREATE INDEX IF NOT EXISTS idx_nodes_parent_id ON nodes(parent_id);

        CREATE TABLE IF NOT EXISTS node_fingerprints (
            node_id TEXT PRIMARY KEY,
            ast_hash TEXT NOT NULL,
            cfg_hash TEXT NOT NULL,
            call_seq_hash TEXT NOT NULL,
            shingles TEXT NOT NULL,
            body_tokens INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_ast ON node_fingerprints(ast_hash);
        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_size ON node_fingerprints(body_tokens);

        CREATE TABLE IF NOT EXISTS memory_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reason TEXT,
            created_at INTEGER NOT NULL,
            files TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS memory_code_areas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            description TEXT,
            last_touched_at INTEGER NOT NULL,
            touch_count INTEGER NOT NULL DEFAULT 1
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_code_areas_path ON memory_code_areas(path);
        CREATE INDEX IF NOT EXISTS idx_memory_decisions_created_at ON memory_decisions(created_at);

        CREATE TABLE IF NOT EXISTS read_cache (
            project_id   TEXT NOT NULL,
            session_id   TEXT NOT NULL,
            file_path    TEXT NOT NULL,
            mtime_ns     INTEGER NOT NULL,
            mode         TEXT NOT NULL,
            args_hash    TEXT NOT NULL,
            digest       TEXT NOT NULL,
            body         BLOB NOT NULL,
            token_count  INTEGER NOT NULL,
            created_at   INTEGER NOT NULL,
            PRIMARY KEY (project_id, session_id, file_path, mode, args_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_read_cache_session
            ON read_cache(session_id, created_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_decisions_fts USING fts5(
            text, reason,
            content='memory_decisions', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_insert
            AFTER INSERT ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_delete
            AFTER DELETE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_update
            AFTER UPDATE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("failed to create schema: {e}"),
        operation: "create_schema".to_string(),
    })?;

    conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to create trait dispatch triggers: {e}"),
            operation: "create_schema".to_string(),
        })?;

    // A fresh database is stamped at LATEST_VERSION and never replays
    // migrations, so anything a migration adds has to be created here too.
    // Sharing one statement keeps the two definitions from drifting.
    conn.execute_batch(AMBIGUOUS_CALLS_SQL)
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to create ambiguous_calls: {e}"),
            operation: "create_schema".to_string(),
        })?;

    set_version(conn, LATEST_VERSION).await?;
    Ok(())
}
```

### `migrate` — src/db/migrations.rs:393

Source: [src/db/migrations.rs:393](../src/db/migrations.rs#L393).

```rust
pub async fn migrate(conn: &Connection) -> Result<bool> {
    let current = get_version(conn).await?;
    super::compatibility::check(conn, LATEST_VERSION).await?;
    if current >= LATEST_VERSION {
        return Ok(false);
    }

    eprintln!("[tokensave] migrating database schema v{current} → v{LATEST_VERSION}…");

    // BEGIN EXCLUSIVE blocks other writers (including other MCP servers or
    // post-commit hooks) until we COMMIT. Readers using WAL mode are not
    // blocked.
    conn.execute("BEGIN EXCLUSIVE", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to acquire exclusive lock: {e}"),
            operation: "migrate_lock".to_string(),
        })?;

    // Re-read inside the lock in case another process migrated between our
    // check and the lock acquisition.
    let current = get_version(conn).await?;

    let result = async {
        // Another process may have changed the header while we waited for
        // the migration lock. Refuse before executing any migration writes.
        super::compatibility::check(conn, LATEST_VERSION).await?;
        if current == 0 {
            // open() can also create an empty database; legacy schemas with
            // tables remain unmarked. The stamp rolls back on migration failure.
            super::compatibility::mark_empty(conn).await?;
        }
        run_migrations(conn, current).await
    }
    .await;

    match result {
        Ok(()) => {
            conn.execute("COMMIT", ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to commit migrations: {e}"),
                    operation: "migrate_commit".to_string(),
                })?;
            Ok(true)
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", ()).await;
            if super::compatibility::is_refusal(&e) {
                return Err(e);
            }
            // A busy, locked, full or failing disk is not a broken schema: the
            // same migration succeeds once the condition clears, and the data
            // must not be thrown away for it. Those keep their own error.
            if is_environmental_error(&e) {
                return Err(TokenSaveError::Database {
                    message: format!(
                        "schema migration v{current} → v{LATEST_VERSION} could not run ({e}). \
                         Retry once no other tokensave process is using the index and the \
                         disk has room."
                    ),
                    operation: "migrate_environment".to_string(),
                });
            }
            // The transaction rolled back, so the database is still the old
            // version and every later open fails the same way. Say how out:
            // `sync --force` rebuilds a database whose migration fails.
            Err(TokenSaveError::Database {
                message: format!(
                    "schema migration v{current} → v{LATEST_VERSION} failed ({e}). \
                     Run `tokensave sync --force` to rebuild the index."
                ),
                operation: MIGRATION_FAILED.to_string(),
            })
        }
    }
}
```

### `migrate_v1` — src/db/migrations.rs:552

Source: [src/db/migrations.rs:552](../src/db/migrations.rs#L552).

```rust
async fn migrate_v1(conn: &Connection) -> Result<()> {
    // Tables
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            start_column INTEGER NOT NULL,
            end_column INTEGER NOT NULL,
            docstring TEXT,
            signature TEXT,
            visibility TEXT NOT NULL DEFAULT 'private',
            is_async INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL,
            node_count INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'code'
        );

        CREATE TABLE IF NOT EXISTS unresolved_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_node_id TEXT NOT NULL,
            reference_name TEXT NOT NULL,
            reference_kind TEXT NOT NULL,
            line INTEGER NOT NULL,
            col INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS vectors (
            node_id TEXT PRIMARY KEY,
            embedding BLOB NOT NULL,
            model TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v1: failed to create tables: {e}"),
        operation: "migrate_v1".to_string(),
    })?;

    // FTS5
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name,
            qualified_name,
            docstring,
            signature,
            content='nodes',
            content_rowid='rowid'
        );

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v1: failed to create FTS: {e}"),
        operation: "migrate_v1".to_string(),
    })?;

    // Indexes
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
        CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);

        CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
        CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
        CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
        CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);

        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v1: failed to create indexes: {e}"),
        operation: "migrate_v1".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v2` — src/db/migrations.rs:683

Source: [src/db/migrations.rs:683](../src/db/migrations.rs#L683).

```rust
async fn migrate_v2(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v2: failed to create metadata table: {e}"),
        operation: "migrate_v2".to_string(),
    })?;

    // Drop the legacy schema_versions table if it exists.
    conn.execute("DROP TABLE IF EXISTS schema_versions", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v2: failed to drop schema_versions: {e}"),
            operation: "migrate_v2".to_string(),
        })?;

    Ok(())
}
```

### `migrate_v3` — src/db/migrations.rs:713

Source: [src/db/migrations.rs:713](../src/db/migrations.rs#L713).

```rust
async fn migrate_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN branches INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN loops INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN returns INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN max_nesting INTEGER NOT NULL DEFAULT 0;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v3: failed to add complexity columns: {e}"),
        operation: "migrate_v3".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v4` — src/db/migrations.rs:734

Source: [src/db/migrations.rs:734](../src/db/migrations.rs#L734).

```rust
async fn migrate_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN unsafe_blocks INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN unchecked_calls INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN assertions INTEGER NOT NULL DEFAULT 0;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v4: failed to add safety metric columns: {e}"),
        operation: "migrate_v4".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v5` — src/db/migrations.rs:756

Source: [src/db/migrations.rs:756](../src/db/migrations.rs#L756).

```rust
async fn migrate_v5(conn: &Connection) -> Result<()> {
    // Rebuild the edges table keeping only distinct rows. We use a temp
    // table + swap because DELETE with a self-join subquery can be very
    // slow on large tables (the reporter had 13.9 M edges).
    conn.execute_batch(
        "CREATE TABLE edges_dedup (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        INSERT INTO edges_dedup (source, target, kind, line)
        SELECT DISTINCT source, target, kind, line FROM edges;

        DROP TABLE edges;
        ALTER TABLE edges_dedup RENAME TO edges;

        CREATE INDEX idx_edges_source ON edges(source);
        CREATE INDEX idx_edges_target ON edges(target);
        CREATE INDEX idx_edges_kind ON edges(kind);
        CREATE INDEX idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX idx_edges_target_kind ON edges(target, kind);
        CREATE UNIQUE INDEX idx_edges_unique
            ON edges(source, target, kind, COALESCE(line, -1));",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v5: failed to deduplicate edges: {e}"),
        operation: "migrate_v5".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v6` — src/db/migrations.rs:800

Source: [src/db/migrations.rs:800](../src/db/migrations.rs#L800).

```rust
async fn migrate_v6(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name))",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v6: failed to create lower(name) index: {e}"),
        operation: "migrate_v6".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v7` — src/db/migrations.rs:825

Source: [src/db/migrations.rs:825](../src/db/migrations.rs#L825).

```rust
async fn migrate_v7(conn: &Connection) -> Result<()> {
    conn.execute(
        "ALTER TABLE nodes ADD COLUMN attrs_start_line INTEGER NOT NULL DEFAULT 0",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v7: failed to add attrs_start_line column: {e}"),
        operation: "migrate_v7".to_string(),
    })?;

    conn.execute(
        "UPDATE nodes SET attrs_start_line = start_line WHERE attrs_start_line = 0",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v7: failed to backfill attrs_start_line: {e}"),
        operation: "migrate_v7".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v8` — src/db/migrations.rs:858

Source: [src/db/migrations.rs:858](../src/db/migrations.rs#L858).

```rust
async fn migrate_v8(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reason TEXT,
            created_at INTEGER NOT NULL,
            files TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS memory_code_areas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            description TEXT,
            last_touched_at INTEGER NOT NULL,
            touch_count INTEGER NOT NULL DEFAULT 1
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_code_areas_path
            ON memory_code_areas(path);
        CREATE INDEX IF NOT EXISTS idx_memory_decisions_created_at
            ON memory_decisions(created_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_decisions_fts USING fts5(
            text, reason,
            content='memory_decisions', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_insert
            AFTER INSERT ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_delete
            AFTER DELETE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_update
            AFTER UPDATE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v8: failed to create memory tables: {e}"),
        operation: "migrate_v8".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v9` — src/db/migrations.rs:929

Source: [src/db/migrations.rs:929](../src/db/migrations.rs#L929).

```rust
async fn migrate_v9(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS read_cache (
            project_id   TEXT NOT NULL,
            session_id   TEXT NOT NULL,
            file_path    TEXT NOT NULL,
            mtime_ns     INTEGER NOT NULL,
            mode         TEXT NOT NULL,
            args_hash    TEXT NOT NULL,
            digest       TEXT NOT NULL,
            body         BLOB NOT NULL,
            token_count  INTEGER NOT NULL,
            created_at   INTEGER NOT NULL,
            PRIMARY KEY (project_id, session_id, file_path, mode, args_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_read_cache_session
            ON read_cache(session_id, created_at);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v9: failed to create read_cache table: {e}"),
        operation: "migrate_v9".to_string(),
    })?;

    // ALTER TABLE has no IF NOT EXISTS for columns in SQLite. Probe
    // PRAGMA table_info first — fresh installs already include parent_id
    // from create_schema, and the test harness exercises that path by
    // resetting user_version to a pre-v9 value.
    let has_parent_id = {
        let mut rows = conn
            .query("PRAGMA table_info(nodes)", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to probe nodes columns: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
        let mut found = false;
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("v9: failed to read table_info row: {e}"),
            operation: "migrate_v9".to_string(),
        })? {
            if let Ok(name) = row.get::<String>(1) {
                if name == "parent_id" {
                    found = true;
                    break;
                }
            }
        }
        found
    };

    if !has_parent_id {
        conn.execute("ALTER TABLE nodes ADD COLUMN parent_id TEXT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to add parent_id column: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
    }

    // Backfill parent_id from existing Contains edges, then drop those
    // rows. Gate on the edges table actually existing — tests seed
    // partial schemas and a real install always has it (migrate_v1).
    let has_edges_table = {
        let mut rows = conn
            .query(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='edges'",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to probe sqlite_master: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
        rows.next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to read sqlite_master row: {e}"),
                operation: "migrate_v9".to_string(),
            })?
            .is_some()
    };

    if has_edges_table {
        // When a node has multiple incoming Contains rows (legacy data
        // anomaly), the first matching row wins — subsequent rows are
        // noise the new schema does not preserve.
        conn.execute(
            "UPDATE nodes SET parent_id = (
                SELECT source FROM edges
                WHERE edges.target = nodes.id AND edges.kind = 'contains'
                LIMIT 1
            )",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v9: failed to backfill parent_id from contains edges: {e}"),
            operation: "migrate_v9".to_string(),
        })?;

        conn.execute("DELETE FROM edges WHERE kind = 'contains'", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to drop contains edges: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_parent_id ON nodes(parent_id)",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v9: failed to create idx_nodes_parent_id: {e}"),
        operation: "migrate_v9".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v10` — src/db/migrations.rs:1060

Source: [src/db/migrations.rs:1060](../src/db/migrations.rs#L1060).

```rust
async fn migrate_v10(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS node_fingerprints (
            node_id TEXT PRIMARY KEY,
            ast_hash TEXT NOT NULL,
            cfg_hash TEXT NOT NULL,
            call_seq_hash TEXT NOT NULL,
            shingles TEXT NOT NULL,
            body_tokens INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_ast ON node_fingerprints(ast_hash);
        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_size ON node_fingerprints(body_tokens);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v10: failed to create node_fingerprints table: {e}"),
        operation: "migrate_v10".to_string(),
    })?;

    Ok(())
}
```

### `migrate_v11` — src/db/migrations.rs:1102

Source: [src/db/migrations.rs:1102](../src/db/migrations.rs#L1102).

```rust
async fn migrate_v11(conn: &Connection) -> Result<()> {
    let existing = node_columns(conn).await?;
    for col in [
        "cognitive_complexity",
        "distinct_operators",
        "distinct_operands",
        "total_operators",
        "total_operands",
    ] {
        if existing.iter().any(|c| c == col) {
            continue;
        }
        conn.execute(
            &format!("ALTER TABLE nodes ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0"),
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v11: failed to add column {col}: {e}"),
            operation: "migrate_v11".to_string(),
        })?;
    }

    Ok(())
}
```

### `migrate_v12` — src/db/migrations.rs:1136

Source: [src/db/migrations.rs:1136](../src/db/migrations.rs#L1136).

```rust
async fn migrate_v12(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS executable_body_fts USING fts5(
            node_id UNINDEXED,
            file_path UNINDEXED,
            body,
            tokenize='unicode61'
        );
        CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );
        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v12: failed to create executable body FTS table: {e}"),
        operation: "migrate_v12".to_string(),
    })?;
    let mut edge_table = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v12: failed to inspect edges table: {e}"),
            operation: "migrate_v12".to_string(),
        })?;
    let has_edges = edge_table
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v12: failed to read edges table status: {e}"),
            operation: "migrate_v12".to_string(),
        })?
        .is_some();
    drop(edge_table);
    if has_edges {
        conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v12: failed to create trait dispatch triggers: {e}"),
                operation: "migrate_v12".to_string(),
            })?;
    }
    Ok(())
}
```

### `migrate_v13` — src/db/migrations.rs:1196

Source: [src/db/migrations.rs:1196](../src/db/migrations.rs#L1196).

```rust
async fn migrate_v13(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );
        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v13: failed to recreate trait dispatch cache table: {e}"),
        operation: "migrate_v13".to_string(),
    })?;

    let mut edge_table = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v13: failed to inspect edges table: {e}"),
            operation: "migrate_v13".to_string(),
        })?;
    let has_edges = edge_table
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v13: failed to read edges table status: {e}"),
            operation: "migrate_v13".to_string(),
        })?
        .is_some();
    drop(edge_table);
    if has_edges {
        conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v13: failed to recreate trait dispatch triggers: {e}"),
                operation: "migrate_v13".to_string(),
            })?;
    }
    Ok(())
}
```

### `migrate_v14` — src/db/migrations.rs:1288

Source: [src/db/migrations.rs:1288](../src/db/migrations.rs#L1288).

```rust
async fn migrate_v14(conn: &Connection) -> Result<()> {
    // --- Part 1: remove phantom `annotates` edges (see #326). -------------
    let mut edge_table = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to inspect edges table: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    let has_edges = edge_table
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read edges table status: {e}"),
            operation: "migrate_v14".to_string(),
        })?
        .is_some();
    drop(edge_table);

    if has_edges {
        conn.execute(
            "DELETE FROM edges
             WHERE kind = 'annotates'
               AND target IN (
                   SELECT id FROM nodes WHERE kind IN ('annotation_usage', 'decorator')
               )",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to delete phantom annotates edges: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    }

    // --- Part 2: search_terms column + porter FTS rebuild (see #316). -----
    let cols = node_columns(conn).await?;
    if !cols.iter().any(|c| c == "search_terms") {
        conn.execute_batch("ALTER TABLE nodes ADD COLUMN search_terms TEXT NOT NULL DEFAULT ''")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v14: failed to add search_terms column: {e}"),
                operation: "migrate_v14".to_string(),
            })?;
    }

    // Drop the old FTS table and its triggers before the backfill so the
    // UPDATEs below don't fire FTS sync triggers against a table that is
    // about to be rebuilt anyway.
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS nodes_fts_insert;
         DROP TRIGGER IF EXISTS nodes_fts_delete;
         DROP TRIGGER IF EXISTS nodes_fts_update;
         DROP TABLE IF EXISTS nodes_fts;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v14: failed to drop old FTS table: {e}"),
        operation: "migrate_v14".to_string(),
    })?;

    // Backfill search_terms for existing nodes. Only rows that actually have
    // camelCase-derived segments need a write; snake_case names produce an
    // empty string, which the column already defaults to.
    let mut rows = conn
        .query("SELECT rowid, name, qualified_name FROM nodes", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read nodes for backfill: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    let mut backfill: Vec<(i64, String)> = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("v14: failed to read backfill row: {e}"),
        operation: "migrate_v14".to_string(),
    })? {
        let rowid: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read rowid: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
        let name: String = row.get(1).map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read name: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
        let qualified_name: String = row.get(2).map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read qualified_name: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
        let terms = crate::text::search_terms(&name, &qualified_name);
        if !terms.is_empty() {
            backfill.push((rowid, terms));
        }
    }
    drop(rows);
    for (rowid, terms) in backfill {
        conn.execute(
            "UPDATE nodes SET search_terms = ?1 WHERE rowid = ?2",
            params![terms, rowid],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to backfill search_terms: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    }

    // Recreate the FTS table with the new column + porter stemming, its sync
    // triggers, and rebuild the index from the content table.
    conn.execute_batch(
        "CREATE VIRTUAL TABLE nodes_fts USING fts5(
            name, qualified_name, docstring, signature, search_terms,
            content='nodes', content_rowid='rowid',
            tokenize='porter unicode61'
        );

        CREATE TRIGGER nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v14: failed to recreate FTS table: {e}"),
        operation: "migrate_v14".to_string(),
    })?;

    Ok(())
}
```

### `table_exists` — src/db/migrations.rs:1456

Source: [src/db/migrations.rs:1456](../src/db/migrations.rs#L1456).

```rust
async fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to probe sqlite_master for table '{name}': {e}"),
            operation: "table_exists".to_string(),
        })?;
    let exists = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read sqlite_master row for table '{name}': {e}"),
            operation: "table_exists".to_string(),
        })?
        .is_some();
    Ok(exists)
}
```

### `migrate_v15` — src/db/migrations.rs:1488

Source: [src/db/migrations.rs:1488](../src/db/migrations.rs#L1488).

```rust
async fn migrate_v15(conn: &Connection) -> Result<()> {
    // --- Node indexes --------------------------------------------------
    // Exactly the set `begin_bulk_load` drops and `end_bulk_load` restores —
    // NOT the full `create_schema` set. `idx_nodes_lower_name` and
    // `idx_nodes_parent_id` are deliberately omitted: bulk load never drops
    // them, so they are never lost, and `idx_nodes_parent_id` would reference a
    // `parent_id` column that only exists from v10 onward (#359).
    if table_exists(conn, "nodes").await? {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
            CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
            CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
            CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
            CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);",
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate node indexes: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- Edge indexes --------------------------------------------------
    if table_exists(conn, "edges").await? {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
            CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
            CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique
                ON edges(source, target, kind, COALESCE(line, -1));",
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate edge indexes: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- Unresolved-ref indexes ----------------------------------------
    if table_exists(conn, "unresolved_refs").await? {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
            CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
            CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);",
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate unresolved_refs indexes: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- FTS sync triggers (v14-era bodies with `search_terms`) ---------
    // `end_bulk_load` may have recreated the pre-v14 triggers that omit
    // `search_terms`.  Drop and recreate to ensure the current bodies. The
    // triggers fire on `nodes` and write to `nodes_fts`, so both must exist;
    // by this point `migrate_v14` has already given both the `search_terms`
    // column.
    if table_exists(conn, "nodes").await? && table_exists(conn, "nodes_fts").await? {
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS nodes_fts_insert;
        DROP TRIGGER IF EXISTS nodes_fts_delete;
        DROP TRIGGER IF EXISTS nodes_fts_update;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;",
    )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate FTS triggers: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- Trait-dispatch triggers ---------------------------------------
    // Fire on `edges` and write to `trait_dispatch_callers`; both must exist.
    if table_exists(conn, "edges").await? && table_exists(conn, "trait_dispatch_callers").await? {
        conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v15: failed to recreate trait dispatch triggers: {e}"),
                operation: "migrate_v15".to_string(),
            })?;
    }

    Ok(())
}
```

### `node_columns` — src/db/migrations.rs:1591

Source: [src/db/migrations.rs:1591](../src/db/migrations.rs#L1591).

```rust
async fn node_columns(conn: &Connection) -> Result<Vec<String>> {
    let mut rows = conn
        .query("PRAGMA table_info(nodes)", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read nodes table_info: {e}"),
            operation: "node_columns".to_string(),
        })?;
    let mut cols = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read table_info row: {e}"),
        operation: "node_columns".to_string(),
    })? {
        // PRAGMA table_info columns: cid(0), name(1), type(2), ...
        let name: String = row.get(1).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read column name: {e}"),
            operation: "node_columns".to_string(),
        })?;
        cols.push(name);
    }
    Ok(cols)
}
```

### `migrate_v16` — src/db/migrations.rs:1628

Source: [src/db/migrations.rs:1628](../src/db/migrations.rs#L1628).

```rust
async fn migrate_v16(conn: &Connection) -> Result<()> {
    if !table_exists(conn, "files").await? {
        return Ok(());
    }
    if table_columns(conn, "files")
        .await?
        .iter()
        .any(|c| c == "kind")
    {
        return Ok(());
    }

    conn.execute_batch("ALTER TABLE files ADD COLUMN kind TEXT NOT NULL DEFAULT 'code';")
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v16: failed to add files.kind: {e}"),
            operation: "migrate_v16".to_string(),
        })?;
    Ok(())
}
```

### `indexes_naming` — src/db/migrations.rs:1673

Source: [src/db/migrations.rs:1673](../src/db/migrations.rs#L1673).

```rust
async fn indexes_naming(conn: &Connection, table: &str, column: &str) -> Result<Vec<String>> {
    let mut rows = conn
        .query(
            // `instr`, not `LIKE`: `_` in a column name is a LIKE wildcard.
            "SELECT name FROM sqlite_master
             WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL AND instr(sql, ?2) > 0",
            params![table, column],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to list indexes on {table}: {e}"),
            operation: "indexes_naming".to_string(),
        })?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read index row: {e}"),
        operation: "indexes_naming".to_string(),
    })? {
        out.push(row.get::<String>(0).unwrap_or_default());
    }
    Ok(out)
}
```

### `table_columns` — src/db/migrations.rs:1697

Source: [src/db/migrations.rs:1697](../src/db/migrations.rs#L1697).

```rust
async fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    // PRAGMA does not accept a bound parameter for the table name, and this is
    // only ever called with literals from this module.
    let mut rows = conn
        .query(&format!("PRAGMA table_info({table})"), ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read {table} table_info: {e}"),
            operation: "table_columns".to_string(),
        })?;
    let mut cols = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read table_info row: {e}"),
        operation: "table_columns".to_string(),
    })? {
        // PRAGMA table_info columns: cid(0), name(1), type(2), ...
        let name: String = row.get(1).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read column name: {e}"),
            operation: "table_columns".to_string(),
        })?;
        cols.push(name);
    }
    Ok(cols)
}
```

### `migrate_v18` — src/db/migrations.rs:1744

Source: [src/db/migrations.rs:1744](../src/db/migrations.rs#L1744).

```rust
async fn migrate_v18(conn: &Connection) -> Result<()> {
    if !table_exists(conn, "edges").await? {
        return Ok(());
    }
    let mut legacy = false;
    let mut current = false;
    let mut rows = conn
        .query("PRAGMA table_info(edges)", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v18: failed to read edges table_info: {e}"),
            operation: "migrate_v18".to_string(),
        })?;
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("v18: failed to read table_info row: {e}"),
        operation: "migrate_v18".to_string(),
    })? {
        // PRAGMA table_info columns: cid(0), name(1), type(2), notnull(3), ...
        let name: String = row.get(1).unwrap_or_default();
        if name != "resolved_by" {
            continue;
        }
        let ty: String = row.get(2).unwrap_or_default();
        let not_null: i64 = row.get(3).unwrap_or(0);
        if ty.eq_ignore_ascii_case("INTEGER") && not_null == 0 {
            current = true;
        } else {
            legacy = true;
        }
    }
    drop(rows);
    if current {
        return Ok(());
    }
    if legacy {
        for index in indexes_naming(conn, "edges", "resolved_by").await? {
            conn.execute_batch(&format!("DROP INDEX IF EXISTS \"{index}\";"))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("v18: failed to drop index {index}: {e}"),
                    operation: "migrate_v18".to_string(),
                })?;
        }
        conn.execute_batch("ALTER TABLE edges DROP COLUMN resolved_by;")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v18: failed to drop legacy edges.resolved_by: {e}"),
                operation: "migrate_v18".to_string(),
            })?;
    }
    conn.execute_batch("ALTER TABLE edges ADD COLUMN resolved_by INTEGER;")
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v18: failed to add edges.resolved_by: {e}"),
            operation: "migrate_v18".to_string(),
        })?;
    Ok(())
}
```

### `clear` — src/db/queries/clear.rs:10

Source: [src/db/queries/clear.rs:10](../src/db/queries/clear.rs#L10).

```rust
    pub async fn clear(&self) -> Result<()> {
        self.conn()
            .execute_batch(
                "DELETE FROM vectors;
                 DELETE FROM executable_body_fts;
                 DELETE FROM trait_dispatch_callers;
                 DELETE FROM unresolved_refs;
                 DELETE FROM edges;
                 DELETE FROM nodes;
                 DELETE FROM files;",
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to clear database: {e}"),
                operation: "clear".to_string(),
            })?;
        self.trait_dispatch_callers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }
```

### `rebuild_ruby_reopenings` — src/db/queries/edges.rs:12

Source: [src/db/queries/edges.rs:12](../src/db/queries/edges.rs#L12).

```rust
    pub async fn rebuild_ruby_reopenings(&self) -> Result<()> {
        let _write_guard = self.write_lock.lock().await;
        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin Ruby reopening rebuild: {e}"),
                operation: "rebuild_ruby_reopenings".to_string(),
            })?;

        // Ruby extraction currently writes file::file::name; the single-prefix branch accepts older or alternate rows, and ltrim removes the absolute :: marker from names such as class ::Entry.
        let result = async {
            self.conn()
                .execute("DELETE FROM edges WHERE kind = 'reopens'", ())
                .await?;
            self.conn()
                .execute(
                    "INSERT INTO edges (source, target, kind, line)
                     SELECT id, canonical_id, 'reopens', start_line
                     FROM (
                         SELECT id, start_line,
                                FIRST_VALUE(id) OVER declaration AS canonical_id,
                                ROW_NUMBER() OVER declaration AS declaration_number
                         FROM (
                             SELECT id, kind, file_path, start_line, start_column,
                                    ltrim(CASE
                                        WHEN substr(qualified_name, 1, length(file_path) * 2 + 4) = file_path || '::' || file_path || '::'
                                        THEN substr(qualified_name, length(file_path) * 2 + 5)
                                        ELSE substr(qualified_name, length(file_path) + 3)
                                    END, ':') AS constant_name
                             FROM nodes
                             WHERE kind IN ('class', 'module')
                               AND (file_path LIKE '%.rb' OR file_path LIKE '%.rake' OR file_path LIKE '%.erb' OR file_path LIKE '%.slim')
                               AND qualified_name NOT LIKE '%<anonymous>%'
                               AND substr(qualified_name, 1, length(file_path) + 2) = file_path || '::'
                         )
                         WINDOW declaration AS (
                             PARTITION BY kind, constant_name
                             ORDER BY file_path, start_line, start_column, id
                         )
                     )
                     WHERE declaration_number > 1",
                    (),
                )
                .await?;
            Ok::<(), libsql::Error>(())
        }
        .await;

        match result {
            Ok(()) => self
                .conn()
                .execute("COMMIT", ())
                .await
                .map(|_| ())
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to commit Ruby reopening rebuild: {e}"),
                    operation: "rebuild_ruby_reopenings".to_string(),
                }),
            Err(e) => {
                let _ = self.conn().execute("ROLLBACK", ()).await;
                Err(TokenSaveError::Database {
                    message: format!("failed to rebuild Ruby reopenings: {e}"),
                    operation: "rebuild_ruby_reopenings".to_string(),
                })
            }
        }
    }
```

### `insert_edge` — src/db/queries/edges.rs:82

Source: [src/db/queries/edges.rs:82](../src/db/queries/edges.rs#L82).

```rust
    pub async fn insert_edge(&self, edge: &Edge) -> Result<()> {
        // Contains is denormalized to nodes.parent_id since v9. Fold the
        // edge into an UPDATE rather than writing a row to the edges table.
        if edge.kind == EdgeKind::Contains {
            self.conn()
                .execute(
                    "UPDATE nodes SET parent_id = ?1 WHERE id = ?2",
                    params![edge.source.as_str(), edge.target.as_str()],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to set parent_id: {e}"),
                    operation: "insert_edge".to_string(),
                })?;
            return Ok(());
        }
        self.conn()
            .execute(
                &format!(
                    "INSERT INTO edges (source, target, kind, line, resolved_by) \
                     SELECT ?1, ?2, ?3, ?4, ?5 \
                     WHERE EXISTS (SELECT 1 FROM nodes WHERE id = ?1) \
                       AND EXISTS (SELECT 1 FROM nodes WHERE id = ?2){EDGE_UPSERT_CLAUSE}"
                ),
                params![
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.kind.as_str(),
                    edge.line.map(i64::from),
                    edge.resolved_by.map(ResolvedBy::code)
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert edge: {e}"),
                operation: "insert_edge".to_string(),
            })?;
        self.refresh_trait_dispatch_callers().await?;
        Ok(())
    }
```

### `insert_edges` — src/db/queries/edges.rs:129

Source: [src/db/queries/edges.rs:129](../src/db/queries/edges.rs#L129).

```rust
    pub async fn insert_edges(&self, edges: &[Edge]) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }

        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin: {e}"),
                operation: "insert_edges".to_string(),
            })?;

        // Conditional INSERT: only insert when both endpoints exist in
        // `nodes`. This avoids FK violations during incremental sync
        // when an edge references a node from a not-yet-indexed file.
        let stmt = self
            .conn()
            .prepare(&format!(
                "INSERT INTO edges (source, target, kind, line, resolved_by) \
                     SELECT ?1, ?2, ?3, ?4, ?5 \
                     WHERE EXISTS (SELECT 1 FROM nodes WHERE id = ?1) \
                       AND EXISTS (SELECT 1 FROM nodes WHERE id = ?2){EDGE_UPSERT_CLAUSE}"
            ))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "insert_edges".to_string(),
            })?;

        let parent_stmt = self
            .conn()
            .prepare("UPDATE nodes SET parent_id = ?1 WHERE id = ?2")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare parent update: {e}"),
                operation: "insert_edges".to_string(),
            })?;

        for edge in edges {
            if edge.kind == EdgeKind::Contains {
                parent_stmt
                    .execute(params![edge.source.as_str(), edge.target.as_str()])
                    .await
                    .map_err(|e| TokenSaveError::Database {
                        message: format!("failed to set parent_id: {e}"),
                        operation: "insert_edges".to_string(),
                    })?;
                parent_stmt.reset();
                continue;
            }
            stmt.execute(params![
                edge.source.as_str(),
                edge.target.as_str(),
                edge.kind.as_str(),
                edge.line.map(i64::from),
                edge.resolved_by.map(ResolvedBy::code),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert edge: {e}"),
                operation: "insert_edges".to_string(),
            })?;
            stmt.reset();
        }

        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit: {e}"),
                operation: "insert_edges".to_string(),
            })?;
        self.refresh_trait_dispatch_callers().await?;
        Ok(())
    }
```

### `get_outgoing_edges` — src/db/queries/edges.rs:209

Source: [src/db/queries/edges.rs:209](../src/db/queries/edges.rs#L209).

```rust
    pub async fn get_outgoing_edges(
        &self,
        source_id: &str,
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut rows = self
                .conn()
                .query(
                    "SELECT source, target, kind, line, resolved_by FROM edges WHERE source = ?1",
                    params![source_id],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query outgoing edges: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_outgoing_edges").await
        } else {
            let placeholders: Vec<String> = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 2))
                .collect();
            let sql = format!(
                "SELECT source, target, kind, line, resolved_by FROM edges WHERE source = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );

            let mut param_values: Vec<libsql::Value> = Vec::new();
            param_values.push(libsql::Value::Text(source_id.to_string()));
            for k in kinds {
                param_values.push(libsql::Value::Text(k.as_str().to_string()));
            }

            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(param_values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query outgoing edges: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_outgoing_edges").await
        }
    }
```

### `get_incoming_edges` — src/db/queries/edges.rs:261

Source: [src/db/queries/edges.rs:261](../src/db/queries/edges.rs#L261).

```rust
    pub async fn get_incoming_edges(
        &self,
        target_id: &str,
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut rows = self
                .conn()
                .query(
                    "SELECT source, target, kind, line, resolved_by FROM edges WHERE target = ?1",
                    params![target_id],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query incoming edges: {e}"),
                    operation: "get_incoming_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_incoming_edges").await
        } else {
            let placeholders: Vec<String> = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 2))
                .collect();
            let sql = format!(
                "SELECT source, target, kind, line, resolved_by FROM edges WHERE target = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );

            let mut param_values: Vec<libsql::Value> = Vec::new();
            param_values.push(libsql::Value::Text(target_id.to_string()));
            for k in kinds {
                param_values.push(libsql::Value::Text(k.as_str().to_string()));
            }

            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(param_values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query incoming edges: {e}"),
                    operation: "get_incoming_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_incoming_edges").await
        }
    }
```

### `rebuild_trait_dispatch_callers` — src/db/queries/edges.rs:313

Source: [src/db/queries/edges.rs:313](../src/db/queries/edges.rs#L313).

```rust
    pub async fn rebuild_trait_dispatch_callers(&self) -> Result<()> {
        self.conn()
            .execute_batch(
                "DELETE FROM trait_dispatch_callers;
                 INSERT OR IGNORE INTO trait_dispatch_callers
                     (concrete_method_id, trait_method_id, caller_id, line)
                 SELECT concrete.id, trait_method.id, call.source, COALESCE(call.line, -1)
                   FROM edges dispatch
                   JOIN nodes trait_method
                     ON trait_method.parent_id = dispatch.target
                    AND trait_method.kind IN ('method', 'function')
                   JOIN nodes concrete
                     ON concrete.parent_id = dispatch.source
                    AND concrete.name = trait_method.name
                    AND concrete.kind IN ('method', 'function')
                   JOIN edges call
                     ON call.target = trait_method.id
                    AND call.kind = 'calls'
                  WHERE dispatch.kind = 'implements';",
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to rebuild reverse trait dispatch callers: {e}"),
                operation: "rebuild_trait_dispatch_callers".to_string(),
            })?;
        self.refresh_trait_dispatch_callers().await?;
        Ok(())
    }
```

### `refresh_trait_dispatch_callers` — src/db/queries/edges.rs:342

Source: [src/db/queries/edges.rs:342](../src/db/queries/edges.rs#L342).

```rust
    pub async fn refresh_trait_dispatch_callers(&self) -> Result<()> {
        let mut rows = self
            .conn()
            .query(
                "SELECT caller.id, caller.kind, caller.name, caller.qualified_name, caller.file_path,
                        caller.start_line, caller.end_line, caller.start_column, caller.end_column,
                        caller.docstring, caller.signature, caller.visibility, caller.is_async,
                        caller.branches, caller.loops, caller.returns, caller.max_nesting,
                        caller.unsafe_blocks, caller.unchecked_calls, caller.assertions,
                        caller.updated_at, caller.attrs_start_line, caller.parent_id,
                        caller.cognitive_complexity, caller.distinct_operators,
                        caller.distinct_operands, caller.total_operators, caller.total_operands,
                        dispatch.concrete_method_id, dispatch.trait_method_id, dispatch.line,
                        EXISTS(
                            SELECT 1 FROM edges upstream
                             WHERE upstream.target = caller.id
                               AND upstream.kind = 'calls'
                        )
                   FROM trait_dispatch_callers dispatch
                   JOIN nodes caller ON caller.id = dispatch.caller_id
                  ORDER BY dispatch.concrete_method_id, caller.file_path, caller.start_line",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to refresh trait dispatch method cache: {e}"),
                operation: "refresh_trait_dispatch_callers".to_string(),
            })?;
        let mut callers: std::collections::HashMap<String, Vec<CachedTraitDispatchCaller>> =
            std::collections::HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read trait dispatch method cache: {e}"),
            operation: "refresh_trait_dispatch_callers".to_string(),
        })? {
            let caller = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map cached trait dispatch caller: {e}"),
                operation: "refresh_trait_dispatch_callers".to_string(),
            })?;
            let concrete_method_id: String = row.get(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map cached concrete method: {e}"),
                operation: "refresh_trait_dispatch_callers".to_string(),
            })?;
            let trait_method_id: String = row.get(29).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map cached trait method: {e}"),
                operation: "refresh_trait_dispatch_callers".to_string(),
            })?;
            let stored_line: i64 = row.get(30).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map cached trait dispatch line: {e}"),
                operation: "refresh_trait_dispatch_callers".to_string(),
            })?;
            let has_upstream: i64 = row.get(31).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map cached caller fan-in: {e}"),
                operation: "refresh_trait_dispatch_callers".to_string(),
            })?;
            let edge = Edge {
                source: caller.id.clone(),
                target: trait_method_id.clone(),
                kind: EdgeKind::Calls,
                line: (stored_line >= 0).then_some(stored_line as u32),
                resolved_by: None,
            };
            callers.entry(concrete_method_id).or_default().push((
                caller,
                edge,
                trait_method_id,
                has_upstream != 0,
            ));
        }
        *self
            .trait_dispatch_callers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = callers;
        Ok(())
    }
```

### `get_incoming_edges_bulk` — src/db/queries/edges.rs:443

Source: [src/db/queries/edges.rs:443](../src/db/queries/edges.rs#L443).

```rust
    pub async fn get_incoming_edges_bulk(
        &self,
        target_ids: &[String],
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        if target_ids.is_empty() {
            return Ok(Vec::new());
        }

        let target_placeholders: Vec<String> =
            (1..=target_ids.len()).map(|i| format!("?{i}")).collect();
        let mut param_values: Vec<libsql::Value> = target_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();

        let sql = if kinds.is_empty() {
            format!(
                "SELECT source, target, kind, line, resolved_by FROM edges WHERE target IN ({})",
                target_placeholders.join(", ")
            )
        } else {
            let kind_placeholders: Vec<String> = (1..=kinds.len())
                .map(|i| format!("?{}", target_ids.len() + i))
                .collect();
            for k in kinds {
                param_values.push(libsql::Value::Text(k.as_str().to_string()));
            }
            format!(
                "SELECT source, target, kind, line, resolved_by FROM edges \
                 WHERE target IN ({}) AND kind IN ({})",
                target_placeholders.join(", "),
                kind_placeholders.join(", ")
            )
        };

        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query bulk incoming edges: {e}"),
                operation: "get_incoming_edges_bulk".to_string(),
            })?;

        collect_rows(&mut rows, row_to_edge, "get_incoming_edges_bulk").await
    }
```

### `get_test_annotated_node_ids` — src/db/queries/edges.rs:494

Source: [src/db/queries/edges.rs:494](../src/db/queries/edges.rs#L494).

```rust
    pub async fn get_test_annotated_node_ids(
        &self,
        candidate_ids: &[String],
    ) -> Result<HashSet<String>> {
        if candidate_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let placeholders: Vec<String> =
            (1..=candidate_ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT DISTINCT e.target \
             FROM edges e \
             JOIN nodes n ON e.source = n.id \
             WHERE n.kind = 'annotation_usage' \
               AND n.name = 'test' \
               AND e.kind = 'annotates' \
               AND e.target IN ({})",
            placeholders.join(", ")
        );
        let param_values: Vec<libsql::Value> = candidate_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query test-annotated nodes: {e}"),
                operation: "get_test_annotated_node_ids".to_string(),
            })?;
        let mut result = HashSet::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read test-annotated row: {e}"),
            operation: "get_test_annotated_node_ids".to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                result.insert(id);
            }
        }
        Ok(result)
    }
```

### `get_annotation_histogram` — src/db/queries/edges.rs:546

Source: [src/db/queries/edges.rs:546](../src/db/queries/edges.rs#L546).

```rust
    pub async fn get_annotation_histogram(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, u64)>> {
        let (sql, args) = if let Some(prefix) = path_prefix {
            let mut sql = String::from(
                "SELECT name, COUNT(*) AS n \
                 FROM nodes \
                 WHERE kind = 'annotation_usage' AND ",
            );
            push_path_prefix_filter(&mut sql, "", prefix);
            sql.push_str(" GROUP BY name ORDER BY n DESC, name ASC");
            (sql, libsql::params_from_iter(Vec::<libsql::Value>::new()))
        } else {
            (
                "SELECT name, COUNT(*) AS n \
                 FROM nodes \
                 WHERE kind = 'annotation_usage' \
                 GROUP BY name ORDER BY n DESC, name ASC"
                    .to_string(),
                libsql::params_from_iter(Vec::<libsql::Value>::new()),
            )
        };
        let mut rows =
            self.conn()
                .query(&sql, args)
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query annotation histogram: {e}"),
                    operation: "get_annotation_histogram".to_string(),
                })?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read annotation row: {e}"),
            operation: "get_annotation_histogram".to_string(),
        })? {
            let name: String = row.get(0).unwrap_or_default();
            let count: i64 = row.get(1).unwrap_or(0);
            if !name.is_empty() {
                out.push((name, count.max(0) as u64));
            }
        }
        Ok(out)
    }
```

### `get_annotation_sites` — src/db/queries/edges.rs:603

Source: [src/db/queries/edges.rs:603](../src/db/queries/edges.rs#L603).

```rust
    pub async fn get_annotation_sites(
        &self,
        name: Option<&str>,
        path_prefix: Option<&str>,
        target_kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        use std::fmt::Write;
        let mut sql = String::from(
            "SELECT a.name AS annotation, a.file_path AS a_file, a.start_line AS a_line, \
                    t.id AS target_id, t.name AS target_name, t.kind AS target_kind, \
                    t.file_path AS target_file, t.start_line AS target_line, t.qualified_name AS target_qname \
             FROM edges e \
             JOIN nodes a ON e.source = a.id \
             JOIN nodes t ON e.target = t.id \
             WHERE a.kind = 'annotation_usage' AND e.kind = 'annotates'",
        );
        let mut params: Vec<libsql::Value> = Vec::new();
        if let Some(n) = name {
            params.push(libsql::Value::Text(n.to_string()));
            let _ = write!(sql, " AND a.name = ?{}", params.len());
        }
        if let Some(prefix) = path_prefix {
            sql.push_str(" AND ");
            push_path_prefix_filter(&mut sql, "t.", prefix);
        }
        if let Some(k) = target_kind {
            params.push(libsql::Value::Text(k.to_string()));
            let _ = write!(sql, " AND t.kind = ?{}", params.len());
        }
        sql.push_str(" ORDER BY a.name ASC, t.file_path ASC, t.start_line ASC");
        params.push(libsql::Value::Integer(limit as i64));
        let _ = write!(sql, " LIMIT ?{}", params.len());

        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(params))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query annotation sites: {e}"),
                operation: "get_annotation_sites".to_string(),
            })?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read annotation site row: {e}"),
            operation: "get_annotation_sites".to_string(),
        })? {
            let annotation: String = row.get(0).unwrap_or_default();
            let a_file: String = row.get(1).unwrap_or_default();
            let a_line: i64 = row.get(2).unwrap_or(0);
            let target_id: String = row.get(3).unwrap_or_default();
            let target_name: String = row.get(4).unwrap_or_default();
            let target_kind: String = row.get(5).unwrap_or_default();
            let target_file: String = row.get(6).unwrap_or_default();
            let target_line: i64 = row.get(7).unwrap_or(0);
            let target_qname: String = row.get(8).unwrap_or_default();
            out.push(serde_json::json!({
                "annotation": annotation,
                "annotation_file": a_file,
                "annotation_line": a_line,
                "target": {
                    "id": target_id,
                    "name": target_name,
                    "kind": target_kind,
                    "file": target_file,
                    "line": target_line,
                    "qualified_name": target_qname,
                },
            }));
        }
        Ok(out)
    }
```

### `get_files_with_test_annotations` — src/db/queries/edges.rs:678

Source: [src/db/queries/edges.rs:678](../src/db/queries/edges.rs#L678).

```rust
    pub async fn get_files_with_test_annotations(&self) -> Result<HashSet<String>> {
        let sql = "SELECT DISTINCT t.file_path \
                   FROM edges e \
                   JOIN nodes n ON e.source = n.id \
                   JOIN nodes t ON e.target = t.id \
                   WHERE n.kind = 'annotation_usage' \
                     AND n.name = 'test' \
                     AND e.kind = 'annotates' \
                     AND t.kind IN ('function', 'method', 'singleton_method')";
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query test-annotation files: {e}"),
                operation: "get_files_with_test_annotations".to_string(),
            })?;
        let mut result = HashSet::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read test-annotation file row: {e}"),
            operation: "get_files_with_test_annotations".to_string(),
        })? {
            if let Ok(path) = row.get::<String>(0) {
                result.insert(path);
            }
        }
        Ok(result)
    }
```

### `get_skip_test_coverage_node_ids` — src/db/queries/edges.rs:708

Source: [src/db/queries/edges.rs:708](../src/db/queries/edges.rs#L708).

```rust
    pub async fn get_skip_test_coverage_node_ids(&self) -> Result<HashSet<String>> {
        let sql = "SELECT id FROM nodes WHERE docstring LIKE '%skip-test-coverage%'";
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query skip-test-coverage nodes: {e}"),
                operation: "get_skip_test_coverage_node_ids".to_string(),
            })?;
        let mut result = HashSet::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read skip-test-coverage row: {e}"),
            operation: "get_skip_test_coverage_node_ids".to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                result.insert(id);
            }
        }
        Ok(result)
    }
```

### `get_nodes_by_name` — src/db/queries/edges.rs:737

Source: [src/db/queries/edges.rs:737](../src/db/queries/edges.rs#L737).

```rust
    pub async fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>> {
        let sql = "SELECT id, kind, name, qualified_name, file_path,
                          start_line, end_line, start_column, end_column,
                          docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                   FROM nodes
                   WHERE name = ?1
                   LIMIT 200";
        let mut rows =
            self.conn()
                .query(sql, params![name])
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query by name: {e}"),
                    operation: "get_nodes_by_name".to_string(),
                })?;
        collect_rows(&mut rows, row_to_node, "get_nodes_by_name").await
    }
```

### `get_nodes_by_qualified_name` — src/db/queries/edges.rs:761

Source: [src/db/queries/edges.rs:761](../src/db/queries/edges.rs#L761).

```rust
    pub async fn get_nodes_by_qualified_name(&self, qname: &str) -> Result<Vec<Node>> {
        // Exact match first — preserves the precise-lookup contract.
        let exact_sql = "SELECT id, kind, name, qualified_name, file_path,
                          start_line, end_line, start_column, end_column,
                          docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                   FROM nodes
                   WHERE qualified_name = ?1";
        let mut rows = self
            .conn()
            .query(exact_sql, params![qname])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query by qualified_name: {e}"),
                operation: "get_nodes_by_qualified_name".to_string(),
            })?;

        let exact: Vec<Node> =
            collect_rows(&mut rows, row_to_node, "get_nodes_by_qualified_name").await?;
        if !exact.is_empty() {
            return Ok(exact);
        }

        // Fallback strategy depends on whether the user passed a qualified
        // form or just a bare identifier:
        //
        // - `Type::method` (contains `::`) → suffix match. Recovers from
        //   extractor quirks (duplicated path segments, file-path prefixes
        //   the caller doesn't know about) and lets callers pass partial
        //   module paths. The leading `%` defeats `idx_nodes_qualified_name`,
        //   so this is a full table scan bounded by `LIMIT 50` — cheap at
        //   typical graph sizes.
        //
        // - `foo` (no `::`) → exact `name = ?` match. Uses `idx_nodes_name`,
        //   so it stays fast. Multiple nodes may share a name (overloads,
        //   `new()` constructors), `LIMIT 50` is a safety net.
        let (sql, pattern) = if qname.contains("::") {
            (
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
                 WHERE qualified_name LIKE ?1 ESCAPE '\\'
                 LIMIT 50",
                format!("%::{}", escape_like(qname)),
            )
        } else {
            (
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
                 WHERE name = ?1
                 LIMIT 50",
                qname.to_string(),
            )
        };
        let mut fallback_rows = self
            .conn()
            .query(sql, params![pattern.as_str()])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query by qualified_name fallback: {e}"),
                operation: "get_nodes_by_qualified_name".to_string(),
            })?;
        collect_rows(
            &mut fallback_rows,
            row_to_node,
            "get_nodes_by_qualified_name",
        )
        .await
    }
```

### `get_ranked_nodes_by_edge_kind` — src/db/queries/edges.rs:842

Source: [src/db/queries/edges.rs:842](../src/db/queries/edges.rs#L842).

```rust
    pub async fn get_ranked_nodes_by_edge_kind(
        &self,
        edge_kind: &EdgeKind,
        node_kind: Option<&NodeKind>,
        incoming: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        debug_assert!(
            limit > 0,
            "get_ranked_nodes_by_edge_kind limit must be positive"
        );
        debug_assert!(
            !edge_kind.as_str().is_empty(),
            "edge_kind must not be empty"
        );
        let (join_col, group_col) = if incoming {
            ("e.target", "e.target")
        } else {
            ("e.source", "e.source")
        };

        let mut conditions = vec!["e.kind = ?1".to_string()];
        let mut param_values: Vec<libsql::Value> =
            vec![libsql::Value::Text(edge_kind.as_str().to_string())];
        let mut param_idx = 2;

        if let Some(nk) = node_kind {
            conditions.push(format!("n.kind = ?{param_idx}"));
            param_values.push(libsql::Value::Text(nk.as_str().to_string()));
            param_idx += 1;
        }
        if let Some(prefix) = path_prefix {
            let mut filter = String::new();
            push_path_prefix_filter(&mut filter, "n.", prefix);
            conditions.push(filter);
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    COUNT(*) AS cnt
             FROM edges e
             JOIN nodes n ON {join_col} = n.id
             WHERE {where_clause}
             GROUP BY {group_col}
             ORDER BY cnt DESC
             LIMIT ?{param_idx}"
        );
        param_values.push(libsql::Value::Integer(limit as i64));

        let op = "get_ranked_nodes_by_edge_kind";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query ranked nodes: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let count = row.get::<u64>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read count column: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, count));
        }

        Ok(items)
    }
```

### `get_largest_nodes` — src/db/queries/edges.rs:926

Source: [src/db/queries/edges.rs:926](../src/db/queries/edges.rs#L926).

```rust
    pub async fn get_largest_nodes(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32)>> {
        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<libsql::Value> = Vec::new();
        let mut param_idx = 1;

        if let Some(nk) = node_kind {
            conditions.push(format!("kind = ?{param_idx}"));
            param_values.push(libsql::Value::Text(nk.as_str().to_string()));
            param_idx += 1;
        }
        if let Some(prefix) = path_prefix {
            let mut filter = String::new();
            push_path_prefix_filter(&mut filter, "", prefix);
            conditions.push(filter);
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands,
                    (end_line - start_line + 1) AS lines
             FROM nodes
             {where_clause}
             ORDER BY lines DESC
             LIMIT ?{param_idx}"
        );
        param_values.push(libsql::Value::Integer(limit as i64));

        let op = "get_largest_nodes";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query largest nodes: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let lines = row.get::<u32>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read lines column: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, lines));
        }

        Ok(items)
    }
```

### `get_file_coupling` — src/db/queries/edges.rs:999

Source: [src/db/queries/edges.rs:999](../src/db/queries/edges.rs#L999).

```rust
    pub async fn get_file_coupling(
        &self,
        fan_in: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, u64)>> {
        let (group_alias, count_alias) = if fan_in {
            ("n_tgt", "n_src")
        } else {
            ("n_src", "n_tgt")
        };

        let path_filter = match path_prefix {
            Some(prefix) => {
                let mut f = String::from("AND ");
                push_path_prefix_filter(&mut f, &format!("{group_alias}."), prefix);
                f
            }
            None => String::new(),
        };

        let sql = format!(
            "SELECT {group_alias}.file_path, COUNT(DISTINCT {count_alias}.file_path) AS coupling
             FROM edges e
             JOIN nodes n_src ON e.source = n_src.id
             JOIN nodes n_tgt ON e.target = n_tgt.id
             WHERE e.kind IN ('calls', 'uses', 'implements', 'extends')
               AND n_src.file_path != n_tgt.file_path
               {path_filter}
             GROUP BY {group_alias}.file_path
             ORDER BY coupling DESC
             LIMIT ?1"
        );

        let op = "get_file_coupling";
        let mut rows = self
            .conn()
            .query(&sql, params![limit as i64])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file coupling: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let file_path = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read file_path: {e}"),
                operation: op.to_string(),
            })?;
            let count = row.get::<u64>(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read coupling count: {e}"),
                operation: op.to_string(),
            })?;
            items.push((file_path, count));
        }

        Ok(items)
    }
```

### `get_inheritance_depth` — src/db/queries/edges.rs:1067

Source: [src/db/queries/edges.rs:1067](../src/db/queries/edges.rs#L1067).

```rust
    pub async fn get_inheritance_depth(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        let path_filter = match path_prefix {
            Some(prefix) => {
                let mut f = String::from("WHERE ");
                push_path_prefix_filter(&mut f, "n.", prefix);
                f
            }
            None => String::new(),
        };

        // Track visited node IDs in `path` to avoid blowing up on cycles in the
        // `extends` graph. Without this guard, a cycle (or trait bound that
        // points back to itself through generics, common in Rust workspaces
        // like polkadot-sdk) makes the CTE explore the cycle up to the depth
        // bound, multiplied by every entry point — `get_inheritance_depth` then
        // takes >60s on polkadot vs 0.3s with cycle detection.
        //
        // Note the predicate order in the recursive step: `h.depth < 50` is a
        // cheap integer compare and is evaluated before the path `instr`
        // string-scan, so cycles still under the depth bound short-circuit
        // without paying for the substring search. Reducing the hierarchy to
        // `(leaf_id, max_depth)` in an inner subquery before joining `nodes`
        // means the `LIKE` path filter only runs against distinct leaves,
        // not against the (potentially huge) full hierarchy table.
        let sql = format!(
            "WITH RECURSIVE hierarchy(leaf_id, current_id, depth, path) AS (
                 SELECT e.source, e.target, 1,
                        ',' || e.source || ',' || e.target || ','
                 FROM edges e
                 WHERE e.kind = 'extends'
                 UNION ALL
                 SELECT h.leaf_id, e.target, h.depth + 1,
                        h.path || e.target || ','
                 FROM hierarchy h
                 JOIN edges e ON e.source = h.current_id AND e.kind = 'extends'
                 WHERE h.depth < 50
                   AND instr(h.path, ',' || e.target || ',') = 0
             ),
             leaf_depths AS (
                 SELECT leaf_id, MAX(depth) AS max_depth
                 FROM hierarchy
                 GROUP BY leaf_id
             )
             SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    ld.max_depth
             FROM leaf_depths ld
             JOIN nodes n ON ld.leaf_id = n.id
             {path_filter}
             ORDER BY ld.max_depth DESC
             LIMIT ?1"
        );

        let op = "get_inheritance_depth";
        let mut rows = self
            .conn()
            .query(&sql, params![limit as i64])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query inheritance depth: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let depth = row.get::<u64>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read depth column: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, depth));
        }

        Ok(items)
    }
```

### `get_node_distribution` — src/db/queries/edges.rs:1158

Source: [src/db/queries/edges.rs:1158](../src/db/queries/edges.rs#L1158).

```rust
    pub async fn get_node_distribution(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, u64)>> {
        let mut sql = String::from("SELECT file_path, kind, COUNT(*) AS cnt FROM nodes");
        if let Some(prefix) = path_prefix {
            sql.push_str(" WHERE ");
            push_path_prefix_filter(&mut sql, "", prefix);
        }
        sql.push_str(" GROUP BY file_path, kind ORDER BY file_path, cnt DESC");

        let op = "get_node_distribution";
        let mut rows = self
            .conn()
            .query(&sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node distribution: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let file_path = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read file_path: {e}"),
                operation: op.to_string(),
            })?;
            let kind = row.get::<String>(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read kind: {e}"),
                operation: op.to_string(),
            })?;
            let count = row.get::<u64>(2).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read count: {e}"),
                operation: op.to_string(),
            })?;
            items.push((file_path, kind, count));
        }

        Ok(items)
    }
```

### `get_call_edges` — src/db/queries/edges.rs:1205

Source: [src/db/queries/edges.rs:1205](../src/db/queries/edges.rs#L1205).

```rust
    pub async fn get_call_edges(&self, path_prefix: Option<&str>) -> Result<Vec<(String, String)>> {
        let op = "get_call_edges";
        let (sql, param_values): (String, Vec<libsql::Value>) = match path_prefix {
            Some(prefix) => {
                let mut sql = String::from(
                    "SELECT e.source, e.target FROM edges e
                 JOIN nodes n ON e.source = n.id
                 WHERE e.kind = 'calls' AND ",
                );
                push_path_prefix_filter(&mut sql, "n.", prefix);
                (sql, vec![])
            }
            None => (
                "SELECT source, target FROM edges WHERE kind = 'calls'".to_string(),
                vec![],
            ),
        };
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query call edges: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let source = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read source: {e}"),
                operation: op.to_string(),
            })?;
            let target = row.get::<String>(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read target: {e}"),
                operation: op.to_string(),
            })?;
            items.push((source, target));
        }

        Ok(items)
    }
```

### `get_call_edges_with_lines` — src/db/queries/edges.rs:1253

Source: [src/db/queries/edges.rs:1253](../src/db/queries/edges.rs#L1253).

```rust
    pub async fn get_call_edges_with_lines(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, Option<u32>)>> {
        let op = "get_call_edges_with_lines";
        let (sql, param_values): (String, Vec<libsql::Value>) = match path_prefix {
            Some(prefix) => {
                let mut sql = String::from(
                    "SELECT e.source, e.target, e.line FROM edges e
                 JOIN nodes n ON e.source = n.id
                 WHERE e.kind = 'calls' AND ",
                );
                push_path_prefix_filter(&mut sql, "n.", prefix);
                (sql, vec![])
            }
            None => (
                "SELECT source, target, line FROM edges WHERE kind = 'calls'".to_string(),
                vec![],
            ),
        };
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query call edges with lines: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let source = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read source: {e}"),
                operation: op.to_string(),
            })?;
            let target = row.get::<String>(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read target: {e}"),
                operation: op.to_string(),
            })?;
            let line = row.get::<u32>(2).ok();
            items.push((source, target, line));
        }

        Ok(items)
    }
```

### `get_complexity_ranked` — src/db/queries/edges.rs:1307

Source: [src/db/queries/edges.rs:1307](../src/db/queries/edges.rs#L1307).

```rust
    pub async fn get_complexity_ranked(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32, u64, u64, u64)>> {
        debug_assert!(limit > 0, "get_complexity_ranked limit must be positive");
        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<libsql::Value> = Vec::new();
        let mut param_idx = 1;

        match node_kind {
            Some(nk) => {
                conditions.push(format!("n.kind = ?{param_idx}"));
                param_values.push(libsql::Value::Text(nk.as_str().to_string()));
                param_idx += 1;
            }
            None => {
                conditions.push("n.kind IN ('function', 'method', 'singleton_method')".to_string());
            }
        }
        if let Some(prefix) = path_prefix {
            let mut filter = String::new();
            push_path_prefix_filter(&mut filter, "n.", prefix);
            conditions.push(filter);
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    (n.end_line - n.start_line + 1) AS lines,
                    COALESCE(out_calls.cnt, 0) AS fan_out,
                    COALESCE(in_calls.cnt, 0) AS fan_in,
                    ((n.end_line - n.start_line + 1) + COALESCE(out_calls.cnt, 0) * 3 + COALESCE(in_calls.cnt, 0)) AS score
             FROM nodes n
             LEFT JOIN (SELECT source, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY source) out_calls ON out_calls.source = n.id
             LEFT JOIN (SELECT target, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY target) in_calls ON in_calls.target = n.id
             WHERE {where_clause}
             ORDER BY score DESC
             LIMIT ?{param_idx}"
        );
        param_values.push(libsql::Value::Integer(limit as i64));

        let op = "get_complexity_ranked";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query complexity ranking: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let lines = row.get::<u32>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read lines: {e}"),
                operation: op.to_string(),
            })?;
            let fan_out = row.get::<u64>(29).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read fan_out: {e}"),
                operation: op.to_string(),
            })?;
            let fan_in = row.get::<u64>(30).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read fan_in: {e}"),
                operation: op.to_string(),
            })?;
            let score = row.get::<u64>(31).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read score: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, lines, fan_out, fan_in, score));
        }

        Ok(items)
    }
```

### `get_undocumented_public_symbols` — src/db/queries/edges.rs:1400

Source: [src/db/queries/edges.rs:1400](../src/db/queries/edges.rs#L1400).

```rust
    pub async fn get_undocumented_public_symbols(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Node>> {
        const DOC_COVERAGE_KINDS: &str =
            "'function', 'method', 'singleton_method', 'class', 'interface', 'trait', \
            'struct', 'enum', 'module', 'field', 'enum_variant', 'const', 'static', 'type_alias', \
            'property', 'csharp_property', 'record', 'data_class', 'sealed_class', 'object', \
            'case_class', 'kotlin_object', 'inner_class', 'abstract_method', 'constructor', \
            'struct_method', 'val', 'var', 'mixin', 'extension', 'union', 'typedef'";

        let path_filter = path_prefix.map(|prefix| {
            let mut filter = String::new();
            push_path_prefix_filter(&mut filter, "", prefix);
            filter
        });
        let (sql, param_values): (String, Vec<libsql::Value>) = match &path_filter {
            Some(path_filter) => (
                format!(
                    "SELECT id, kind, name, qualified_name, file_path,
                            start_line, end_line, start_column, end_column,
                            docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                     FROM nodes
                     WHERE visibility = 'public'
                       AND (docstring IS NULL OR docstring = '')
                       AND kind IN ({DOC_COVERAGE_KINDS})
                       AND {path_filter}
                     ORDER BY file_path, start_line
                     LIMIT ?1"
                ),
                vec![libsql::Value::Integer(limit as i64)],
            ),
            None => (
                format!(
                    "SELECT id, kind, name, qualified_name, file_path,
                            start_line, end_line, start_column, end_column,
                            docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                     FROM nodes
                     WHERE visibility = 'public'
                       AND (docstring IS NULL OR docstring = '')
                       AND kind IN ({DOC_COVERAGE_KINDS})
                     ORDER BY file_path, start_line
                     LIMIT ?1"
                ),
                vec![libsql::Value::Integer(limit as i64)],
            ),
        };

        let op = "get_undocumented_public_symbols";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query undocumented symbols: {e}"),
                operation: op.to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, op).await
    }
```

### `get_god_classes` — src/db/queries/edges.rs:1465

Source: [src/db/queries/edges.rs:1465](../src/db/queries/edges.rs#L1465).

```rust
    pub async fn get_god_classes(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64, u64, u64)>> {
        let path_filter = match path_prefix {
            Some(prefix) => {
                let mut f = String::from("AND ");
                push_path_prefix_filter(&mut f, "n.", prefix);
                f
            }
            None => String::new(),
        };

        // After v9, containment is `nodes.parent_id`, not Contains edges.
        // Join each candidate container directly to its children via parent_id.
        let sql = format!(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    SUM(CASE WHEN c.kind IN ('method', 'singleton_method', 'abstract_method', 'constructor') THEN 1 ELSE 0 END) AS methods,
                    SUM(CASE WHEN c.kind = 'field' THEN 1 ELSE 0 END) AS fields,
                    COUNT(*) AS total
             FROM nodes n
             JOIN nodes c ON c.parent_id = n.id
             WHERE n.kind IN ('class', 'struct', 'inner_class', 'object')
               {path_filter}
             GROUP BY n.id
             ORDER BY total DESC
             LIMIT ?1"
        );

        let op = "get_god_classes";
        let mut rows = self
            .conn()
            .query(&sql, params![limit as i64])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query god classes: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let methods = row.get::<u64>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read methods: {e}"),
                operation: op.to_string(),
            })?;
            let fields = row.get::<u64>(29).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read fields: {e}"),
                operation: op.to_string(),
            })?;
            let total = row.get::<u64>(30).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read total: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, methods, fields, total));
        }

        Ok(items)
    }
```

### `get_all_edges` — src/db/queries/edges.rs:1535

Source: [src/db/queries/edges.rs:1535](../src/db/queries/edges.rs#L1535).

```rust
    pub async fn get_all_edges(&self) -> Result<Vec<Edge>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT source, target, kind, line, resolved_by FROM edges",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all edges: {e}"),
                operation: "get_all_edges".to_string(),
            })?;

        collect_rows(&mut rows, row_to_edge, "get_all_edges").await
    }
```

### `get_edges_by_kind` — src/db/queries/edges.rs:1556

Source: [src/db/queries/edges.rs:1556](../src/db/queries/edges.rs#L1556).

```rust
    pub async fn get_edges_by_kind(&self, kind: EdgeKind) -> Result<Vec<Edge>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT source, target, kind, line, resolved_by FROM edges WHERE kind = ?1",
                params![kind.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query edges by kind: {e}"),
                operation: "get_edges_by_kind".to_string(),
            })?;

        collect_rows(&mut rows, row_to_edge, "get_edges_by_kind").await
    }
```

### `get_edges_by_kinds` — src/db/queries/edges.rs:1577

Source: [src/db/queries/edges.rs:1577](../src/db/queries/edges.rs#L1577).

```rust
    pub async fn get_edges_by_kinds(&self, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = (0..kinds.len()).map(|i| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT source, target, kind, line, resolved_by FROM edges WHERE kind IN ({})",
            placeholders.join(", ")
        );
        let param_values: Vec<libsql::Value> = kinds
            .iter()
            .map(|k| libsql::Value::Text(k.as_str().to_string()))
            .collect();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query edges by kinds: {e}"),
                operation: "get_edges_by_kinds".to_string(),
            })?;

        collect_rows(&mut rows, row_to_edge, "get_edges_by_kinds").await
    }
```

### `variant_group_candidates` — src/db/queries/edges.rs:1628

Source: [src/db/queries/edges.rs:1628](../src/db/queries/edges.rs#L1628).

```rust
    pub async fn variant_group_candidates(&self) -> Result<Vec<(String, String)>> {
        let kinds = crate::resolution::CALLABLE_KIND_NAMES
            .iter()
            .map(|k| format!("'{k}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT 'rs' || char(1) || n.qualified_name, n.id \
             FROM nodes n \
             JOIN edges e ON e.target = n.id AND e.kind = 'annotates' \
             JOIN nodes a ON a.id = e.source \
             WHERE a.kind = 'annotation_usage' AND a.name IN ('cfg', 'cfg_attr') \
               AND lower(n.file_path) LIKE '%.rs' \
               AND n.kind IN ({kinds})"
        );
        let mut rows = self
            .conn()
            .query(&sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query rust variant candidates: {e}"),
                operation: "variant_group_candidates".to_string(),
            })?;
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            if let (Ok(key), Ok(id)) = (row.get::<String>(0), row.get::<String>(1)) {
                out.push((key, id));
            }
        }
        Ok(out)
    }
```

### `go_variant_candidates` — src/db/queries/edges.rs:1666

Source: [src/db/queries/edges.rs:1666](../src/db/queries/edges.rs#L1666).

```rust
    pub async fn go_variant_candidates(&self) -> Result<Vec<(String, String, String)>> {
        let sql = "SELECT file_path, name, id FROM nodes                    WHERE kind = 'function' AND lower(file_path) LIKE '%.go'                      AND name IN (                        SELECT name FROM nodes                        WHERE kind = 'function' AND lower(file_path) LIKE '%.go'                        GROUP BY name HAVING COUNT(*) >= 2                      )";
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query go variant candidates: {e}"),
                operation: "go_variant_candidates".to_string(),
            })?;
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            if let (Ok(path), Ok(name), Ok(id)) = (
                row.get::<String>(0),
                row.get::<String>(1),
                row.get::<String>(2),
            ) {
                out.push((path, name, id));
            }
        }
        Ok(out)
    }
```

### `calls_edges_into` — src/db/queries/edges.rs:1693

Source: [src/db/queries/edges.rs:1693](../src/db/queries/edges.rs#L1693).

```rust
    pub async fn calls_edges_into(&self, targets: &[String]) -> Result<Vec<Edge>> {
        const CHUNK: usize = 900;
        let mut out = Vec::new();
        for chunk in targets.chunks(CHUNK) {
            let placeholders: Vec<String> =
                (0..chunk.len()).map(|i| format!("?{}", i + 1)).collect();
            let sql = format!(
                "SELECT source, target, kind, line, resolved_by FROM edges \
                 WHERE kind = 'calls' AND target IN ({})",
                placeholders.join(", ")
            );
            let values: Vec<libsql::Value> = chunk
                .iter()
                .map(|t| libsql::Value::Text(t.clone()))
                .collect();
            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query calls edges into targets: {e}"),
                    operation: "calls_edges_into".to_string(),
                })?;
            out.extend(collect_rows(&mut rows, row_to_edge, "calls_edges_into").await?);
        }
        Ok(out)
    }
```

### `get_top_degree_nodes` — src/db/queries/edges.rs:1739

Source: [src/db/queries/edges.rs:1739](../src/db/queries/edges.rs#L1739).

```rust
    pub async fn get_top_degree_nodes(&self, limit: usize) -> Result<Vec<(String, u32, u32)>> {
        let sql = "SELECT id, \
                          SUM(inc) AS in_degree, \
                          SUM(outg) AS out_degree \
                   FROM ( \
                     SELECT target AS id, 1 AS inc, 0 AS outg FROM edges \
                       WHERE source <> target \
                     UNION ALL \
                     SELECT source AS id, 0 AS inc, 1 AS outg FROM edges \
                       WHERE source <> target \
                   ) \
                   GROUP BY id \
                   ORDER BY (in_degree + out_degree) DESC, id ASC \
                   LIMIT ?1";
        let mut rows = self
            .conn()
            .query(sql, params![limit as i64])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query top-degree nodes: {e}"),
                operation: "get_top_degree_nodes".to_string(),
            })?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read top-degree row: {e}"),
            operation: "get_top_degree_nodes".to_string(),
        })? {
            let read = |i: i32, what: &str| -> Result<i64> {
                row.get::<i64>(i).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read {what}: {e}"),
                    operation: "get_top_degree_nodes".to_string(),
                })
            };
            let id = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read id: {e}"),
                operation: "get_top_degree_nodes".to_string(),
            })?;
            let in_degree = u32::try_from(read(1, "in_degree")?).unwrap_or(u32::MAX);
            let out_degree = u32::try_from(read(2, "out_degree")?).unwrap_or(u32::MAX);
            out.push((id, in_degree, out_degree));
        }
        Ok(out)
    }
```

### `delete_edges_by_source` — src/db/queries/edges.rs:1785

Source: [src/db/queries/edges.rs:1785](../src/db/queries/edges.rs#L1785).

```rust
    pub async fn delete_edges_by_source(&self, source_id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM edges WHERE source = ?1", params![source_id])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete edges by source: {e}"),
                operation: "delete_edges_by_source".to_string(),
            })?;
        Ok(())
    }
```

### `delete_call_edges_at_sites` — src/db/queries/edges.rs:1799

Source: [src/db/queries/edges.rs:1799](../src/db/queries/edges.rs#L1799).

```rust
    pub async fn delete_call_edges_at_sites(&self, sites: &[(String, u32, String)]) -> Result<()> {
        if sites.is_empty() {
            return Ok(());
        }
        for (source, line, name) in sites {
            self.conn()
                .execute(
                    "DELETE FROM edges
                     WHERE kind = 'calls' AND source = ?1 AND line = ?2
                       AND target IN (SELECT id FROM nodes WHERE name = ?3)",
                    params![source.as_str(), i64::from(*line), name.as_str()],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to delete call edges at a site: {e}"),
                    operation: "delete_call_edges_at_sites".to_string(),
                })?;
        }
        Ok(())
    }
```

### `upsert_files` — src/db/queries/files.rs:11

Source: [src/db/queries/files.rs:11](../src/db/queries/files.rs#L11).

```rust
    pub async fn upsert_files(&self, files: &[FileRecord]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin: {e}"),
                operation: "upsert_files".to_string(),
            })?;

        let stmt = self.conn()
            .prepare("INSERT OR REPLACE INTO files (path,content_hash,size,modified_at,indexed_at,node_count,kind) VALUES (?1,?2,?3,?4,?5,?6,?7)")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "upsert_files".to_string(),
            })?;

        for file in files {
            stmt.execute(params![
                file.path.as_str(),
                file.content_hash.as_str(),
                file.size as i64,
                file.modified_at,
                file.indexed_at,
                i64::from(file.node_count),
                file.kind.as_str(),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to upsert file: {e}"),
                operation: "upsert_files".to_string(),
            })?;
            stmt.reset();
        }

        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit: {e}"),
                operation: "upsert_files".to_string(),
            })?;
        Ok(())
    }
```

### `upsert_file` — src/db/queries/files.rs:60

Source: [src/db/queries/files.rs:60](../src/db/queries/files.rs#L60).

```rust
    pub async fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO files
                (path, content_hash, size, modified_at, indexed_at, node_count, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    file.path.as_str(),
                    file.content_hash.as_str(),
                    file.size as i64,
                    file.modified_at,
                    file.indexed_at,
                    i64::from(file.node_count),
                    file.kind.as_str(),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to upsert file: {e}"),
                operation: "upsert_file".to_string(),
            })?;
        Ok(())
    }
```

### `get_file` — src/db/queries/files.rs:85

Source: [src/db/queries/files.rs:85](../src/db/queries/files.rs#L85).

```rust
    pub async fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count, kind
                 FROM files WHERE path = ?1",
                params![path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file: {e}"),
                operation: "get_file".to_string(),
            })?;

        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read file row: {e}"),
            operation: "get_file".to_string(),
        })? {
            Some(row) => {
                let file = row_to_file(&row).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to map file row: {e}"),
                    operation: "get_file".to_string(),
                })?;
                Ok(Some(file))
            }
            None => Ok(None),
        }
    }
```

### `get_all_files` — src/db/queries/files.rs:115

Source: [src/db/queries/files.rs:115](../src/db/queries/files.rs#L115).

```rust
    pub async fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count, kind FROM files",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all files: {e}"),
                operation: "get_all_files".to_string(),
            })?;

        collect_rows(&mut rows, row_to_file, "get_all_files").await
    }
```

### `get_code_files` — src/db/queries/files.rs:136

Source: [src/db/queries/files.rs:136](../src/db/queries/files.rs#L136).

```rust
    pub async fn get_code_files(&self) -> Result<Vec<FileRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count, kind
                 FROM files WHERE kind = 'code'",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query code files: {e}"),
                operation: "get_code_files".to_string(),
            })?;

        collect_rows(&mut rows, row_to_file, "get_code_files").await
    }
```

### `delete_file` — src/db/queries/files.rs:154

Source: [src/db/queries/files.rs:154](../src/db/queries/files.rs#L154).

```rust
    pub async fn delete_file(&self, path: &str) -> Result<()> {
        self.delete_nodes_by_file(path).await?;
        self.conn()
            .execute("DELETE FROM files WHERE path = ?1", params![path])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete file: {e}"),
                operation: "delete_file".to_string(),
            })?;
        Ok(())
    }
```

### `upsert_fingerprint` — src/db/queries/fingerprints.rs:22

Source: [src/db/queries/fingerprints.rs:22](../src/db/queries/fingerprints.rs#L22).

```rust
    pub async fn upsert_fingerprint(
        &self,
        node_id: &str,
        fp: &crate::redundancy::Fingerprint,
    ) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO node_fingerprints
                 (node_id, ast_hash, cfg_hash, call_seq_hash, shingles, body_tokens, source_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    node_id,
                    fp.ast_hash.as_str(),
                    fp.cfg_hash.as_str(),
                    fp.call_seq_hash.as_str(),
                    fp.shingles_to_string(),
                    i64::try_from(fp.body_tokens).unwrap_or(i64::MAX),
                    fp.source_hash.as_str(),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to upsert fingerprint: {e}"),
                operation: "upsert_fingerprint".to_string(),
            })?;
        Ok(())
    }
```

### `get_fingerprint` — src/db/queries/fingerprints.rs:51

Source: [src/db/queries/fingerprints.rs:51](../src/db/queries/fingerprints.rs#L51).

```rust
    pub async fn get_fingerprint(&self, node_id: &str) -> Result<Option<StoredFingerprint>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT node_id, ast_hash, cfg_hash, call_seq_hash, shingles, body_tokens, source_hash
                   FROM node_fingerprints WHERE node_id = ?1",
                params![node_id],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query fingerprint: {e}"),
                operation: "get_fingerprint".to_string(),
            })?;
        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read fingerprint row: {e}"),
            operation: "get_fingerprint".to_string(),
        })? {
            Some(row) => Ok(Some(row_to_fingerprint(&row)?)),
            None => Ok(None),
        }
    }
```

### `get_all_fingerprints` — src/db/queries/fingerprints.rs:76

Source: [src/db/queries/fingerprints.rs:76](../src/db/queries/fingerprints.rs#L76).

```rust
    pub async fn get_all_fingerprints(&self) -> Result<Vec<StoredFingerprint>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT node_id, ast_hash, cfg_hash, call_seq_hash, shingles, body_tokens, source_hash
                   FROM node_fingerprints",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query fingerprints: {e}"),
                operation: "get_all_fingerprints".to_string(),
            })?;
        let mut out: Vec<StoredFingerprint> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read fingerprint row: {e}"),
            operation: "get_all_fingerprints".to_string(),
        })? {
            out.push(row_to_fingerprint(&row)?);
        }
        Ok(out)
    }
```

### `get_metadata` — src/db/queries/metadata.rs:10

Source: [src/db/queries/metadata.rs:10](../src/db/queries/metadata.rs#L10).

```rust
    pub async fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let mut rows = self
            .conn()
            .query("SELECT value FROM metadata WHERE key = ?1", params![key])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query metadata: {e}"),
                operation: "get_metadata".to_string(),
            })?;

        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read metadata row: {e}"),
            operation: "get_metadata".to_string(),
        })? {
            Some(row) => {
                let value: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read metadata value: {e}"),
                    operation: "get_metadata".to_string(),
                })?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
```

### `set_metadata` — src/db/queries/metadata.rs:36

Source: [src/db/queries/metadata.rs:36](../src/db/queries/metadata.rs#L36).

```rust
    pub async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to set metadata: {e}"),
                operation: "set_metadata".to_string(),
            })?;
        Ok(())
    }
```

### `get_nodes_by_dir` — src/db/queries/metadata.rs:53

Source: [src/db/queries/metadata.rs:53](../src/db/queries/metadata.rs#L53).

```rust
    pub async fn get_nodes_by_dir(&self, dir: &str, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }

        let kind_placeholders: Vec<String> = kinds
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let mut path_filter = String::new();
        push_path_prefix_filter(&mut path_filter, "", dir);
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async,
                    branches, loops, returns, max_nesting,
                    unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes
             WHERE {path_filter} AND kind IN ({})
             ORDER BY file_path, start_line",
            kind_placeholders.join(", ")
        );

        let mut param_values: Vec<libsql::Value> = Vec::new();
        for k in kinds {
            param_values.push(libsql::Value::Text(k.as_str().to_string()));
        }

        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query nodes by dir: {e}"),
                operation: "get_nodes_by_dir".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_by_dir").await
    }
```

### `get_internal_edges` — src/db/queries/metadata.rs:97

Source: [src/db/queries/metadata.rs:97](../src/db/queries/metadata.rs#L97).

```rust
    pub async fn get_internal_edges(&self, node_ids: &[String]) -> Result<Vec<Edge>> {
        const BATCH_SIZE: usize = 500;
        if node_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Build a set of IDs for filtering targets in memory, then query
        // edges from each batch of sources.
        let id_set: std::collections::HashSet<&str> =
            node_ids.iter().map(std::string::String::as_str).collect();
        let mut all_edges = Vec::new();
        let mut offset = 0;
        while offset < node_ids.len() {
            let end = (offset + BATCH_SIZE).min(node_ids.len());
            let batch = &node_ids[offset..end];

            let placeholders: Vec<String> = batch
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect();
            let sql = format!(
                "SELECT source, target, kind, line, resolved_by FROM edges WHERE source IN ({})",
                placeholders.join(", ")
            );

            let param_values: Vec<libsql::Value> = batch
                .iter()
                .map(|id| libsql::Value::Text(id.clone()))
                .collect();

            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(param_values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query internal edges: {e}"),
                    operation: "get_internal_edges".to_string(),
                })?;

            let batch_edges: Vec<Edge> =
                collect_rows(&mut rows, row_to_edge, "get_internal_edges").await?;

            // Keep only edges whose target is also in the node set.
            for edge in batch_edges {
                if id_set.contains(edge.target.as_str()) {
                    all_edges.push(edge);
                }
            }

            offset = end;
        }

        Ok(all_edges)
    }
```

### `collect_test_marker_ids` — src/db/queries/metadata.rs:165

Source: [src/db/queries/metadata.rs:165](../src/db/queries/metadata.rs#L165).

```rust
    pub async fn collect_test_marker_ids(&self) -> Result<Vec<String>> {
        let op = "collect_test_marker_ids";
        let sql = "SELECT id FROM nodes
                   WHERE kind = 'annotation_usage'
                     AND (
                         name = 'test'
                         OR name LIKE '%::test'
                         OR name = 'wasm_bindgen_test'
                         OR name LIKE '%::wasm_bindgen_test'
                     )";
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query test marker ids: {e}"),
                operation: op.to_string(),
            })?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read marker id row: {e}"),
            operation: op.to_string(),
        })? {
            let id: String = row.get(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read marker id column: {e}"),
                operation: op.to_string(),
            })?;
            ids.push(id);
        }
        Ok(ids)
    }
```

### `insert_node` — src/db/queries/nodes.rs:138

Source: [src/db/queries/nodes.rs:138](../src/db/queries/nodes.rs#L138).

```rust
    pub async fn insert_node(&self, node: &Node) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO nodes
                (id, kind, name, qualified_name, file_path,
                 start_line, end_line, start_column, end_column,
                 docstring, signature, visibility, is_async,
                 branches, loops, returns, max_nesting,
                 unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands, search_terms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)",
                params![
                    node.id.as_str(),
                    node.kind.as_str(),
                    node.name.as_str(),
                    node.qualified_name.as_str(),
                    node.file_path.as_str(),
                    i64::from(node.start_line),
                    i64::from(node.end_line),
                    i64::from(node.start_column),
                    i64::from(node.end_column),
                    opt_str(node.docstring.as_deref()),
                    opt_str(node.signature.as_deref()),
                    node.visibility.as_str(),
                    i64::from(node.is_async),
                    i64::from(node.branches),
                    i64::from(node.loops),
                    i64::from(node.returns),
                    i64::from(node.max_nesting),
                    i64::from(node.unsafe_blocks),
                    i64::from(node.unchecked_calls),
                    i64::from(node.assertions),
                    node.updated_at as i64,
                    i64::from(node.attrs_start_line),
                    opt_str(node.parent_id.as_deref()),
                    i64::from(node.cognitive_complexity),
                    i64::from(node.distinct_operators),
                    i64::from(node.distinct_operands),
                    i64::from(node.total_operators),
                    i64::from(node.total_operands),
                    crate::text::search_terms(&node.name, &node.qualified_name),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert node: {e}"),
                operation: "insert_node".to_string(),
            })?;
        Ok(())
    }
```

### `insert_all` — src/db/queries/nodes.rs:195

Source: [src/db/queries/nodes.rs:195](../src/db/queries/nodes.rs#L195).

```rust
    pub async fn insert_all(
        &self,
        nodes: &[Node],
        edges: &[Edge],
        files: &[FileRecord],
    ) -> Result<()> {
        // Pull every Contains edge out: build target_id -> parent_id map, then
        // filter the surviving edges list. When a node has multiple incoming
        // Contains rows (extractor anomaly), the first one wins — matching
        // the migration's `LIMIT 1` backfill behavior.
        let mut parent_map: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::new();
        let mut surviving_edges: Vec<&Edge> = Vec::with_capacity(edges.len());
        for edge in edges {
            if edge.kind == crate::types::EdgeKind::Contains {
                parent_map
                    .entry(edge.target.as_str())
                    .or_insert(edge.source.as_str());
            } else {
                surviving_edges.push(edge);
            }
        }
        // Apply the hoisted parents to the node slice without cloning every
        // node: we materialize only when parent_map has something to say.
        let nodes_owned: Vec<Node>;
        let nodes_ref: &[Node] = if parent_map.is_empty() {
            nodes
        } else {
            nodes_owned = nodes
                .iter()
                .map(|n| {
                    if let Some(parent) = parent_map.get(n.id.as_str()) {
                        let mut copy = n.clone();
                        copy.parent_id = Some((*parent).to_string());
                        copy
                    } else {
                        n.clone()
                    }
                })
                .collect();
            &nodes_owned
        };

        let mut sql = String::with_capacity(
            nodes_ref.len() * 400 + surviving_edges.len() * 120 + files.len() * 120,
        );
        sql.push_str("BEGIN;\n");

        // Nodes
        for chunk in nodes_ref.chunks(200) {
            sql.push_str(
                "INSERT OR REPLACE INTO nodes \
                 (id,kind,name,qualified_name,file_path,\
                 start_line,end_line,start_column,end_column,\
                 docstring,signature,visibility,is_async,\
                 branches,loops,returns,max_nesting,\
                 unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands,search_terms) VALUES ",
            );
            for (i, node) in chunk.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('(');
                push_quoted(&mut sql, &node.id);
                sql.push(',');
                push_quoted(&mut sql, node.kind.as_str());
                sql.push(',');
                push_quoted(&mut sql, &node.name);
                sql.push(',');
                push_quoted(&mut sql, &node.qualified_name);
                sql.push(',');
                push_quoted(&mut sql, &node.file_path);
                sql.push(',');
                push_int(&mut sql, i64::from(node.start_line));
                sql.push(',');
                push_int(&mut sql, i64::from(node.end_line));
                sql.push(',');
                push_int(&mut sql, i64::from(node.start_column));
                sql.push(',');
                push_int(&mut sql, i64::from(node.end_column));
                sql.push(',');
                push_opt_quoted(&mut sql, node.docstring.as_deref());
                sql.push(',');
                push_opt_quoted(&mut sql, node.signature.as_deref());
                sql.push(',');
                push_quoted(&mut sql, node.visibility.as_str());
                sql.push(',');
                push_int(&mut sql, i64::from(node.is_async));
                sql.push(',');
                push_int(&mut sql, i64::from(node.branches));
                sql.push(',');
                push_int(&mut sql, i64::from(node.loops));
                sql.push(',');
                push_int(&mut sql, i64::from(node.returns));
                sql.push(',');
                push_int(&mut sql, i64::from(node.max_nesting));
                sql.push(',');
                push_int(&mut sql, i64::from(node.unsafe_blocks));
                sql.push(',');
                push_int(&mut sql, i64::from(node.unchecked_calls));
                sql.push(',');
                push_int(&mut sql, i64::from(node.assertions));
                sql.push(',');
                push_int(&mut sql, node.updated_at as i64);
                sql.push(',');
                push_int(&mut sql, i64::from(node.attrs_start_line));
                sql.push(',');
                push_opt_quoted(&mut sql, node.parent_id.as_deref());
                sql.push(',');
                push_int(&mut sql, i64::from(node.cognitive_complexity));
                sql.push(',');
                push_int(&mut sql, i64::from(node.distinct_operators));
                sql.push(',');
                push_int(&mut sql, i64::from(node.distinct_operands));
                sql.push(',');
                push_int(&mut sql, i64::from(node.total_operators));
                sql.push(',');
                push_int(&mut sql, i64::from(node.total_operands));
                sql.push(',');
                push_quoted(
                    &mut sql,
                    &crate::text::search_terms(&node.name, &node.qualified_name),
                );
                sql.push(')');
            }
            sql.push_str(";\n");
        }

        // Edges (Contains has already been hoisted out into parent_id)
        for chunk in surviving_edges.chunks(500) {
            sql.push_str("INSERT INTO edges (source,target,kind,line,resolved_by) VALUES ");
            for (i, edge) in chunk.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('(');
                push_quoted(&mut sql, &edge.source);
                sql.push(',');
                push_quoted(&mut sql, &edge.target);
                sql.push(',');
                push_quoted(&mut sql, edge.kind.as_str());
                sql.push(',');
                match edge.line {
                    Some(l) => push_int(&mut sql, i64::from(l)),
                    None => sql.push_str("NULL"),
                }
                sql.push(',');
                match edge.resolved_by {
                    Some(r) => push_int(&mut sql, r.code()),
                    None => sql.push_str("NULL"),
                }
                sql.push(')');
            }
            sql.push_str(EDGE_UPSERT_CLAUSE);
            sql.push_str(";\n");
        }

        // Files
        for chunk in files.chunks(500) {
            sql.push_str(
                "INSERT OR REPLACE INTO files \
                 (path,content_hash,size,modified_at,indexed_at,node_count) VALUES ",
            );
            for (i, file) in chunk.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('(');
                push_quoted(&mut sql, &file.path);
                sql.push(',');
                push_quoted(&mut sql, &file.content_hash);
                sql.push(',');
                push_int(&mut sql, file.size as i64);
                sql.push(',');
                push_int(&mut sql, file.modified_at);
                sql.push(',');
                push_int(&mut sql, file.indexed_at);
                sql.push(',');
                push_int(&mut sql, i64::from(file.node_count));
                sql.push(')');
            }
            sql.push_str(";\n");
        }

        sql.push_str("COMMIT;\n");

        self.conn()
            .execute_batch(&sql)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to bulk insert: {e}"),
                operation: "insert_all".to_string(),
            })?;
        Ok(())
    }
```

### `insert_nodes` — src/db/queries/nodes.rs:393

Source: [src/db/queries/nodes.rs:393](../src/db/queries/nodes.rs#L393).

```rust
    pub async fn insert_nodes(&self, nodes: &[Node]) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin: {e}"),
                operation: "insert_nodes".to_string(),
            })?;

        let stmt = self.conn()
            .prepare(
                "INSERT OR REPLACE INTO nodes \
                 (id,kind,name,qualified_name,file_path,\
                 start_line,end_line,start_column,end_column,\
                 docstring,signature,visibility,is_async,\
                 branches,loops,returns,max_nesting,\
                 unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands,search_terms) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29)"
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "insert_nodes".to_string(),
            })?;

        for node in nodes {
            stmt.execute(params![
                node.id.as_str(),
                node.kind.as_str(),
                node.name.as_str(),
                node.qualified_name.as_str(),
                node.file_path.as_str(),
                i64::from(node.start_line),
                i64::from(node.end_line),
                i64::from(node.start_column),
                i64::from(node.end_column),
                opt_str(node.docstring.as_deref()),
                opt_str(node.signature.as_deref()),
                node.visibility.as_str(),
                i64::from(node.is_async),
                i64::from(node.branches),
                i64::from(node.loops),
                i64::from(node.returns),
                i64::from(node.max_nesting),
                i64::from(node.unsafe_blocks),
                i64::from(node.unchecked_calls),
                i64::from(node.assertions),
                node.updated_at as i64,
                i64::from(node.attrs_start_line),
                opt_str(node.parent_id.as_deref()),
                i64::from(node.cognitive_complexity),
                i64::from(node.distinct_operators),
                i64::from(node.distinct_operands),
                i64::from(node.total_operators),
                i64::from(node.total_operands),
                crate::text::search_terms(&node.name, &node.qualified_name),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert node: {e}"),
                operation: "insert_nodes".to_string(),
            })?;
            stmt.reset();
        }

        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit: {e}"),
                operation: "insert_nodes".to_string(),
            })?;
        Ok(())
    }
```

### `get_node_by_id` — src/db/queries/nodes.rs:473

Source: [src/db/queries/nodes.rs:473](../src/db/queries/nodes.rs#L473).

```rust
    pub async fn get_node_by_id(&self, id: &str) -> Result<Option<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE id = ?1",
                params![id],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node by id: {e}"),
                operation: "get_node_by_id".to_string(),
            })?;

        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read node row: {e}"),
            operation: "get_node_by_id".to_string(),
        })? {
            Some(row) => {
                let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to map node row: {e}"),
                    operation: "get_node_by_id".to_string(),
                })?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }
```

### `get_nodes_by_ids` — src/db/queries/nodes.rs:506

Source: [src/db/queries/nodes.rs:506](../src/db/queries/nodes.rs#L506).

```rust
    pub async fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<Node>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Build `?, ?, ?, …` in one allocation instead of `Vec<String>` of
        // `?1`/`?2`/`?N`. libsql binds anonymous `?` parameters in order, so
        // dropping the numbered form changes nothing for the driver. Large
        // BFS frontiers (`traverse_bfs` calls this once per level) hit this
        // path often enough that the per-id `format!` allocations showed up
        // on profiles.
        let placeholders = build_qmark_placeholders(ids.len());
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes WHERE id IN ({placeholders})",
        );
        let param_values: Vec<libsql::Value> = ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to batch query nodes: {e}"),
                operation: "get_nodes_by_ids".to_string(),
            })?;
        collect_rows(&mut rows, row_to_node, "get_nodes_by_ids").await
    }
```

### `get_nodes_by_file` — src/db/queries/nodes.rs:539

Source: [src/db/queries/nodes.rs:539](../src/db/queries/nodes.rs#L539).

```rust
    pub async fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE file_path = ?1 ORDER BY start_line",
                params![file_path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query nodes by file: {e}"),
                operation: "get_nodes_by_file".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_by_file").await
    }
```

### `get_children_of` — src/db/queries/nodes.rs:561

Source: [src/db/queries/nodes.rs:561](../src/db/queries/nodes.rs#L561).

```rust
    pub async fn get_children_of(&self, parent_id: &str) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE parent_id = ?1 ORDER BY start_line",
                params![parent_id],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query children: {e}"),
                operation: "get_children_of".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_children_of").await
    }
```

### `get_children_of_many` — src/db/queries/nodes.rs:581

Source: [src/db/queries/nodes.rs:581](../src/db/queries/nodes.rs#L581).

```rust
    pub async fn get_children_of_many(&self, parent_ids: &[String]) -> Result<Vec<Node>> {
        if parent_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = build_qmark_placeholders(parent_ids.len());
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes WHERE parent_id IN ({placeholders}) ORDER BY file_path, start_line"
        );
        let values = parent_ids
            .iter()
            .cloned()
            .map(libsql::Value::Text)
            .collect::<Vec<_>>();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query children for parent set: {e}"),
                operation: "get_children_of_many".to_string(),
            })?;
        collect_rows(&mut rows, row_to_node, "get_children_of_many").await
    }
```

### `get_trait_methods_for_impl_method` — src/db/queries/nodes.rs:610

Source: [src/db/queries/nodes.rs:610](../src/db/queries/nodes.rs#L610).

```rust
    pub async fn get_trait_methods_for_impl_method(
        &self,
        impl_id: &str,
        method_name: &str,
    ) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT method.id, method.kind, method.name, method.qualified_name, method.file_path,
                        method.start_line, method.end_line, method.start_column, method.end_column,
                        method.docstring, method.signature, method.visibility, method.is_async,
                        method.branches, method.loops, method.returns, method.max_nesting,
                        method.unsafe_blocks, method.unchecked_calls, method.assertions,
                        method.updated_at, method.attrs_start_line, method.parent_id,
                        method.cognitive_complexity, method.distinct_operators,
                        method.distinct_operands, method.total_operators, method.total_operands
                 FROM edges dispatch
                 JOIN nodes trait ON trait.id = dispatch.target AND trait.kind = 'trait'
                 JOIN nodes method ON method.parent_id = trait.id
                 WHERE dispatch.source = ?1
                   AND dispatch.kind = 'implements'
                   AND method.name = ?2
                   AND method.kind IN ('method', 'function')
                 ORDER BY method.file_path, method.start_line",
                params![impl_id, method_name],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to resolve reverse trait dispatch: {e}"),
                operation: "get_trait_methods_for_impl_method".to_string(),
            })?;
        collect_rows(&mut rows, row_to_node, "get_trait_methods_for_impl_method").await
    }
```

### `get_nodes_by_kind` — src/db/queries/nodes.rs:645

Source: [src/db/queries/nodes.rs:645](../src/db/queries/nodes.rs#L645).

```rust
    pub async fn get_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE kind = ?1",
                params![kind.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query nodes by kind: {e}"),
                operation: "get_nodes_by_kind".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_by_kind").await
    }
```

### `get_all_nodes` — src/db/queries/nodes.rs:665

Source: [src/db/queries/nodes.rs:665](../src/db/queries/nodes.rs#L665).

```rust
    pub async fn get_all_nodes(&self) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all nodes: {e}"),
                operation: "get_all_nodes".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_all_nodes").await
    }
```

### `replace_ambiguous_calls` — src/db/queries/nodes.rs:691

Source: [src/db/queries/nodes.rs:691](../src/db/queries/nodes.rs#L691).

```rust
    pub async fn replace_ambiguous_calls(
        &self,
        files: &[String],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        for file in files {
            self.conn()
                .execute(
                    "DELETE FROM ambiguous_calls WHERE file_path = ?1",
                    params![file.as_str()],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to clear ambiguous calls: {e}"),
                    operation: "replace_ambiguous_calls".to_string(),
                })?;
        }
        self.insert_ambiguous_calls(calls).await
    }
```

### `replace_ambiguous_calls_for_refs` — src/db/queries/nodes.rs:724

Source: [src/db/queries/nodes.rs:724](../src/db/queries/nodes.rs#L724).

```rust
    pub async fn replace_ambiguous_calls_for_refs(
        &self,
        refs: &[AmbiguityRefKey],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        if refs.is_empty() {
            return self.insert_ambiguous_calls(calls).await;
        }

        // One prepared statement inside one transaction, as `insert_unresolved_refs`
        // does: this loop is per re-attempted reference rather than per file, so
        // a sync that touches a lot of names would otherwise pay a round trip
        // and an implicit transaction for each one.
        let err = |e: libsql::Error, what: &str| TokenSaveError::Database {
            message: format!("failed to {what}: {e}"),
            operation: "replace_ambiguous_calls_for_refs".to_string(),
        };
        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| err(e, "begin"))?;
        let stmt = self
            .conn()
            .prepare(
                "DELETE FROM ambiguous_calls
                 WHERE from_node_id = ?1 AND reference_name = ?2
                   AND file_path = ?3 AND line = ?4",
            )
            .await
            .map_err(|e| err(e, "prepare"))?;
        for key in refs {
            stmt.execute(params![
                key.from_node_id.as_str(),
                key.reference_name.as_str(),
                key.file_path.as_str(),
                i64::from(key.line)
            ])
            .await
            .map_err(|e| err(e, "clear ambiguous call"))?;
            stmt.reset();
        }
        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| err(e, "commit"))?;

        self.insert_ambiguous_calls(calls).await
    }
```

### `insert_ambiguous_calls` — src/db/queries/nodes.rs:773

Source: [src/db/queries/nodes.rs:773](../src/db/queries/nodes.rs#L773).

```rust
    async fn insert_ambiguous_calls(&self, calls: &[AmbiguousCall]) -> Result<()> {
        for call in calls {
            let encoded = serde_json::to_string(&call.candidate_node_ids).map_err(|e| {
                TokenSaveError::Database {
                    message: format!("failed to encode ambiguity candidates: {e}"),
                    operation: "replace_ambiguous_calls".to_string(),
                }
            })?;
            self.conn()
                .execute(
                    "INSERT OR REPLACE INTO ambiguous_calls
                     (from_node_id, reference_name, file_path, line, candidate_node_ids)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        call.from_node_id.as_str(),
                        call.reference_name.as_str(),
                        call.file_path.as_str(),
                        i64::from(call.line),
                        encoded.as_str()
                    ],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to record ambiguous call: {e}"),
                    operation: "replace_ambiguous_calls".to_string(),
                })?;
        }
        Ok(())
    }
```

### `get_ambiguous_calls` — src/db/queries/nodes.rs:808

Source: [src/db/queries/nodes.rs:808](../src/db/queries/nodes.rs#L808).

```rust
    pub async fn get_ambiguous_calls(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AmbiguousCall>> {
        let mut sql = String::from(
            "SELECT from_node_id, reference_name, file_path, line, candidate_node_ids
             FROM ambiguous_calls",
        );
        if let Some(prefix) = path_prefix {
            let exact = prefix.trim_end_matches('/');
            sql.push_str(" WHERE (file_path = ");
            push_quoted(&mut sql, exact);
            sql.push_str(" OR file_path LIKE ");
            push_quoted(
                &mut sql,
                &format!(
                    "{}/%",
                    exact
                        .replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                ),
            );
            sql.push_str(" ESCAPE '\\')");
        }
        let _ = write!(sql, " ORDER BY file_path, line LIMIT {limit}");

        let mut rows = self
            .conn()
            .query(&sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query ambiguous calls: {e}"),
                operation: "get_ambiguous_calls".to_string(),
            })?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read ambiguous call row: {e}"),
            operation: "get_ambiguous_calls".to_string(),
        })? {
            let encoded = get_string_lossy(&row, 4).unwrap_or_default();
            out.push(AmbiguousCall {
                from_node_id: get_string_lossy(&row, 0).unwrap_or_default(),
                reference_name: get_string_lossy(&row, 1).unwrap_or_default(),
                file_path: get_string_lossy(&row, 2).unwrap_or_default(),
                line: row.get::<i64>(3).unwrap_or(0) as u32,
                column: 0,
                candidate_node_ids: serde_json::from_str(&encoded).unwrap_or_default(),
            });
        }
        Ok(out)
    }
```

### `ambiguous_candidate_ids` — src/db/queries/nodes.rs:869

Source: [src/db/queries/nodes.rs:869](../src/db/queries/nodes.rs#L869).

```rust
    pub async fn ambiguous_candidate_ids(&self) -> Result<HashSet<String>> {
        let mut rows = self
            .conn()
            .query("SELECT candidate_node_ids FROM ambiguous_calls", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query ambiguity candidates: {e}"),
                operation: "ambiguous_candidate_ids".to_string(),
            })?;
        let mut out = HashSet::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read ambiguity candidate row: {e}"),
            operation: "ambiguous_candidate_ids".to_string(),
        })? {
            let encoded = get_string_lossy(&row, 0).unwrap_or_default();
            let ids: Vec<String> = serde_json::from_str(&encoded).unwrap_or_default();
            out.extend(ids);
        }
        Ok(out)
    }
```

### `get_ambiguous_calls_naming` — src/db/queries/nodes.rs:894

Source: [src/db/queries/nodes.rs:894](../src/db/queries/nodes.rs#L894).

```rust
    pub async fn get_ambiguous_calls_naming(&self, node_id: &str) -> Result<Vec<AmbiguousCall>> {
        let pattern = format!(
            "%\"{}\"%",
            node_id
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut rows = self
            .conn()
            .query(
                "SELECT from_node_id, reference_name, file_path, line, candidate_node_ids
                 FROM ambiguous_calls
                 WHERE candidate_node_ids LIKE ?1 ESCAPE '\\'
                 ORDER BY file_path, line",
                params![pattern],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query ambiguous calls for a candidate: {e}"),
                operation: "get_ambiguous_calls_naming".to_string(),
            })?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read ambiguous call row: {e}"),
            operation: "get_ambiguous_calls_naming".to_string(),
        })? {
            let encoded = get_string_lossy(&row, 4).unwrap_or_default();
            let candidate_node_ids: Vec<String> =
                serde_json::from_str(&encoded).unwrap_or_default();
            if !candidate_node_ids.iter().any(|id| id == node_id) {
                continue;
            }
            out.push(AmbiguousCall {
                from_node_id: get_string_lossy(&row, 0).unwrap_or_default(),
                reference_name: get_string_lossy(&row, 1).unwrap_or_default(),
                file_path: get_string_lossy(&row, 2).unwrap_or_default(),
                line: row.get::<i64>(3).unwrap_or(0) as u32,
                column: 0,
                candidate_node_ids,
            });
        }
        Ok(out)
    }
```

### `get_node_paths` — src/db/queries/nodes.rs:951

Source: [src/db/queries/nodes.rs:951](../src/db/queries/nodes.rs#L951).

```rust
    pub async fn get_node_paths(&self) -> Result<HashMap<String, String>> {
        let mut rows = self
            .conn()
            .query("SELECT id, file_path FROM nodes", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node paths: {e}"),
                operation: "get_node_paths".to_string(),
            })?;

        let mut map = HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read node path row: {e}"),
            operation: "get_node_paths".to_string(),
        })? {
            let id = get_string_lossy(&row, 0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read node id: {e}"),
                operation: "get_node_paths".to_string(),
            })?;
            let path = get_string_lossy(&row, 1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read node file_path: {e}"),
                operation: "get_node_paths".to_string(),
            })?;
            map.insert(id, path);
        }
        Ok(map)
    }
```

### `get_nodes_filtered` — src/db/queries/nodes.rs:992

Source: [src/db/queries/nodes.rs:992](../src/db/queries/nodes.rs#L992).

```rust
    pub async fn get_nodes_filtered(&self, filter: &NodeFilter) -> Result<Vec<Node>> {
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                start_line, end_line, start_column, end_column,
                docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes{}",
            filter.where_clause()
        );
        let mut rows = self
            .conn()
            .query(&sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query filtered nodes: {e}"),
                operation: "get_nodes_filtered".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_filtered").await
    }
```

### `get_all_nodes_for_resolution` — src/db/queries/nodes.rs:1041

Source: [src/db/queries/nodes.rs:1041](../src/db/queries/nodes.rs#L1041).

```rust
    pub async fn get_all_nodes_for_resolution(&self) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    NULL AS docstring,
                    CASE WHEN file_path LIKE '%.gd'
                              AND kind IN ('class', 'inner_class', 'function', 'method', 'field')
                         THEN signature
                         WHEN file_path LIKE '%.cs'
                              AND kind IN ('class', 'inner_class', 'struct', 'interface', 'record',
                                           'function', 'method', 'field', 'csharp_property')
                         THEN signature END AS signature,
                    visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all nodes for resolution: {e}"),
                operation: "get_all_nodes_for_resolution".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_all_nodes_for_resolution").await
    }
```

### `touched_nodes_by_file` — src/db/queries/nodes.rs:1076

Source: [src/db/queries/nodes.rs:1076](../src/db/queries/nodes.rs#L1076).

```rust
    pub async fn touched_nodes_by_file(&self, file_path: &str) -> Result<Vec<TouchedNode>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT kind, name, qualified_name FROM nodes WHERE file_path = ?1",
                params![file_path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node names: {e}"),
                operation: "touched_nodes_by_file".to_string(),
            })?;

        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let kind = NodeKind::from_str(&row.get::<String>(0).unwrap_or_default())
                .unwrap_or(NodeKind::Function);
            out.push(TouchedNode {
                kind,
                name: row.get::<String>(1).unwrap_or_default(),
                qualified_name: row.get::<String>(2).unwrap_or_default(),
            });
        }
        Ok(out)
    }
```

### `delete_nodes_by_file` — src/db/queries/nodes.rs:1103

Source: [src/db/queries/nodes.rs:1103](../src/db/queries/nodes.rs#L1103).

```rust
    pub async fn delete_nodes_by_file(&self, file_path: &str) -> Result<()> {
        debug_assert!(
            !file_path.is_empty(),
            "delete_nodes_by_file called with empty file_path"
        );
        debug_assert!(
            !file_path.starts_with('/'),
            "delete_nodes_by_file expects relative path, got absolute"
        );
        // Serialize the whole delete against other write transactions on the
        // shared connection: a concurrent edit reindex or background sync can
        // otherwise open a second transaction while this one is active, which
        // SQLite rejects as "cannot start a transaction within a transaction"
        // (#563).
        let _write_guard = self.write_lock.lock().await;
        self.conn()
            .execute(
                "DELETE FROM executable_body_fts WHERE file_path = ?1",
                params![file_path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete executable body documents: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

        // Gather node IDs for the file first.
        let node_ids: Vec<String> = {
            let mut rows = self
                .conn()
                .query(
                    "SELECT id FROM nodes WHERE file_path = ?1",
                    params![file_path],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query node ids: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?;

            let mut ids = Vec::new();
            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read node id: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })? {
                ids.push(row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read node id value: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?);
            }
            ids
        };

        if node_ids.is_empty() {
            return Ok(());
        }

        let tx = self
            .conn()
            .transaction()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

        for id in &node_ids {
            tx.execute(
                "DELETE FROM edges WHERE source = ?1 OR target = ?1",
                params![id.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete edges: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

            tx.execute(
                "DELETE FROM unresolved_refs WHERE from_node_id = ?1",
                params![id.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete unresolved refs: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

            tx.execute(
                "DELETE FROM vectors WHERE node_id = ?1",
                params![id.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete vectors: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;
        }

        tx.execute("DELETE FROM nodes WHERE file_path = ?1", params![file_path])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete nodes: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

        tx.commit().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "delete_nodes_by_file".to_string(),
        })
    }
```

### `rebuild_rails_routes_inner` — src/db/queries/routes.rs:255

Source: [src/db/queries/routes.rs:255](../src/db/queries/routes.rs#L255).

```rust
    async fn rebuild_rails_routes_inner(&self, start: std::time::Instant) -> Result<()> {
        let custom_inflections = !self
            .rails_evidence(RUBY_CUSTOM_INFLECTIONS_REFERENCE)
            .await?
            .is_empty();
        // By source, so the delete uses the edges source index.
        self.conn()
            .execute(
                "DELETE FROM edges WHERE resolved_by = ?1
                    AND source IN (SELECT id FROM nodes WHERE kind = 'route')",
                params![ResolvedBy::RailsRoute.code()],
            )
            .await
            .map_err(|error| route_error(&error))?;
        let routes = self.route_rows().await?;
        let contexts = self.route_contexts().await?;
        let mut status_by_route: Vec<(&RouteRow, RouteStatus<'_>)> = Vec::new();
        for route in &routes {
            let context = contexts.resolve(&route.file, route.line, &mut Vec::new());
            let resolved = match (parse_route_target(&route.target), context) {
                (None, _) => continue,
                (_, Err(status)) => Err(status),
                (Some(_), Ok(_)) if custom_inflections => Err("custom-inflections"),
                (Some(target), Ok(context)) if target.unless_api_only && context.api_only => {
                    Err("api-only")
                }
                (Some(target), Ok(context)) => Ok((
                    if target.absolute || context.module.is_empty() {
                        target.constant.to_string()
                    } else {
                        format!("{}::{}", camelize(&context.module), target.constant)
                    },
                    target.action.to_string(),
                )),
            };
            status_by_route.push((route, resolved));
        }
        let constants: BTreeSet<&str> = status_by_route
            .iter()
            .filter_map(|(_, r)| r.as_ref().ok().map(|(c, _)| c.as_str()))
            .collect();
        let owners = self.ruby_owners(&constants).await?;
        let owner_ids: Vec<String> = owners
            .values()
            .flatten()
            .map(|(id, _)| id.clone())
            .collect();
        let methods = self.owner_methods(&owner_ids).await?;
        let hidden = self.owner_visibility_evidence(&owner_ids).await?;

        let mut counts = serde_json::Map::new();
        for (route, resolved) in status_by_route {
            let status = match resolved {
                Err(status) => status,
                Ok((constant, action)) => {
                    let mut candidates = ActionCandidates::default();
                    for (owner, kind) in owners.get(&constant).into_iter().flatten() {
                        candidates.owner_found = true;
                        candidates.conflicting_kind |= kind == "module";
                        candidates.unresolved_visibility |= hidden
                            .get(owner)
                            .is_some_and(|names| names.contains(&action) || names.contains("*"));
                        for (id, name, kind, visibility) in methods.get(owner).into_iter().flatten()
                        {
                            if *name == action {
                                candidates.targets.push((
                                    id.clone(),
                                    kind.clone(),
                                    visibility.clone(),
                                ));
                            }
                        }
                    }
                    let ActionCandidates {
                        owner_found,
                        conflicting_kind,
                        unresolved_visibility,
                        targets,
                    } = candidates;
                    if !owner_found {
                        "missing-controller"
                    } else if conflicting_kind || targets.len() > 1 {
                        "ambiguous-action"
                    } else if targets.is_empty() {
                        "missing-action"
                    } else if targets[0].1 != "method" || targets[0].2 != "public" {
                        "non-public-instance-action"
                    } else if unresolved_visibility {
                        "unresolved-visibility"
                    } else {
                        self.conn().execute("INSERT INTO edges(source,target,kind,line,resolved_by) VALUES (?1,?2,'calls',?3,?4)", params![route.id.as_str(), targets[0].0.as_str(), i64::from(route.line), ResolvedBy::RailsRoute.code()]).await.map_err(|error| route_error(&error))?;
                        "resolved"
                    }
                }
            };
            let count = counts.entry(status).or_insert(serde_json::json!(0));
            *count = serde_json::json!(count.as_u64().unwrap_or(0) + 1);
        }
        let counts = serde_json::to_string(&counts).map_err(|e| TokenSaveError::Config {
            message: e.to_string(),
        })?;
        self.set_metadata("rails_route_counts", &counts).await?;
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO metadata(key,value) VALUES ('rails_routes_pending','0')",
                (),
            )
            .await
            .map_err(|error| route_error(&error))?;

        self.set_metadata(
            "rails_route_duration_ms",
            &start.elapsed().as_millis().to_string(),
        )
        .await?;
        Ok(())
    }
```

### `ruby_owners` — src/db/queries/routes.rs:605

Source: [src/db/queries/routes.rs:605](../src/db/queries/routes.rs#L605).

```rust
    async fn ruby_owners(
        &self,
        constants: &BTreeSet<&str>,
    ) -> Result<HashMap<String, Vec<(String, String)>>> {
        let mut names = BTreeSet::new();
        for constant in constants {
            let segments: Vec<&str> = constant.split("::").collect();
            for start in 0..segments.len() {
                names.insert(segments[start..].join("::"));
            }
        }
        let names: Vec<String> = names.into_iter().collect();
        let mut owners: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for chunk in names.chunks(BATCH) {
            let sql = format!(
                "SELECT id, kind, qualified_name, file_path FROM nodes
                 WHERE name IN ({}) AND kind IN ('class','module')",
                build_qmark_placeholders(chunk.len())
            );
            let mut rows = self
                .conn()
                .query(
                    &sql,
                    libsql::params_from_iter(chunk.iter().cloned().map(text)),
                )
                .await
                .map_err(|error| route_error(&error))?;
            while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
                let id: String = row.get(0).map_err(|error| route_error(&error))?;
                let kind: String = row.get(1).map_err(|error| route_error(&error))?;
                let qualified_name: String = row.get(2).map_err(|error| route_error(&error))?;
                let file_path: String = row.get(3).map_err(|error| route_error(&error))?;
                if !is_ruby_file(&file_path) {
                    continue;
                }
                if let Some(constant) = ruby_constant(&qualified_name, &file_path) {
                    if constants.contains(constant.as_str()) {
                        owners.entry(constant).or_default().push((id, kind));
                    }
                }
            }
        }
        for list in owners.values_mut() {
            list.sort();
        }
        Ok(owners)
    }
```

### `owner_methods` — src/db/queries/routes.rs:654

Source: [src/db/queries/routes.rs:654](../src/db/queries/routes.rs#L654).

```rust
    async fn owner_methods(
        &self,
        owner_ids: &[String],
    ) -> Result<HashMap<String, Vec<(String, String, String, String)>>> {
        let mut methods: HashMap<String, Vec<(String, String, String, String)>> = HashMap::new();
        for chunk in owner_ids.chunks(BATCH) {
            let sql = format!(
                "SELECT parent_id, id, name, kind, visibility FROM nodes
                 WHERE parent_id IN ({}) AND kind IN ('method','singleton_method')",
                build_qmark_placeholders(chunk.len())
            );
            let mut rows = self
                .conn()
                .query(
                    &sql,
                    libsql::params_from_iter(chunk.iter().cloned().map(text)),
                )
                .await
                .map_err(|error| route_error(&error))?;
            while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
                let parent: String = row.get(0).map_err(|error| route_error(&error))?;
                methods.entry(parent).or_default().push((
                    row.get(1).map_err(|error| route_error(&error))?,
                    row.get(2).map_err(|error| route_error(&error))?,
                    row.get(3).map_err(|error| route_error(&error))?,
                    row.get(4).map_err(|error| route_error(&error))?,
                ));
            }
        }
        for list in methods.values_mut() {
            list.sort();
        }
        Ok(methods)
    }
```

### `owner_visibility_evidence` — src/db/queries/routes.rs:690

Source: [src/db/queries/routes.rs:690](../src/db/queries/routes.rs#L690).

```rust
    async fn owner_visibility_evidence(
        &self,
        owner_ids: &[String],
    ) -> Result<HashMap<String, HashSet<String>>> {
        let mut hidden: HashMap<String, HashSet<String>> = HashMap::new();
        for chunk in owner_ids.chunks(BATCH) {
            let sql = format!(
                "SELECT from_node_id, reference_name FROM unresolved_refs
                 WHERE from_node_id IN ({}) AND reference_kind = 'uses'
                    AND reference_name >= ? AND reference_name < ?",
                build_qmark_placeholders(chunk.len())
            );
            let params = chunk.iter().cloned().map(text).chain([
                text(RUBY_VISIBILITY_REFERENCE_PREFIX.into()),
                text(prefix_end(RUBY_VISIBILITY_REFERENCE_PREFIX)),
            ]);
            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(params))
                .await
                .map_err(|error| route_error(&error))?;
            while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
                let owner: String = row.get(0).map_err(|error| route_error(&error))?;
                let name: String = row.get(1).map_err(|error| route_error(&error))?;
                if let Some(name) = name.strip_prefix(RUBY_VISIBILITY_REFERENCE_PREFIX) {
                    hidden.entry(owner).or_default().insert(name.to_string());
                }
            }
        }
        Ok(hidden)
    }
```

### `route_invalidation_waits_for_concurrent_transaction` — src/db/queries/routes.rs:730

Source: [src/db/queries/routes.rs:730](../src/db/queries/routes.rs#L730).

```rust
    async fn route_invalidation_waits_for_concurrent_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("routes.db");
        let (db, _) = Database::initialize(&path).await.unwrap();
        db.set_metadata("rails_routes_pending", "0").await.unwrap();
        db.set_metadata("rails_route_counts", r#"{"resolved":1}"#)
            .await
            .unwrap();
        let guard = db.write_lock.lock().await;
        db.conn().execute("BEGIN", ()).await.unwrap();
        let mut writer = Box::pin(db.invalidate_rails_routes());
        let attempted = tokio::time::timeout(Duration::from_millis(50), writer.as_mut()).await;
        db.conn().execute("ROLLBACK", ()).await.unwrap();
        drop(guard);
        assert!(
            attempted.is_err(),
            "invalidation wrote inside another task's transaction"
        );
        writer.await.unwrap();
        drop(db);
        let (db, _) = Database::open(&path).await.unwrap();
        assert_eq!(
            db.get_metadata("rails_routes_pending")
                .await
                .unwrap()
                .as_deref(),
            Some("1")
        );
    }
```

### `route_rebuild_checks_pending_after_waiting_for_write_transaction` — src/db/queries/routes.rs:761

Source: [src/db/queries/routes.rs:761](../src/db/queries/routes.rs#L761).

```rust
    async fn route_rebuild_checks_pending_after_waiting_for_write_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let (db, _) = Database::initialize(&dir.path().join("routes.db"))
            .await
            .unwrap();
        db.set_metadata("rails_routes_pending", "1").await.unwrap();
        let guard = db.write_lock.lock().await;
        db.conn().execute("BEGIN", ()).await.unwrap();
        db.set_metadata("rails_routes_pending", "0").await.unwrap();
        let mut rebuild = Box::pin(db.rebuild_rails_routes());
        let attempted = tokio::time::timeout(Duration::from_millis(50), rebuild.as_mut()).await;
        db.conn().execute("ROLLBACK", ()).await.unwrap();
        drop(guard);
        assert!(
            attempted.is_err(),
            "rebuild used an uncommitted pending flag"
        );
        rebuild.await.unwrap();
        assert_eq!(
            db.get_metadata("rails_routes_pending")
                .await
                .unwrap()
                .as_deref(),
            Some("0")
        );
        assert_eq!(
            db.get_metadata("rails_route_counts")
                .await
                .unwrap()
                .as_deref(),
            Some("{}")
        );
    }
```

### `insert_executable_body_documents` — src/db/queries/search.rs:51

Source: [src/db/queries/search.rs:51](../src/db/queries/search.rs#L51).

```rust
    pub async fn insert_executable_body_documents(
        &self,
        documents: &[ExecutableBodyDocument],
    ) -> Result<()> {
        if documents.is_empty() {
            return Ok(());
        }
        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin executable body insert: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
        let stmt = self
            .conn()
            .prepare(
                "INSERT INTO executable_body_fts (node_id, file_path, body) VALUES (?1, ?2, ?3)",
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare executable body insert: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
        for document in documents {
            stmt.execute(params![
                document.node_id.as_str(),
                document.file_path.as_str(),
                document.body.as_str(),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert executable body: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
            stmt.reset();
        }
        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit executable body insert: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
        Ok(())
    }
```

### `search_executable_bodies` — src/db/queries/search.rs:102

Source: [src/db/queries/search.rs:102](../src/db/queries/search.rs#L102).

```rust
    pub async fn search_executable_bodies(
        &self,
        terms: &[String],
        candidate_limit: usize,
    ) -> Result<Vec<(Node, usize)>> {
        if terms.len() < 2 || candidate_limit == 0 {
            return Ok(Vec::new());
        }
        let mut seen = std::collections::HashSet::new();
        let prefixes: Vec<String> = terms
            .iter()
            .filter_map(|term| {
                let prefix = term
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(5)
                    .collect::<String>()
                    .to_lowercase();
                (prefix.len() >= 4 && seen.insert(prefix.clone())).then_some(prefix)
            })
            .collect();
        if prefixes.len() < 2 {
            return Ok(Vec::new());
        }

        // Ask FTS for documents containing any pair of distinct concepts. This
        // guarantees useful co-occurrence candidates while letting LIMIT stop
        // the index walk early; a BM25 ORDER BY would rank every matching body
        // in a large project before applying the bound.
        let fts_query = (0..prefixes.len())
            .flat_map(|left| {
                let prefixes = &prefixes;
                ((left + 1)..prefixes.len()).map(move |right| {
                    format!("(\"{}\"* AND \"{}\"*)", prefixes[left], prefixes[right])
                })
            })
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut rows = self
            .conn()
            .query(
                "SELECT node_id, body FROM executable_body_fts
                 WHERE executable_body_fts MATCH ?1
                 LIMIT ?2",
                params![fts_query, candidate_limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to search executable bodies: {e}"),
                operation: "search_executable_bodies".to_string(),
            })?;
        let mut hits = std::collections::HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read executable body hit: {e}"),
            operation: "search_executable_bodies".to_string(),
        })? {
            let node_id: String = row.get(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read executable body node id: {e}"),
                operation: "search_executable_bodies".to_string(),
            })?;
            let body: String = row.get(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read executable body hit: {e}"),
                operation: "search_executable_bodies".to_string(),
            })?;
            let body = body.to_lowercase();
            let matched = prefixes
                .iter()
                .filter(|prefix| {
                    body.split(|c: char| !c.is_alphanumeric())
                        .any(|token| token.starts_with(prefix.as_str()))
                })
                .count();
            if matched >= 2 {
                hits.insert(node_id, matched);
            }
        }

        let ids: Vec<String> = hits.keys().cloned().collect();
        let mut nodes: Vec<(Node, usize)> = self
            .get_nodes_by_ids(&ids)
            .await?
            .into_iter()
            .filter_map(|node| hits.get(&node.id).copied().map(|count| (node, count)))
            .collect();
        nodes.sort_by(|(a, a_hits), (b, b_hits)| {
            b_hits.cmp(a_hits).then_with(|| {
                a.end_line
                    .saturating_sub(a.start_line)
                    .cmp(&b.end_line.saturating_sub(b.start_line))
            })
        });
        Ok(nodes)
    }
```

### `search_nodes` — src/db/queries/search.rs:220

Source: [src/db/queries/search.rs:220](../src/db/queries/search.rs#L220).

```rust
    pub async fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        debug_assert!(!query.is_empty(), "search_nodes called with empty query");
        debug_assert!(limit > 0, "search_nodes limit must be positive");
        let Some(fts_query) = to_fts_match_query(query) else {
            return Ok(Vec::new());
        };

        // Exact-phrase channel. A caller who types a quoted-sounding phrase is
        // giving the most specific evidence they have; it must not lose to
        // symbols that merely share one common word (#362). Best-effort: any
        // error here falls through to the ordinary path below.
        let phrase_hits: Vec<SearchResult> = match to_fts_phrase_query(query) {
            Some(phrase) => self
                .search_nodes_fts(&phrase, limit)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let merge = |general: Vec<SearchResult>| -> Vec<SearchResult> {
            if phrase_hits.is_empty() {
                return general;
            }
            let seen: HashSet<String> = phrase_hits.iter().map(|r| r.node.id.clone()).collect();
            let mut merged = phrase_hits.clone();
            merged.extend(general.into_iter().filter(|r| !seen.contains(&r.node.id)));
            merged.truncate(limit);
            merged
        };

        // Try FTS search, with one self-healing retry on corruption.
        let fts_result = self.search_nodes_fts(&fts_query, limit).await;
        match fts_result {
            Ok(results) if !results.is_empty() => return Ok(merge(results)),
            Ok(_) => {} // empty — fall through to LIKE
            Err(ref e) if Self::is_corruption_error(e) => {
                eprintln!("[tokensave] FTS index corruption detected — rebuilding…");
                if self.rebuild_fts().await.is_ok() {
                    match self.search_nodes_fts(&fts_query, limit).await {
                        Ok(results) if !results.is_empty() => return Ok(merge(results)),
                        Ok(_) => {} // fall through to LIKE
                        Err(e) => return Err(e),
                    }
                }
                // rebuild_fts failed — fall through to LIKE as last resort
            }
            Err(e) => return Err(e),
        }

        // Fallback: LIKE query
        let like_pattern = format!("%{}%", escape_like(query));
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
                 WHERE name LIKE ?1 ESCAPE '\\' OR qualified_name LIKE ?1 ESCAPE '\\' OR docstring LIKE ?1 ESCAPE '\\' OR signature LIKE ?1 ESCAPE '\\'
                 LIMIT ?2",
                params![like_pattern.as_str(), limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to execute LIKE query: {e}"),
                operation: "search_nodes".to_string(),
            })?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read search result: {e}"),
            operation: "search_nodes".to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map search result: {e}"),
                operation: "search_nodes".to_string(),
            })?;
            results.push(SearchResult { node, score: 1.0 });
        }
        Ok(merge(results))
    }
```

### `search_nodes_bounded_fts` — src/db/queries/search.rs:343

Source: [src/db/queries/search.rs:343](../src/db/queries/search.rs#L343).

```rust
    async fn search_nodes_bounded_fts(
        &self,
        fts_query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0) AS rank
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.rowid
                 WHERE nodes_fts MATCH ?1
                   AND n.kind NOT IN ('file', 'doc')
                 ORDER BY rank
                 LIMIT ?2",
                params![fts_query, limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to execute bounded FTS query: {e}"),
                operation: "search_nodes_bounded".to_string(),
            })?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read bounded search result: {e}"),
            operation: "search_nodes_bounded".to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map bounded search result: {e}"),
                operation: "search_nodes_bounded".to_string(),
            })?;
            let rank: f64 = row.get::<f64>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read bounded rank: {e}"),
                operation: "search_nodes_bounded".to_string(),
            })?;
            results.push(SearchResult { node, score: -rank });
        }
        Ok(results)
    }
```

### `search_nodes_fts` — src/db/queries/search.rs:388

Source: [src/db/queries/search.rs:388](../src/db/queries/search.rs#L388).

```rust
    pub(crate) async fn search_nodes_fts(
        &self,
        fts_query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0) AS rank
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.rowid
                 WHERE nodes_fts MATCH ?1
                 ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0)
                 LIMIT ?2",
                params![fts_query, limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to execute FTS query: {e}"),
                operation: "search_nodes".to_string(),
            })?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read search result: {e}"),
            operation: "search_nodes".to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map search result: {e}"),
                operation: "search_nodes".to_string(),
            })?;
            let rank: f64 = row.get::<f64>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read rank: {e}"),
                operation: "search_nodes".to_string(),
            })?;
            results.push(SearchResult { node, score: -rank });
        }
        Ok(results)
    }
```

### `batch_incoming_call_counts` — src/db/queries/search.rs:433

Source: [src/db/queries/search.rs:433](../src/db/queries/search.rs#L433).

```rust
    pub async fn batch_incoming_call_counts(
        &self,
        node_ids: &[String],
    ) -> Result<std::collections::HashMap<String, u64>> {
        let mut counts = std::collections::HashMap::new();
        if node_ids.is_empty() {
            return Ok(counts);
        }
        let placeholders = build_qmark_placeholders(node_ids.len());
        let sql = format!(
            "SELECT target, COUNT(*) AS cnt FROM edges WHERE target IN ({placeholders}) AND kind = 'calls' GROUP BY target",
        );
        let param_values: Vec<libsql::Value> = node_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to batch count incoming calls: {e}"),
                operation: "batch_incoming_call_counts".to_string(),
            })?;
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read batch call count row: {e}"),
            operation: "batch_incoming_call_counts".to_string(),
        })? {
            let id: String = row.get(0).unwrap_or_default();
            let cnt: u64 = row.get::<u64>(1).unwrap_or(0);
            counts.insert(id, cnt);
        }
        Ok(counts)
    }
```

### `search_nodes_by_exact_name` — src/db/queries/search.rs:471

Source: [src/db/queries/search.rs:471](../src/db/queries/search.rs#L471).

```rust
    pub async fn search_nodes_by_exact_name(
        &self,
        names: &[String],
        limit: usize,
    ) -> Result<Vec<Node>> {
        if names.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let placeholders = build_qmark_placeholders(names.len());
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async,
                    branches, loops, returns, max_nesting,
                    unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes
             WHERE LOWER(name) IN ({placeholders})
             LIMIT ?",
        );
        let mut param_values: Vec<libsql::Value> = names
            .iter()
            .map(|n| libsql::Value::Text(n.to_lowercase()))
            .collect();
        param_values.push(libsql::Value::Integer(limit as i64));

        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to search by exact name: {e}"),
                operation: "search_nodes_by_exact_name".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "search_nodes_by_exact_name").await
    }
```

### `get_stats_with_languages` — src/db/queries/stats.rs:15

Source: [src/db/queries/stats.rs:15](../src/db/queries/stats.rs#L15).

```rust
    pub(crate) async fn get_stats_with_languages(
        &self,
        language_override: impl Fn(&str) -> Option<String>,
    ) -> Result<GraphStats> {
        // Single query for all scalar counts: nodes, edges, files, last_updated, total_source_bytes
        let mut counts_rows = self
            .conn()
            .query(
                "SELECT \
                   (SELECT COUNT(*) FROM nodes), \
                   (SELECT COUNT(*) FROM edges), \
                   (SELECT COUNT(*) FROM files), \
                   (SELECT COALESCE(MAX(indexed_at), 0) FROM files), \
                   (SELECT COALESCE(SUM(size), 0) FROM files)",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query counts: {e}"),
                operation: "get_stats".to_string(),
            })?;
        let counts_row = counts_rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read counts row: {e}"),
                operation: "get_stats".to_string(),
            })?;
        let (node_count, edge_count, file_count, last_updated, total_source_bytes) =
            match counts_row {
                Some(r) => {
                    let nc: i64 = r.get(0).unwrap_or(0);
                    let ec: i64 = r.get(1).unwrap_or(0);
                    let fc: i64 = r.get(2).unwrap_or(0);
                    let lu: i64 = r.get(3).unwrap_or(0);
                    let ts: i64 = r.get(4).unwrap_or(0);
                    (nc as u64, ec as u64, fc as u64, lu as u64, ts as u64)
                }
                None => (0, 0, 0, 0, 0),
            };

        // Nodes grouped by kind
        let nodes_by_kind = query_kind_counts(
            self.conn(),
            "SELECT kind, COUNT(*) FROM nodes GROUP BY kind",
        )
        .await?;

        // Edges grouped by kind
        let edges_by_kind = query_kind_counts(
            self.conn(),
            "SELECT kind, COUNT(*) FROM edges GROUP BY kind",
        )
        .await?;

        let db_size_bytes = self.size().await.unwrap_or(0);

        // Files grouped by language. Done in Rust (not SQL) so the label set
        // stays in sync with the extractor registry without an ever-growing
        // CASE expression. See `display_language_for_path`.
        let files_by_language = {
            let mut rows = self
                .conn()
                .query("SELECT path FROM files", ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query files for language stats: {e}"),
                    operation: "get_stats".to_string(),
                })?;
            let mut map: HashMap<String, u64> = HashMap::new();
            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read file row: {e}"),
                operation: "get_stats".to_string(),
            })? {
                let path: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read file path: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                *map.entry(
                    language_override(&path)
                        .unwrap_or_else(|| display_language_for_path(&path).to_string()),
                )
                .or_insert(0) += 1;
            }
            map
        };

        let last_sync_at = self
            .get_metadata("last_sync_at")
            .await?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let last_full_sync_at = self
            .get_metadata("last_full_sync_at")
            .await?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let last_full_index_version = self
            .get_metadata("last_full_index_version")
            .await?
            .unwrap_or_default();
        let last_sync_duration_ms = self
            .get_metadata("last_sync_duration_ms")
            .await?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        Ok(GraphStats {
            node_count,
            edge_count,
            file_count,
            nodes_by_kind,
            edges_by_kind,
            db_size_bytes,
            last_updated,
            total_source_bytes,
            files_by_language,
            last_sync_at,
            last_full_sync_at,
            last_full_index_version,
            last_sync_duration_ms,
        })
    }
```

### `get_gdscript_typed_refs` — src/db/queries/unresolved.rs:13

Source: [src/db/queries/unresolved.rs:13](../src/db/queries/unresolved.rs#L13).

```rust
    pub async fn get_gdscript_typed_refs(&self) -> Result<Vec<UnresolvedRef>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT from_node_id, reference_name, line, col, file_path
                 FROM unresolved_refs
                 WHERE reference_kind = 'calls'
                   AND (file_path LIKE '%.gd' OR file_path LIKE '%.cs')
                   AND instr(reference_name, '::') > 0",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query typed-receiver refs: {e}"),
                operation: "get_gdscript_typed_refs".to_string(),
            })?;
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            out.push(UnresolvedRef {
                from_node_id: row.get::<String>(0).unwrap_or_default(),
                reference_name: row.get::<String>(1).unwrap_or_default(),
                reference_kind: EdgeKind::Calls,
                line: row.get::<u32>(2).unwrap_or(0),
                column: row.get::<u32>(3).unwrap_or(0),
                file_path: row.get::<String>(4).unwrap_or_default(),
            });
        }
        Ok(out)
    }
```

### `insert_unresolved_ref` — src/db/queries/unresolved.rs:44

Source: [src/db/queries/unresolved.rs:44](../src/db/queries/unresolved.rs#L44).

```rust
    pub async fn insert_unresolved_ref(&self, uref: &UnresolvedRef) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO unresolved_refs
                (from_node_id, reference_name, reference_kind, line, col, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    uref.from_node_id.as_str(),
                    uref.reference_name.as_str(),
                    uref.reference_kind.as_str(),
                    i64::from(uref.line),
                    i64::from(uref.column),
                    uref.file_path.as_str(),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert unresolved ref: {e}"),
                operation: "insert_unresolved_ref".to_string(),
            })?;
        Ok(())
    }
```

### `insert_unresolved_refs` — src/db/queries/unresolved.rs:68

Source: [src/db/queries/unresolved.rs:68](../src/db/queries/unresolved.rs#L68).

```rust
    pub async fn insert_unresolved_refs(&self, refs: &[UnresolvedRef]) -> Result<()> {
        if refs.is_empty() {
            return Ok(());
        }

        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;

        let stmt = self.conn()
            .prepare("INSERT INTO unresolved_refs (from_node_id,reference_name,reference_kind,line,col,file_path) VALUES (?1,?2,?3,?4,?5,?6)")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;

        for uref in refs {
            stmt.execute(params![
                uref.from_node_id.as_str(),
                uref.reference_name.as_str(),
                uref.reference_kind.as_str(),
                i64::from(uref.line),
                i64::from(uref.column),
                uref.file_path.as_str(),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert unresolved ref: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;
            stmt.reset();
        }

        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;
        Ok(())
    }
```

### `get_unresolved_refs` — src/db/queries/unresolved.rs:117

Source: [src/db/queries/unresolved.rs:117](../src/db/queries/unresolved.rs#L117).

```rust
    pub async fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query unresolved refs: {e}"),
                operation: "get_unresolved_refs".to_string(),
            })?;

        collect_rows(&mut rows, row_to_unresolved_ref, "get_unresolved_refs").await
    }
```

### `count_unresolved_refs` — src/db/queries/unresolved.rs:138

Source: [src/db/queries/unresolved.rs:138](../src/db/queries/unresolved.rs#L138).

```rust
    pub async fn count_unresolved_refs(&self) -> Result<usize> {
        let mut rows = self
            .conn()
            .query("SELECT COUNT(*) FROM unresolved_refs", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to count unresolved refs: {e}"),
                operation: "count_unresolved_refs".to_string(),
            })?;
        let count = rows
            .next()
            .await
            .ok()
            .flatten()
            .and_then(|r| r.get::<i64>(0).ok())
            .unwrap_or(0);
        Ok(usize::try_from(count).unwrap_or(0))
    }
```

### `get_unresolved_refs_after` — src/db/queries/unresolved.rs:171

Source: [src/db/queries/unresolved.rs:171](../src/db/queries/unresolved.rs#L171).

```rust
    pub async fn get_unresolved_refs_after(
        &self,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<(i64, UnresolvedRef)>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs
                 WHERE id > ?1
                 ORDER BY id
                 LIMIT ?2",
                params![after_id, i64::try_from(limit).unwrap_or(i64::MAX)],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to page unresolved refs: {e}"),
                operation: "get_unresolved_refs_after".to_string(),
            })?;

        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let Ok(id) = row.get::<i64>(0) else { continue };
            // Column 0 is the id, so the shared mapper's offsets are shifted;
            // build the record here rather than teaching it two layouts.
            out.push((
                id,
                UnresolvedRef {
                    from_node_id: row.get::<String>(1).unwrap_or_default(),
                    reference_name: row.get::<String>(2).unwrap_or_default(),
                    reference_kind: EdgeKind::from_str(&row.get::<String>(3).unwrap_or_default())
                        .unwrap_or(EdgeKind::Calls),
                    line: row.get::<u32>(4).unwrap_or(0),
                    column: row.get::<u32>(5).unwrap_or(0),
                    file_path: row.get::<String>(6).unwrap_or_default(),
                },
            ));
        }
        Ok(out)
    }
```

### `get_unresolved_refs_for_sources` — src/db/queries/unresolved.rs:220

Source: [src/db/queries/unresolved.rs:220](../src/db/queries/unresolved.rs#L220).

```rust
    pub async fn get_unresolved_refs_for_sources(
        &self,
        source_ids: &[String],
    ) -> Result<Vec<UnresolvedRef>> {
        let mut out = Vec::new();
        for chunk in source_ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs WHERE from_node_id IN ({})",
                placeholders.join(", ")
            );
            let values: Vec<libsql::Value> = chunk
                .iter()
                .map(|id| libsql::Value::Text(id.clone()))
                .collect();
            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query unresolved refs by source: {e}"),
                    operation: "get_unresolved_refs_for_sources".to_string(),
                })?;
            out.extend(
                collect_rows(
                    &mut rows,
                    row_to_unresolved_ref,
                    "get_unresolved_refs_for_sources",
                )
                .await?,
            );
        }
        Ok(out)
    }
```

### `clear_unresolved_refs` — src/db/queries/unresolved.rs:257

Source: [src/db/queries/unresolved.rs:257](../src/db/queries/unresolved.rs#L257).

```rust
    pub async fn clear_unresolved_refs(&self) -> Result<()> {
        self.conn()
            .execute("DELETE FROM unresolved_refs", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to clear unresolved refs: {e}"),
                operation: "clear_unresolved_refs".to_string(),
            })?;
        Ok(())
    }
```

### `check_database` — src/doctor.rs:77

Source: [src/doctor.rs:77](../src/doctor.rs#L77).

```rust
async fn check_database(dc: &mut DoctorCounters, project_path: &Path) {
    let db_path = crate::config::get_tokensave_dir(project_path).join("tokensave.db");
    let size_before = std::fs::metadata(&db_path).map_or(0, |m| m.len());

    let ts = match TokenSave::open(project_path).await {
        Ok(ts) => ts,
        Err(e) => {
            dc.fail(&format!("Could not open database: {e}"));
            return;
        }
    };

    dc.pass(&format!("DB size: {}", format_bytes(size_before)));

    eprintln!("    Compacting database (VACUUM)…");
    match ts.optimize().await {
        Ok(()) => {
            let size_after = std::fs::metadata(&db_path).map_or(size_before, |m| m.len());
            if size_before > size_after {
                let reclaimed = size_before - size_after;
                dc.pass(&format!(
                    "Compacted: {} → {} (reclaimed {})",
                    format_bytes(size_before),
                    format_bytes(size_after),
                    format_bytes(reclaimed),
                ));
            } else {
                dc.pass("Database already compact");
            }
        }
        Err(e) => {
            dc.warn(&format!("VACUUM failed: {e}"));
        }
    }
}
```

### `audit` — src/edge_audit.rs:106

Source: [src/edge_audit.rs:106](../src/edge_audit.rs#L106).

```rust
pub async fn audit(db: &Database, limit: usize) -> Result<EdgeAudit> {
    // Import evidence per file. Imports are graph nodes of kind `use`, which
    // is what lets the gate's predicate be evaluated after the fact rather
    // than only during resolution.
    let mut imports: HashMap<String, HashSet<String>> = HashMap::new();
    let mut rows = query_rows(db, "SELECT file_path, name FROM nodes WHERE kind = 'use'").await?;
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("edge audit read failed: {e}"),
        operation: "edge_audit".to_string(),
    })? {
        let file: String = row.get(0).unwrap_or_default();
        let name: String = row.get(1).unwrap_or_default();
        imports.entry(file).or_default().insert(name);
    }

    // How many symbols share each name, so "sole candidate" is a lookup.
    let mut name_counts: HashMap<String, i64> = HashMap::new();
    let mut rows = query_rows(
        db,
        "SELECT name, COUNT(*) FROM nodes WHERE kind <> 'file' AND kind <> 'use' GROUP BY name",
    )
    .await?;
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("edge audit read failed: {e}"),
        operation: "edge_audit".to_string(),
    })? {
        name_counts.insert(
            row.get(0).unwrap_or_default(),
            row.get(1).unwrap_or_default(),
        );
    }

    // Parent names, for the "the class that owns the method is the evidence"
    // arm of the resolver's predicate.
    let mut node_name: HashMap<String, String> = HashMap::new();
    let mut rows = query_rows(db, "SELECT id, name FROM nodes").await?;
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("edge audit read failed: {e}"),
        operation: "edge_audit".to_string(),
    })? {
        node_name.insert(
            row.get(0).unwrap_or_default(),
            row.get(1).unwrap_or_default(),
        );
    }

    let mut total_edges = 0i64;
    let mut gated_edges = 0i64;
    let mut cross_file = 0i64;
    let mut sole_candidate_cross_file = 0i64;
    let mut unreachable = 0i64;
    let mut per_target: HashMap<String, (EdgeRow, i64, HashSet<String>)> = HashMap::new();

    let mut rows = query_rows(
        db,
        "SELECT s.file_path, n.id, n.name, n.file_path, n.kind, n.start_line, n.parent_id \
         FROM edges e \
         JOIN nodes n ON n.id = e.target \
         JOIN nodes s ON s.id = e.source",
    )
    .await?;

    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("edge audit read failed: {e}"),
        operation: "edge_audit".to_string(),
    })? {
        total_edges += 1;
        let r = EdgeRow {
            src_file: row.get(0).unwrap_or_default(),
            dst_id: row.get(1).unwrap_or_default(),
            dst_name: row.get(2).unwrap_or_default(),
            dst_file: row.get(3).unwrap_or_default(),
            dst_kind: row.get(4).unwrap_or_default(),
            dst_line: row.get(5).unwrap_or_default(),
            dst_parent: row.get::<Option<String>>(6).unwrap_or_default(),
        };

        if !is_gated_path(&r.src_file) {
            continue;
        }
        gated_edges += 1;

        if r.src_file == r.dst_file {
            continue;
        }
        cross_file += 1;

        if name_counts.get(&r.dst_name).copied().unwrap_or(0) != 1 {
            continue;
        }
        sole_candidate_cross_file += 1;

        // The resolver's evidence model, evaluated after the fact: same
        // directory, the name imported, or the owning class imported.
        if dir_of(&r.src_file) == dir_of(&r.dst_file) {
            continue;
        }
        let file_imports = imports.get(&r.src_file);
        let reachable = file_imports.is_some_and(|imports| {
            imports_name(imports, &r.dst_name)
                || r.dst_parent
                    .as_deref()
                    .and_then(|id| node_name.get(id))
                    .is_some_and(|parent| imports_name(imports, parent))
        });
        if reachable {
            continue;
        }

        unreachable += 1;
        let entry = per_target
            .entry(r.dst_id.clone())
            .or_insert_with(|| (r, 0, HashSet::new()));
        entry.1 += 1;
        entry.2.insert(entry.0.src_file.clone());
    }

    // `src_file` on the stored row is whichever edge landed first, so the
    // distinct-source count is tracked separately rather than read off it.
    let mut hot: Vec<(EdgeRow, i64, HashSet<String>)> = per_target.into_values().collect();
    hot.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.dst_name.cmp(&b.0.dst_name)));
    let hot_targets = hot
        .into_iter()
        .take(limit)
        .map(|(r, edges, files)| HotTarget {
            name: r.dst_name,
            file_path: r.dst_file,
            start_line: r.dst_line,
            kind: r.dst_kind,
            source_files: files.len() as i64,
            edges,
        })
        .collect();

    Ok(EdgeAudit {
        total_edges,
        gated_edges,
        cross_file,
        sole_candidate_cross_file,
        unreachable,
        hot_targets,
    })
}
```

### `delete_projects` — src/global_db.rs:588

Source: [src/global_db.rs:588](../src/global_db.rs#L588).

```rust
    pub async fn delete_projects(&self, project_paths: &[String]) -> usize {
        const CHUNK: usize = 256;
        let mut total: usize = 0;
        for chunk in project_paths.chunks(CHUNK) {
            if chunk.is_empty() {
                continue;
            }
            let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
            let sql = format!(
                "DELETE FROM projects WHERE path IN ({})",
                placeholders.join(",")
            );
            let values: Vec<libsql::Value> = chunk
                .iter()
                .map(|p| libsql::Value::Text(p.clone()))
                .collect();
            if let Ok(n) = self.conn.execute(&sql, values).await {
                total = total.saturating_add(n as usize);
            }
        }
        total
    }
```

### `migration_merges_duplicates_and_drops_relative_rows` — src/global_db.rs:1063

Source: [src/global_db.rs:1063](../src/global_db.rs#L1063).

```rust
    async fn migration_merges_duplicates_and_drops_relative_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("global.db");
        let project = dir.path().join("proj");
        std::fs::create_dir(&project).unwrap();
        let canonical = normalize_project_key(&project);
        let trailing = format!("{canonical}/");

        {
            // Simulate a pre-canonicalisation DB: one duplicate pair and a `.` row.
            let db = GlobalDb::open_at(&db_path).await.unwrap();
            db.conn
                .execute_batch(
                    "DELETE FROM meta WHERE key = 'projects_path_normalized';
                     DELETE FROM projects;",
                )
                .await
                .unwrap();
            for (path, tokens) in [
                (canonical.as_str(), 14_000_i64),
                (trailing.as_str(), 12_000),
                (".", 999),
            ] {
                db.conn
                    .execute(
                        "INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)",
                        params![path, tokens],
                    )
                    .await
                    .unwrap();
            }
            // Raw insert: record_savings would canonicalise it for us.
            db.conn
                .execute(
                    "INSERT INTO savings_ledger (ts, project_path, tool_name, before_tokens, after_tokens) \
                     VALUES (42, ?1, 'tokensave_context', 1000, 100)",
                    params![trailing.as_str()],
                )
                .await
                .unwrap();
        }

        let db = GlobalDb::open_at(&db_path).await.unwrap();
        let paths = db.list_project_paths().await;
        assert_eq!(paths, vec![canonical.clone()]);
        assert_eq!(db.get_project_tokens(&project).await, 26_000);
        // The ledger row written under the old spelling still answers for the project.
        assert_eq!(db.sum_savings(Some(&canonical), 0).await.calls, 1);
    }
```

### `build_module_import_graph` — src/graph/imports.rs:183

Source: [src/graph/imports.rs:183](../src/graph/imports.rs#L183).

```rust
    pub async fn build_module_import_graph(&self, depth: usize) -> Result<ModuleImportGraph> {
        // `n1` is the Use node the extractor emitted for the import statement;
        // `n2` is whatever the resolver bound it to. `Contains` is stored as
        // `nodes.parent_id` rather than an edge row, and that parent is what
        // distinguishes a module-level import (parent is the file) from a lazy
        // one written inside a function body.
        let sql = "SELECT DISTINCT n1.file_path, n1.start_line, n1.name, n1.signature, \
                   n2.file_path, COALESCE(parent.kind, 'file') \
                   FROM edges e \
                   JOIN nodes n1 ON e.source = n1.id \
                   JOIN nodes n2 ON e.target = n2.id \
                   LEFT JOIN nodes parent ON parent.id = n1.parent_id \
                   WHERE e.kind = 'uses' AND n1.kind = 'use' \
                   AND n1.file_path != n2.file_path";

        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query import graph: {e}"),
                operation: "build_module_import_graph".to_string(),
            })?;

        let mut graph = ModuleImportGraph::default();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read import row: {e}"),
            operation: "build_module_import_graph".to_string(),
        })? {
            let file: String = row.get(0).unwrap_or_default();
            let line: u32 = row.get(1).unwrap_or(0);
            let imported: String = row.get(2).unwrap_or_default();
            let statement: Option<String> = row.get(3).ok();
            let resolved_file: String = row.get(4).unwrap_or_default();
            let parent_kind: String = row.get(5).unwrap_or_else(|_| "file".to_string());

            let from = module_of(&file, depth);
            let to = module_of(&resolved_file, depth);
            graph.modules.insert(from.clone());
            graph.modules.insert(to.clone());
            if from == to {
                continue;
            }

            let site = ImportSite {
                file,
                line: line.saturating_add(1),
                imported,
                type_only: is_type_only(statement.as_deref()),
                statement,
                resolved_file,
                lazy: parent_kind != "file",
            };
            let sites = graph.edges.entry((from, to)).or_default();
            if !sites.contains(&site) {
                sites.push(site);
            }
        }

        Ok(graph)
    }
```

### `find_dead_code` — src/graph/queries.rs:170

Source: [src/graph/queries.rs:170](../src/graph/queries.rs#L170).

```rust
    pub async fn find_dead_code(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
        include_trait_impls: bool,
    ) -> Result<Vec<Node>> {
        let kind_filter = if kinds.is_empty() {
            String::new()
        } else {
            let kind_strs: Vec<String> =
                kinds.iter().map(|k| format!("'{}'", k.as_str())).collect();
            format!(" AND kind IN ({})", kind_strs.join(", "))
        };
        let visibility_filter = if include_public {
            ""
        } else {
            " AND visibility != 'public'"
        };

        // Trait-impl methods (Rust `impl Trait for Type` blocks) are reached
        // implicitly by the compiler — `Display::fmt` via `{}`, `Deref::deref`
        // via auto-deref, `Drop::drop` via drop glue, `From::from` via `?`/
        // `.into()` — so they carry no explicit incoming `calls` edge. They
        // also can't be written `pub` in Rust, so they're stored as `private`
        // and the visibility filter above never excludes them. The result is
        // a flood of false positives (issue #137). Exclude any method whose
        // parent is an `Impl`-kind node that is a *trait* impl (vs an inherent
        // `impl Type` block, whose un-called methods are genuine dead code).
        //
        // We detect a trait impl two ways, OR'd together:
        //   1) the impl node's signature contains ` for ` — i.e. the header is
        //      `impl <Trait> for <Type>`. This is the PRIMARY signal and the
        //      only one that works for the common case, because…
        //   2) …the impl node bears an outgoing `implements` edge. This edge
        //      only materializes when the trait resolves to an indexed node,
        //      which it does NOT for stdlib/external traits (`Display`, `Drop`,
        //      `Deref`, `From`, `AsRef`, …) — precisely the traits in the bug
        //      report. So the edge alone misses almost every real case (mex has
        //      zero `implements` edges); the signature scan is what carries the
        //      fix. The edge is kept as a fallback for multi-line impl headers
        //      where `extract_first_line` truncates before the ` for `.
        //
        // Inherent-impl signatures (`impl Type`) and ObjC `@implementation Foo`
        // blocks contain no ` for `, so they are untouched — inherent dead
        // methods are still reported. Class-based languages attach `implements`
        // to the `Class` node (not an `impl`) and store interface methods
        // `public`, so the visibility filter already covers them. The guarding
        // `parent_id IS NULL OR` is required: `NULL NOT IN (...)` is NULL, not
        // true, which would silently drop every top-level function.
        let trait_impl_filter = if include_trait_impls {
            ""
        } else {
            " AND (parent_id IS NULL OR parent_id NOT IN (
                 SELECT p.id FROM nodes p
                 WHERE p.kind = 'impl'
                   AND (p.signature LIKE '% for %'
                        OR p.id IN (SELECT source FROM edges WHERE kind = 'implements'))
             ))"
        };

        // Only true "use sites" count as evidence that a symbol is alive:
        // `calls`, `implements`, `extends`, `type_of`, `returns`, `receives`,
        // `uses`. Bookkeeping edges (`contains`, `annotates`, `derives_macro`)
        // are emitted by the extractor even for unused code — every Rust
        // function tagged `#[inline]`, every `#[derive(Debug)]` on an unused
        // struct, every annotation_usage adds an incoming edge that is NOT a
        // real reference. Previously we excluded only `contains`, so any
        // attribute on an otherwise-unused function masked it from
        // dead-code detection — which is why the sonium run reported zero
        // dead functions across 5,715. The narrower allowlist below restores
        // the intended semantics: "no real caller / referencer = dead".
        // Test-marker exclusion uses a two-step "resolve + filter in SQL":
        //   1) `collect_test_marker_ids` runs the leading-wildcard `LIKE`
        //      scan exactly once over `kind = 'annotation_usage'`.
        //   2) The marker ids go to the dead-code SELECT as ONE bound
        //      parameter, a JSON array built with `serde_json`, and the
        //      SELECT drops test-annotated candidates with
        //      `id NOT IN (SELECT target FROM edges WHERE kind = 'annotates'
        //      AND source IN (SELECT value FROM json_each(?)))`.
        //      The subquery is uncorrelated, so SQLite plans it as a
        //      `LIST SUBQUERY`: it walks the JSON array once, probes
        //      `idx_edges_source_kind` per marker, and builds a transient
        //      in-memory index of the targets that each candidate probes.
        //      That is the same shape as the old two-temp-table form,
        //      without writing anything: selected graphs are opened with
        //      `PRAGMA query_only = 1`, and the transient index is not a
        //      database write. A single parameter also avoids SQLite's
        //      bound-parameter limit, so there is no chunking.
        //
        // History — DO NOT regress this:
        // - The pre-4.14.8 form joined `nodes a ON a.id = e2.source` inside
        //   the correlated `NOT EXISTS` and ran the `a.name LIKE '%::test'`
        //   chain per matching edge. Fast on scirs (0.1 s, 76 K
        //   annotation_usage) but timed out on chromium at the 25 s probe
        //   ceiling, cascade-poisoning every subsequent MCP tool call.
        // - 4.14.8's `WITH test_marker_ids AS (...)` CTE attempt was even
        //   worse: SQLite inlined the single-reference CTE, turning every
        //   dead-candidate row into a full annotation_usage scan. Regressed
        //   scirs from 0.1 s to >60 s, reverted in 4.14.9.
        // - 4.14.12's first attempt (single temp table + correlated
        //   `IN (SELECT id FROM temp.test_markers)` inside `NOT EXISTS`)
        //   also failed on chromium: SQLite picked `idx_edges_unique
        //   (source, target, kind)` for the subquery and iterated every
        //   marker as the outer driver for every candidate
        //   (~13K markers × ~134K candidates ≈ 1.7B probes). Timed out.
        // - The next pre-join form (two temp tables, indexed `NOT IN`)
        //   measured 0.75 s on chromium and 0.6 s on scirs. The
        //   wildcard scan ran exactly once per call; the per-row
        //   probe was against a ~15K-row indexed lookup table, not a
        //   correlated subquery the optimiser can re-shape.
        // - The two-temp-table form above wrote `temp.test_markers` and
        //   `temp.test_annotated_targets`, which fails on selected graphs
        //   (opened with `PRAGMA query_only = 1`) and let concurrent
        //   read-only calls on the shared connection drop or refill each
        //   other's tables.
        // - The next form collected the targets into an in-memory `HashSet`
        //   (chunked SELECTs of 500 marker ids) and skipped matching rows
        //   while reading the main query. It was read-only safe but ~11%
        //   slower per call: rows SQL used to filter out (~2,200 of ~2,970
        //   candidates on a 24 K-node project) were read and decoded, then
        //   discarded. Filtering in SQL with `json_each` restores that.
        let marker_ids = self.db.collect_test_marker_ids().await?;
        // Always bind the array, even when empty: `NOT IN` over an empty
        // set is true, so every candidate is kept.
        let marker_ids_json =
            serde_json::to_string(&marker_ids).map_err(|e| TokenSaveError::Database {
                message: format!("failed to encode test marker ids: {e}"),
                operation: "find_dead_code".to_string(),
            })?;
        drop(marker_ids);
        let godot_virtuals = GODOT_ENGINE_VIRTUALS
            .iter()
            .map(|n| format!("'{n}'"))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path, start_line, end_line,
                    start_column, end_column, docstring, signature, visibility,
                    is_async, branches, loops, returns, max_nesting, unsafe_blocks,
                    unchecked_calls, assertions, updated_at, attrs_start_line
             FROM nodes
             WHERE kind != 'route'
             AND name != 'main'
             AND name != '_bind_methods'
             AND name NOT LIKE 'test%'
             -- Go package-level `init` is invoked implicitly by the runtime at
             -- package load and can never be called explicitly, so it carries
             -- no incoming `calls` edge — the same reason `main`, `test*`, and
             -- Rust trait-impl methods are exempt. Scoped to `.go` so a callable
             -- function named `init` in another language is still checked (#346).
             AND NOT (name = 'init' AND file_path LIKE '%.go')
             -- Godot engine virtuals (`_ready`, `_process`, ...) are called by the
             -- engine, never by project code, so they have no incoming edge either.
             -- Scoped to `.gd` for the same reason as Go `init` (#598).
             AND NOT (file_path LIKE '%.gd' AND name IN ({godot_virtuals}))
             {visibility_filter}
             {kind_filter}
             {trait_impl_filter}
             AND NOT EXISTS (
                 SELECT 1 FROM edges
                 WHERE target = nodes.id
                 AND kind IN ('calls', 'implements', 'extends', 'type_of', 'returns', 'receives', 'uses')
             )
             AND id NOT IN (
                 SELECT target FROM edges
                 WHERE kind = 'annotates'
                 AND source IN (SELECT value FROM json_each(?1))
             )"
        );

        let mut rows = self
            .db
            .conn()
            .query(&sql, libsql::params![marker_ids_json])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to find dead code: {e}"),
                operation: "find_dead_code".to_string(),
            })?;

        let mut dead = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: "find_dead_code".to_string(),
        })? {
            dead.push(row_to_node_dead_code(&row)?);
        }

        if dead
            .iter()
            .any(|n| crate::resolution::is_gdscript(&n.file_path))
        {
            // Best effort: a failed lookup leaves the overrides reported, and
            // must not fail the whole dead-code query.
            let live = self.gdscript_live_overrides().await.unwrap_or_default();
            if !live.is_empty() {
                dead.retain(|n| !live.contains(&n.id));
            }
        }
        Ok(dead)
    }
```

### `gdscript_live_overrides` — src/graph/queries.rs:384

Source: [src/graph/queries.rs:384](../src/graph/queries.rs#L384).

```rust
    async fn gdscript_live_overrides(&self) -> Result<HashSet<String>> {
        let sql = "WITH RECURSIVE ancestry(cls, base) AS (
                 SELECT e.source, e.target FROM edges e
                 WHERE e.kind = 'extends'
                   AND e.source IN (SELECT id FROM nodes
                                    WHERE file_path LIKE '%.gd'
                                      AND kind IN ('class', 'inner_class'))
                 UNION
                 SELECT a.cls, e.target FROM ancestry a
                 JOIN edges e ON e.source = a.base AND e.kind = 'extends'
             )
             SELECT DISTINCT m.id FROM ancestry a
             JOIN nodes m ON m.parent_id = a.cls AND m.kind IN ('function', 'method')
             JOIN nodes b ON b.parent_id = a.base AND b.name = m.name
                         AND b.kind IN ('function', 'method')
             WHERE EXISTS (SELECT 1 FROM edges c
                           WHERE c.target = b.id AND c.kind IN ('calls', 'uses'))";
        let mut rows =
            self.db
                .conn()
                .query(sql, ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to find live GDScript overrides: {e}"),
                    operation: "gdscript_live_overrides".to_string(),
                })?;
        let mut live = HashSet::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: "gdscript_live_overrides".to_string(),
        })? {
            live.insert(row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read override id: {e}"),
                operation: "gdscript_live_overrides".to_string(),
            })?);
        }
        Ok(live)
    }
```

### `get_file_dependencies` — src/graph/queries.rs:457

Source: [src/graph/queries.rs:457](../src/graph/queries.rs#L457).

```rust
    pub async fn get_file_dependencies(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path).await?;
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let placeholders: Vec<String> = (1..=node_ids.len()).map(|i| format!("?{i}")).collect();
        let kind_filter = "('uses', 'calls')";

        let sql = format!(
            "SELECT DISTINCT e.target FROM edges e \
             WHERE e.source IN ({}) AND e.kind IN {kind_filter}",
            placeholders.join(", ")
        );

        let param_values: Vec<libsql::Value> = node_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();

        let mut rows = self
            .db
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file dependencies: {e}"),
                operation: "get_file_dependencies".to_string(),
            })?;

        let mut target_ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read target id: {e}"),
            operation: "get_file_dependencies".to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                target_ids.push(id);
            }
        }

        if target_ids.is_empty() {
            return Ok(Vec::new());
        }

        let target_nodes = self.db.get_nodes_by_ids(&target_ids).await?;
        let dep_files: HashSet<String> = target_nodes
            .into_iter()
            .filter(|n| n.file_path != file_path)
            .map(|n| n.file_path)
            .collect();

        let mut result: Vec<String> = dep_files.into_iter().collect();
        result.sort();
        Ok(result)
    }
```

### `get_file_dependents` — src/graph/queries.rs:519

Source: [src/graph/queries.rs:519](../src/graph/queries.rs#L519).

```rust
    pub async fn get_file_dependents(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path).await?;
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let placeholders: Vec<String> = (1..=node_ids.len()).map(|i| format!("?{i}")).collect();
        let kind_filter = "('uses', 'calls')";

        let sql = format!(
            "SELECT DISTINCT e.source FROM edges e \
             WHERE e.target IN ({}) AND e.kind IN {kind_filter}",
            placeholders.join(", ")
        );

        let param_values: Vec<libsql::Value> = node_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();

        let mut rows = self
            .db
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file dependents: {e}"),
                operation: "get_file_dependents".to_string(),
            })?;

        let mut source_ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read source id: {e}"),
            operation: "get_file_dependents".to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                source_ids.push(id);
            }
        }

        if source_ids.is_empty() {
            return Ok(Vec::new());
        }

        let source_nodes = self.db.get_nodes_by_ids(&source_ids).await?;
        let dependent_files: HashSet<String> = source_nodes
            .into_iter()
            .filter(|n| n.file_path != file_path)
            .map(|n| n.file_path)
            .collect();

        let mut result: Vec<String> = dependent_files.into_iter().collect();
        result.sort();
        Ok(result)
    }
```

### `build_file_adjacency` — src/graph/queries.rs:619

Source: [src/graph/queries.rs:619](../src/graph/queries.rs#L619).

```rust
    pub async fn build_file_adjacency(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<HashMap<String, HashSet<String>>> {
        let sql = "SELECT DISTINCT n1.file_path AS src_file, n2.file_path AS tgt_file \
                   FROM edges e \
                   JOIN nodes n1 ON e.source = n1.id \
                   JOIN nodes n2 ON e.target = n2.id \
                   WHERE e.kind IN ('calls', 'uses') \
                   AND n1.file_path != n2.file_path";

        let mut rows =
            self.db
                .conn()
                .query(sql, ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query file adjacency: {e}"),
                    operation: "build_file_adjacency".to_string(),
                })?;

        // Normalise the prefix once: ensure it ends with '/'.
        let prefix: Option<String> = path_prefix.map(|p| {
            if p.ends_with('/') {
                p.to_string()
            } else {
                format!("{p}/")
            }
        });

        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();

        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read adjacency row: {e}"),
            operation: "build_file_adjacency".to_string(),
        })? {
            let src: String = row.get(0).unwrap_or_default();
            let tgt: String = row.get(1).unwrap_or_default();

            if let Some(ref pfx) = prefix {
                if !src.starts_with(pfx.as_str()) || !tgt.starts_with(pfx.as_str()) {
                    continue;
                }
            }

            adj.entry(src).or_default().insert(tgt);
        }

        // Ensure every known file appears as a key (even leaf nodes with no deps).
        let all_files = self.db.get_all_files().await?;
        for file in all_files {
            if let Some(ref pfx) = prefix {
                if !file.path.starts_with(pfx.as_str()) {
                    continue;
                }
            }
            adj.entry(file.path).or_default();
        }

        Ok(adj)
    }
```

### `any_symbol_indexed` — src/hooks.rs:403

Source: [src/hooks.rs:403](../src/hooks.rs#L403).

```rust
fn any_symbol_indexed(env: &HookEnv, symbols: &[String]) -> bool {
    let Some(root) = env.project_root.as_deref() else {
        return true;
    };
    if symbols.is_empty() {
        return true;
    }
    let db_path = crate::config::get_tokensave_dir(root).join("tokensave.db");
    let placeholders = vec!["?"; symbols.len()].join(",");
    let sql = format!("SELECT 1 FROM nodes WHERE LOWER(name) IN ({placeholders}) LIMIT 1");
    let params: Vec<libsql::Value> = symbols
        .iter()
        .map(|s| libsql::Value::Text(s.to_lowercase()))
        .collect();
    let probe = async move {
        let db = libsql::Builder::new_local(&db_path)
            .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .build()
            .await?;
        let conn = db.connect()?;
        let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
        Ok::<bool, libsql::Error>(rows.next().await?.is_some())
    };
    // The hook runs inside `#[tokio::main]`'s multi-thread runtime (main.rs),
    // where block_in_place + the ambient handle is the house pattern
    // (monitor.rs); outside a runtime (sync tests) use a throwaway
    // current-thread runtime.
    // shortcut: a current-thread ambient runtime would panic here; no caller
    // runs the hook on one.
    let ran = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(probe)).ok(),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .build()
            .ok()
            .and_then(|rt| rt.block_on(probe).ok()),
    };
    ran.unwrap_or(true)
}
```

### `handle_run_affected_tests` — src/mcp/tools/handlers/workflow.rs:140

Source: [src/mcp/tools/handlers/workflow.rs:140](../src/mcp/tools/handlers/workflow.rs#L140).

```rust
pub(super) async fn handle_run_affected_tests(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let explicit_paths: Option<Vec<String>> = args.get("changed_paths").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
    });
    let profile = args
        .get("profile")
        .and_then(|v| v.as_str())
        .unwrap_or("debug")
        .to_string();
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(300);
    let max_tests = args
        .get("max_tests")
        .and_then(serde_json::Value::as_u64)
        .map_or(100_usize, |v| (v as usize).min(MAX_TESTS_HARD_CAP));

    let project_root = cg.project_root().to_path_buf();

    // 1) Resolve changed paths — explicit list, or fall back to `git diff`.
    let changed_paths = match explicit_paths {
        Some(p) => p,
        None => git_changed_paths(&project_root).await,
    };
    if changed_paths.is_empty() {
        return Ok(empty_result("no changed files detected"));
    }

    // 2) Find tests that cover the changed paths.
    //
    //    Two paths feed into the test set:
    //    a) Indirect coverage — for every callable in a changed file, walk
    //       callers and keep test-shaped ones (test file or `#[test]`
    //       annotated). This is the common case when source changes are
    //       what's being tested.
    //    b) Direct change — when a changed path itself is a test file (or
    //       holds `#[test]` annotations), dispatch its test functions
    //       directly. `#[test]` fns are leaves with no callers, so the
    //       indirect-coverage walk above always misses them and the tool
    //       used to silently skip PRs that only edit `tests/foo.rs`.
    let mut test_targets: HashMap<String, Vec<String>> = HashMap::new();
    let mut covered_sources: HashSet<String> = HashSet::new();
    for path in &changed_paths {
        let nodes = cg.get_nodes_by_file(path).await.unwrap_or_default();

        // (b) Direct dispatch from changed test files.
        let path_is_test_file = cg.is_test_file(path);
        if path_is_test_file || !nodes.is_empty() {
            let candidate_ids: Vec<String> = nodes
                .iter()
                .filter(|n| {
                    matches!(
                        n.kind,
                        NodeKind::Function | NodeKind::Method | NodeKind::SingletonMethod
                    )
                })
                .map(|n| n.id.clone())
                .collect();
            let test_annotated_in_file = cg
                .get_test_annotated_node_ids(&candidate_ids)
                .await
                .unwrap_or_default();
            for node in &nodes {
                if !matches!(
                    node.kind,
                    NodeKind::Function | NodeKind::Method | NodeKind::SingletonMethod
                ) {
                    continue;
                }
                let looks_like_test =
                    path_is_test_file || test_annotated_in_file.contains(&node.id);
                if !looks_like_test {
                    continue;
                }
                // The test "covers itself" — record the node id as the
                // source so the per-test `covers_source_ids` field
                // remains useful.
                test_targets
                    .entry(node.name.clone())
                    .or_default()
                    .push(node.id.clone());
                covered_sources.insert(node.id.clone());
            }
        }

        // (a) Indirect coverage — walk callers of every callable in the file.
        for node in &nodes {
            if !matches!(
                node.kind,
                NodeKind::Function | NodeKind::Method | NodeKind::SingletonMethod
            ) {
                continue;
            }
            let callers = cg.get_callers(&node.id, 3).await.unwrap_or_default();
            let caller_ids: Vec<String> = callers.iter().map(|(n, _)| n.id.clone()).collect();
            let test_annotated = cg
                .get_test_annotated_node_ids(&caller_ids)
                .await
                .unwrap_or_default();
            for (caller, _) in callers {
                if !cg.is_test_file(&caller.file_path) && !test_annotated.contains(&caller.id) {
                    continue;
                }
                if !matches!(
                    caller.kind,
                    NodeKind::Function | NodeKind::Method | NodeKind::SingletonMethod
                ) {
                    continue;
                }
                test_targets
                    .entry(caller.name.clone())
                    .or_default()
                    .push(node.id.clone());
                covered_sources.insert(node.id.clone());
            }
        }
    }

    if test_targets.is_empty() {
        return Ok(empty_result(&format!(
            "no tests cover the changed paths ({} file(s))",
            changed_paths.len()
        )));
    }

    let mut test_names: Vec<String> = test_targets.keys().cloned().collect();
    test_names.sort();
    let total_tests = test_names.len();
    test_names.truncate(max_tests);

    // 3) Run cargo test --no-fail-fast with each test name as a libtest
    // filter. We use `--` to pass them through.
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&project_root)
        .arg("test")
        .arg("--no-fail-fast");
    if profile == "release" {
        cmd.arg("--release");
    }
    cmd.arg("--");
    for name in &test_names {
        cmd.arg(name);
    }
    cmd.kill_on_drop(true);

    let run = cmd.output();
    let output = match timeout(Duration::from_secs(timeout_secs), run).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return Ok(error_result(&format!("failed to spawn cargo test: {e}")));
        }
        Err(_) => {
            return Ok(error_result(&format!(
                "cargo test timed out after {timeout_secs}s"
            )));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let results = parse_libtest_output(&stdout);

    let passed = results.iter().filter(|(_, ok)| *ok).count();
    let failed = results.iter().filter(|(_, ok)| !*ok).count();
    let touched_files: Vec<String> = unique_file_paths(changed_paths.iter().map(String::as_str));

    let body = json!({
        "exit_code": output.status.code(),
        "passed": passed,
        "failed": failed,
        "total_observed": results.len(),
        "dispatched_tests": test_names,
        "truncated": total_tests > test_names.len(),
        "results": results
            .iter()
            .map(|(name, ok)| {
                // libtest emits `module::path::test_fn` while the graph keys
                // by the short fn name. Look up by both so we resolve either
                // shape.
                let short = name.rsplit("::").next().unwrap_or(name);
                let covers = test_targets
                    .get(name)
                    .or_else(|| test_targets.get(short))
                    .cloned()
                    .unwrap_or_default();
                json!({
                    "test": name,
                    "passed": ok,
                    "covers_source_ids": covers,
                })
            })
            .collect::<Vec<_>>(),
        "stderr_tail": tail(&stderr, 2000),
        "stdout_tail": tail(&stdout, 2000),
    });

    let formatted = serde_json::to_string_pretty(&body).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}
```

### `compare` — src/quality_diff.rs:517

Source: [src/quality_diff.rs:517](../src/quality_diff.rs#L517).

```rust
pub async fn compare(
    root: &Path,
    base: &str,
    target: &str,
    options: &QualityOptions,
) -> Result<QualityReport> {
    let (base_name, base_commit) = resolve(root, base)?;
    let (target_name, target_commit) = resolve(root, target)?;
    let renamed = git(
        root,
        &[
            "diff",
            "--name-status",
            "--relative",
            "-z",
            "--find-renames",
            &base_commit,
            &target_commit,
            "--",
        ],
    )?;
    let fields: Vec<_> = renamed.split(|b| *b == 0).collect();
    let mut renames = BTreeMap::new();
    let mut i = 0;
    while i + 1 < fields.len() {
        if fields[i].starts_with(b"R") && i + 2 < fields.len() {
            renames.insert(
                String::from_utf8_lossy(fields[i + 1]).into_owned(),
                String::from_utf8_lossy(fields[i + 2]).into_owned(),
            );
            i += 3;
        } else {
            i += 2;
        }
    }
    let base_cg = TokenSave::open_read_only(root, Some(&base_name)).await?;
    base_cg
        .db()
        .conn()
        .execute_batch("BEGIN")
        .await
        .map_err(|e| error(e.to_string()))?;
    let mut stale = verify(&base_cg, &base_commit).await?;
    if !stale.is_empty() && !options.allow_stale {
        return Err(error(format!(
            "base index is stale or incomplete: {}. Sync explicitly, or opt in with --allow-stale (MCP thresholds.allow_stale).",
            stale[0]
        )));
    }
    let base_version = base_cg.db().get_metadata("last_full_index_version").await?;
    let mut base_snapshot = snapshot(&base_cg, options, &renames).await?;
    drop(base_cg);
    // Reuse the pinned metrics for an identical index. A live server may sync
    // between sequential reads; that must never manufacture same-branch changes.
    let mut target_snapshot = if base_name == target_name {
        if base_version.is_none() {
            stale.push("analysis version provenance is missing".into());
            if !options.allow_stale {
                return Err(error(
                    "analysis version provenance is missing; sync the index explicitly",
                ));
            }
        }
        base_snapshot.clone()
    } else {
        let target_cg = TokenSave::open_read_only(root, Some(&target_name)).await?;
        target_cg
            .db()
            .conn()
            .execute_batch("BEGIN")
            .await
            .map_err(|e| error(e.to_string()))?;
        let target_stale = verify(&target_cg, &target_commit).await?;
        if !target_stale.is_empty() && !options.allow_stale {
            return Err(error(format!(
            "target index is stale or incomplete: {}. Sync explicitly, or opt in with --allow-stale (MCP thresholds.allow_stale).",
            target_stale[0]
        )));
        }
        let target_version = target_cg
            .db()
            .get_metadata("last_full_index_version")
            .await?;
        if base_version.is_none() || target_version.is_none() || base_version != target_version {
            stale.push("analysis version provenance is missing or differs between indexes; metrics may not be comparable".into());
            if !options.allow_stale {
                return Err(error("analysis version provenance is missing or differs; explicitly sync both indexes with the same version"));
            }
        }
        stale.extend(target_stale);
        snapshot(&target_cg, options, &BTreeMap::new()).await?
    };
    if resolve(root, base)?.1 != base_commit || resolve(root, target)?.1 != target_commit {
        return Err(error(
            "Git refs changed during comparison; retry with stable branch tips",
        ));
    }
    let ambiguous: BTreeSet<_> = base_snapshot
        .iter()
        .chain(target_snapshot.iter())
        .filter(|(_, metric)| metric.ambiguous)
        .map(|(key, _)| key.clone())
        .collect();
    base_snapshot.retain(|key, _| !ambiguous.contains(key));
    target_snapshot.retain(|key, _| !ambiguous.contains(key));
    let findings = classify(&base_snapshot, &target_snapshot, options.min_increase);
    let mut summary: BTreeMap<String, usize> =
        ["new", "worsened", "resolved", "improved", "unchanged"]
            .into_iter()
            .map(|s| (s.into(), 0))
            .collect();
    for f in &findings {
        *summary.entry(f.status.clone()).or_default() += 1;
    }
    let complete = stale.is_empty() && ambiguous.is_empty();
    let mut limitations = vec![
        "Tip-to-tip comparison, not merge-base attribution: diverged branch differences may originate on either branch.".into(),
        "Only indexed files and resolved calls/uses edges are analyzed. Excluded, unindexed or unsupported source and missing edges are not evidence of clean code.".into(),
        "Git-detected file renames are normalized; symbol renames and ambiguous moves remain unmatched. Unmatched findings require review.".into(),
        "Architecture policy, dead-code, test reachability, execution coverage and module-size regressions are not analyzed.".into(),
        "A small increase below min_increase is classified unchanged for gating. No claim of execution behavior or confirmed defects.".into(),
    ];
    if !ambiguous.is_empty() {
        limitations.push(format!("{} ambiguous metrics (same qualified name, kind and signature) excluded from both snapshots; comparison is incomplete.", ambiguous.len()));
    }
    limitations.extend(stale);
    Ok(QualityReport {
        base: base.into(),
        target: target.into(),
        base_commit,
        target_commit,
        baseline: "branch_tips".into(),
        complete,
        summary,
        findings,
        limitations,
    })
}
```

### `read_journal_mode` — src/runtime_telemetry.rs:238

Source: [src/runtime_telemetry.rs:238](../src/runtime_telemetry.rs#L238).

```rust
async fn read_journal_mode(cg: &crate::tokensave::TokenSave) -> Result<String> {
    let mut rows = cg
        .db()
        .conn()
        .query("PRAGMA journal_mode", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read journal_mode: {e}"),
            operation: "read_journal_mode".to_string(),
        })?;
    let row = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read journal_mode row: {e}"),
            operation: "read_journal_mode".to_string(),
        })?
        .ok_or_else(|| TokenSaveError::Database {
            message: "no journal_mode row returned".to_string(),
            operation: "read_journal_mode".to_string(),
        })?;
    row.get::<String>(0).map_err(|e| TokenSaveError::Database {
        message: format!("failed to decode journal_mode: {e}"),
        operation: "read_journal_mode".to_string(),
    })
}
```

### `read_source_total_bytes` — src/runtime_telemetry.rs:265

Source: [src/runtime_telemetry.rs:265](../src/runtime_telemetry.rs#L265).

```rust
async fn read_source_total_bytes(cg: &crate::tokensave::TokenSave) -> Result<u64> {
    let mut rows = cg
        .db()
        .conn()
        .query("SELECT COALESCE(SUM(size), 0) FROM files", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to sum source bytes: {e}"),
            operation: "read_source_total_bytes".to_string(),
        })?;
    let row = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read source-sum row: {e}"),
            operation: "read_source_total_bytes".to_string(),
        })?
        .ok_or_else(|| TokenSaveError::Database {
            message: "no source-sum row returned".to_string(),
            operation: "read_source_total_bytes".to_string(),
        })?;
    let v: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
        message: format!("failed to decode source-sum: {e}"),
        operation: "read_source_total_bytes".to_string(),
    })?;
    Ok(u64::try_from(v).unwrap_or(0))
}
```

### `record_decision` — src/tokensave/memory.rs:79

Source: [src/tokensave/memory.rs:79](../src/tokensave/memory.rs#L79).

```rust
    pub async fn record_decision(
        &self,
        text: &str,
        reason: Option<&str>,
        files: &[String],
        tags: &[String],
    ) -> crate::errors::Result<i64> {
        debug_assert!(!text.is_empty(), "decision text must not be empty");
        let files_json =
            serde_json::to_string(files).map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("record_decision files serialization failed: {e}"),
                operation: "record_decision".to_string(),
            })?;
        let tags_json =
            serde_json::to_string(tags).map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("record_decision tags serialization failed: {e}"),
                operation: "record_decision".to_string(),
            })?;
        let now = current_timestamp();
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO memory_decisions (text, reason, created_at, files, tags) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            libsql::params![text, reason, now, files_json, tags_json],
        )
        .await
        .map_err(|e| crate::errors::TokenSaveError::Database {
            message: format!("record_decision insert failed: {e}"),
            operation: "record_decision".to_string(),
        })?;
        Ok(conn.last_insert_rowid())
    }
```

### `session_recall` — src/tokensave/memory.rs:116

Source: [src/tokensave/memory.rs:116](../src/tokensave/memory.rs#L116).

```rust
    pub async fn session_recall(
        &self,
        query: Option<&str>,
        since: Option<i64>,
        limit: usize,
    ) -> crate::errors::Result<Vec<DecisionRecord>> {
        let limit = limit.clamp(1, MAX_RECALL_LIMIT) as i64;
        let conn = self.db.conn();

        let db_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall query failed: {e}"),
            operation: "session_recall".to_string(),
        };

        // FTS5 parses the bound MATCH string as a query expression even
        // through a bound parameter, so raw terms containing `-`, `.`, `/`
        // etc. (`data-api`) are syntax errors (#218). Escape into quoted
        // prefix terms first; a query with no tokenizable content degrades
        // to the unfiltered recency-ordered arms.
        let fts_query = query.and_then(crate::db::to_fts_match_query);

        let mut rows = match (fts_query.as_deref(), since) {
            (Some(q), Some(ts)) => conn
                .query(
                    "SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags \
                     FROM memory_decisions d \
                     JOIN memory_decisions_fts f ON f.rowid = d.id \
                     WHERE memory_decisions_fts MATCH ?1 AND d.created_at >= ?2 \
                     ORDER BY d.created_at DESC LIMIT ?3",
                    libsql::params![q, ts, limit],
                )
                .await
                .map_err(db_err)?,
            (Some(q), None) => conn
                .query(
                    "SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags \
                     FROM memory_decisions d \
                     JOIN memory_decisions_fts f ON f.rowid = d.id \
                     WHERE memory_decisions_fts MATCH ?1 \
                     ORDER BY d.created_at DESC LIMIT ?2",
                    libsql::params![q, limit],
                )
                .await
                .map_err(db_err)?,
            (None, Some(ts)) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions WHERE created_at >= ?1 \
                     ORDER BY created_at DESC LIMIT ?2",
                    libsql::params![ts, limit],
                )
                .await
                .map_err(db_err)?,
            (None, None) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions ORDER BY created_at DESC LIMIT ?1",
                    libsql::params![limit],
                )
                .await
                .map_err(db_err)?,
        };

        let row_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall row read failed: {e}"),
            operation: "session_recall".to_string(),
        };
        let json_err = |e: serde_json::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall JSON parse failed: {e}"),
            operation: "session_recall".to_string(),
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(row_err)? {
            let files_json: String = row.get(4).map_err(row_err)?;
            let tags_json: String = row.get(5).map_err(row_err)?;
            out.push(DecisionRecord {
                id: row.get(0).map_err(row_err)?,
                text: row.get(1).map_err(row_err)?,
                reason: row.get::<Option<String>>(2).map_err(row_err)?,
                created_at: row.get(3).map_err(row_err)?,
                files: serde_json::from_str(&files_json).map_err(json_err)?,
                tags: serde_json::from_str(&tags_json).map_err(json_err)?,
            });
        }

        // Recency-decay ranking for the no-explicit-query path. The SQL above
        // already LIMITed to the newest N rows (which, because the decay is
        // monotonic in `created_at`, are exactly the N highest-weighted rows),
        // so we only need to (re)order the selected set by decay weight here.
        // Higher weight first; ties keep newest-first. No row is dropped — old
        // decisions still surface, just lower down.
        if query.is_none() {
            let now = current_timestamp();
            out.sort_by(|a, b| {
                let wa = recency_decay_weight(a.created_at, now);
                let wb = recency_decay_weight(b.created_at, now);
                wb.partial_cmp(&wa)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.created_at.cmp(&a.created_at))
            });
        }
        Ok(out)
    }
```

### `record_code_area` — src/tokensave/memory.rs:265

Source: [src/tokensave/memory.rs:265](../src/tokensave/memory.rs#L265).

```rust
    pub async fn record_code_area(
        &self,
        path: &str,
        description: Option<&str>,
    ) -> crate::errors::Result<()> {
        debug_assert!(!path.is_empty(), "code area path must not be empty");
        let now = current_timestamp();
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO memory_code_areas (path, description, last_touched_at, touch_count) \
             VALUES (?1, ?2, ?3, 1) \
             ON CONFLICT(path) DO UPDATE SET \
                description = COALESCE(excluded.description, memory_code_areas.description), \
                last_touched_at = excluded.last_touched_at, \
                touch_count = memory_code_areas.touch_count + 1",
            libsql::params![path, description, now],
        )
        .await
        .map_err(|e| crate::errors::TokenSaveError::Database {
            message: format!("record_code_area upsert failed: {e}"),
            operation: "record_code_area".to_string(),
        })?;
        Ok(())
    }
```

### `list_code_areas` — src/tokensave/memory.rs:291

Source: [src/tokensave/memory.rs:291](../src/tokensave/memory.rs#L291).

```rust
    pub async fn list_code_areas(
        &self,
        limit: usize,
    ) -> crate::errors::Result<Vec<CodeAreaRecord>> {
        let limit = limit.clamp(1, MAX_CODE_AREAS_LIMIT) as i64;
        let conn = self.db.conn();
        let mut rows = conn
            .query(
                "SELECT id, path, description, last_touched_at, touch_count \
                 FROM memory_code_areas ORDER BY last_touched_at DESC LIMIT ?1",
                libsql::params![limit],
            )
            .await
            .map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("list_code_areas query failed: {e}"),
                operation: "list_code_areas".to_string(),
            })?;
        let row_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("list_code_areas row read failed: {e}"),
            operation: "list_code_areas".to_string(),
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(row_err)? {
            out.push(CodeAreaRecord {
                id: row.get(0).map_err(row_err)?,
                path: row.get(1).map_err(row_err)?,
                description: row.get::<Option<String>>(2).map_err(row_err)?,
                last_touched_at: row.get(3).map_err(row_err)?,
                touch_count: row.get::<i64>(4).map_err(row_err)? as u32,
            });
        }
        Ok(out)
    }
```

### `project` — src/tokensave/runtime.rs:97

Source: [src/tokensave/runtime.rs:97](../src/tokensave/runtime.rs#L97).

```rust
    async fn project(root: &Path) -> TokenSave {
        git(root, &["init", "-b", "main"]);
        std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
        std::fs::write(root.join("base.rs"), "fn base() {}\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "base"]);
        let cg = TokenSave::init(root).await.unwrap();
        cg.sync().await.unwrap();
        let mut config = load_config(root).unwrap();
        config.auto_track = true;
        save_config(root, &config).unwrap();
        TokenSave::open(root).await.unwrap()
    }
```

### `round_trip_tracks_and_syncs_without_polluting_parent` — src/tokensave/runtime.rs:112

Source: [src/tokensave/runtime.rs:112](../src/tokensave/runtime.rs#L112).

```rust
    async fn round_trip_tracks_and_syncs_without_polluting_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::default();
        git(tmp.path(), &["checkout", "-b", "feature"]);
        std::fs::remove_file(tmp.path().join("base.rs")).unwrap();
        std::fs::write(tmp.path().join("feature.rs"), "fn feature() {}\n").unwrap();
        git(tmp.path(), &["add", "."]);
        git(tmp.path(), &["commit", "-m", "feature"]);
        let lease = runtime.lease(&initial).await.unwrap();
        let next = lease.as_ref().unwrap();
        assert_eq!(next.active_branch(), Some("feature"));
        assert!(next.db.get_file("feature.rs").await.unwrap().is_some());
        assert!(next.db.get_file("base.rs").await.unwrap().is_none());
        assert!(initial.db.get_file("feature.rs").await.unwrap().is_none());
        // Reusing the same context does not copy or reopen its database.
        let saved = Arc::clone(next);
        drop(lease);
        let lease = runtime.lease(&initial).await.unwrap();
        assert!(Arc::ptr_eq(&saved, lease.as_ref().unwrap()));
        drop(lease);
        git(tmp.path(), &["checkout", "main"]);
        let lease = runtime.lease(&initial).await.unwrap();
        let main = lease.as_ref().unwrap();
        assert_eq!(main.active_branch(), Some("main"));
        assert!(main.db.get_file("base.rs").await.unwrap().is_some());
        assert!(main.db.get_file("feature.rs").await.unwrap().is_none());
    }
```

### `locate_verified_asset` — src/upgrade.rs:126

Source: [src/upgrade.rs:126](../src/upgrade.rs#L126).

```rust
fn locate_verified_asset(tag: &str, expected_asset: &str) -> Result<VerifiedAsset> {
    let assets = fetch_release_assets(tag)?;

    let url = assets
        .iter()
        .find(|a| a.name == expected_asset)
        .map(|a| a.browser_download_url.clone())
        .ok_or_else(|| TokenSaveError::Config {
            message: format!(
                "release {tag} exists but asset '{expected_asset}' is not yet available.\n  \
                 CI build may still be in progress — try again in a few minutes.\n  \
                 https://github.com/{GITHUB_REPO}/releases/tag/{tag}",
            ),
        })?;

    let sums_url = assets
        .iter()
        .find(|a| a.name == SUMS_ASSET)
        .map(|a| a.browser_download_url.clone())
        .ok_or_else(|| TokenSaveError::Update {
            message: format!(
                "release {tag} publishes no {SUMS_ASSET}, so the download cannot be \
                 verified — refusing to install.\n  \
                 If CI is still finishing, try again in a few minutes:\n  \
                 https://github.com/{GITHUB_REPO}/releases/tag/{tag}",
            ),
        })?;

    let listing = fetch_text_asset(&sums_url)?;
    let sha256 =
        sha256_for_asset(&listing, expected_asset).ok_or_else(|| TokenSaveError::Update {
            message: format!(
                "release {tag} publishes {SUMS_ASSET} but it lists no hash for \
                 '{expected_asset}' — refusing to install.\n  \
                 https://github.com/{GITHUB_REPO}/releases/tag/{tag}",
            ),
        })?;

    Ok(VerifiedAsset { url, sha256 })
}
```

### `inspect` — src/verify.rs:178

Source: [src/verify.rs:178](../src/verify.rs#L178).

```rust
async fn inspect(root: &Path, deep: bool, report: &mut Report) -> Result<bool> {
    if !crate::config::get_tokensave_dir(root)
        .join("config.json")
        .is_file()
    {
        report.add(
            "PROJECT_NOT_INITIALIZED",
            "error",
            root.display().to_string(),
            "Run tokensave init in the intended project.",
        );
        return Ok(false);
    }
    let Some(path) = database_path(root, report)? else {
        return Ok(false);
    };
    if !path.is_file() {
        report.add(
            "DATABASE_MISSING",
            "error",
            path.display().to_string(),
            "Restore the database from backup.",
        );
        return Ok(false);
    }
    crate::db::compatibility::preflight(&path, u32::MAX).await?;
    let db = libsql::Builder::new_local(&path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await?;
    let conn = db.connect()?;
    conn.execute_batch("PRAGMA query_only=1; PRAGMA busy_timeout=1000; PRAGMA cache_size=-8192; PRAGMA mmap_size=0;").await?;
    // An owned read transaction gives every SQL check the same committed snapshot.
    // Dropping it rolls back on every return/error, without holding a writer lock.
    let tx = conn.transaction().await?;
    let mut rows = tx.query("PRAGMA user_version", ()).await?;
    let version: i64 = rows
        .next()
        .await?
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing schema version".into(),
        })?
        .get(0)?;
    drop(rows);
    if version != i64::from(crate::db::migrations::latest_version()) {
        report.add(
            "SCHEMA_INCOMPATIBLE",
            "error",
            version.to_string(),
            "Use a compatible Tokensave version; verification never migrates schemas.",
        );
        return Ok(false);
    }
    let pragma = if deep {
        "PRAGMA integrity_check"
    } else {
        "PRAGMA quick_check"
    };
    let mut rows = tx.query(pragma, ()).await?;
    let mut corrupt = false;
    while let Some(row) = rows.next().await? {
        let message: String = row.get(0)?;
        if message != "ok" {
            corrupt = true;
            report.add(
                "DATABASE_INTEGRITY",
                "error",
                message,
                "Preserve the database and restore from a known-good backup.",
            );
        }
    }
    drop(rows);
    if corrupt {
        return Ok(false);
    }
    for (code, sql) in [
        ("DANGLING_EDGE", "SELECT COUNT(*) FROM edges e WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=e.source) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=e.target)"),
        ("ORPHAN_NODE", "SELECT COUNT(*) FROM nodes n WHERE NOT EXISTS (SELECT 1 FROM files f WHERE f.path=n.file_path)"),
        ("DANGLING_PARENT", "SELECT COUNT(*) FROM nodes n WHERE n.parent_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM nodes p WHERE p.id=n.parent_id)"),
        ("DANGLING_REFERENCE", "SELECT COUNT(*) FROM unresolved_refs r WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=r.from_node_id)"),
        ("ORPHAN_VECTOR", "SELECT COUNT(*) FROM vectors v WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=v.node_id)"),
        ("ORPHAN_AMBIGUOUS_CALL", "SELECT COUNT(*) FROM ambiguous_calls a WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=a.from_node_id)"),
        ("INVALID_DISPATCH_RELATION", "SELECT COUNT(*) FROM trait_dispatch_callers d WHERE NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.concrete_method_id) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.trait_method_id) OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.id=d.caller_id)"),
    ] {
        let mut rows = tx.query(sql, ()).await?;
        if let Some(row) = rows.next().await? {
            let count: i64 = row.get(0)?;
            if count != 0 {
                report.add(code, "error", format!("{count} records"), "Preserve a backup; targeted structural repair is not yet implemented.");
            }
        }
    }
    let canonical_root = root.canonicalize()?;
    let mut rows = tx
        .query(
            "SELECT path, content_hash, modified_at FROM files ORDER BY path",
            (),
        )
        .await?;
    while let Some(row) = rows.next().await? {
        report.checked_files += 1;
        let path: String = row.get(0)?;
        let hash: String = row.get(1)?;
        let modified: i64 = row.get(2)?;
        if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            report.add(
                "INVALID_FINGERPRINT",
                "error",
                &path,
                "Reindex the affected file after preserving a backup.",
            );
        }
        if !relative(&path) {
            report.add(
                "INVALID_FILE_PATH",
                "error",
                &path,
                "Restore a project-relative indexed file path.",
            );
            continue;
        }
        let absolute = root.join(&path);
        match absolute.canonicalize() {
            Ok(actual) if actual.starts_with(&canonical_root) => {}
            Ok(_) => {
                report.add(
                    "SOURCE_OUTSIDE_PROJECT",
                    "warning",
                    &path,
                    "Verify this symlink target manually.",
                );
                continue;
            }
            Err(error) => {
                let code = if error.kind() == std::io::ErrorKind::NotFound {
                    "SOURCE_MISSING"
                } else {
                    "SOURCE_UNREADABLE"
                };
                report.add(
                    code,
                    "warning",
                    &path,
                    "Run tokensave sync after confirming the source change.",
                );
                continue;
            }
        }
        if deep {
            match read_bounded(&absolute, MAX_SOURCE_BYTES)
                .and_then(|bytes| crate::sync::decode_source_bytes(&bytes))
            {
                Ok(source) if crate::sync::content_hash(&source) != hash => report.add(
                    "SOURCE_STALE",
                    "warning",
                    &path,
                    "Run tokensave sync; source drift is not database corruption.",
                ),
                Ok(_) => {}
                Err(_) => report.add(
                    "SOURCE_CHECK_SKIPPED",
                    "warning",
                    &path,
                    "Check readability, encoding, and the 16 MiB deep-check limit manually.",
                ),
            }
        } else if crate::sync::file_stat(&absolute).is_none_or(|(mtime, _)| mtime != modified) {
            report.add(
                "SOURCE_STALE",
                "warning",
                &path,
                "Run tokensave sync; source drift is not database corruption.",
            );
        }
    }
    Ok(true)
}
```

### `flags_mismatch_when_started_from_linked_worktree` — src/worktree.rs:206

Source: [src/worktree.rs:206](../src/worktree.rs#L206).

```rust
    fn flags_mismatch_when_started_from_linked_worktree() {
        // Two real git working trees: a main checkout and a linked
        // worktree. start_path = the linked worktree; index_root = the
        // main checkout. Expect a mismatch.
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        fs::create_dir_all(&main).unwrap();
        run_git(&main, &["init", "--quiet"]);
        // git worktree add requires at least one commit
        fs::write(main.join("README.md"), "hi").unwrap();
        run_git(&main, &["add", "."]);
        run_git(
            &main,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "--quiet",
                "-m",
                "init",
            ],
        );
        let worktree = tmp.path().join("wt");
        run_git(
            &main,
            &["worktree", "add", "--detach", worktree.to_str().unwrap()],
        );
        let mismatch = detect_worktree_index_mismatch(&worktree, &main)
            .expect("expected mismatch when started from linked worktree but index is main");
        assert_eq!(
            mismatch.worktree_root,
            std::fs::canonicalize(&worktree).unwrap()
        );
        assert_eq!(mismatch.index_root, std::fs::canonicalize(&main).unwrap());
    }
```

### `main_with_sibling_worktree` — src/worktree.rs:260

Source: [src/worktree.rs:260](../src/worktree.rs#L260).

```rust
    fn main_with_sibling_worktree(tmp: &Path) -> (PathBuf, PathBuf) {
        let main = tmp.join("main");
        fs::create_dir_all(main.join("packages").join("app")).unwrap();
        run_git(&main, &["init", "--quiet"]);
        fs::write(main.join("packages").join("app").join("README.md"), "hi").unwrap();
        run_git(&main, &["add", "."]);
        run_git(
            &main,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "--quiet",
                "-m",
                "init",
            ],
        );
        let worktree = tmp.join("elsewhere").join("wt");
        run_git(
            &main,
            &["worktree", "add", "--detach", worktree.to_str().unwrap()],
        );
        (main, worktree)
    }
```

### `build_qmark_placeholders` — src/db/queries/mod.rs:31

Source: [src/db/queries/mod.rs:31](../src/db/queries/mod.rs#L31).

```rust
pub(crate) fn build_qmark_placeholders(n: usize) -> String {
    debug_assert!(n > 0, "build_qmark_placeholders called with n == 0");
    // Each "?, " occupies 3 bytes; the last one drops the trailing ", ".
    let mut s = String::with_capacity(n * 3);
    for i in 0..n {
        if i > 0 {
            s.push_str(", ");
        }
        s.push('?');
    }
    s
}
```

### `push_quoted` — src/db/queries/mod.rs:218

Source: [src/db/queries/mod.rs:218](../src/db/queries/mod.rs#L218).

```rust
pub(crate) fn push_quoted(buf: &mut String, s: &str) {
    buf.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            buf.push_str("''");
        } else {
            buf.push(ch);
        }
    }
    buf.push('\'');
}
```

### `push_path_prefix_filter` — src/db/queries/mod.rs:250

Source: [src/db/queries/mod.rs:250](../src/db/queries/mod.rs#L250).

```rust
pub(crate) fn push_path_prefix_filter(buf: &mut String, alias: &str, prefix: &str) {
    let exact = prefix.trim_end_matches('/');
    buf.push('(');
    buf.push_str(alias);
    buf.push_str("file_path = ");
    push_quoted(buf, exact);
    buf.push_str(" OR ");
    buf.push_str(alias);
    buf.push_str("file_path LIKE ");
    push_quoted(buf, &format!("{}/%", escape_like(exact)));
    buf.push_str(" ESCAPE '\\')");
}
```

### `where_clause` — src/db/queries/nodes.rs:78

Source: [src/db/queries/nodes.rs:78](../src/db/queries/nodes.rs#L78).

```rust
    fn where_clause(&self) -> String {
        let mut clauses: Vec<String> = Vec::new();

        if let Some(prefix) = &self.path_prefix {
            let mut sql = String::new();
            push_path_prefix_filter(&mut sql, "", prefix);
            clauses.push(sql);
        }

        if let Some(kinds) = &self.kinds {
            if kinds.is_empty() {
                // An explicit empty set matches nothing; without this the
                // `IN ()` below would be a syntax error.
                return " WHERE 0".to_string();
            }
            let mut sql = String::from("kind IN (");
            for (i, kind) in kinds.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                push_quoted(&mut sql, kind);
            }
            sql.push(')');
            clauses.push(sql);
        }

        if self.public_only {
            let mut sql = String::from("visibility = ");
            push_quoted(&mut sql, Visibility::Pub.as_str());
            clauses.push(sql);
        }

        if let Some(lines) = self.min_lines {
            // `end_line - start_line + 1`, matching the handler. Written with
            // the subtraction on the right so a node whose end precedes its
            // start cannot underflow into a huge span the way saturating
            // integer arithmetic would.
            clauses.push(format!("(end_line + 1 - start_line) >= {lines}"));
        }

        if let Some(needle) = &self.name_contains {
            let pattern = format!("%{}%", escape_like(needle));
            let mut sql = String::from("(LOWER(name) LIKE ");
            push_quoted(&mut sql, &pattern);
            sql.push_str(" ESCAPE '\\' OR LOWER(qualified_name) LIKE ");
            push_quoted(&mut sql, &pattern);
            sql.push_str(" ESCAPE '\\')");
            clauses.push(sql);
        }

        if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        }
    }
```

### `upsert` — src/global_db.rs:441

Source: [src/global_db.rs:441](../src/global_db.rs#L441).

```rust
    pub async fn upsert(&self, project_path: &Path, tokens_saved: u64) {
        let path_str = normalize_project_key(project_path);
        let _ = self
            .conn
            .execute(
                "INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET tokens_saved = ?2",
                params![path_str, tokens_saved as i64],
            )
            .await;
    }
```

### `upsert_droid_turn` — src/global_db.rs:691

Source: [src/global_db.rs:691](../src/global_db.rs#L691).

```rust
    pub async fn upsert_droid_turn(&self, turn: &crate::types::CostTurn) -> bool {
        self.conn
            .execute(
                "INSERT INTO turns
                 (message_id, project_hash, session_id, model, timestamp,
                  input_tokens, output_tokens, cache_write_tokens, cache_read_tokens,
                  cost_usd, category, tool_names, agent, credits)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(message_id) DO UPDATE SET
                     project_hash        = excluded.project_hash,
                     session_id          = excluded.session_id,
                     model               = excluded.model,
                     input_tokens        = MAX(excluded.input_tokens,        turns.input_tokens),
                     output_tokens       = MAX(excluded.output_tokens,       turns.output_tokens),
                     cache_write_tokens  = MAX(excluded.cache_write_tokens,  turns.cache_write_tokens),
                     cache_read_tokens   = MAX(excluded.cache_read_tokens,   turns.cache_read_tokens),
                     cost_usd            = excluded.cost_usd,
                     category            = excluded.category,
                     tool_names          = excluded.tool_names,
                     agent               = excluded.agent,
                     credits             = COALESCE(excluded.credits, turns.credits)
                 WHERE
                     excluded.input_tokens        >= turns.input_tokens
                     AND excluded.output_tokens       >= turns.output_tokens
                     AND excluded.cache_write_tokens  >= turns.cache_write_tokens
                     AND excluded.cache_read_tokens   >= turns.cache_read_tokens
                     AND (excluded.credits IS NULL
                          OR turns.credits IS NULL
                          OR excluded.credits >= turns.credits)
                     AND (
                         excluded.input_tokens        > turns.input_tokens
                         OR excluded.output_tokens       > turns.output_tokens
                         OR excluded.cache_write_tokens  > turns.cache_write_tokens
                         OR excluded.cache_read_tokens   > turns.cache_read_tokens
                         OR (excluded.credits IS NOT NULL
                             AND (turns.credits IS NULL
                                  OR excluded.credits > turns.credits))
                     )",
                params![
                    turn.message_id.clone(),
                    turn.project_hash.clone(),
                    turn.session_id.clone(),
                    turn.model.clone(),
                    turn.timestamp as i64,
                    turn.input_tokens as i64,
                    turn.output_tokens as i64,
                    turn.cache_write_tokens as i64,
                    turn.cache_read_tokens as i64,
                    turn.cost_usd,
                    turn.category.clone(),
                    turn.tool_names.clone(),
                    turn.agent.clone(),
                    turn.credits.map(|c| c as i64),
                ],
            )
            .await
            .is_ok_and(|n| n > 0)
    }
```

### `scalar` — src/db/compatibility.rs:26

Source: [src/db/compatibility.rs:26](../src/db/compatibility.rs#L26).

```rust
async fn scalar(conn: &Connection, sql: &str) -> Result<i64> {
    let mut rows = conn.query(sql, ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| refusal("missing database header"))?;
    Ok(row.get(0)?)
}
```

### `check_channel` — src/db/compatibility.rs:41

Source: [src/db/compatibility.rs:41](../src/db/compatibility.rs#L41).

```rust
async fn check_channel(conn: &Connection, latest: u32, beta: bool) -> Result<()> {
    let origin = scalar(conn, "PRAGMA application_id").await?;
    match origin {
        0 | STABLE => {},
        BETA if beta => {},
        BETA => return Err(refusal(
            "database was created by beta; use a beta build to access it. \
             Stable requires its own index. Preserve this database; do not force a rebuild."
        )),
        other => return Err(refusal(format!(
            "unrecognized database application ID {other}; preserve this database and use a compatible build."
        ))),
    }
    let version = scalar(conn, "PRAGMA user_version").await?;
    if version < 0 || version > i64::from(latest) {
        return Err(refusal(format!(
            "database schema version {version} is newer than or unsupported by this build (maximum {latest}); \
             use a compatible newer build. Preserve this database; do not force a rebuild."
        )));
    }
    Ok(())
}
```

### `mark_empty` — src/db/compatibility.rs:90

Source: [src/db/compatibility.rs:90](../src/db/compatibility.rs#L90).

```rust
pub async fn mark_empty(conn: &Connection) -> Result<()> {
    let count = scalar(conn, "SELECT count(*) FROM sqlite_schema").await?;
    let origin = scalar(conn, "PRAGMA application_id").await?;
    if count == 0 && origin == 0 {
        let origin = if crate::cloud::is_beta() {
            BETA
        } else {
            STABLE
        };
        conn.execute(&format!("PRAGMA application_id = {origin}"), ())
            .await?;
    }
    Ok(())
}
```

### `channel_policy_preserves_origin` — src/db/compatibility.rs:111

Source: [src/db/compatibility.rs:111](../src/db/compatibility.rs#L111).

```rust
    async fn channel_policy_preserves_origin() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        for (origin, stable_ok, beta_ok) in [
            (0, true, true),
            (STABLE, true, true),
            (BETA, false, true),
            (17, false, false),
        ] {
            conn.execute(&format!("PRAGMA application_id = {origin}"), ())
                .await
                .unwrap();
            assert_eq!(check_channel(&conn, 18, false).await.is_ok(), stable_ok);
            assert_eq!(check_channel(&conn, 18, true).await.is_ok(), beta_ok);
            assert_eq!(
                scalar(&conn, "PRAGMA application_id").await.unwrap(),
                origin
            );
        }
    }
```

## Coverage and limitations

Coverage was checked against SQL-bearing string literals and database execution entry points (`query`, `execute`, `prepare`, and `execute_batch`) in the current `src/` tree. SQL passed through variables or constants is represented by the defining template and, for constructed SQL, its builder context. This static inventory does not enumerate concrete values supplied by users or runtime-generated batches. It includes historical migrations and inline tests; their presence does not imply that every statement runs on every invocation. Source line links apply to the recorded snapshot and may move after edits.

The tokensave graph tools were consulted first, but reported a stale index and a different served checkout. The inventory therefore uses the current working-tree source rather than relying on graph contents. This documentation-only change does not modify application behavior; no database statements or application tests were run.


## Graphify export additions (Refs #791)

Source: [src/graphify.rs](../src/graphify.rs), `export_file`. This section adds
five occurrences and one construction/binding context to the existing inventory.
No existing query was changed, moved or removed. Snapshot inspected 2026-10-28.
The dedicated read-only connection uses master's libsql API. The transaction API owns BEGIN,
COMMIT and rollback on drop; those driver-generated statements introduce no new
application SQL literals.

#### Q0384 — `export_file`: Read-only connection limits

```sql
PRAGMA query_only=1; PRAGMA busy_timeout=1000; PRAGMA cache_size=-2048;
```

Fixed batch, no parameters; executes before the read transaction.

#### Q0385 — `export_file`: Current-schema check

```sql
PRAGMA user_version
```

No bindings; exactly one scalar row. Refuse versions unequal to latest_version().

#### Q0386 — `export_file`: Selected-file freshness

```sql
SELECT substr(content_hash,1,65) FROM files WHERE path=?1 AND kind='code'
```

?1 is the validated repository-relative file path. At most one row; compare with the bounded UTF-8 source SHA-256.

#### Q0387 — `export_file`: Bounded node projection

```sql
SELECT substr(id,1,4097), substr(qualified_name,1,4097), substr(kind,1,4097), start_line, end_line FROM nodes WHERE file_path=?1 ORDER BY id LIMIT ?2
```

?1 is the selected file; ?2 is max_nodes + 1, integer <=1001. Extra row or text >4096 characters refuses export. No source body/signature/docstring projected.

#### Q0388 — `export_file`: Bounded internal edges

```sql
SELECT substr(e.source,1,4097), substr(e.target,1,4097), substr(e.kind,1,4097), e.line, e.resolved_by FROM edges e JOIN nodes s ON s.id=e.source JOIN nodes t ON t.id=e.target WHERE s.file_path=?1 AND t.file_path=?1 ORDER BY e.source,e.target,e.kind,e.line,e.id LIMIT ?2
```

?1 is the same file for both endpoints; ?2 is max_edges + 1, integer <=5001. Extra row refuses export. id breaks ties without collapsing multiplicity; line and nullable numeric resolver provenance are retained.

Construction context: max_nodes is validated in 1..=1000 and max_edges in 1..=5000 before integer conversion. SQL templates are fixed; no user text is interpolated. Queries Q0385–Q0388 use one read transaction for a consistent DB snapshot. Output is published only after successful reads, budget checks, source/branch rechecks and capped serialization. No migrations or writes occur.
