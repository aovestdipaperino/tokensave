//! Minimal engine-neutral connection wrapper.
//!
//! Only the operations the FTS adapters need: batches, parameterized
//! statements, row-returning queries and prepared-statement bulk inserts.
//! Values are converted to one neutral [`Val`] so adapters share result code.

use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

impl Val {
    pub fn text(&self) -> String {
        match self {
            Val::Null => String::new(),
            Val::Int(i) => i.to_string(),
            Val::Real(r) => r.to_string(),
            Val::Text(t) => t.clone(),
        }
    }

    pub fn real(&self) -> f64 {
        match self {
            Val::Int(i) => *i as f64,
            Val::Real(r) => *r,
            _ => f64::NAN,
        }
    }

    pub fn int(&self) -> i64 {
        match self {
            Val::Int(i) => *i,
            Val::Real(r) => *r as i64,
            _ => 0,
        }
    }
}

impl From<&str> for Val {
    fn from(s: &str) -> Self {
        Val::Text(s.to_string())
    }
}

impl From<String> for Val {
    fn from(s: String) -> Self {
        Val::Text(s)
    }
}

impl From<Option<String>> for Val {
    fn from(s: Option<String>) -> Self {
        s.map_or(Val::Null, Val::Text)
    }
}

impl From<i64> for Val {
    fn from(i: i64) -> Self {
        Val::Int(i)
    }
}

fn to_libsql(v: &Val) -> libsql::Value {
    match v {
        Val::Null => libsql::Value::Null,
        Val::Int(i) => libsql::Value::Integer(*i),
        Val::Real(r) => libsql::Value::Real(*r),
        Val::Text(t) => libsql::Value::Text(t.clone()),
    }
}

fn to_turso(v: &Val) -> turso::Value {
    match v {
        Val::Null => turso::Value::Null,
        Val::Int(i) => turso::Value::Integer(*i),
        Val::Real(r) => turso::Value::Real(*r),
        Val::Text(t) => turso::Value::Text(t.clone()),
    }
}

fn from_libsql(v: libsql::Value) -> Val {
    match v {
        libsql::Value::Null | libsql::Value::Blob(_) => Val::Null,
        libsql::Value::Integer(i) => Val::Int(i),
        libsql::Value::Real(r) => Val::Real(r),
        libsql::Value::Text(t) => Val::Text(t),
    }
}

fn from_turso(v: turso::Value) -> Val {
    match v {
        turso::Value::Null | turso::Value::Blob(_) => Val::Null,
        turso::Value::Integer(i) => Val::Int(i),
        turso::Value::Real(r) => Val::Real(r),
        turso::Value::Text(t) => Val::Text(t),
    }
}

pub type Row = Vec<Val>;

// One handle per adapter; boxing would only add noise.
#[allow(clippy::large_enum_variant)]
pub enum Db {
    Libsql {
        conn: libsql::Connection,
        _db: libsql::Database,
    },
    Turso {
        conn: turso::Connection,
        db: turso::Database,
    },
}

impl Db {
    pub async fn open_libsql(path: &Path) -> Result<Self, String> {
        let db = libsql::Builder::new_local(path)
            .build()
            .await
            .map_err(|e| e.to_string())?;
        let conn = db.connect().map_err(|e| e.to_string())?;
        Ok(Self::Libsql { conn, _db: db })
    }

    /// Opens turso with the experimental index-method feature, which is the
    /// switch that enables `CREATE INDEX … USING fts` in 0.8.x.
    pub async fn open_turso(path: &Path) -> Result<Self, String> {
        let path_str = path.to_str().ok_or("non-UTF-8 database path")?;
        let db = turso::Builder::new_local(path_str)
            .experimental_index_method(true)
            .build()
            .await
            .map_err(|e| e.to_string())?;
        let conn = db.connect().map_err(|e| e.to_string())?;
        Ok(Self::Turso { conn, db })
    }

