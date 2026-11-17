//! The two search backends under comparison.
//!
//! * [`AdapterKind::LibsqlFts5`]: tokensave's production schema verbatim —
//!   external-content `nodes_fts` (`porter unicode61`) kept in sync by
//!   triggers, contentful `executable_body_fts` (`unicode61`), and
//!   `memory_decisions_fts` — and production's query shapes.
//! * [`AdapterKind::TursoTantivy`] and its tokenizer variants: turso's native
//!   `CREATE INDEX … USING fts` (Tantivy) on the same base tables. The index
//!   is maintained by the engine on every INSERT/UPDATE/DELETE, so no
//!   triggers exist. FTS5 virtual-table syntax is never sent to turso.
//!
//! Both arms share the base-table definitions, pragmas, source rows,
//! transaction boundaries and the engine-neutral query intents of
//! [`crate::query`].

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::corpus::{BodyRec, Decision, NodeRec};
use crate::db::{Db, Val};
use crate::query::{render_fts5, render_tantivy, PrefixPolicy, Q};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterKind {
    /// `LibsqlFts5Adapter`: production engine and schema.
    LibsqlFts5,
    /// `TursoTantivyAdapter` with the `default` tokenizer (lowercase,
    /// alphanumeric split, tokens over 40 bytes dropped).
    TursoTantivy,
    /// `TursoTantivyAdapter` with the `simple` tokenizer: keeps long tokens
    /// but does not lowercase. Configuration-sensitivity arm.
    TursoTantivySimple,
    /// `TursoTantivyAdapter` with the `ngram` tokenizer: substring matching
    /// as the only native stand-in for prefix queries. Approximation arm.
    TursoTantivyNgram,
}

pub const NGRAM_WINDOW: (usize, usize) = (2, 8);

