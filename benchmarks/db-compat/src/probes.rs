//! Probe catalog. Each probe exercises one SQL feature tokensave depends on
//! (see `docs/benchmarks/database-workload-inventory.md`, section 4), using
//! statements shaped like the production ones.
//!
//! Every expectation is literal SQLite behavior: the libsql control arm must
//! reproduce it, which validates the probe before any other engine's result
//! is interpreted.

/// Which engine runs a step: the engine under test, or the libsql control.
/// Mixing them tests on-disk file compatibility in both directions.
#[derive(Clone, Copy)]
pub enum Who {
    Tested,
    Control,
}

pub enum Expect {
    /// Exact rendered rows, in order.
    Rows(Vec<String>),
    /// The query must fail (e.g. a constraint violation) and leave the
    /// database as `verify` shows; the failure alone could have another cause.
    Error {
        verify: &'static str,
        rows: Vec<String>,
    },
}

pub struct Probe {
    pub id: &'static str,
    pub gate: &'static str,
    /// Batch run on `main.db`; `{dir}` is replaced with the probe directory.
    pub setup: String,
    pub setup_by: Who,
    /// When set, the query runs on a fresh connection to this file (relative
    /// to the probe directory) instead of the setup connection.
    pub reopen: Option<&'static str>,
    /// Keep the setup connection open while the query runs on the reopened
    /// file: two independent handles on one database.
    pub keep_setup_open: bool,
    /// Statement run with `query()` (rows ignored) after `setup`, for
    /// row-returning pragmas that turso's `execute_batch` rejects.
    pub pre_query: Option<&'static str>,
    pub query_by: Who,
    pub query: String,
    pub expect: Expect,
}

fn rows(r: &[&str]) -> Expect {
    Expect::Rows(r.iter().map(|s| (*s).to_string()).collect())
}

