//! Profiled execution API (#754).
//!
//! Query methods name their statement with a static [`DbOp`], so the metric
//! key never contains SQL text or parameters. With profiling off each call
//! costs one relaxed atomic load on top of the plain libsql call.
//!
//! What is measured per op:
//! - latency from the call to the end of row iteration (execution plus
//!   reading rows), as one span: libsql's local driver prepares, steps and
//!   decodes lazily inside `Rows::next`, so the phases cannot be separated
//!   honestly and are not reported separately;
//! - rows returned;
//! - errors from execution or row iteration. An error while mapping a row
//!   that was read successfully is not seen here.
//!
//! Write-lock wait and write transactions have their own ops
//! ([`DbOp::WriteLockWait`], [`DbOp::Transaction`]).

use libsql::params::IntoParams;
use libsql::{Row, Rows};

use crate::profile::{self, DbOp, Span};

use super::Database;

/// Something rows can be read from: plain or profiled rows.
pub(crate) trait RowSource: Send {
    fn next_row(&mut self)
        -> impl std::future::Future<Output = libsql::Result<Option<Row>>> + Send;
}

impl RowSource for Rows {
    fn next_row(
        &mut self,
    ) -> impl std::future::Future<Output = libsql::Result<Option<Row>>> + Send {
        self.next()
    }
}

/// Rows of a profiled query. The span ends when iteration reaches the end
/// or fails, or when the rows are dropped early (a lookup that only needs
/// the first row), which counts as success.
pub(crate) struct ProfiledRows {
    rows: Rows,
    span: Option<Span>,
    read: u64,
}

impl ProfiledRows {
    pub(crate) async fn next(&mut self) -> libsql::Result<Option<Row>> {
        match self.rows.next().await {
            Ok(Some(row)) => {
                self.read += 1;
                Ok(Some(row))
            }
            Ok(None) => {
                self.finish();
                Ok(None)
            }
            Err(e) => {
                if let Some(span) = self.span.take() {
                    span.fail();
                }
                Err(e)
            }
        }
    }

    fn finish(&mut self) {
        if let Some(mut span) = self.span.take() {
            span.add_rows(self.read, 0);
            span.finish();
        }
    }
}

impl RowSource for ProfiledRows {
    fn next_row(
        &mut self,
    ) -> impl std::future::Future<Output = libsql::Result<Option<Row>>> + Send {
        self.next()
    }
}

impl Drop for ProfiledRows {
    fn drop(&mut self) {
        self.finish();
    }
}

impl Database {
    /// `conn().query(sql, params)`, measured under `op`.
    pub(crate) async fn profiled_query(
        &self,
        op: DbOp,
        sql: &str,
        params: impl IntoParams,
    ) -> libsql::Result<ProfiledRows> {
        let span = profile::span(op);
        match self.conn().query(sql, params).await {
            Ok(rows) => Ok(ProfiledRows {
                rows,
                span: span.is_active().then_some(span),
                read: 0,
            }),
            Err(e) => {
                span.fail();
                Err(e)
            }
        }
    }

    /// Acquires the write lock (#563), measuring the wait as
    /// [`DbOp::WriteLockWait`].
    pub(crate) async fn lock_writes(&self) -> tokio::sync::MutexGuard<'_, ()> {
        let span = profile::span(DbOp::WriteLockWait);
        let guard = self.write_lock.lock().await;
        span.finish();
        guard
    }
}

/// Starts a [`DbOp::Transaction`] span. Call `finish()` after COMMIT; a span
/// dropped on ROLLBACK or an early return counts as a failed transaction.
pub(crate) fn transaction_span() -> Span {
    profile::span(DbOp::Transaction)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profile::{Mode, SeriesSnapshot};

    async fn temp_db() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.db");
        let (db, _) = Database::initialize(&path).await.unwrap();
        (dir, db)
    }

    /// Runs `f` with profiling on and returns what it recorded for `op`.
    /// Other tests may record concurrently, so callers assert lower bounds.
    async fn recorded<F: std::future::Future<Output = ()>>(op: DbOp, f: F) -> SeriesSnapshot {
        let _guard = profile::TEST_MODE_LOCK.lock().await;
        profile::set_mode(Mode::Light);
        let recorder = profile::recorder().unwrap();
        let _ = recorder.drain();
        f.await;
        let snap = recorder.drain();
        profile::set_mode(Mode::Off);
        snap.get(op.name()).cloned().unwrap_or_default()
    }

    #[tokio::test]
    async fn successful_query_records_latency_and_rows() {
        let (_dir, db) = temp_db().await;
        let s = recorded(DbOp::GetNodesByIds, async {
            let mut rows = db
                .profiled_query(DbOp::GetNodesByIds, "SELECT 1 UNION ALL SELECT 2", ())
                .await
                .unwrap();
            while rows.next().await.unwrap().is_some() {}
        })
        .await;
        assert!(s.count >= 1);
        assert!(s.rows_returned >= 2);
        assert_eq!(s.histogram.count(), s.count);
    }

    #[tokio::test]
    async fn failed_query_is_an_error_outside_the_histogram() {
        let (_dir, db) = temp_db().await;
        let s = recorded(DbOp::GetNodeById, async {
            assert!(db
                .profiled_query(DbOp::GetNodeById, "SELECT * FROM no_such_table", ())
                .await
                .is_err());
        })
        .await;
        assert!(s.errors >= 1);
        assert_eq!(s.histogram.count(), s.count);
    }

    #[tokio::test]
    async fn rows_dropped_early_count_as_success() {
        let (_dir, db) = temp_db().await;
        let s = recorded(DbOp::GetChildrenOf, async {
            let mut rows = db
                .profiled_query(DbOp::GetChildrenOf, "SELECT 1 UNION ALL SELECT 2", ())
                .await
                .unwrap();
            assert!(rows.next().await.unwrap().is_some());
        })
        .await;
        assert!(s.count >= 1);
        assert!(s.rows_returned >= 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn write_lock_wait_is_recorded_under_contention() {
        let (_dir, db) = temp_db().await;
        let db = std::sync::Arc::new(db);
        let s = recorded(DbOp::WriteLockWait, async {
            let held = db.lock_writes().await;
            let waiter = {
                let db = std::sync::Arc::clone(&db);
                tokio::spawn(async move {
                    drop(db.lock_writes().await);
                })
            };
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            drop(held);
            waiter.await.unwrap();
        })
        .await;
        assert!(s.count >= 2);
        assert!(s.max_us >= 20_000, "max wait {} us", s.max_us);
    }

    #[tokio::test]
    async fn rolled_back_transaction_is_an_error() {
        let s = recorded(DbOp::Transaction, async {
            let committed = transaction_span();
            committed.finish();
            let _rolled_back = transaction_span();
        })
        .await;
        assert!(s.count >= 1);
        assert!(s.errors >= 1);
    }
}
