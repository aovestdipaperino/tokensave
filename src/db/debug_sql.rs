//! Opt-in, local SQL diagnostics. No history, files, tasks or global recorder.
//!
//! The disabled path delegates directly, without clock reads or SQL copies.
//! Enabled records are bounded and streamed to stderr; pipe retention belongs
//! to the caller. Connections snapshot global settings when opened. Raw libsql
//! access through `Deref` intentionally remains available for compatibility.

use std::io::Write;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use libsql::params::{IntoParams, Params};
use libsql::{BatchRows, Rows};
use serde_json::{json, Value};

static CONNECTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const MAX_SQL_BYTES: usize = 8192;
const MAX_PLAN_ROWS: usize = 64;
const MAX_DETAIL_BYTES: usize = 1024;

type Sink = dyn Fn(&Value) + Send + Sync;

struct DebugState {
    database: &'static str,
    connection_id: u64,
    explain: bool,
    sequence: AtomicU64,
    sink: Box<Sink>,
}

/// A libsql connection with optional SQL diagnostics for runtime operations.
///
/// `Deref` preserves access to libsql APIs; explicitly coercing to a raw
/// `libsql::Connection` bypasses diagnostics. Use this type for SQL helpers.
#[derive(Clone)]
pub struct Connection {
    inner: libsql::Connection,
    debug: Option<Arc<DebugState>>,
}

