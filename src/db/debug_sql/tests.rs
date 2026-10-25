#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use std::sync::Mutex;

type Records = Arc<Mutex<Vec<Value>>>;

async fn fixture(enabled: bool, explain: bool) -> (libsql::Database, Connection, Records) {
    let db = libsql::Builder::new_local(":memory:")
        .build()
        .await
        .unwrap();
    let inner = db.connect().unwrap();
    inner.execute_batch("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT); CREATE INDEX item_name ON items(name);")
        .await.unwrap();
    let records: Records = Arc::default();
    let sink = records.clone();
    let conn = Connection {
        inner,
        debug: enabled.then(|| {
            Arc::new(DebugState {
                database: "test",
                connection_id: 0,
                explain,
                sequence: AtomicU64::new(0),
                sink: Box::new(move |record| sink.lock().unwrap().push(record.clone())),
            })
        }),
    };
    (db, conn, records)
}

async fn count(conn: &Connection) -> i64 {
    conn.query("SELECT count(*) FROM items", ())
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap()
}

#[tokio::test]
async fn disabled_delegates_without_records_or_statement_context() {
    // Explain=true alone cannot activate debug mode.
    let (_db, conn, records) = fixture(false, true).await;
    let stmt = conn
        .prepare("INSERT INTO items(name) VALUES (?)")
        .await
        .unwrap();
    assert!(stmt.context.is_none());
    assert_eq!(stmt.execute(["private"]).await.unwrap(), 1);
    assert_eq!(count(&conn).await, 1);
    assert!(records.lock().unwrap().is_empty());
}

#[tokio::test]
async fn trace_redacts_bindings_and_reports_errors_without_plans() {
    let (_db, conn, records) = fixture(true, false).await;
    conn.execute(
        "INSERT INTO items(name) VALUES (:name)",
        libsql::named_params! {":name": "SECRET_BOUND_VALUE"},
    )
    .await
    .unwrap();
    assert!(conn.query("SELECT missing FROM items", ()).await.is_err());
    let records = records.lock().unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["parameter_count"], 1);
    assert_eq!(records[0]["ok"], true);
    assert_eq!(records[1]["ok"], false);
    assert_eq!(records[1]["sequence"], 1);
    assert!(records.iter().all(|r| r["plan"].is_null()));
    assert!(!serde_json::to_string(&*records)
        .unwrap()
        .contains("SECRET_BOUND_VALUE"));
}

#[tokio::test]
async fn explains_index_with_real_bindings_and_never_replays_writes() {
    let (_db, conn, records) = fixture(true, true).await;
    conn.execute("INSERT INTO items(name) VALUES (?)", ["SECRET_BOUND_VALUE"])
        .await
        .unwrap();
    let mut rows = conn
        .query(
            "SELECT id FROM items WHERE name = ?",
            ["SECRET_BOUND_VALUE"],
        )
        .await
        .unwrap();
    assert_eq!(
        rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
        1
    );
    assert_eq!(count(&conn).await, 1);
    let records = records.lock().unwrap();
    assert!(records[1]["plan"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["detail"].as_str().unwrap().contains("item_name")));
    assert!(!serde_json::to_string(&*records)
        .unwrap()
        .contains("SECRET_BOUND_VALUE"));
}

#[tokio::test]
async fn diagnostic_failure_does_not_fail_valid_sql_or_recurse() {
    let (_db, conn, records) = fixture(true, true).await;
    // Prefixing EXPLAIN again is invalid, but the original query is valid.
    let mut rows = conn
        .query("EXPLAIN QUERY PLAN SELECT * FROM items", ())
        .await
        .unwrap();
    assert!(rows.next().await.unwrap().is_some());
    let records = records.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["ok"], true);
    assert_eq!(records[0]["plan"]["error"], true);
}

#[tokio::test]
async fn prepared_statements_transactions_and_batches_keep_behavior() {
    let (_db, conn, records) = fixture(true, true).await;
    let tx = conn.transaction().await.unwrap();
    let stmt = tx
        .prepare("INSERT INTO items(name) VALUES (?)")
        .await
        .unwrap();
    stmt.execute(["private"]).await.unwrap();
    stmt.execute(["private2"]).await.unwrap();
    drop(stmt);
    tx.rollback().await.unwrap();
    assert_eq!(count(&conn).await, 0);
    let tx = conn.transaction().await.unwrap();
    tx.execute("INSERT INTO items(name) VALUES (?)", ["committed"])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    conn.execute_transactional_batch(
        "INSERT INTO items(name) VALUES ('batch'); INSERT INTO items(name) VALUES ('batch2');",
    )
    .await
    .unwrap();
    let mut stmt = conn.prepare("SELECT count(*) FROM items").await.unwrap();
    assert_eq!(stmt.query_row(()).await.unwrap().get::<i64>(0).unwrap(), 3);
    stmt.finalize();
    let records = records.lock().unwrap();
    for op in [
        "begin",
        "prepare",
        "statement_execute",
        "statement_query",
        "rollback",
        "commit",
        "transactional_batch",
    ] {
        assert!(records.iter().any(|r| r["operation"] == op), "missing {op}");
    }
    assert!(records
        .iter()
        .filter(|r| r["operation"] == "transactional_batch")
        .all(|r| r["plan"].is_null()));
}

#[tokio::test]
async fn records_and_plans_are_bounded_during_repeated_use() {
    let (_db, conn, records) = fixture(true, true).await;
    for _ in 0..100 {
        let mut rows = conn
            .query("SELECT id FROM items WHERE name = ?", ["private"])
            .await
            .unwrap();
        assert!(rows.next().await.unwrap().is_none());
        // No production history: the test sink drains each record.
        let mut records = records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert!(serde_json::to_vec(&records[0]).unwrap().len() < 4096);
        records.clear();
    }
    let sql = format!("SELECT 1 /*{}*/", "é".repeat(MAX_SQL_BYTES));
    conn.query(&sql, ()).await.unwrap();
    let record = records.lock().unwrap().pop().unwrap();
    assert_eq!(record["sql_truncated"], true);
    assert!(record["sql"].as_str().unwrap().len() <= MAX_SQL_BYTES);
    let sql = std::iter::repeat_n("SELECT id FROM items", MAX_PLAN_ROWS + 5)
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    conn.query(&sql, ()).await.unwrap();
    let record = records.lock().unwrap().pop().unwrap();
    assert_eq!(record["plan"]["truncated"], true);
    assert_eq!(
        record["plan"]["rows"].as_array().unwrap().len(),
        MAX_PLAN_ROWS
    );
}

#[tokio::test]
async fn dropping_transaction_still_rolls_back() {
    let (_db, conn, _records) = fixture(true, true).await;
    let tx = conn.transaction().await.unwrap();
    tx.execute("INSERT INTO items(name) VALUES (?)", ["private"])
        .await
        .unwrap();
    drop(tx);
    assert_eq!(count(&conn).await, 0);
}
