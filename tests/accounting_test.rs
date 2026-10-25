//! Integration tests for the accounting schema migration and multi-agent tracking.
//!
//! Tests:
//! - Legacy DB migration adds agent/credits columns with correct defaults.
//! - cost_by_agent_since groups correctly with AgentCostSummary fields.
//! - NULL credits aggregate when any row in a group has NULL credits.
//! - total_tokens_since counts all agents (no filter).
//! - token_breakdown_since, cost_by_model_since, cost_by_category_since,
//!   nav_turns_since remain Claude-only.

use tempfile::TempDir;
use tokensave::global_db::GlobalDb;
use tokensave::types::CostTurn;

async fn open_isolated_db(tmp: &TempDir) -> GlobalDb {
    let db_path = tmp.path().join(".tokensave").join("global.db");
    GlobalDb::open_at(&db_path).await.expect("global db open")
}

fn claude_turn(id: &str, input: u64, output: u64) -> CostTurn {
    CostTurn {
        message_id: id.to_string(),
        project_hash: "proj".to_string(),
        session_id: "sess".to_string(),
        model: "claude-opus-4-6".to_string(),
        timestamp: 1_000_000,
        input_tokens: input,
        output_tokens: output,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        cost_usd: 0.01,
        category: "exploration".to_string(),
        tool_names: "Read".to_string(),
        agent: "claude".to_string(),
        credits: None,
    }
}

fn droid_turn(id: &str, input: u64, output: u64, credits: Option<u64>) -> CostTurn {
    CostTurn {
        message_id: id.to_string(),
        project_hash: "proj".to_string(),
        session_id: "sess".to_string(),
        model: "gemini-pro".to_string(),
        timestamp: 1_000_001,
        input_tokens: input,
        output_tokens: output,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        cost_usd: 0.0,
        category: "exploration".to_string(),
        tool_names: "Read".to_string(),
        agent: "droid".to_string(),
        credits,
    }
}

/// A legacy DB (no agent/credits columns) is upgraded transparently.
/// The existing row must have agent="claude" and credits=None after migration.
#[tokio::test]
async fn legacy_migration_adds_agent_and_credits() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join(".tokensave").join("global.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();

    // Build an old-schema DB without agent/credits columns.
    {
        let db = libsql::Builder::new_local(&db_path).build().await.unwrap();
        let conn = db.connect().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE turns (
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
                 tool_names TEXT NOT NULL DEFAULT ''
             );",
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO turns VALUES \
             ('legacy-1','proj','sess','claude-opus-4',1000,100,50,0,0,0.05,'exploration','')",
            libsql::params![],
        )
        .await
        .unwrap();
    }

    // Open through GlobalDb — migration must add agent and credits.
    let db = GlobalDb::open_at(&db_path)
        .await
        .expect("open after migration");

    let summaries = db.cost_by_agent_since(0).await;
    assert_eq!(summaries.len(), 1, "one agent group expected");
    let row = &summaries[0];
    assert_eq!(row.agent, "claude", "legacy row defaults to claude");
    assert_eq!(row.credits, None, "legacy row has NULL credits");
    assert!((row.cost_usd - 0.05).abs() < 1e-9, "cost_usd preserved");
    assert_eq!(row.turns, 1);
}

/// cost_by_agent_since groups by agent and returns all AgentCostSummary fields.
/// Droid cost is 0.0 and credits are exact when all rows have them.
#[tokio::test]
async fn cost_by_agent_groups_correctly() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    assert!(db.insert_turn(&claude_turn("c1", 100, 20)).await);
    assert!(db.insert_turn(&droid_turn("d1", 200, 30, Some(1000))).await);

    let summaries = db.cost_by_agent_since(0).await;
    assert_eq!(summaries.len(), 2, "two agent groups");

    let claude = summaries
        .iter()
        .find(|s| s.agent == "claude")
        .expect("claude group");
    let droid = summaries
        .iter()
        .find(|s| s.agent == "droid")
        .expect("droid group");

    assert_eq!(claude.input_tokens, 100);
    assert_eq!(claude.output_tokens, 20);
    assert_eq!(claude.cache_write_tokens, 0);
    assert_eq!(claude.cache_read_tokens, 0);
    assert_eq!(claude.credits, None);
    assert_eq!(claude.turns, 1);
    assert!(claude.cost_usd > 0.0);

    assert_eq!(droid.input_tokens, 200);
    assert_eq!(droid.output_tokens, 30);
    assert_eq!(droid.cost_usd, 0.0);
    assert_eq!(droid.credits, Some(1000));
    assert_eq!(droid.turns, 1);
}