fn bounded(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn parameter_count(params: &Params) -> usize {
    match params {
        Params::None => 0,
        Params::Positional(values) => values.len(),
        Params::Named(values) => values.len(),
    }
}

fn stderr_sink(record: &Value) {
    // A closed/full stderr must not turn a successful SQL operation into an error.
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{record}");
}

impl Connection {
    pub(crate) fn new(inner: libsql::Connection, database: &'static str) -> Self {
        let (enabled, explain) = crate::user_config::sql_debug_preferences();
        Self {
            inner,
            debug: enabled.then(|| {
                Arc::new(DebugState {
                    database,
                    connection_id: CONNECTION_SEQUENCE.fetch_add(1, Ordering::Relaxed),
                    explain,
                    sequence: AtomicU64::new(0),
                    sink: Box::new(stderr_sink),
                })
            }),
        }
    }

    async fn plan(&self, sql: &str, params: &Params) -> Option<Value> {
        let debug = self.debug.as_ref()?;
        if !debug.explain {
            return None;
        }
        // Only compile the plan, never execute the original statement here.
        // Use the raw connection so diagnostics cannot recurse into themselves.
        let plan_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let result: libsql::Result<Value> = async {
            let mut rows = self.inner.query(&plan_sql, params.clone()).await?;
            let mut plan = Vec::new();
            while let Some(row) = rows.next().await? {
                if plan.len() == MAX_PLAN_ROWS {
                    return Ok(json!({"rows": plan, "truncated": true}));
                }
                let detail: String = row.get(3)?;
                plan.push(json!({
                    "id": row.get::<i64>(0)?,
                    "parent": row.get::<i64>(1)?,
                    "detail": bounded(&detail, MAX_DETAIL_BYTES),
                    "detail_truncated": detail.len() > MAX_DETAIL_BYTES,
                }));
            }
            Ok(json!({"rows": plan, "truncated": false}))
        }
        .await;
        // Error text can contain values or user data. Keep the application
        // error unchanged, but record only a diagnostic failure indicator.
        Some(result.unwrap_or_else(|_| json!({"error": true})))
    }

    fn record(
        &self,
        operation: &'static str,
        sql: &str,
        count: usize,
        start: Instant,
        ok: bool,
        plan: Option<Value>,
    ) {
        if let Some(debug) = &self.debug {
            let mut record = json!({
                "event": "tokensave.sql",
                "pid": std::process::id(),
                "database": debug.database,
                "connection_id": debug.connection_id,
                "sequence": debug.sequence.fetch_add(1, Ordering::Relaxed),
                "operation": operation,
                "sql": bounded(sql, MAX_SQL_BYTES),
                "sql_truncated": sql.len() > MAX_SQL_BYTES,
                "parameter_count": count,
                "duration_us": start.elapsed().as_micros(),
                "ok": ok,
            });
            record["plan"] = plan.unwrap_or(Value::Null);
            (debug.sink)(&record);
        }
    }

    /// Query duration covers preparing/binding and obtaining rows, not iteration.
    pub async fn query(&self, sql: &str, params: impl IntoParams) -> libsql::Result<Rows> {
        if self.debug.is_none() {
            return self.inner.query(sql, params).await;
        }
        let params = params.into_params()?;
        let count = parameter_count(&params);
        let plan = self.plan(sql, &params).await;
        let start = Instant::now();
        let result = self.inner.query(sql, params).await;
        self.record("query", sql, count, start, result.is_ok(), plan);
        result
    }

    pub async fn execute(&self, sql: &str, params: impl IntoParams) -> libsql::Result<u64> {
        if self.debug.is_none() {
            return self.inner.execute(sql, params).await;
        }
        let params = params.into_params()?;
        let count = parameter_count(&params);
        let plan = self.plan(sql, &params).await;
        let start = Instant::now();
        let result = self.inner.execute(sql, params).await;
        self.record("execute", sql, count, start, result.is_ok(), plan);
        result
    }

    /// Batches are logged once. They are never split or replayed for plans.
    pub async fn execute_batch(&self, sql: &str) -> libsql::Result<BatchRows> {
        if self.debug.is_none() {
            return self.inner.execute_batch(sql).await;
        }
        let start = Instant::now();
        let result = self.inner.execute_batch(sql).await;
        self.record("batch", sql, 0, start, result.is_ok(), None);
        result
    }

    pub async fn execute_transactional_batch(&self, sql: &str) -> libsql::Result<BatchRows> {
        if self.debug.is_none() {
            return self.inner.execute_transactional_batch(sql).await;
        }
        let start = Instant::now();
        let result = self.inner.execute_transactional_batch(sql).await;
        self.record("transactional_batch", sql, 0, start, result.is_ok(), None);
        result
    }

    pub async fn prepare(&self, sql: &str) -> libsql::Result<Statement> {
        let start = self.debug.as_ref().map(|_| Instant::now());
        let result = self.inner.prepare(sql).await;
        if let Some(start) = start {
            self.record("prepare", sql, 0, start, result.is_ok(), None);
        }
        Ok(Statement {
            inner: result?,
            context: self.debug.as_ref().map(|_| (self.clone(), sql.into())),
        })
    }

    pub async fn transaction(&self) -> libsql::Result<Transaction> {
        self.transaction_with_behavior(libsql::TransactionBehavior::Deferred)
            .await
    }

    pub async fn transaction_with_behavior(
        &self,
        behavior: libsql::TransactionBehavior,
    ) -> libsql::Result<Transaction> {
        let sql = match behavior {
            libsql::TransactionBehavior::Deferred => "BEGIN DEFERRED",
            libsql::TransactionBehavior::Immediate => "BEGIN IMMEDIATE",
            libsql::TransactionBehavior::Exclusive => "BEGIN EXCLUSIVE",
            libsql::TransactionBehavior::ReadOnly => "BEGIN READONLY",
        };
        let start = self.debug.as_ref().map(|_| Instant::now());
        let result = self.inner.transaction_with_behavior(behavior).await;
        if let Some(start) = start {
            self.record("begin", sql, 0, start, result.is_ok(), None);
        }
        let inner = result?;
        let connection = Self {
            inner: (*inner).clone(),
            debug: self.debug.clone(),
        };
        Ok(Transaction { inner, connection })
    }
}

impl Deref for Connection {
    type Target = libsql::Connection;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A prepared statement. Its SQL and connection are retained only when enabled,
/// scoped to this statement's lifetime, with no connection/statement cycle.
pub struct Statement {
    inner: libsql::Statement,
    context: Option<(Connection, Arc<str>)>,
}

impl Statement {
    pub async fn execute(&self, params: impl IntoParams) -> libsql::Result<usize> {
        let Some((connection, sql)) = &self.context else {
            return self.inner.execute(params).await;
        };
        let params = params.into_params()?;
        let plan = connection.plan(sql, &params).await;
        let count = parameter_count(&params);
        let start = Instant::now();
        let result = self.inner.execute(params).await;
        connection.record("statement_execute", sql, count, start, result.is_ok(), plan);
        result
    }

    pub async fn query(&self, params: impl IntoParams) -> libsql::Result<Rows> {
        let Some((connection, sql)) = &self.context else {
            return self.inner.query(params).await;
        };
        let params = params.into_params()?;
        let plan = connection.plan(sql, &params).await;
        let count = parameter_count(&params);
        let start = Instant::now();
        let result = self.inner.query(params).await;
        connection.record("statement_query", sql, count, start, result.is_ok(), plan);
        result
    }

    pub async fn run(&self, params: impl IntoParams) -> libsql::Result<()> {
        let Some((connection, sql)) = &self.context else {
            return self.inner.run(params).await;
        };
        let params = params.into_params()?;
        let plan = connection.plan(sql, &params).await;
        let count = parameter_count(&params);
        let start = Instant::now();
        let result = self.inner.run(params).await;
        connection.record("statement_run", sql, count, start, result.is_ok(), plan);
        result
    }

    pub async fn query_row(&mut self, params: impl IntoParams) -> libsql::Result<libsql::Row> {
        self.query(params)
            .await?
            .next()
            .await?
            .ok_or(libsql::Error::QueryReturnedNoRows)
    }
}

impl Deref for Statement {
    type Target = libsql::Statement;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Statement {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// The underlying libsql transaction owns commit/rollback/drop cleanup.
pub struct Transaction {
    inner: libsql::Transaction,
    connection: Connection,
}

impl Transaction {
    pub async fn commit(self) -> libsql::Result<()> {
        let start = self.connection.debug.as_ref().map(|_| Instant::now());
        let result = self.inner.commit().await;
        if let Some(start) = start {
            self.connection
                .record("commit", "COMMIT", 0, start, result.is_ok(), None);
        }
        result
    }

    pub async fn rollback(self) -> libsql::Result<()> {
        let start = self.connection.debug.as_ref().map(|_| Instant::now());
        let result = self.inner.rollback().await;
        if let Some(start) = start {
            self.connection
                .record("rollback", "ROLLBACK", 0, start, result.is_ok(), None);
        }
        result
    }
}

impl Deref for Transaction {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

#[cfg(test)]
mod tests;
