//! Integration tests for the `tokensave discover` analyzer over a temp DB.
//!
//! Seeds a small set of `turns` (navigation vs non-navigation) into an isolated
//! `GlobalDb`, then asserts `nav_turns_since` + `discover::analyze` bucket and
//! count them correctly, and that the recoverable estimate is non-negative,
//! monotonic, and never exceeds the addressable total.

use tempfile::TempDir;
use tokensave::accounting::discover::{self, NavBucket};
use tokensave::global_db::GlobalDb;
use tokensave::types::CostTurn;

async fn open_isolated_db(tmp: &TempDir) -> GlobalDb {
    let db_path = tmp.path().join(".tokensave").join("global.db");
    GlobalDb::open_at(&db_path).await.expect("global db open")
}

fn turn(id: &str, ts: u64, input: u64, tool_names: &str) -> CostTurn {
    CostTurn {
        message_id: id.to_string(),
        project_hash: "proj".to_string(),
        session_id: "sess".to_string(),
        model: "claude-opus-4-6".to_string(),
        timestamp: ts,
        input_tokens: input,
        output_tokens: 10,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        cost_usd: 0.01,
        category: "exploration".to_string(),
        tool_names: tool_names.to_string(),
    }
}

#[tokio::test]
async fn analyzer_buckets_seeded_turns() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let base: u64 = 1_715_000_000;
    // Replaceable navigation turns.
    assert!(db.insert_turn(&turn("m1", base, 1000, "Read")).await);
    assert!(db.insert_turn(&turn("m2", base + 1, 2000, "Read")).await);
    assert!(db.insert_turn(&turn("m3", base + 2, 5000, "Grep")).await);
    assert!(db.insert_turn(&turn("m4", base + 3, 900, "Glob")).await);
    assert!(
        db.insert_turn(&turn("m5", base + 4, 1500, "Read,Grep"))
            .await
    ); // -> Grep
       // Non-navigation / mixed turns that must NOT be counted.
    assert!(
        db.insert_turn(&turn("m6", base + 5, 9999, "Read,Edit"))
            .await
    );
    assert!(db.insert_turn(&turn("m7", base + 6, 9999, "Bash")).await);
    assert!(db.insert_turn(&turn("m8", base + 7, 9999, "")).await);

    let rows = db.nav_turns_since(0).await;
    assert_eq!(rows.len(), 8, "all 8 turns fetched");

    let report = discover::analyze(&rows);
    assert_eq!(report.total_turns, 8);
    // Replaceable: m1, m2, m3, m4, m5 = 5 turns.
    assert_eq!(report.total_replaceable_turns(), 5);

    // Three buckets fired, ranked by addressable input tokens descending.
    assert_eq!(report.buckets.len(), 3);
    assert_eq!(report.buckets[0].bucket, NavBucket::Grep); // 5000 + 1500 = 6500
    assert_eq!(report.buckets[0].turns, 2);
    assert_eq!(report.buckets[0].addressable_input_tokens, 6500);
    assert_eq!(report.buckets[1].bucket, NavBucket::Read); // 1000 + 2000 = 3000
    assert_eq!(report.buckets[1].turns, 2);
    assert_eq!(report.buckets[1].addressable_input_tokens, 3000);
    assert_eq!(report.buckets[2].bucket, NavBucket::Glob); // 900
    assert_eq!(report.buckets[2].turns, 1);

    // Addressable excludes the mixed/non-nav turns (3 * 9999).
    assert_eq!(report.total_addressable_input_tokens(), 6500 + 3000 + 900);

    // Estimate is a non-negative lower bound never exceeding addressable.
    assert!(report.total_recoverable_input_tokens() <= report.total_addressable_input_tokens());
    assert_eq!(
        report.total_recoverable_input_tokens(),
        ((6500 + 3000 + 900) as f64 * discover::RECOVERABLE_FRACTION) as u64
    );
}

#[tokio::test]
async fn since_filter_restricts_window_and_estimate_is_monotonic() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let base: u64 = 1_715_000_000;
    assert!(db.insert_turn(&turn("old", base, 1000, "Read")).await);
    assert!(
        db.insert_turn(&turn("new", base + 10_000, 4000, "Grep"))
            .await
    );

    // Full window sees both turns.
    let full = discover::analyze(&db.nav_turns_since(0).await);
    assert_eq!(full.total_replaceable_turns(), 2);

    // Narrowed window sees only the newer turn.
    let narrowed = discover::analyze(&db.nav_turns_since(base + 5_000).await);
    assert_eq!(narrowed.total_replaceable_turns(), 1);
    assert_eq!(narrowed.buckets[0].bucket, NavBucket::Grep);

    // A superset window never reports fewer recoverable tokens.
    assert!(full.total_recoverable_input_tokens() >= narrowed.total_recoverable_input_tokens());
}

#[tokio::test]
async fn empty_db_yields_empty_report() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let report = discover::analyze(&db.nav_turns_since(0).await);
    assert_eq!(report.total_turns, 0);
    assert!(report.buckets.is_empty());
    assert_eq!(report.total_recoverable_input_tokens(), 0);
}