fn fails(verify: &'static str, r: &[&str]) -> Expect {
    Expect::Error {
        verify,
        rows: r.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn probe(id: &'static str, gate: &'static str, setup: &str, query: &str, expect: Expect) -> Probe {
    Probe {
        id,
        gate,
        setup: setup.to_string(),
        setup_by: Who::Tested,
        reopen: None,
        keep_setup_open: false,
        pre_query: None,
        query_by: Who::Tested,
        query: query.to_string(),
        expect,
    }
}

/// Production `nodes` / `nodes_fts` shape (migrations.rs `create_schema`),
/// reduced to the FTS-relevant columns, with the three sync triggers.
const NODES_FTS: &str = "
CREATE TABLE nodes (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, qualified_name TEXT NOT NULL,
    docstring TEXT, signature TEXT, search_terms TEXT NOT NULL DEFAULT ''
);
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
INSERT INTO nodes (id, name, qualified_name, docstring, signature) VALUES
    ('a', 'parse_file', 'crate::parser::parse_file', 'Parses a file into nodes', 'fn parse_file(path)'),
    ('b', 'ParserState', 'crate::parser::ParserState', 'Running state of the parser', 'struct ParserState'),
    ('c', 'run_query', 'crate::db::run_query', 'Runs a database query', 'fn run_query(sql)'),
    ('d', 'format_output', 'crate::fmt::format_output', 'Formats data-api output', 'fn format_output()');
";

/// `SELECT` prefix matching `search.rs`: join FTS rows back to `nodes`.
const FTS_IDS: &str =
    "SELECT n.id FROM nodes_fts f JOIN nodes n ON n.rowid = f.rowid WHERE nodes_fts MATCH";

/// `extends` graph with a cycle (A→B→C→A) and an entry (D→B).
const EXTENDS: &str = "
CREATE TABLE edges (source TEXT NOT NULL, target TEXT NOT NULL, kind TEXT NOT NULL, line INTEGER);
INSERT INTO edges VALUES
    ('A', 'B', 'extends', 1), ('B', 'C', 'extends', 2),
    ('C', 'A', 'extends', 3), ('D', 'B', 'extends', 4),
    ('D', 'X', 'calls', 5);
";

/// `trait_dispatch_call_insert` / `_call_delete` from migrations.rs, verbatim
/// apart from the reduced `nodes` table.
const TRAIT_DISPATCH: &str = "
CREATE TABLE nodes (id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, parent_id TEXT);
CREATE TABLE edges (id INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT NOT NULL, target TEXT NOT NULL, kind TEXT NOT NULL, line INTEGER);
CREATE TABLE trait_dispatch_callers (
    concrete_method_id TEXT NOT NULL, trait_method_id TEXT NOT NULL,
    caller_id TEXT NOT NULL, line INTEGER NOT NULL DEFAULT -1,
    PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
);
CREATE TRIGGER trait_dispatch_call_insert
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
CREATE TRIGGER trait_dispatch_call_delete
AFTER DELETE ON edges WHEN OLD.kind = 'calls' BEGIN
    DELETE FROM trait_dispatch_callers
     WHERE caller_id = OLD.source
       AND trait_method_id = OLD.target
       AND line = COALESCE(OLD.line, -1);
END;
INSERT INTO nodes VALUES
    ('T', 'trait', 'T', NULL), ('T.m', 'method', 'm', 'T'),
    ('S', 'struct', 'S', NULL), ('S.m', 'method', 'm', 'S'),
    ('X', 'function', 'x', NULL);
INSERT INTO edges (source, target, kind, line) VALUES ('S', 'T', 'implements', 1);
INSERT INTO edges (source, target, kind, line) VALUES ('X', 'T.m', 'calls', 7);
";

const FK: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE nodes (id TEXT PRIMARY KEY);
CREATE TABLE edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT NOT NULL, target TEXT NOT NULL,
    FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
);
INSERT INTO nodes VALUES ('a'), ('b'), ('c');
INSERT INTO edges (source, target) VALUES ('a', 'b'), ('b', 'c'), ('c', 'a');
";

const UNIQUE_EDGES: &str = "
CREATE TABLE edges (id INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT NOT NULL, target TEXT NOT NULL, kind TEXT NOT NULL, line INTEGER);
CREATE UNIQUE INDEX idx_edges_unique ON edges(source, target, kind, COALESCE(line, -1));
INSERT INTO edges (source, target, kind, line) VALUES ('a', 'b', 'calls', NULL);
";

/// Non-FTS fixture for lifecycle probes, so an FTS gap cannot mask them.
const PLAIN: &str = "
CREATE TABLE nodes (id TEXT PRIMARY KEY, name TEXT NOT NULL);
CREATE INDEX idx_nodes_lower_name ON nodes(lower(name));
INSERT INTO nodes VALUES ('a', 'Alpha'), ('b', 'Beta'), ('c', 'Gamma');
";

const PLAIN_QUERY: &str = "SELECT id FROM nodes WHERE lower(name) <> 'beta' ORDER BY id";

#[allow(clippy::too_many_lines)]
pub fn catalog() -> Vec<Probe> {
    let mut p = vec![
        // --- FTS5 -------------------------------------------------------
        probe(
            "fts5.external_content_prefix_or",
            "FTS5",
            NODES_FTS,
            &format!("{FTS_IDS} '\"pars\"* OR \"zzz\"*' ORDER BY n.id"),
            rows(&["a", "b"]),
        ),
        probe(
            "fts5.porter_stemming",
            "FTS5",
            NODES_FTS,
            &format!("{FTS_IDS} 'running' ORDER BY n.id"),
            rows(&["b", "c"]),
        ),
        probe(
            "fts5.phrase",
            "FTS5",
            NODES_FTS,
            &format!("{FTS_IDS} '\"database query\"' ORDER BY n.id"),
            rows(&["c"]),
        ),
        probe(
            "fts5.quoted_punctuation_prefix",
            "FTS5",
            NODES_FTS,
            &format!("{FTS_IDS} '\"data-api\"*' ORDER BY n.id"),
            rows(&["d"]),
        ),
        probe(
            "fts5.bm25_weighted_order",
            "FTS5",
            NODES_FTS,
            "SELECT n.id FROM nodes_fts f \
             JOIN nodes n ON n.rowid = f.rowid WHERE nodes_fts MATCH '\"pars\"* OR \"run\"*' \
             ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0), n.id",
            rows(&["b", "a", "c"]),
        ),
        probe(
            "fts5.delete_trigger",
            "FTS5",
            &format!("{NODES_FTS} DELETE FROM nodes WHERE id = 'a';"),
            &format!("{FTS_IDS} '\"pars\"*' ORDER BY n.id"),
            rows(&["b"]),
        ),
        probe(
            "fts5.update_trigger",
            "FTS5",
            &format!("{NODES_FTS} UPDATE nodes SET name = 'zzz_renamed' WHERE id = 'd';"),
            &format!("{FTS_IDS} '\"zzz\"*' ORDER BY n.id"),
            rows(&["d"]),
        ),
        probe(
            "fts5.rebuild_command",
            "FTS5",
            &format!("{NODES_FTS} INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');"),
            &format!("{FTS_IDS} '\"pars\"*' ORDER BY n.id"),
            rows(&["a", "b"]),
        ),
        probe(
            "fts5.unindexed_columns",
            "FTS5",
            "CREATE VIRTUAL TABLE executable_body_fts USING fts5(
                 node_id UNINDEXED, file_path UNINDEXED, body, tokenize='unicode61');
             INSERT INTO executable_body_fts VALUES ('n1', 'src/a.rs', 'for item in items loop');
             INSERT INTO executable_body_fts VALUES ('n2', 'src/b.rs', 'n1 appears only here');",
            "SELECT node_id FROM executable_body_fts WHERE executable_body_fts MATCH 'n1' \
             UNION ALL SELECT node_id FROM executable_body_fts WHERE executable_body_fts MATCH 'loop'",
            rows(&["n2", "n1"]),
        ),
        // --- Recursive CTEs --------------------------------------------
        probe(
            "cte.recursive_union_all_cycle_guard",
            "Recursive CTE",
            EXTENDS,
            "WITH RECURSIVE hierarchy(leaf_id, current_id, depth, path) AS (
                 SELECT e.source, e.target, 1, ',' || e.source || ',' || e.target || ','
                 FROM edges e WHERE e.kind = 'extends'
                 UNION ALL
                 SELECT h.leaf_id, e.target, h.depth + 1, h.path || e.target || ','
                 FROM hierarchy h
                 JOIN edges e ON e.source = h.current_id AND e.kind = 'extends'
                 WHERE h.depth < 50 AND instr(h.path, ',' || e.target || ',') = 0
             ),
             leaf_depths AS (
                 SELECT leaf_id, MAX(depth) AS max_depth FROM hierarchy GROUP BY leaf_id
             )
             SELECT leaf_id, max_depth FROM leaf_depths ORDER BY leaf_id",
            rows(&["A|2", "B|2", "C|2", "D|3"]),
        ),
        probe(
            "cte.recursive_union_dedup",
            "Recursive CTE",
            EXTENDS,
            "WITH RECURSIVE ancestry(cls, base) AS (
                 SELECT e.source, e.target FROM edges e WHERE e.kind = 'extends' AND e.source = 'D'
                 UNION
                 SELECT a.cls, e.target FROM ancestry a
                 JOIN edges e ON e.source = a.base AND e.kind = 'extends'
             )
             SELECT base FROM ancestry ORDER BY base",
            rows(&["A", "B", "C"]),
        ),
        // --- Window functions ------------------------------------------
        probe(
            "window.named_first_value_row_number",
            "Window",
            "CREATE TABLE decl (id INTEGER, kind TEXT, name TEXT, file TEXT, line INTEGER);
             INSERT INTO decl VALUES (1, 'class', 'Foo', 'a.rb', 1), (2, 'class', 'Foo', 'b.rb', 3),
                                     (3, 'module', 'Foo', 'a.rb', 5), (4, 'class', 'Foo', 'a.rb', 9);",
            "SELECT id, FIRST_VALUE(id) OVER declaration, ROW_NUMBER() OVER declaration
             FROM decl
             WINDOW declaration AS (PARTITION BY kind, name ORDER BY file, line, id)
             ORDER BY id",
            rows(&["1|1|1", "2|1|3", "3|3|1", "4|1|2"]),
        ),
        // --- Triggers --------------------------------------------------
        probe(
            "trigger.when_join_insert",
            "Triggers",
            TRAIT_DISPATCH,
            "SELECT concrete_method_id, trait_method_id, caller_id, line FROM trait_dispatch_callers",
            rows(&["S.m|T.m|X|7"]),
        ),
        probe(
            "trigger.when_delete",
            "Triggers",
            &format!("{TRAIT_DISPATCH} DELETE FROM edges WHERE kind = 'calls';"),
            "SELECT count(*) FROM trait_dispatch_callers",
            rows(&["0"]),
        ),
        // --- Foreign keys ----------------------------------------------
        probe(
            "fk.on_delete_cascade",
            "Foreign keys",
            &format!("{FK} DELETE FROM nodes WHERE id = 'a';"),
            "SELECT source || '>' || target FROM edges ORDER BY id",
            rows(&["b>c"]),
        ),
        probe(
            "fk.enforced_when_on",
            "Foreign keys",
            FK,
            "INSERT INTO edges (source, target) VALUES ('missing', 'a')",
            fails("SELECT count(*) FROM edges", &["3"]),
        ),
        probe(
            "fk.toggle_off_for_bulk_load",
            "Foreign keys",
            &format!("{FK} PRAGMA foreign_keys = OFF; INSERT INTO edges (source, target) VALUES ('missing', 'a');"),
            "SELECT count(*) FROM edges",
            rows(&["4"]),
        ),
        // --- Indexes and conflict handling ------------------------------
        probe(
            "index.expression_lower",
            "Expression index",
            "CREATE TABLE nodes (id TEXT PRIMARY KEY, name TEXT NOT NULL);
             CREATE INDEX idx_nodes_lower_name ON nodes(lower(name));
             INSERT INTO nodes VALUES ('1', 'ParseFile'), ('2', 'other');",
            "SELECT id FROM nodes WHERE lower(name) = 'parsefile'",
            rows(&["1"]),
        ),
        probe(
            "index.unique_expression_rejects_duplicate",
            "Expression index",
            UNIQUE_EDGES,
            "INSERT INTO edges (source, target, kind, line) VALUES ('a', 'b', 'calls', NULL)",
            fails("SELECT count(*) FROM edges", &["1"]),
        ),
        probe(
            "conflict.insert_or_ignore_expression_unique",
            "Conflict clauses",
            &format!(
                "{UNIQUE_EDGES} INSERT OR IGNORE INTO edges (source, target, kind, line) VALUES ('a', 'b', 'calls', NULL);
                 INSERT OR IGNORE INTO edges (source, target, kind, line) VALUES ('a', 'b', 'calls', 3);"
            ),
            "SELECT count(*) FROM edges",
            rows(&["2"]),
        ),
        probe(
            "conflict.on_conflict_do_update",
            "Conflict clauses",
            "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO metadata VALUES ('k', 'old');
             INSERT INTO metadata VALUES ('k', 'new') ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            "SELECT value FROM metadata",
            rows(&["new"]),
        ),
        probe(
            "conflict.insert_or_replace",
            "Conflict clauses",
            "CREATE TABLE files (path TEXT PRIMARY KEY, hash TEXT NOT NULL);
             INSERT INTO files VALUES ('a', 'h1');
             INSERT OR REPLACE INTO files VALUES ('a', 'h2');",
            "SELECT path, hash FROM files",
            rows(&["a|h2"]),
        ),
        probe(
            "table.autoincrement_no_reuse",
            "Conflict clauses",
            "CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT, v TEXT);
             INSERT INTO t (v) VALUES ('x'), ('y');
             DELETE FROM t WHERE id = 2;
             INSERT INTO t (v) VALUES ('z');",
            "SELECT id, v FROM t ORDER BY id",
            rows(&["1|x", "3|z"]),
        ),
        // --- Transactions ----------------------------------------------
        probe(
            "txn.rollback",
            "Transactions",
            "CREATE TABLE t (v INTEGER); BEGIN; INSERT INTO t VALUES (1); ROLLBACK;",
            "SELECT count(*) FROM t",
            rows(&["0"]),
        ),
        probe(
            "txn.begin_exclusive",
            "Transactions",
            "CREATE TABLE t (v INTEGER); BEGIN EXCLUSIVE; INSERT INTO t VALUES (1); COMMIT;",
            "SELECT count(*) FROM t",
            rows(&["1"]),
        ),
        // --- Pragmas (read back: accepted is not the same as effective) --
        probe(
            "pragma.page_size",
            "Pragmas",
            "PRAGMA page_size = 8192; CREATE TABLE t (v INTEGER);",
            "PRAGMA page_size",
            rows(&["8192"]),
        ),
        probe(
            "api.execute_batch_row_returning_pragma",
            "Rust API",
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;",
            "PRAGMA journal_mode",
            rows(&["wal"]),
        ),
        probe(
            "pragma.journal_mode_wal",
            "Pragmas",
            "",
            "PRAGMA journal_mode = WAL",
            rows(&["wal"]),
        ),
        probe(
            "pragma.synchronous_normal",
            "Pragmas",
            "PRAGMA synchronous = NORMAL;",
            "PRAGMA synchronous",
            rows(&["1"]),
        ),
        probe(
            "pragma.busy_timeout",
            "Pragmas",
            "PRAGMA busy_timeout = 120000;",
            "PRAGMA busy_timeout",
            rows(&["120000"]),
        ),
        probe(
            "pragma.cache_size",
            "Pragmas",
            "PRAGMA cache_size = -65536;",
            "PRAGMA cache_size",
            rows(&["-65536"]),
        ),
        probe(
            "pragma.temp_store_memory",
            "Pragmas",
            "PRAGMA temp_store = MEMORY;",
            "PRAGMA temp_store",
            rows(&["2"]),
        ),
        probe(
            "pragma.mmap_size",
            "Pragmas",
            "PRAGMA mmap_size = 268435456;",
            "PRAGMA mmap_size",
            rows(&["268435456"]),
        ),
        probe(
            "pragma.user_version",
            "Pragmas",
            "PRAGMA user_version = 18;",
            "PRAGMA user_version",
            rows(&["18"]),
        ),
        probe(
            "pragma.table_info",
            "Pragmas",
            "CREATE TABLE nodes (id TEXT PRIMARY KEY, kind TEXT NOT NULL, search_terms TEXT NOT NULL DEFAULT '');",
            "PRAGMA table_info(nodes)",
            rows(&["0|id|TEXT|0|NULL|1", "1|kind|TEXT|1|NULL|0", "2|search_terms|TEXT|1|''|0"]),
        ),
        probe(
            "pragma.query_only_blocks_writes",
            "Pragmas",
            "CREATE TABLE t (v INTEGER); PRAGMA query_only = 1;",
            "INSERT INTO t VALUES (1)",
            fails("SELECT count(*) FROM t", &["0"]),
        ),
        probe(
            "pragma.quick_check",
            "Integrity",
            PLAIN,
            "PRAGMA quick_check",
            rows(&["ok"]),
        ),
        probe(
            "pragma.integrity_check",
            "Integrity",
            PLAIN,
            "PRAGMA integrity_check",
            rows(&["ok"]),
        ),
        // --- Maintenance -----------------------------------------------
        probe(
            "maint.vacuum_analyze",
            "Maintenance",
            "CREATE TABLE t (v INTEGER); INSERT INTO t VALUES (1); DELETE FROM t; VACUUM; ANALYZE;",
            "SELECT count(*) FROM t",
            rows(&["0"]),
        ),
        // --- Scalar functions used in production SQL ---------------------
        probe(
            "func.scalars",
            "Functions",
            "",
            "SELECT ltrim('::a::b', ':'), substr('abcdef', 2, 3), instr('abc', 'c'), \
             length('héllo'), lower('ÀB'), COALESCE(NULL, -1), 'x.rb' LIKE '%.rb'",
            rows(&["a::b|bcd|3|5|Àb|-1|1"]),
        ),
    ];

    let mut checkpoint = probe(
        "pragma.wal_checkpoint_truncate",
        "Pragmas",
        "CREATE TABLE t (v INTEGER); INSERT INTO t VALUES (1);",
        "PRAGMA wal_checkpoint(TRUNCATE)",
        rows(&["0|0|0"]),
    );
    checkpoint.pre_query = Some("PRAGMA journal_mode = WAL");
    p.push(checkpoint);

    // Snapshot and persistence probes reopen a file in a fresh connection.
    let mut vacuum_into = probe(
        "maint.vacuum_into_snapshot",
        "Maintenance",
        &format!("{PLAIN} VACUUM INTO '{{dir}}/snapshot.db';"),
        PLAIN_QUERY,
        rows(&["a", "c"]),
    );
    vacuum_into.reopen = Some("snapshot.db");
    p.push(vacuum_into);

    let mut reopen = probe(
        "persist.reopen",
        "Persistence",
        PLAIN,
        PLAIN_QUERY,
        rows(&["a", "c"]),
    );
    reopen.reopen = Some("main.db");
    p.push(reopen);

    let mut two_handles = probe(
        "concurrency.second_handle_sees_commit",
        "Concurrency",
        PLAIN,
        PLAIN_QUERY,
        rows(&["a", "c"]),
    );
    two_handles.reopen = Some("main.db");
    two_handles.keep_setup_open = true;
    p.push(two_handles);

    // File compatibility: one engine writes, the other reads.
    let mut plain_sqlite_written = probe(
        "file.read_sqlite_written_db",
        "File compatibility",
        PLAIN,
        PLAIN_QUERY,
        rows(&["a", "c"]),
    );
    plain_sqlite_written.setup_by = Who::Control;
    plain_sqlite_written.reopen = Some("main.db");
    p.push(plain_sqlite_written);

    let mut plain_tested_written = probe(
        "file.sqlite_reads_tested_written_db",
        "File compatibility",
        PLAIN,
        PLAIN_QUERY,
        rows(&["a", "c"]),
    );
    plain_tested_written.query_by = Who::Control;
    plain_tested_written.reopen = Some("main.db");
    p.push(plain_tested_written);

    let mut sqlite_written = probe(
        "file.read_sqlite_written_fts_db",
        "File compatibility",
        NODES_FTS,
        &format!("{FTS_IDS} '\"pars\"*' ORDER BY n.id"),
        rows(&["a", "b"]),
    );
    sqlite_written.setup_by = Who::Control;
    sqlite_written.reopen = Some("main.db");
    p.push(sqlite_written);

    let mut tested_written = probe(
        "file.sqlite_reads_tested_written_fts_db",
        "File compatibility",
        NODES_FTS,
        &format!("{FTS_IDS} '\"pars\"*' ORDER BY n.id"),
        rows(&["a", "b"]),
    );
    tested_written.query_by = Who::Control;
    tested_written.reopen = Some("main.db");
    p.push(tested_written);

    p
}