/// If any row in an agent group has credits NULL, the aggregate must be None.
#[tokio::test]
async fn credits_null_if_any_row_missing() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    assert!(db.insert_turn(&droid_turn("d1", 100, 10, Some(500))).await);
    assert!(db.insert_turn(&droid_turn("d2", 100, 10, None)).await);

    let summaries = db.cost_by_agent_since(0).await;
    let droid = summaries
        .iter()
        .find(|s| s.agent == "droid")
        .expect("droid group");
    assert_eq!(
        droid.credits, None,
        "partial credits must aggregate to NULL"
    );
    assert_eq!(droid.turns, 2);
}

/// total_tokens_since sums ALL agents — Droid must not be excluded.
#[tokio::test]
async fn total_tokens_includes_all_agents() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    // Claude: 100 input + 20 output = 120
    assert!(db.insert_turn(&claude_turn("c1", 100, 20)).await);
    // Droid: 200 input + 30 output = 230
    assert!(db.insert_turn(&droid_turn("d1", 200, 30, None)).await);

    let total = db.total_tokens_since(0).await.unwrap();
    assert_eq!(total, 350, "total_tokens_since must sum all agents");
}

/// token_breakdown_since, cost_by_model_since, cost_by_category_since, and
/// nav_turns_since must only return Claude rows; Droid must not leak in.
#[tokio::test]
async fn legacy_views_exclude_droid() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let mut c = claude_turn("c1", 100, 20);
    c.cache_write_tokens = 5;
    c.cache_read_tokens = 7;
    assert!(db.insert_turn(&c).await);
    assert!(db.insert_turn(&droid_turn("d1", 200, 30, None)).await);

    // token_breakdown_since: claude only
    let (inp, out, cache_read, cache_write) = db.token_breakdown_since(0).await.unwrap();
    assert_eq!(inp, 100, "token_breakdown_since must exclude droid");
    assert_eq!(out, 20);
    assert_eq!(cache_read, 7);
    assert_eq!(
        cache_write, 5,
        "cache creation is priced and must be reported"
    );

    // cost_by_model_since: only claude model present
    let by_model = db.cost_by_model_since(0).await;
    assert_eq!(by_model.len(), 1, "cost_by_model_since must exclude droid");
    assert_eq!(by_model[0].0, "claude-opus-4-6");

    // cost_by_category_since: only claude category
    let by_cat = db.cost_by_category_since(0).await;
    assert_eq!(by_cat.len(), 1, "cost_by_category_since must exclude droid");

    // nav_turns_since: only claude turns
    let nav = db.nav_turns_since(0).await;
    assert_eq!(nav.len(), 1, "nav_turns_since must exclude droid");
}

/// cost_summary efficiency denominator uses ALL agent tokens (not just Claude).
/// Display input/output tokens remain Claude-only via token_breakdown_since.
#[tokio::test]
async fn cost_summary_efficiency_uses_all_agents() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    // Claude: 100 input + 20 output = 120 tokens
    assert!(db.insert_turn(&claude_turn("c1", 100, 20)).await);
    // Droid: 200 input + 30 output = 230 tokens
    assert!(db.insert_turn(&droid_turn("d1", 200, 30, None)).await);
    // total_tokens_since = 350; the ledger holds 350 saved
    // efficiency = 350 / (350 + 350) = 0.5
    db.record_savings("/p", "tokensave_search", 400, 50, 1_000)
        .await;
    let summary = tokensave::accounting::metrics::cost_summary(&db, 0)
        .await
        .expect("summary must exist");

    assert!(
        (summary.efficiency_ratio - 0.5).abs() < 1e-9,
        "efficiency_ratio should be 0.5, got {}",
        summary.efficiency_ratio
    );
    assert_eq!(
        summary.total_input_tokens, 100,
        "input tokens must be Claude-only"
    );
    assert_eq!(
        summary.total_output_tokens, 20,
        "output tokens must be Claude-only"
    );
    assert_eq!(summary.by_agent.len(), 2, "by_agent must have two entries");
}

