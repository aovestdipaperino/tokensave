//! Minimal engine adapters: execute a batch, run a query, render rows.
//!
//! Rows are rendered to strings (`col|col|…`) so every engine is compared
//! against the same literal expectation. Reals are rounded to 6 decimals so
//! BM25 scores compare on order, not on the last floating-point bit.

use std::path::Path;

pub type Rows = Vec<String>;

/// Engine configurations under test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineKind {
    /// Production engine: vendored libsql 0.9.30 (SQLite 3.45.1).
    Libsql,
    /// turso 0.8.3 with default builder options.
    Turso,
    /// turso 0.8.3 with experimental `VACUUM` and index methods enabled, to
    /// separate "unsupported" from "behind a flag".
    TursoExperimental,
    /// turso 0.8.3 with experimental multi-process WAL. Separate config
    /// because turso rejects `VACUUM` when this flag is on.
    TursoMultiprocessWal,
}

impl EngineKind {
    pub const ALL: [EngineKind; 4] = [
        Self::Libsql,
        Self::Turso,
        Self::TursoExperimental,
        Self::TursoMultiprocessWal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Libsql => "libsql-0.9.30",
            Self::Turso => "turso-0.8.3",
            Self::TursoExperimental => "turso-0.8.3-experimental",
            Self::TursoMultiprocessWal => "turso-0.8.3-mp-wal",
        }
    }
}

// One short-lived handle per probe; boxing would only add noise.
#[allow(clippy::large_enum_variant)]
pub enum Engine {
    Libsql {
        conn: libsql::Connection,
        _db: libsql::Database,
    },
    Turso {
        conn: turso::Connection,
        _db: turso::Database,
    },
}

impl Engine {
    pub async fn open(kind: EngineKind, path: &Path) -> Result<Self, String> {
        let path_str = path.to_str().ok_or("non-UTF-8 database path")?;
        match kind {
            EngineKind::Libsql => {
                let db = libsql::Builder::new_local(path)
                    .build()
                    .await
                    .map_err(|e| e.to_string())?;
                let conn = db.connect().map_err(|e| e.to_string())?;
                Ok(Self::Libsql { conn, _db: db })
            }
            EngineKind::Turso
            | EngineKind::TursoExperimental
            | EngineKind::TursoMultiprocessWal => {
                let mut builder = turso::Builder::new_local(path_str);
                if kind == EngineKind::TursoExperimental {
                    builder = builder
                        .experimental_vacuum(true)
                        .experimental_index_method(true);
                }
                if kind == EngineKind::TursoMultiprocessWal {
                    builder = builder.experimental_multiprocess_wal(true);
                }
                let db = builder.build().await.map_err(|e| e.to_string())?;
                let conn = db.connect().map_err(|e| e.to_string())?;
                Ok(Self::Turso { conn, _db: db })
            }
        }
    }

    pub async fn execute_batch(&self, sql: &str) -> Result<(), String> {
        match self {
            Self::Libsql { conn, .. } => conn
                .execute_batch(sql)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::Turso { conn, .. } => conn.execute_batch(sql).await.map_err(|e| e.to_string()),
        }
    }

    pub async fn query(&self, sql: &str) -> Result<Rows, String> {
        let mut out = Vec::new();
        match self {
            Self::Libsql { conn, .. } => {
                let mut rows = conn.query(sql, ()).await.map_err(|e| e.to_string())?;
                while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
                    let mut cols = Vec::new();
                    for i in 0..row.column_count() {
                        let v = row.get_value(i).map_err(|e| e.to_string())?;
                        cols.push(render_libsql(&v));
                    }
                    out.push(cols.join("|"));
                }
            }
            Self::Turso { conn, .. } => {
                let mut rows = conn.query(sql, ()).await.map_err(|e| e.to_string())?;
                while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
                    let mut cols = Vec::new();
                    for i in 0..row.column_count() {
                        let v = row.get_value(i).map_err(|e| e.to_string())?;
                        cols.push(render_turso(&v));
                    }
                    out.push(cols.join("|"));
                }
            }
        }
        Ok(out)
    }
}

fn render_real(r: f64) -> String {
    format!("{r:.6}")
}

fn render_libsql(v: &libsql::Value) -> String {
    match v {
        libsql::Value::Null => "NULL".into(),
        libsql::Value::Integer(i) => i.to_string(),
        libsql::Value::Real(r) => render_real(*r),
        libsql::Value::Text(t) => t.clone(),
        libsql::Value::Blob(b) => format!("blob:{}", b.len()),
    }
}

fn render_turso(v: &turso::Value) -> String {
    match v {
        turso::Value::Null => "NULL".into(),
        turso::Value::Integer(i) => i.to_string(),
        turso::Value::Real(r) => render_real(*r),
        turso::Value::Text(t) => t.clone(),
        turso::Value::Blob(b) => format!("blob:{}", b.len()),
    }
}
