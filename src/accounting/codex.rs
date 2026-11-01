//! `OpenAI` Codex CLI session accounting parser.
//!
//! Discovers `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` rollout files, reads
//! the cumulative `token_count` events inside each, and exposes the highest
//! cumulative usage per session as an optional [`CostTurn`].
//!
//! Design constraints:
//! - USD cost is an estimate: Codex meters in real tokens against `OpenAI` models,
//!   so cost is `tokens * model price` via [`crate::accounting::pricing`], exactly
//!   the Claude path. Codex plans are subscription-based, so this is an estimate
//!   of API-equivalent spend, not a billed amount.
//! - A model with no known price is left unpriced (cost `0.0`) and marks the
//!   session coverage [`CoverageState::Partial`] rather than silently reading as
//!   free. See `.overlay/notes/adding-a-model.md`.
//! - Token mapping mirrors Claude: `cached_input_tokens` are billed at the
//!   cache-read rate, so they move to `cache_read_tokens` and are subtracted from
//!   the billable `input_tokens`. Native `output_tokens` already includes
//!   `reasoning_output_tokens`; never add reasoning again. Codex has no cache-write line.
//! - No network or browser auth; all parsing is local and offline.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use walkdir::WalkDir;

use crate::accounting::parser::{parse_timestamp, CoverageState, SourceCoverage};
use crate::accounting::pricing;
use crate::types::CostTurn;

/// One line of a Codex rollout file: `{"timestamp","type","payload"}`.
///
/// The payload shape depends on `type`, so it is kept as an untyped value and
/// the relevant fields are pulled out per event. Lines that are not JSON objects
/// fail to deserialize and are skipped without failing the whole file.
#[derive(Debug, Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    payload: Option<serde_json::Value>,
}

/// Cumulative token usage scraped from the winning `token_count` event.
#[derive(Clone, Copy, Default)]
struct Usage {
    input: u64,
    cached_input: u64,
    output: u64,
    reasoning_output: u64,
    total: u64,
}

/// What one session rollout file distils to before pricing.
struct SessionScan {
    session_id: String,
    start_timestamp: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
    invalid_usage: bool,
}

fn parse_usage(obj: &serde_json::Value) -> Option<Usage> {
    let usage = Usage {
        input: obj.get("input_tokens")?.as_u64()?,
        cached_input: obj.get("cached_input_tokens")?.as_u64()?,
        output: obj.get("output_tokens")?.as_u64()?,
        reasoning_output: obj
            .get("reasoning_output_tokens")
            .map_or(Some(0), serde_json::Value::as_u64)?,
        total: obj.get("total_tokens")?.as_u64()?,
    };
    (usage.cached_input <= usage.input
        && usage.reasoning_output <= usage.output
        && usage.input.checked_add(usage.output) == Some(usage.total)
        && i64::try_from(usage.total).is_ok()
        && usage
            .output
            .checked_add(usage.reasoning_output)
            .is_some_and(|n| i64::try_from(n).is_ok()))
    .then_some(usage)
}