    /// A second connection to the same file (for visibility and concurrency
    /// checks). libsql opens a new local `Database` on the path, as two
    /// tokensave processes would; turso shares its `Database`, which is how
    /// its in-process connections are meant to be created.
    pub async fn connect_another(&self, path: &Path) -> Result<Self, String> {
        match self {
            Self::Libsql { .. } => Self::open_libsql(path).await,
            Self::Turso { db, .. } => {
                let conn = db.connect().map_err(|e| e.to_string())?;
                Ok(Self::Turso {
                    conn,
                    db: db.clone(),
                })
            }
        }
    }

    pub async fn batch(&self, sql: &str) -> Result<(), String> {
        match self {
            Self::Libsql { conn, .. } => conn
                .execute_batch(sql)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::Turso { conn, .. } => conn.execute_batch(sql).await.map_err(|e| e.to_string()),
        }
    }

    pub async fn exec(&self, sql: &str, params: &[Val]) -> Result<u64, String> {
        match self {
            Self::Libsql { conn, .. } => conn
                .execute(sql, libsql::params_from_iter(params.iter().map(to_libsql)))
                .await
                .map_err(|e| e.to_string()),
            Self::Turso { conn, .. } => conn
                .execute(sql, turso::params_from_iter(params.iter().map(to_turso)))
                .await
                .map_err(|e| e.to_string()),
        }
    }

    pub async fn query(&self, sql: &str, params: &[Val]) -> Result<Vec<Row>, String> {
        let mut out = Vec::new();
        match self {
            Self::Libsql { conn, .. } => {
                let mut rows = conn
                    .query(sql, libsql::params_from_iter(params.iter().map(to_libsql)))
                    .await
                    .map_err(|e| e.to_string())?;
                while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
                    let mut cols = Vec::with_capacity(row.column_count() as usize);
                    for i in 0..row.column_count() {
                        cols.push(from_libsql(row.get_value(i).map_err(|e| e.to_string())?));
                    }
                    out.push(cols);
                }
            }
            Self::Turso { conn, .. } => {
                let mut rows = conn
                    .query(sql, turso::params_from_iter(params.iter().map(to_turso)))
                    .await
                    .map_err(|e| e.to_string())?;
                while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
                    let mut cols = Vec::with_capacity(row.column_count());
                    for i in 0..row.column_count() {
                        cols.push(from_turso(row.get_value(i).map_err(|e| e.to_string())?));
                    }
                    out.push(cols);
                }
            }
        }
        Ok(out)
    }

    /// Executes one prepared statement per row. The caller owns the
    /// transaction boundary.
    pub async fn exec_many<I>(&self, sql: &str, rows: I) -> Result<(), String>
    where
        I: IntoIterator<Item = Vec<Val>>,
    {
        match self {
            Self::Libsql { conn, .. } => {
                let stmt = conn.prepare(sql).await.map_err(|e| e.to_string())?;
                for row in rows {
                    stmt.execute(libsql::params_from_iter(row.iter().map(to_libsql)))
                        .await
                        .map_err(|e| e.to_string())?;
                    stmt.reset();
                }
            }
            Self::Turso { conn, .. } => {
                let mut stmt = conn.prepare(sql).await.map_err(|e| e.to_string())?;
                for row in rows {
                    stmt.execute(turso::params_from_iter(row.iter().map(to_turso)))
                        .await
                        .map_err(|e| e.to_string())?;
                    stmt.reset().map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }

    /// Opcode names of `EXPLAIN sql` (turso only; libsql returns `None`).
    pub async fn explain_opcodes(&self, sql: &str, params: &[Val]) -> Option<Vec<String>> {
        match self {
            Self::Libsql { .. } => None,
            Self::Turso { .. } => {
                let rows = self.query(&format!("EXPLAIN {sql}"), params).await.ok()?;
                Some(
                    rows.iter()
                        .filter_map(|r| r.get(1).map(Val::text))
                        .collect(),
                )
            }
        }
    }
}