impl AdapterKind {
    pub const ALL: [AdapterKind; 4] = [
        Self::LibsqlFts5,
        Self::TursoTantivy,
        Self::TursoTantivySimple,
        Self::TursoTantivyNgram,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::LibsqlFts5 => "libsql-fts5",
            Self::TursoTantivy => "turso-tantivy",
            Self::TursoTantivySimple => "turso-tantivy-simple",
            Self::TursoTantivyNgram => "turso-tantivy-ngram",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.label() == s)
    }

    pub fn is_turso(self) -> bool {
        self != Self::LibsqlFts5
    }

    /// Prefix handling used when a caller does not ask for degradation.
    pub fn native_prefix_policy(self) -> PrefixPolicy {
        match self {
            Self::TursoTantivyNgram => PrefixPolicy::NgramSubstring,
            _ => PrefixPolicy::Strict,
        }
    }

    fn tokenizer_clause(self) -> String {
        match self {
            Self::LibsqlFts5 | Self::TursoTantivy => "tokenizer='default'".into(),
            Self::TursoTantivySimple => "tokenizer='simple'".into(),
            Self::TursoTantivyNgram => format!(
                "tokenizer='ngram', min_gram={}, max_gram={}",
                NGRAM_WINDOW.0, NGRAM_WINDOW.1
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub id: String,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SearchError {
    /// The engine cannot express the intent (no substitution was made).
    Unsupported(String),
    /// turso planned the statement without the FTS index method, so
    /// `fts_match` would run as a naive any-token scalar and `fts_score`
    /// would return 0.0.
    PlanFallback(String),
    Engine(String),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(m) => write!(f, "unsupported: {m}"),
            Self::PlanFallback(m) => write!(f, "plan fallback: {m}"),
            Self::Engine(m) => write!(f, "engine error: {m}"),
        }
    }
}

pub const NODE_COLS: &str = "name, qualified_name, docstring, signature, search_terms";
/// Production `bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0)` weights.
pub const NODE_WEIGHTS: &str =
    "name=10.0,qualified_name=5.0,docstring=1.0,signature=2.0,search_terms=3.0";

const BASE_SCHEMA: &str = "
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    docstring TEXT,
    signature TEXT,
    search_terms TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_nodes_file_path ON nodes(file_path);
CREATE TABLE memory_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    text TEXT NOT NULL,
    reason TEXT,
    created_at INTEGER NOT NULL,
    files TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX idx_memory_decisions_created_at ON memory_decisions(created_at);
";

/// Production `create_schema` FTS objects (migrations.rs), verbatim.
const FTS5_SCHEMA: &str = "
CREATE VIRTUAL TABLE nodes_fts USING fts5(
    name, qualified_name, docstring, signature, search_terms,
    content='nodes', content_rowid='rowid',
    tokenize='porter unicode61'
);
CREATE VIRTUAL TABLE executable_body_fts USING fts5(
    node_id UNINDEXED,
    file_path UNINDEXED,
    body,
    tokenize='unicode61'
);
CREATE VIRTUAL TABLE memory_decisions_fts USING fts5(
    text, reason,
    content='memory_decisions', content_rowid='id'
);
CREATE TRIGGER memory_decisions_fts_insert
    AFTER INSERT ON memory_decisions BEGIN
        INSERT INTO memory_decisions_fts(rowid, text, reason)
        VALUES (NEW.id, NEW.text, NEW.reason);
    END;
CREATE TRIGGER memory_decisions_fts_delete
    AFTER DELETE ON memory_decisions BEGIN
        INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
        VALUES ('delete', OLD.id, OLD.text, OLD.reason);
    END;
CREATE TRIGGER memory_decisions_fts_update
    AFTER UPDATE ON memory_decisions BEGIN
        INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
        VALUES ('delete', OLD.id, OLD.text, OLD.reason);
        INSERT INTO memory_decisions_fts(rowid, text, reason)
        VALUES (NEW.id, NEW.text, NEW.reason);
    END;
";

/// Production `nodes_fts` triggers (dropped by `begin_bulk_load`, restored
/// by `end_bulk_load`).
const FTS5_NODE_TRIGGERS: &str = "
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
";

/// Production `end_bulk_load` FTS repopulation.
const FTS5_BULK_POPULATE: &str = "
INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
    SELECT rowid, name, qualified_name, docstring, signature, search_terms FROM nodes;
";

/// Production-equivalent pragmas. `mmap_size` is omitted for both engines:
/// turso accepts it without effect (see the SQL probes), so keeping it for
/// libsql only would give the control arm an advantage the other cannot use.
pub const PRAGMAS: [&str; 5] = [
    "PRAGMA page_size = 8192",
    "PRAGMA journal_mode = WAL",
    "PRAGMA synchronous = NORMAL",
    "PRAGMA cache_size = -65536",
    "PRAGMA temp_store = MEMORY",
];

pub struct Adapter {
    pub kind: AdapterKind,
    pub db: Db,
    pub path: PathBuf,
    /// Statements whose plan was checked once (bounded by the fixed set of
    /// query shapes the harness issues).
    verified_plans: Mutex<HashSet<String>>,
}

pub const DB_FILE: &str = "fts.db";
/// Rows per multi-row node INSERT, as in production `insert_nodes`.
pub const NODE_INSERT_CHUNK: usize = 200;

impl Adapter {
    /// Opens (or creates) `dir/fts.db` and applies the shared pragmas.
    pub async fn open(kind: AdapterKind, dir: &Path) -> Result<Self, String> {
        let path = dir.join(DB_FILE);
        let db = if kind.is_turso() {
            Db::open_turso(&path).await?
        } else {
            Db::open_libsql(&path).await?
        };
        for pragma in PRAGMAS {
            // turso rejects row-returning pragmas in `execute_batch`; `query`
            // works on both engines.
            db.query(pragma, &[])
                .await
                .map_err(|e| format!("{pragma}: {e}"))?;
        }
        Ok(Self {
            kind,
            db,
            path,
            verified_plans: Mutex::new(HashSet::new()),
        })
    }

    pub async fn connect_another(&self) -> Result<Self, String> {
        Ok(Self {
            kind: self.kind,
            db: self.db.connect_another(&self.path).await?,
            path: self.path.clone(),
            verified_plans: Mutex::new(HashSet::new()),
        })
    }

    /// Base tables plus FTS objects, with triggers live (incremental mode).
    pub async fn create_schema(&self) -> Result<(), String> {
        self.create_base().await?;
        self.build_fts().await
    }

    /// Base tables only. For libsql this includes the FTS5 virtual tables
    /// and memory triggers but not the node triggers, which is the state
    /// production's `begin_bulk_load` leaves behind.
    pub async fn create_base(&self) -> Result<(), String> {
        self.db.batch(BASE_SCHEMA).await?;
        match self.kind {
            AdapterKind::LibsqlFts5 => self.db.batch(FTS5_SCHEMA).await,
            _ => {
                self.db
                    .batch("CREATE TABLE executable_bodies (node_id TEXT NOT NULL, file_path TEXT NOT NULL, body TEXT NOT NULL); CREATE INDEX idx_executable_bodies_file ON executable_bodies(file_path);")
                    .await
            }
        }
    }

    /// Builds the full-text indexes over whatever rows exist: libsql runs
    /// production's `end_bulk_load` FTS steps; turso creates its FTS indexes
    /// (which index existing rows and then track DML).
    pub async fn build_fts(&self) -> Result<(), String> {
        match self.kind {
            AdapterKind::LibsqlFts5 => {
                self.db.batch(FTS5_NODE_TRIGGERS).await?;
                self.db.batch(FTS5_BULK_POPULATE).await
            }
            kind => {
                let tok = kind.tokenizer_clause();
                self.db
                    .batch(&format!(
                        "CREATE INDEX nodes_fts ON nodes USING fts ({NODE_COLS}) WITH ({tok}, weights='{NODE_WEIGHTS}');
                         CREATE INDEX executable_body_fts ON executable_bodies USING fts (body) WITH ({tok});
                         CREATE INDEX memory_decisions_fts ON memory_decisions USING fts (text, reason) WITH ({tok});"
                    ))
                    .await
            }
        }
    }

    pub async fn begin(&self) -> Result<(), String> {
        self.db.exec("BEGIN", &[]).await.map(|_| ())
    }

    pub async fn commit(&self) -> Result<(), String> {
        self.db.exec("COMMIT", &[]).await.map(|_| ())
    }

    pub async fn rollback(&self) -> Result<(), String> {
        self.db.exec("ROLLBACK", &[]).await.map(|_| ())
    }

    /// Production `insert_nodes` statement shape: multi-row `INSERT … VALUES`
    /// in chunks of 200 rows (nodes.rs), here with bound parameters.
    pub async fn insert_nodes(&self, nodes: &[NodeRec]) -> Result<(), String> {
        for chunk in nodes.chunks(NODE_INSERT_CHUNK) {
            let placeholders: Vec<String> = (0..chunk.len())
                .map(|r| {
                    let b = r * 8;
                    format!(
                        "(?{}, ?{}, ?{}, ?{}, ?{}, ?{}, ?{}, ?{})",
                        b + 1,
                        b + 2,
                        b + 3,
                        b + 4,
                        b + 5,
                        b + 6,
                        b + 7,
                        b + 8
                    )
                })
                .collect();
            let sql = format!(
                "INSERT INTO nodes (id, kind, name, qualified_name, file_path, docstring, signature, search_terms) VALUES {}",
                placeholders.join(", ")
            );
            let params: Vec<Val> = chunk
                .iter()
                .flat_map(|n| {
                    [
                        Val::from(n.id.as_str()),
                        Val::from(n.kind.as_str()),
                        Val::from(n.name.as_str()),
                        Val::from(n.qualified_name.as_str()),
                        Val::from(n.file_path.as_str()),
                        Val::from(n.docstring.clone()),
                        Val::from(n.signature.clone()),
                        Val::from(n.search_terms.as_str()),
                    ]
                })
                .collect();
            self.db.exec(&sql, &params).await?;
        }
        Ok(())
    }

    /// Production `insert_executable_body_documents` shape: one prepared
    /// single-row INSERT per body.
    pub async fn insert_bodies(&self, bodies: &[BodyRec]) -> Result<(), String> {
        let table = self.body_table();
        self.db
            .exec_many(
                &format!("INSERT INTO {table} (node_id, file_path, body) VALUES (?1, ?2, ?3)"),
                bodies.iter().map(|b| {
                    vec![
                        Val::from(b.node_id.as_str()),
                        Val::from(b.file_path.as_str()),
                        Val::from(b.body.as_str()),
                    ]
                }),
            )
            .await
    }

    pub async fn insert_decisions(&self, decisions: &[Decision]) -> Result<(), String> {
        self.db
            .exec_many(
                "INSERT INTO memory_decisions (text, reason, created_at) VALUES (?1, ?2, ?3)",
                decisions.iter().map(|d| {
                    vec![
                        Val::from(d.text.as_str()),
                        Val::from(d.reason.clone()),
                        Val::Int(d.created_at),
                    ]
                }),
            )
            .await
    }

    /// Production sync: delete a file's nodes and bodies before reinserting.
    pub async fn delete_file(&self, file_path: &str) -> Result<(), String> {
        let table = self.body_table();
        self.db
            .exec(
                "DELETE FROM nodes WHERE file_path = ?1",
                &[Val::from(file_path)],
            )
            .await?;
        self.db
            .exec(
                &format!("DELETE FROM {table} WHERE file_path = ?1"),
                &[Val::from(file_path)],
            )
            .await
            .map(|_| ())
    }

    pub async fn update_node(&self, n: &NodeRec) -> Result<u64, String> {
        self.db
            .exec(
                "UPDATE nodes SET name = ?2, qualified_name = ?3, docstring = ?4, signature = ?5, search_terms = ?6 WHERE id = ?1",
                &[
                    Val::from(n.id.as_str()),
                    Val::from(n.name.as_str()),
                    Val::from(n.qualified_name.as_str()),
                    Val::from(n.docstring.clone()),
                    Val::from(n.signature.clone()),
                    Val::from(n.search_terms.as_str()),
                ],
            )
            .await
    }

    pub async fn delete_node(&self, id: &str) -> Result<u64, String> {
        self.db
            .exec("DELETE FROM nodes WHERE id = ?1", &[Val::from(id)])
            .await
    }

    fn body_table(&self) -> &'static str {
        if self.kind.is_turso() {
            "executable_bodies"
        } else {
            "executable_body_fts"
        }
    }

    /// Renders an intent for this engine.
    pub fn render(&self, q: &Q, policy: Option<PrefixPolicy>) -> Result<String, SearchError> {
        if self.kind.is_turso() {
            render_tantivy(q, policy.unwrap_or(self.kind.native_prefix_policy()))
                .map_err(|u| SearchError::Unsupported(u.0))
        } else {
            Ok(render_fts5(q))
        }
    }

    /// turso only: refuses to run a statement unless `EXPLAIN` shows the
    /// `IndexMethodQuery` opcode, because otherwise the FTS functions run
    /// as scalar fallbacks with different semantics and no error.
    async fn require_index_plan(&self, sql: &str, params: &[Val]) -> Result<(), SearchError> {
        let known = self
            .verified_plans
            .lock()
            .map(|set| set.contains(sql))
            .unwrap_or(false);
        if !self.kind.is_turso() || known {
            return Ok(());
        }
        let ops = self
            .db
            .explain_opcodes(sql, params)
            .await
            .ok_or_else(|| SearchError::Engine("EXPLAIN failed".into()))?;
        if !ops.iter().any(|op| op == "IndexMethodQuery") {
            return Err(SearchError::PlanFallback(format!(
                "no IndexMethodQuery in plan for: {sql}"
            )));
        }
        if let Ok(mut set) = self.verified_plans.lock() {
            set.insert(sql.to_string());
        }
        Ok(())
    }

    async fn run(&self, sql: &str, params: &[Val]) -> Result<Vec<Vec<Val>>, SearchError> {
        self.require_index_plan(sql, params).await?;
        self.db
            .query(sql, params)
            .await
            .map_err(SearchError::Engine)
    }

    /// Production `search_nodes_fts` (all kinds) or `search_nodes_bounded`
    /// (`exclude_file_doc`): BM25-weighted top `limit`, best first. Scores
    /// are "higher is better" for both engines (FTS5 rank is negated, as
    /// production does).
    pub async fn search_nodes(
        &self,
        q: &Q,
        limit: usize,
        exclude_file_doc: bool,
        policy: Option<PrefixPolicy>,
    ) -> Result<Vec<Hit>, SearchError> {
        let text = self.render(q, policy)?;
        let params = [Val::Text(text), Val::Int(limit as i64)];
        let rows = if self.kind.is_turso() {
            let filter = if exclude_file_doc {
                " AND kind NOT IN ('file', 'doc')"
            } else {
                ""
            };
            let sql = format!(
                "SELECT id, fts_score({NODE_COLS}, ?1) AS score FROM nodes \
                 WHERE fts_match({NODE_COLS}, ?1){filter} ORDER BY score DESC LIMIT ?2"
            );
            self.run(&sql, &params).await?
        } else {
            let filter = if exclude_file_doc {
                " AND n.kind NOT IN ('file', 'doc')"
            } else {
                ""
            };
            let sql = format!(
                "SELECT n.id, -bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0) AS score \
                 FROM nodes_fts JOIN nodes n ON nodes_fts.rowid = n.rowid \
                 WHERE nodes_fts MATCH ?1{filter} \
                 ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0) LIMIT ?2"
            );
            self.run(&sql, &params).await?
        };
        Ok(rows
            .into_iter()
            .map(|r| Hit {
                id: r[0].text(),
                score: r[1].real(),
            })
            .collect())
    }

    /// Production `search_nodes`: exact-phrase channel first, then the
    /// general OR-of-prefixes channel, merged without duplicates.
    pub async fn production_search(
        &self,
        raw: &str,
        limit: usize,
        policy: Option<PrefixPolicy>,
    ) -> Result<Vec<Hit>, SearchError> {
        let Some(general) = Q::production_match(raw) else {
            return Ok(Vec::new());
        };
        let phrase_hits = match Q::production_phrase(raw) {
            // Best-effort in production: errors yield no phrase hits.
            Some(p) => self
                .search_nodes(&p, limit, false, policy)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let general = self.search_nodes(&general, limit, false, policy).await?;
        let seen: HashSet<String> = phrase_hits.iter().map(|h| h.id.clone()).collect();
        let mut merged = phrase_hits;
        merged.extend(general.into_iter().filter(|h| !seen.contains(&h.id)));
        merged.truncate(limit);
        Ok(merged)
    }

    /// Production `search_executable_bodies` candidate fetch (unranked).
    pub async fn search_bodies(
        &self,
        q: &Q,
        limit: usize,
        policy: Option<PrefixPolicy>,
    ) -> Result<Vec<String>, SearchError> {
        let text = self.render(q, policy)?;
        let params = [Val::Text(text), Val::Int(limit as i64)];
        let sql = if self.kind.is_turso() {
            "SELECT node_id FROM executable_bodies WHERE fts_match(body, ?1) LIMIT ?2"
        } else {
            "SELECT node_id FROM executable_body_fts WHERE executable_body_fts MATCH ?1 LIMIT ?2"
        };
        Ok(self
            .run(sql, &params)
            .await?
            .into_iter()
            .map(|r| r[0].text())
            .collect())
    }

    /// Production `session_recall` with a query: matches newest first.
    ///
    /// turso: the production shape (`WHERE fts_match(…) ORDER BY created_at
    /// DESC LIMIT n`) is planned with the index method's LIMIT pattern, which
    /// takes `n` arbitrary matches *before* sorting (wrong rows). The
    /// subquery form keeps the same semantics and the native index; set
    /// `direct` to run the production shape unchanged.
    pub async fn recall(
        &self,
        q: &Q,
        since: Option<i64>,
        limit: usize,
        direct: bool,
    ) -> Result<Vec<String>, SearchError> {
        let text = self.render(q, None)?;
        let (sql, params): (String, Vec<Val>) = match (self.kind.is_turso(), since) {
            (false, Some(ts)) => (
                "SELECT d.id FROM memory_decisions d JOIN memory_decisions_fts f ON f.rowid = d.id \
                 WHERE memory_decisions_fts MATCH ?1 AND d.created_at >= ?2 \
                 ORDER BY d.created_at DESC LIMIT ?3"
                    .into(),
                vec![Val::Text(text), Val::Int(ts), Val::Int(limit as i64)],
            ),
            (false, None) => (
                "SELECT d.id FROM memory_decisions d JOIN memory_decisions_fts f ON f.rowid = d.id \
                 WHERE memory_decisions_fts MATCH ?1 ORDER BY d.created_at DESC LIMIT ?2"
                    .into(),
                vec![Val::Text(text), Val::Int(limit as i64)],
            ),
            (true, Some(ts)) if direct => (
                "SELECT id FROM memory_decisions WHERE fts_match(text, reason, ?1) AND created_at >= ?2 \
                 ORDER BY created_at DESC LIMIT ?3"
                    .into(),
                vec![Val::Text(text), Val::Int(ts), Val::Int(limit as i64)],
            ),
            (true, None) if direct => (
                "SELECT id FROM memory_decisions WHERE fts_match(text, reason, ?1) \
                 ORDER BY created_at DESC LIMIT ?2"
                    .into(),
                vec![Val::Text(text), Val::Int(limit as i64)],
            ),
            (true, Some(ts)) => (
                "SELECT id FROM (SELECT id, created_at FROM memory_decisions \
                 WHERE fts_match(text, reason, ?1) AND created_at >= ?2) \
                 ORDER BY created_at DESC LIMIT ?3"
                    .into(),
                vec![Val::Text(text), Val::Int(ts), Val::Int(limit as i64)],
            ),
            (true, None) => (
                "SELECT id FROM (SELECT id, created_at FROM memory_decisions \
                 WHERE fts_match(text, reason, ?1)) ORDER BY created_at DESC LIMIT ?2"
                    .into(),
                vec![Val::Text(text), Val::Int(limit as i64)],
            ),
        };
        Ok(self
            .run(&sql, &params)
            .await?
            .into_iter()
            .map(|r| r[0].text())
            .collect())
    }

    /// Runs an arbitrary FTS statement through the plan guard (functional
    /// checks of planner behavior).
    pub async fn guarded_query(
        &self,
        sql: &str,
        params: &[Val],
    ) -> Result<Vec<Vec<Val>>, SearchError> {
        self.run(sql, params).await
    }

    /// Merges the node index into one structure: FTS5's `'optimize'`
    /// command, turso's `OPTIMIZE INDEX`.
    pub async fn optimize_nodes_fts(&self) -> Result<(), String> {
        if self.kind.is_turso() {
            self.db.batch("OPTIMIZE INDEX nodes_fts").await
        } else {
            self.db
                .exec("INSERT INTO nodes_fts(nodes_fts) VALUES('optimize')", &[])
                .await
                .map(|_| ())
        }
    }

    pub async fn checkpoint(&self) -> Result<(), String> {
        self.db
            .query("PRAGMA wal_checkpoint(TRUNCATE)", &[])
            .await
            .map(|_| ())
    }

    /// Size of the database file plus its WAL and shared-memory files.
    pub fn disk_bytes(&self) -> u64 {
        disk_bytes(&self.path)
    }
}

pub fn disk_bytes(db_path: &Path) -> u64 {
    let Some(dir) = db_path.parent() else {
        return 0;
    };
    let Some(stem) = db_path.file_name().and_then(|f| f.to_str()) else {
        return 0;
    };
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with(stem)))
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}