/// Scan one rollout file for the session id, start timestamp, active model, and
/// the highest cumulative `token_count` usage.
///
/// The last (highest) `total_token_usage` wins; comparing on `total` is robust to
/// out-of-order or truncated tails, mirroring the Copilot checkpoint scan. The
/// model is taken from the most recent `turn_context`, so a session that switched
/// models is priced at its last active model (documented approximation).
fn scan_session(contents: &str) -> SessionScan {
    let mut session_id: Option<String> = None;
    let mut start_timestamp: Option<String> = None;
    let mut model: Option<String> = None;
    let mut best: Option<Usage> = None;
    let mut invalid_usage = false;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<Line>(trimmed) else {
            continue;
        };
        let Some(payload) = ev.payload else {
            continue;
        };
        match ev.kind.as_str() {
            "session_meta" => {
                if session_id.is_none() {
                    session_id = payload
                        .get("session_id")
                        .or_else(|| payload.get("id"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                }
                if start_timestamp.is_none() {
                    start_timestamp = payload
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                }
            }
            "turn_context" => {
                if let Some(m) = payload.get("model").and_then(|v| v.as_str()) {
                    model = Some(m.to_string());
                }
            }
            "event_msg" => {
                if payload.get("type").and_then(|v| v.as_str()) != Some("token_count") {
                    continue;
                }
                let Some(total) = payload.get("info").and_then(|i| i.get("total_token_usage"))
                else {
                    continue;
                };
                let Some(candidate) = parse_usage(total) else {
                    invalid_usage = true;
                    continue;
                };
                match &best {
                    Some(b) if candidate.total <= b.total => {}
                    _ => best = Some(candidate),
                }
            }
            _ => {}
        }
    }

    SessionScan {
        session_id: session_id.unwrap_or_default(),
        start_timestamp,
        model,
        usage: best,
        invalid_usage,
    }
}

/// Recursively discover `rollout-*.jsonl` files below `root`, counting `WalkDir`
/// errors. Returns `(files, walk_errors)`, sorted for determinism. Symlinks are
/// not followed.
fn collect_rollouts(root: &Path) -> (Vec<PathBuf>, u64) {
    if !root.exists() {
        return (Vec::new(), 0);
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let mut walk_errors: u64 = 0;
    for entry in WalkDir::new(root).follow_links(false) {
        match entry {
            Err(_) => walk_errors += 1,
            Ok(e) => {
                let name = e.file_name().to_str().unwrap_or_default();
                let is_rollout = name.starts_with("rollout-")
                    && Path::new(name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"));
                if e.file_type().is_file() && is_rollout {
                    files.push(e.into_path());
                }
            }
        }
    }
    files.sort();
    (files, walk_errors)
}

/// Recursively discover all `rollout-*.jsonl` files below `root`.
#[cfg(test)]
pub(crate) fn find_rollout_files(root: &Path) -> Vec<PathBuf> {
    collect_rollouts(root).0
}

/// Outcome of parsing one rollout file, carrying whether the model was priced so
/// the caller can decide coverage without re-looking-up.
pub(crate) struct ParsedSession {
    pub turn: CostTurn,
    pub priced: bool,
    legacy_output_tokens: u64,
}

/// Parse a single Codex rollout file at `path`.
///
/// # Return value
///
/// | Condition | Result |
/// |-----------|--------|
/// | File unreadable | `Err(())` |
/// | No `token_count` usage, or no positive total tokens, or missing session id | `Ok(None)` |
/// | Usage present; start `timestamp` absent or unparseable | `Err(())` |
/// | Usage present; start `timestamp` present and parseable | `Ok(Some(parsed))` |
///
/// `credits` is always `None` (Codex is USD-metered). `cost_usd` is the priced
/// estimate, or `0.0` when the model is unpriced (`parsed.priced == false`).
pub(crate) fn parse_rollout_file(path: &Path) -> Result<Option<ParsedSession>, ()> {
    let contents = std::fs::read_to_string(path).map_err(|_| ())?;
    let scan = scan_session(&contents);
    if scan.invalid_usage {
        return Err(());
    }

    let Some(usage) = scan.usage else {
        return Ok(None);
    };
    if usage.total == 0 || scan.session_id.is_empty() {
        return Ok(None);
    }

    let timestamp = scan
        .start_timestamp
        .as_deref()
        .and_then(parse_timestamp)
        .ok_or(())?;

    // Cached input is billed at the cache-read rate; the billable input is the
    // non-cached remainder. Reasoning is already part of native output.
    let cache_read_tokens = usage.cached_input;
    let input_tokens = usage.input.saturating_sub(usage.cached_input);
    let output_tokens = usage.output;

    let model = scan.model.unwrap_or_else(|| "unknown".to_string());
    let priced = pricing::is_priced(&model);
    let cost_usd = pricing::cost_of_turn(&model, input_tokens, output_tokens, 0, cache_read_tokens);

    let message_id = format!("codex:{}", scan.session_id);

    Ok(Some(ParsedSession {
        turn: CostTurn {
            message_id,
            project_hash: "unknown".to_string(),
            session_id: scan.session_id,
            model,
            timestamp,
            input_tokens,
            output_tokens,
            cache_write_tokens: 0,
            cache_read_tokens,
            cost_usd,
            category: "unclassified".to_string(),
            tool_names: String::new(),
            agent: "codex".to_string(),
            credits: None,
        },
        priced,
        legacy_output_tokens: usage.output + usage.reasoning_output,
    }))
}

/// Result of a Codex session directory ingestion.
pub(crate) struct CodexIngestResult {
    pub coverage: SourceCoverage,
    pub rows_changed: u64,
}

/// Ingest Codex rollout files from `root` (`~/.codex/sessions`).
///
/// Each rollout file owns exactly one session, so message ids are unique per
/// file and no cross-file dedupe is required.
///
/// Coverage rules:
/// - [`CoverageState::Absent`]: no files found and no walk errors.
/// - [`CoverageState::Partial`]: any parse/read/walk failure, or any priced
///   session whose model is unknown to the pricing table (cost is unpriced, not
///   free).
/// - [`CoverageState::Complete`]: all files parse and every session's model is
///   priced.
pub(crate) async fn ingest_dir(gdb: &crate::global_db::GlobalDb, root: &Path) -> CodexIngestResult {
    let (files, walk_errors) = collect_rollouts(root);

    let mut parse_errors: u64 = 0;
    let mut unpriced: u64 = 0;
    let mut turns: Vec<ParsedSession> = Vec::new();

    for file in &files {
        match parse_rollout_file(file) {
            Err(()) => parse_errors += 1,
            Ok(None) => {}
            Ok(Some(parsed)) => {
                if !parsed.priced {
                    unpriced += 1;
                }
                turns.push(parsed);
            }
        }
    }

    let sessions = turns.len() as u64;

    let mut rows_changed: u64 = 0;
    for parsed in &turns {
        match gdb
            .upsert_codex_turn(&parsed.turn, parsed.legacy_output_tokens)
            .await
        {
            Ok(true) => rows_changed += 1,
            Ok(false) => {}
            Err(error) => {
                eprintln!("[tokensave] Codex accounting snapshot failed: {error}");
                parse_errors += 1;
            }
        }
    }
    let repair_pending = match gdb.legacy_codex_rows().await {
        Ok(rows) => rows > 0,
        Err(error) => {
            eprintln!("[tokensave] Codex legacy repair status unknown: {error}");
            true
        }
    };
    let state = if parse_errors > 0 || walk_errors > 0 || unpriced > 0 || repair_pending {
        CoverageState::Partial
    } else if sessions == 0 {
        CoverageState::Absent
    } else {
        CoverageState::Complete
    };

    CodexIngestResult {
        coverage: SourceCoverage {
            agent: "codex",
            state,
            sessions,
        },
        rows_changed,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Write `content` to `<root>/2026/06/30/rollout-<id>.jsonl` and return the path.
    fn write_rollout(root: &Path, id: &str, content: &str) -> PathBuf {
        let dir = root.join("2026").join("06").join("30");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("rollout-{id}.jsonl"));
        fs::write(&path, content).unwrap();
        path
    }

    fn meta_line(session_id: &str, ts: &str) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"session_meta","payload":{{"session_id":"{session_id}","id":"{session_id}","timestamp":"{ts}","model_provider":"openai"}}}}"#
        )
    }

    fn turn_context_line(model: &str) -> String {
        format!(
            r#"{{"timestamp":"2026-06-30T06:00:00.000Z","type":"turn_context","payload":{{"model":"{model}","cwd":"/x"}}}}"#
        )
    }

    fn token_count_line(
        input: u64,
        cached: u64,
        output: u64,
        reasoning: u64,
        total: u64,
    ) -> String {
        format!(
            r#"{{"timestamp":"2026-06-30T06:05:00.000Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"output_tokens":{output},"reasoning_output_tokens":{reasoning},"total_tokens":{total}}}}}}}}}"#
        )
    }

    /// A full realistic session: meta, `turn_context`, two cumulative `token_count`s.
    fn real_shape_session(session_id: &str, model: &str) -> String {
        format!(
            "{}\n{}\n{}\n{}\n",
            meta_line(session_id, "2026-06-30T05:59:02.805Z"),
            turn_context_line(model),
            token_count_line(1_000, 400, 200, 100, 1_200),
            // Higher cumulative snapshot arrives later.
            token_count_line(5_000, 4_000, 800, 300, 5_800),
        )
    }

    async fn stored_usage(gdb: &crate::global_db::GlobalDb) -> (u64, u64, u64, u64) {
        let rows = gdb.cost_by_agent_since(0).await;
        let row = rows.iter().find(|row| row.agent == "codex").unwrap();
        (
            row.input_tokens,
            row.output_tokens,
            row.cache_read_tokens,
            row.cache_write_tokens,
        )
    }

    #[test]
    fn real_shape_session_parses_with_usd_and_token_mapping() {
        let tmp = TempDir::new().unwrap();
        // claude prefix is always priced via the embedded table, so cost is nonzero
        // and the model is priced regardless of any host pricing cache.
        let path = write_rollout(
            tmp.path(),
            "abc",
            &real_shape_session("sess-a", "claude-opus-4"),
        );
        let parsed = parse_rollout_file(&path).unwrap().unwrap();
        let turn = &parsed.turn;
        assert_eq!(turn.agent, "codex");
        assert_eq!(turn.message_id, "codex:sess-a");
        assert_eq!(turn.session_id, "sess-a");
        assert_eq!(turn.model, "claude-opus-4");
        assert_eq!(turn.credits, None);
        // billable input = 5000 - 4000 cached = 1000; cache_read = 4000
        assert_eq!(turn.input_tokens, 1_000);
        assert_eq!(turn.cache_read_tokens, 4_000);
        assert_eq!(turn.output_tokens, 800);
        assert_eq!(
            turn.input_tokens + turn.output_tokens + turn.cache_read_tokens,
            5_800
        );
        assert_eq!(turn.cache_write_tokens, 0);
        assert!(parsed.priced);
        assert!((turn.cost_usd - 0.027).abs() < 1e-12);
    }

    #[test]
    fn cumulative_max_usage_wins_over_earlier_snapshot() {
        let tmp = TempDir::new().unwrap();
        let path = write_rollout(
            tmp.path(),
            "b",
            &real_shape_session("sess-b", "claude-opus-4"),
        );
        let turn = parse_rollout_file(&path).unwrap().unwrap().turn;
        // total 5800 snapshot wins over 1200: cache_read reflects the 4000 cached.
        assert_eq!(turn.cache_read_tokens, 4_000);
    }

    #[test]
    fn unknown_model_is_unpriced_not_free() {
        let tmp = TempDir::new().unwrap();
        let path = write_rollout(
            tmp.path(),
            "u",
            &real_shape_session("sess-u", "totally-unknown-model-2099"),
        );
        let parsed = parse_rollout_file(&path).unwrap().unwrap();
        assert!(!parsed.priced);
        // Cost collapses to 0.0, but tokens are retained and coverage will be Partial.
        assert_eq!(parsed.turn.cost_usd, 0.0);
        assert!(parsed.turn.input_tokens > 0);
    }

    #[test]
    fn no_token_count_returns_none() {
        let tmp = TempDir::new().unwrap();
        let content = format!(
            "{}\n{}\n",
            meta_line("sess-c", "2026-06-30T05:59:02.805Z"),
            turn_context_line("gpt-5.4"),
        );
        let path = write_rollout(tmp.path(), "c", &content);
        assert!(parse_rollout_file(&path).unwrap().is_none());
    }

    #[test]
    fn zero_total_tokens_returns_none() {
        let tmp = TempDir::new().unwrap();
        let content = format!(
            "{}\n{}\n{}\n",
            meta_line("sess-z", "2026-06-30T05:59:02.805Z"),
            turn_context_line("gpt-5.4"),
            token_count_line(0, 0, 0, 0, 0),
        );
        let path = write_rollout(tmp.path(), "z", &content);
        assert!(parse_rollout_file(&path).unwrap().is_none());
    }

    #[test]
    fn usage_missing_timestamp_is_err() {
        let tmp = TempDir::new().unwrap();
        // session_meta without a timestamp, but real usage present.
        let content = format!(
            "{}\n{}\n{}\n",
            r#"{"type":"session_meta","payload":{"session_id":"sess-t","id":"sess-t"}}"#,
            turn_context_line("gpt-5.4"),
            token_count_line(5_000, 4_000, 800, 300, 5_800),
        );
        let path = write_rollout(tmp.path(), "t", &content);
        assert!(parse_rollout_file(&path).is_err());
    }

    #[test]
    fn model_defaults_to_unknown_when_no_turn_context() {
        let tmp = TempDir::new().unwrap();
        let content = format!(
            "{}\n{}\n",
            meta_line("sess-m", "2026-06-30T05:59:02.805Z"),
            token_count_line(5_000, 4_000, 800, 300, 5_800),
        );
        let path = write_rollout(tmp.path(), "m", &content);
        let parsed = parse_rollout_file(&path).unwrap().unwrap();
        assert_eq!(parsed.turn.model, "unknown");
        assert!(!parsed.priced);
    }

    #[test]
    fn discovery_is_sorted_and_rollouts_only() {
        let tmp = TempDir::new().unwrap();
        write_rollout(tmp.path(), "b-sess", "{}\n");
        write_rollout(tmp.path(), "a-sess", "{}\n");
        // A stray non-rollout file must be ignored.
        fs::write(
            tmp.path()
                .join("2026")
                .join("06")
                .join("30")
                .join("notes.jsonl"),
            "{}",
        )
        .unwrap();
        let files = find_rollout_files(tmp.path());
        assert_eq!(files.len(), 2);
        assert!(files[0].to_string_lossy().contains("a-sess"));
        assert!(files[1].to_string_lossy().contains("b-sess"));
    }

    #[test]
    fn missing_root_returns_empty() {
        let (files, errors) = collect_rollouts(Path::new("/no/such/codex/root"));
        assert!(files.is_empty());
        assert_eq!(errors, 0);
    }

    #[tokio::test]
    async fn ingest_dir_absent_when_root_missing() {
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let res = ingest_dir(&gdb, Path::new("/no/such/codex/root")).await;
        assert_eq!(res.coverage.state, CoverageState::Absent);
        assert_eq!(res.coverage.sessions, 0);
        assert_eq!(res.rows_changed, 0);
    }

    #[tokio::test]
    async fn ingest_dir_complete_when_all_priced() {
        let root_tmp = TempDir::new().unwrap();
        write_rollout(
            root_tmp.path(),
            "1",
            &real_shape_session("sess-1", "claude-opus-4"),
        );
        write_rollout(
            root_tmp.path(),
            "2",
            &real_shape_session("sess-2", "claude-sonnet-4"),
        );
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let res = ingest_dir(&gdb, root_tmp.path()).await;
        assert_eq!(res.coverage.state, CoverageState::Complete);
        assert_eq!(res.coverage.sessions, 2);
        assert_eq!(res.rows_changed, 2);
    }

    #[tokio::test]
    async fn ingest_dir_partial_when_a_model_is_unpriced() {
        let root_tmp = TempDir::new().unwrap();
        write_rollout(
            root_tmp.path(),
            "1",
            &real_shape_session("sess-1", "claude-opus-4"),
        );
        write_rollout(
            root_tmp.path(),
            "2",
            &real_shape_session("sess-2", "totally-unknown-model-2099"),
        );
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let res = ingest_dir(&gdb, root_tmp.path()).await;
        assert_eq!(res.coverage.state, CoverageState::Partial);
        assert_eq!(res.coverage.sessions, 2);
    }

    #[test]
    fn inconsistent_native_checkpoints_are_not_authoritative() {
        let tmp = TempDir::new().unwrap();
        let valid = real_shape_session("invalid", "claude-opus-4");
        for contents in [
            valid.replace("\"total_tokens\":5800", "\"total_tokens\":5801"),
            valid.replace(
                "\"reasoning_output_tokens\":300",
                "\"reasoning_output_tokens\":900",
            ),
            valid.replace("\"input_tokens\":5000", "\"input_tokens\":\"5000\""),
        ] {
            let path = write_rollout(tmp.path(), "invalid", &contents);
            assert!(parse_rollout_file(&path).is_err(), "{contents}");
        }
    }

    #[tokio::test]
    async fn legacy_overcount_is_repaired_after_reopen_then_remains_monotonic() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("sessions");
        let contents = real_shape_session("repair", "claude-opus-4");
        let path = write_rollout(&root, "repair", &contents);
        let db_path = tmp.path().join("global.db");
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let mut legacy = parse_rollout_file(&path).unwrap().unwrap().turn;
        legacy.output_tokens = 1_100;
        legacy.cost_usd = 0.0345;
        assert!(gdb.insert_turn(&legacy).await);
        drop(gdb);

        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let repaired = ingest_dir(&gdb, &root).await;
        assert_eq!(repaired.rows_changed, 1);
        assert_eq!(repaired.coverage.state, CoverageState::Complete);
        assert_eq!(stored_usage(&gdb).await, (1_000, 800, 4_000, 0));
        assert!((gdb.total_cost_since(0).await.unwrap() - 0.027).abs() < 1e-12);
        assert_eq!(ingest_dir(&gdb, &root).await.rows_changed, 0);

        let grown = contents.replace(
            &token_count_line(5_000, 4_000, 800, 300, 5_800),
            &token_count_line(6_000, 4_500, 1_000, 400, 7_000),
        );
        fs::write(&path, grown).unwrap();
        assert_eq!(ingest_dir(&gdb, &root).await.rows_changed, 1);
        assert_eq!(stored_usage(&gdb).await, (1_500, 1_000, 4_500, 0));
        fs::write(&path, contents).unwrap();
        assert_eq!(ingest_dir(&gdb, &root).await.rows_changed, 0);
        assert_eq!(stored_usage(&gdb).await, (1_500, 1_000, 4_500, 0));
        // An older API must not downgrade the repaired row in this runtime.
        legacy.output_tokens = 2_000;
        assert!(!gdb.upsert_droid_turn(&legacy).await);
    }

    #[tokio::test]
    async fn stale_first_repair_retains_legacy_counters_and_partial_coverage() {
        for (input, output, cached) in [
            (1_000, 1_500, 4_000),
            (2_000, 1_100, 4_000),
            (1_000, 1_100, 4_500),
        ] {
            let tmp = TempDir::new().unwrap();
            let root = tmp.path().join("sessions");
            let path = write_rollout(
                &root,
                "stale",
                &real_shape_session("stale", "claude-opus-4"),
            );
            let gdb = crate::global_db::GlobalDb::open_at(&tmp.path().join("global.db"))
                .await
                .unwrap();
            let mut legacy = parse_rollout_file(&path).unwrap().unwrap().turn;
            legacy.input_tokens = input;
            legacy.output_tokens = output;
            legacy.cache_read_tokens = cached;
            assert!(gdb.insert_turn(&legacy).await);
            let result = ingest_dir(&gdb, &root).await;
            assert_eq!(result.rows_changed, 0);
            assert_eq!(result.coverage.state, CoverageState::Partial);
            assert_eq!(stored_usage(&gdb).await, (input, output, cached, 0));
        }
    }

    #[tokio::test]
    async fn missing_source_preserves_unrepaired_legacy_rows() {
        let tmp = TempDir::new().unwrap();
        let path = write_rollout(
            tmp.path(),
            "missing",
            &real_shape_session("missing", "claude-opus-4"),
        );
        let gdb = crate::global_db::GlobalDb::open_at(&tmp.path().join("global.db"))
            .await
            .unwrap();
        let mut legacy = parse_rollout_file(&path).unwrap().unwrap().turn;
        legacy.output_tokens = 1_100;
        assert!(gdb.insert_turn(&legacy).await);
        fs::remove_file(path).unwrap();
        let result = ingest_dir(&gdb, tmp.path()).await;
        assert_eq!(result.coverage.state, CoverageState::Partial);
        assert_eq!(result.rows_changed, 0);
        assert_eq!(stored_usage(&gdb).await, (1_000, 1_100, 4_000, 0));
    }

    #[tokio::test]
    async fn failed_repair_count_is_unknown_not_absent() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("global.db");
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let db = libsql::Builder::new_local(&db_path).build().await.unwrap();
        db.connect()
            .unwrap()
            .execute("DROP TABLE turns", ())
            .await
            .unwrap();
        let result = ingest_dir(&gdb, &tmp.path().join("absent")).await;
        assert_eq!(result.coverage.state, CoverageState::Partial);
    }
}