/// #473: `tokens_saved` must be scoped to the summary's own range.
///
/// It used to be passed in by the caller, which always passed a lifetime,
/// all-projects counter — so the savings figure for `today` equalled the one
/// for `all`, and `efficiency_ratio` moved across ranges only because a fixed
/// numerator was divided by a growing denominator.
#[tokio::test]
async fn cost_summary_tokens_saved_is_scoped_to_the_range() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;
    assert!(db.insert_turn(&claude_turn("c1", 100, 20)).await);

    // One old saving and one recent one, so a range that excludes the old one
    // has to report a smaller figure than all-time.
    db.record_savings("/p", "tokensave_search", 1_000, 0, 1_000)
        .await;
    db.record_savings("/p", "tokensave_search", 300, 0, 9_000)
        .await;

    let all = tokensave::accounting::metrics::cost_summary(&db, 0)
        .await
        .expect("summary must exist");
    let recent = tokensave::accounting::metrics::cost_summary(&db, 5_000)
        .await
        .expect("summary must exist");

    assert_eq!(
        all.tokens_saved, 1_300,
        "all-time must sum both ledger rows"
    );
    assert_eq!(
        recent.tokens_saved, 300,
        "a narrower range must exclude the older saving"
    );
    assert!(
        recent.efficiency_ratio < all.efficiency_ratio,
        "the ratio must follow the range it was computed for"
    );
}

/// The same figure must agree with what `tokensave gain` reports for the same
/// scope: both read the savings ledger, so they cannot disagree by source.
#[tokio::test]
async fn cost_summary_tokens_saved_agrees_with_the_gain_ledger() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;
    assert!(db.insert_turn(&claude_turn("c1", 100, 20)).await);
    db.record_savings("/p", "tokensave_search", 900, 100, 2_000)
        .await;

    let summary = tokensave::accounting::metrics::cost_summary(&db, 0)
        .await
        .expect("summary must exist");
    assert_eq!(
        summary.tokens_saved,
        db.sum_savings(None, 0).await.saved_tokens,
        "cost and gain must report one quantity, not two"
    );
}

/// #472: the exported tokens must account for the exported cost.
///
/// The summary published only uncached input and output, while the cost was
/// computed from the full usage record. Agent traffic is dominated by the
/// cached context resent each turn, so the omission implied a price per million
/// several times any published rate, and made the output:input ratio look
/// backwards.
#[tokio::test]
async fn cost_summary_counts_every_priced_token_category() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let mut turn = claude_turn("c1", 100, 20);
    turn.cache_read_tokens = 50_000;
    turn.cache_write_tokens = 3_000;
    assert!(db.insert_turn(&turn).await);

    let summary = tokensave::accounting::metrics::cost_summary(&db, 0)
        .await
        .expect("summary must exist");

    assert_eq!(summary.total_input_tokens, 100);
    assert_eq!(summary.total_output_tokens, 20);
    assert_eq!(summary.total_cache_read_tokens, 50_000);
    assert_eq!(
        summary.total_cache_write_tokens, 3_000,
        "cache creation is priced and must be reported"
    );

    // `by_model` is the payload's own cross-check: a consumer that sums it must
    // arrive at the same total the four fields do, or the cost is unexplainable.
    let by_model_total: u64 = summary.by_model.iter().map(|(_, _, t)| *t).sum();
    assert_eq!(
        by_model_total,
        summary.total_input_tokens
            + summary.total_output_tokens
            + summary.total_cache_read_tokens
            + summary.total_cache_write_tokens,
        "sum(by_model[].tokens) must reconcile with the summary totals"
    );
}

/// Stale Droid rows must not change legacy output after the local Droid source disappears.
#[tokio::test]
async fn cost_summary_excludes_stale_droid_when_source_is_absent() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    assert!(db.insert_turn(&claude_turn("c1", 100, 20)).await);
    assert!(db.insert_turn(&droid_turn("d1", 200, 30, None)).await);

    db.record_savings("/p", "tokensave_search", 240, 120, 1_000)
        .await;
    let summary = tokensave::accounting::metrics::cost_summary_with_droid_presence(&db, 0, false)
        .await
        .expect("summary must exist");

    assert!(
        (summary.efficiency_ratio - 0.5).abs() < 1e-9,
        "stale Droid tokens must not affect efficiency"
    );
    assert_eq!(summary.by_agent.len(), 1);
    assert_eq!(summary.by_agent[0].agent, "claude");
}

/// Initial upsert inserts the row and returns true.
#[tokio::test]
async fn upsert_droid_turn_inserts_initial_row() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let t = droid_turn("droid:sess-1", 100, 20, Some(500));
    assert!(
        db.upsert_droid_turn(&t).await,
        "first upsert should return true"
    );

    let summaries = db.cost_by_agent_since(0).await;
    let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
    assert_eq!(droid.input_tokens, 100);
    assert_eq!(droid.credits, Some(500));
}

/// Second upsert with strictly larger per-counter+credits snapshot updates the row.
#[tokio::test]
async fn upsert_droid_turn_accepts_larger_snapshot() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let initial = droid_turn("droid:sess-2", 100, 20, Some(500));
    assert!(db.upsert_droid_turn(&initial).await);

    let larger = droid_turn("droid:sess-2", 200, 40, Some(600));
    assert!(
        db.upsert_droid_turn(&larger).await,
        "larger snapshot should return true"
    );

    let summaries = db.cost_by_agent_since(0).await;
    let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
    assert_eq!(droid.input_tokens, 200, "stored tokens should be larger");
    assert_eq!(droid.credits, Some(600), "stored credits should be larger");
}

/// Upsert with smaller counters returns false; stored totals remain the larger values.
#[tokio::test]
async fn upsert_droid_turn_rejects_stale_smaller_snapshot() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let initial = droid_turn("droid:sess-3", 200, 40, Some(600));
    assert!(db.upsert_droid_turn(&initial).await);

    let smaller = droid_turn("droid:sess-3", 100, 20, Some(300));
    assert!(
        !db.upsert_droid_turn(&smaller).await,
        "stale smaller snapshot should return false"
    );

    let summaries = db.cost_by_agent_since(0).await;
    let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
    assert_eq!(droid.input_tokens, 200, "stored tokens must remain larger");
    assert_eq!(
        droid.credits,
        Some(600),
        "stored credits must remain larger"
    );
}

/// Upsert with same tokens but previously-absent credits fills the credits field.
#[tokio::test]
async fn upsert_droid_fills_missing_credits() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let initial = droid_turn("droid:sess-4", 100, 20, None);
    assert!(db.upsert_droid_turn(&initial).await);

    let with_credits = droid_turn("droid:sess-4", 100, 20, Some(500));
    assert!(
        db.upsert_droid_turn(&with_credits).await,
        "filling absent credits should return true"
    );

    let summaries = db.cost_by_agent_since(0).await;
    let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
    assert_eq!(droid.credits, Some(500), "credits should be filled");
}

/// Upsert with larger tokens but absent credits preserves the known stored credits.
#[tokio::test]
async fn upsert_droid_preserves_known_credits_when_later_absent() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let initial = droid_turn("droid:sess-5", 100, 20, Some(500));
    assert!(db.upsert_droid_turn(&initial).await);

    let larger_no_credits = droid_turn("droid:sess-5", 200, 40, None);
    assert!(
        db.upsert_droid_turn(&larger_no_credits).await,
        "token growth with absent credits should return true"
    );

    let summaries = db.cost_by_agent_since(0).await;
    let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
    assert_eq!(droid.input_tokens, 200, "tokens should be updated");
    assert_eq!(droid.credits, Some(500), "known credits must be preserved");
}
