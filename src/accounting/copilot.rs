//! GitHub Copilot CLI session accounting parser.
//!
//! Discovers `~/.copilot/session-state/<session-id>/events.jsonl` files, reads the
//! cumulative `session.usage_checkpoint` records inside each, and exposes the
//! latest checkpoint per session as an optional [`CostTurn`].
//!
//! Design constraints:
//! - No USD cost conversion (Copilot meters in AIU, not dollars).
//! - No token counts: the checkpoint carries no stable token totals, so every
//!   token field on the emitted turn is zero. Copilot's native consumption unit,
//!   nano-AIU (`totalNanoAiu`), is stored in the `credits` field. Displays divide
//!   by `1_000_000_000` to recover whole AIU.
//! - `totalPremiumRequests` is out of scope for v1 (a separate quota unit that
//!   must not be added to AIU, and whose storage would need a schema column). See
//!   `.overlay/notes/monitor-spent-investigation.md`.
//! - No network or browser auth; all parsing is local and offline.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use walkdir::WalkDir;

use crate::accounting::parser::{parse_timestamp, CoverageState, SourceCoverage};
use crate::types::CostTurn;

/// Nano-AIU per whole AIU. `totalNanoAiu` is fixed-point with nine fractional digits.
pub const NANO_AIU_PER_AIU: u64 = 1_000_000_000;

/// Usage fields deserialized only after the top-level `type` is matched as a
/// `session.usage_checkpoint`; unrelated event fields are ignored.
#[derive(Debug, Deserialize)]
struct EventEnvelope {
    timestamp: Option<String>,
    data: Option<CheckpointData>,
}

/// The `data` payload of a `session.usage_checkpoint` event.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckpointData {
    total_nano_aiu: i64,
    #[serde(default)]
    model_cache_state: Vec<ModelCacheEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelCacheEntry {
    model_id: Option<String>,
}

/// The winning (highest cumulative) checkpoint scraped from one session file.
struct LatestCheckpoint {
    nano_aiu: i64,
    timestamp: Option<String>,
    model: Option<String>,
}

struct ParsedEventFile {
    turn: Option<CostTurn>,
    parse_error: bool,
}

/// Format a nano-AIU counter as a human-readable whole-AIU string, e.g. `1.1K`.
///
/// Rounds to the nearest whole AIU (checkpoints are cumulative, sub-AIU
/// precision is not meaningful in the monitor panel).
pub fn format_aiu(nano_aiu: u64) -> String {
    let aiu = (nano_aiu + NANO_AIU_PER_AIU / 2) / NANO_AIU_PER_AIU;
    crate::display::format_token_count(aiu)
}

/// Recursively discover `events.jsonl` files below `root`, counting `WalkDir`
/// errors (e.g. permission-denied on a subdirectory).
///
/// Returns `(files, walk_errors)`. The file list is sorted for determinism.
/// Symlinks are not followed.
fn collect_events(root: &Path) -> (Vec<PathBuf>, u64) {
    if !root.exists() {
        return (Vec::new(), 0);
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let mut walk_errors: u64 = 0;
    for entry in WalkDir::new(root).follow_links(false) {
        match entry {
            Err(_) => walk_errors += 1,
            Ok(e) => {
                if e.file_type().is_file()
                    && e.file_name().to_str().is_some_and(|n| n == "events.jsonl")
                {
                    files.push(e.into_path());
                }
            }
        }
    }
    files.sort();
    (files, walk_errors)
}

/// Recursively discover all `events.jsonl` files below `root`.
#[cfg(test)]
pub(crate) fn find_event_files(root: &Path) -> Vec<PathBuf> {
    collect_events(root).0
}

/// Scan one `events.jsonl` file for the checkpoint with the highest cumulative
/// `totalNanoAiu`. Also reports whether a line or checkpoint record was malformed.
///
/// Non-checkpoint lines and non-object JSON are skipped, since both are common
/// in `events.jsonl`.
fn latest_checkpoint(contents: &str) -> (Option<LatestCheckpoint>, bool) {
    let mut best: Option<LatestCheckpoint> = None;
    let mut parse_error = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            parse_error = true;
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("session.usage_checkpoint")
        {
            continue;
        }
        let Ok(ev) = serde_json::from_value::<EventEnvelope>(value) else {
            parse_error = true;
            continue;
        };
        let Some(data) = ev.data else {
            parse_error = true;
            continue;
        };
        if data.total_nano_aiu < 0 {
            parse_error = true;
            continue;
        }
        if data.total_nano_aiu > 0 && ev.timestamp.as_deref().and_then(parse_timestamp).is_none() {
            parse_error = true;
            continue;
        }
        let candidate = LatestCheckpoint {
            nano_aiu: data.total_nano_aiu,
            timestamp: ev.timestamp,
            model: data.model_cache_state.into_iter().find_map(|m| m.model_id),
        };
        match &best {
            Some(b) if candidate.nano_aiu <= b.nano_aiu => {}
            _ => best = Some(candidate),
        }
    }
    (best, parse_error)
}

/// Parse a single Copilot session `events.jsonl` file at `path`.
///
/// # Return value
///
/// | Condition | Result |
/// |-----------|--------|
/// | File unreadable | `Err(())` |
/// | No `session.usage_checkpoint` line, or latest checkpoint has no AIU | `Ok(None)` |
/// | AIU present; checkpoint `timestamp` absent or unparseable | `Err(())` |
/// | AIU present; checkpoint `timestamp` present and parseable | `Ok(Some(turn))` |
///
/// All token fields are zero. `cost_usd` is always `0.0`. `credits` holds the
/// cumulative `totalNanoAiu` (always positive here, since a non-positive AIU
/// checkpoint returns `Ok(None)` above).
#[cfg(test)]
pub(crate) fn parse_events_file(path: &Path) -> Result<Option<CostTurn>, ()> {
    parse_events_file_detailed(path).and_then(|parsed| {
        if parsed.turn.is_none() && parsed.parse_error {
            Err(())
        } else {
            Ok(parsed.turn)
        }
    })
}

fn parse_events_file_detailed(path: &Path) -> Result<ParsedEventFile, ()> {
    let contents = std::fs::read_to_string(path).map_err(|_| ())?;

    // Session id is the immediate parent directory name.
    let session_id = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .ok_or(())?
        .to_string();

    let (latest, parse_error) = latest_checkpoint(&contents);
    let Some(cp) = latest else {
        return Ok(ParsedEventFile {
            turn: None,
            parse_error,
        });
    };

    // A checkpoint with no AIU is a fresh session with no accountable usage
    // (premium requests are out of scope), so it stays out of parse_errors and
    // coverage rather than becoming a phantom zero-AIU row.
    if cp.nano_aiu <= 0 {
        return Ok(ParsedEventFile {
            turn: None,
            parse_error,
        });
    }

    // Nonzero usage: the checkpoint timestamp is required to bucket the turn.
    let timestamp = cp
        .timestamp
        .as_deref()
        .and_then(parse_timestamp)
        .ok_or(())?;

    let credits = Some(cp.nano_aiu as u64);

    let model = cp.model.unwrap_or_else(|| "unknown".to_string());
    let message_id = format!("copilot:{session_id}");

    Ok(ParsedEventFile {
        turn: Some(CostTurn {
            message_id,
            project_hash: "unknown".to_string(),
            session_id,
            model,
            timestamp,
            input_tokens: 0,
            output_tokens: 0,
            cache_write_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            category: "unclassified".to_string(),
            tool_names: String::new(),
            agent: "copilot".to_string(),
            credits,
        }),
        parse_error,
    })
}

/// Result of a Copilot session directory ingestion.
pub(crate) struct CopilotIngestResult {
    pub coverage: SourceCoverage,
    pub rows_changed: u64,
}

/// Ingest Copilot session files from `root` (`~/.copilot/session-state`).
///
/// Each session directory owns exactly one `events.jsonl`, so message ids are
/// unique per file and no cross-file dedupe is required.
///
/// Coverage rules:
/// - [`CoverageState::Absent`]: no files found and no walk errors.
/// - [`CoverageState::Partial`]: any malformed checkpoint or read/walk failure,
///   or any valid session lacks AIU credits.
/// - [`CoverageState::Complete`]: all files parse and every valid session
///   carries AIU credits.
pub(crate) async fn ingest_dir(
    gdb: &crate::global_db::GlobalDb,
    root: &Path,
) -> CopilotIngestResult {
    let (files, walk_errors) = collect_events(root);

    if files.is_empty() && walk_errors == 0 {
        return CopilotIngestResult {
            coverage: SourceCoverage {
                agent: "copilot",
                state: CoverageState::Absent,
                sessions: 0,
            },
            rows_changed: 0,
        };
    }

    let mut parse_errors: u64 = 0;
    let mut turns: Vec<CostTurn> = Vec::new();

    for file in &files {
        match parse_events_file_detailed(file) {
            Err(()) => parse_errors += 1,
            Ok(parsed) => {
                if parsed.parse_error {
                    parse_errors += 1;
                }
                if let Some(turn) = parsed.turn {
                    turns.push(turn);
                }
            }
        }
    }

    let sessions = turns.len() as u64;
    let has_missing_credits = turns.iter().any(|t| t.credits.is_none());

    let state = if parse_errors > 0 || walk_errors > 0 || has_missing_credits {
        CoverageState::Partial
    } else if sessions == 0 {
        CoverageState::Absent
    } else {
        CoverageState::Complete
    };

    let mut rows_changed: u64 = 0;
    for turn in &turns {
        if gdb.upsert_copilot_turn(turn).await {
            rows_changed += 1;
        }
    }

    CopilotIngestResult {
        coverage: SourceCoverage {
            agent: "copilot",
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

    /// Write `content` to `<dir>/<session>/events.jsonl` and return the path.
    fn write_session(root: &Path, session: &str, content: &str) -> PathBuf {
        let dir = root.join(session);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        fs::write(&path, content).unwrap();
        path
    }

    fn checkpoint_line(nano_aiu: i64, premium: i64, model: &str, ts: &str) -> String {
        format!(
            r#"{{"type":"session.usage_checkpoint","data":{{"totalNanoAiu":{nano_aiu},"totalPremiumRequests":{premium},"modelCacheState":[{{"modelId":"{model}","cacheTtlSeconds":1800}}]}},"id":"x","timestamp":"{ts}"}}"#
        )
    }

    #[test]
    fn real_schema_checkpoint_parses_to_turn() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "sess-a",
            &checkpoint_line(1_090_866_150_000, 0, "gpt-5.4", "2026-07-31T09:23:09.601Z"),
        );
        let turn = parse_events_file(&path).unwrap().unwrap();
        assert_eq!(turn.agent, "copilot");
        assert_eq!(turn.message_id, "copilot:sess-a");
        assert_eq!(turn.session_id, "sess-a");
        assert_eq!(turn.model, "gpt-5.4");
        assert_eq!(turn.credits, Some(1_090_866_150_000));
        assert_eq!(turn.input_tokens, 0);
        assert_eq!(turn.output_tokens, 0);
        assert_eq!(turn.cost_usd, 0.0);
    }

    #[test]
    fn cumulative_max_checkpoint_wins_and_ignores_other_events() {
        // Interleave non-checkpoint events and an out-of-order lower checkpoint.
        let tmp = TempDir::new().unwrap();
        let content = format!(
            "{}\n{}\n{}\n{}\n",
            r#"{"type":"assistant.turn_start","id":"a","timestamp":"2026-07-31T09:00:00.000Z"}"#,
            checkpoint_line(500_000_000_000, 0, "gpt-5.4", "2026-07-31T09:10:00.000Z"),
            r#"["not","an","object"]"#,
            checkpoint_line(900_000_000_000, 1, "gpt-5.4", "2026-07-31T09:20:00.000Z"),
        );
        let path = write_session(tmp.path(), "sess-b", &content);
        let turn = parse_events_file(&path).unwrap().unwrap();
        assert_eq!(turn.credits, Some(900_000_000_000));
    }

    #[test]
    fn no_checkpoint_line_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "sess-c",
            "{\"type\":\"assistant.message\",\"id\":\"m\"}\n",
        );
        assert!(parse_events_file(&path).unwrap().is_none());
    }

    #[test]
    fn all_zero_checkpoint_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "sess-d",
            &checkpoint_line(0, 0, "gpt-5.4", "2026-07-31T09:00:00.000Z"),
        );
        assert!(parse_events_file(&path).unwrap().is_none());
    }

    #[test]
    fn premium_only_usage_without_aiu_is_dropped() {
        // Premium requests are out of scope; a checkpoint with premium but no
        // AIU carries no accountable value and must not become a phantom row.
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "sess-e",
            &checkpoint_line(0, 3, "gpt-5.4", "2026-07-31T09:00:00.000Z"),
        );
        assert!(parse_events_file(&path).unwrap().is_none());
    }

    #[test]
    fn nonzero_usage_missing_timestamp_is_err() {
        let tmp = TempDir::new().unwrap();
        let line = r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":900000000000,"totalPremiumRequests":0}}"#;
        let path = write_session(tmp.path(), "sess-f", line);
        assert!(parse_events_file(&path).is_err());
    }

    #[test]
    fn model_defaults_to_unknown_when_cache_state_empty() {
        let tmp = TempDir::new().unwrap();
        let line = r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":900000000000,"totalPremiumRequests":0,"modelCacheState":[]},"timestamp":"2026-07-31T09:00:00.000Z"}"#;
        let path = write_session(tmp.path(), "sess-g", line);
        let turn = parse_events_file(&path).unwrap().unwrap();
        assert_eq!(turn.model, "unknown");
    }

    #[test]
    fn discovery_is_sorted_and_events_only() {
        let tmp = TempDir::new().unwrap();
        write_session(tmp.path(), "b-sess", "{}\n");
        write_session(tmp.path(), "a-sess", "{}\n");
        // A stray non-events file must be ignored.
        fs::write(tmp.path().join("a-sess").join("other.json"), "{}").unwrap();
        let files = find_event_files(tmp.path());
        assert_eq!(files.len(), 2);
        assert!(files[0].to_string_lossy().contains("a-sess"));
        assert!(files[1].to_string_lossy().contains("b-sess"));
    }

    #[test]
    fn missing_root_returns_empty() {
        let (files, errors) = collect_events(Path::new("/no/such/copilot/root"));
        assert!(files.is_empty());
        assert_eq!(errors, 0);
    }

    #[test]
    fn format_aiu_rounds_to_whole_units() {
        assert_eq!(format_aiu(1_090_866_150_000), "1.1k");
        assert_eq!(format_aiu(0), "0");
        assert_eq!(format_aiu(NANO_AIU_PER_AIU), "1");
    }

    #[tokio::test]
    async fn ingest_dir_absent_when_root_missing() {
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let res = ingest_dir(&gdb, Path::new("/no/such/copilot/root")).await;
        assert_eq!(res.coverage.state, CoverageState::Absent);
        assert_eq!(res.coverage.sessions, 0);
        assert_eq!(res.rows_changed, 0);
    }

    #[tokio::test]
    async fn ingest_dir_complete_with_valid_sessions() {
        let root_tmp = TempDir::new().unwrap();
        write_session(
            root_tmp.path(),
            "sess-1",
            &checkpoint_line(900_000_000_000, 0, "gpt-5.4", "2026-07-31T09:00:00.000Z"),
        );
        write_session(
            root_tmp.path(),
            "sess-2",
            &checkpoint_line(1_500_000_000_000, 0, "kimi-k2", "2026-07-31T10:00:00.000Z"),
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
    async fn malformed_higher_checkpoint_does_not_drop_valid_usage() {
        let root_tmp = TempDir::new().unwrap();
        let content = format!(
            "{}\n{}",
            checkpoint_line(900_000_000_000, 0, "gpt-5.4", "2026-07-31T09:00:00.000Z"),
            r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":1500000000000,"modelCacheState":[]},"timestamp":"invalid"}"#
        );
        write_session(root_tmp.path(), "sess-checkpoint-fallback", &content);
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        let res = ingest_dir(&gdb, root_tmp.path()).await;

        assert_eq!(res.coverage.state, CoverageState::Partial);
        assert_eq!(res.coverage.sessions, 1);
        assert_eq!(res.rows_changed, 1);
        let rows = gdb.cost_by_agent_since(0).await;
        let credits = rows
            .iter()
            .find(|summary| summary.agent == "copilot")
            .and_then(|summary| summary.credits);
        assert_eq!(credits, Some(900_000_000_000));
    }

    #[tokio::test]
    async fn ingest_dir_negative_checkpoint_is_partial_without_dropping_valid_usage() {
        let root_tmp = TempDir::new().unwrap();
        let content = format!(
            "{}\n{}",
            checkpoint_line(900_000_000_000, 0, "gpt-5.4", "2026-07-31T09:00:00.000Z"),
            r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":-1,"modelCacheState":[]},"timestamp":"2026-07-31T10:00:00.000Z"}"#,
        );
        write_session(root_tmp.path(), "sess-negative-counter", &content);
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        let res = ingest_dir(&gdb, root_tmp.path()).await;

        assert_eq!(res.coverage.state, CoverageState::Partial);
        assert_eq!(res.coverage.sessions, 1);
        assert_eq!(res.rows_changed, 1);
        let rows = gdb.cost_by_agent_since(0).await;
        let credits = rows
            .iter()
            .find(|summary| summary.agent == "copilot")
            .and_then(|summary| summary.credits);
        assert_eq!(credits, Some(900_000_000_000));
    }

    #[tokio::test]
    async fn ingest_dir_reingest_moves_checkpoint_to_later_range() {
        let root_tmp = TempDir::new().unwrap();
        let session_path = write_session(
            root_tmp.path(),
            "sess-changing",
            &checkpoint_line(900_000_000_000, 0, "gpt-5.4", "2026-07-31T09:00:00.000Z"),
        );
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        let first = ingest_dir(&gdb, root_tmp.path()).await;
        assert_eq!(first.rows_changed, 1);

        fs::write(
            session_path,
            checkpoint_line(1_500_000_000_000, 0, "gpt-5.4", "2026-08-03T09:00:00.000Z"),
        )
        .unwrap();
        let second = ingest_dir(&gdb, root_tmp.path()).await;
        assert_eq!(second.rows_changed, 1);

        let later_start = parse_timestamp("2026-08-03T09:00:00.000Z").unwrap();
        let rows = gdb.cost_by_agent_since(later_start).await;
        let copilot_credits = rows
            .iter()
            .find(|summary| summary.agent == "copilot")
            .and_then(|summary| summary.credits);
        assert_eq!(copilot_credits, Some(1_500_000_000_000));
    }

    #[tokio::test]
    async fn ingest_dir_malformed_checkpoint_is_partial() {
        let root_tmp = TempDir::new().unwrap();
        write_session(
            root_tmp.path(),
            "sess-malformed",
            r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":"invalid"},"timestamp":"2026-07-31T09:00:00.000Z"}"#,
        );
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        let res = ingest_dir(&gdb, root_tmp.path()).await;

        assert_eq!(res.coverage.state, CoverageState::Partial);
        assert_eq!(res.coverage.sessions, 0);
    }

    #[tokio::test]
    async fn ingest_dir_invalid_json_line_is_partial() {
        let root_tmp = TempDir::new().unwrap();
        let content = format!(
            "{}\nnot-json",
            checkpoint_line(900_000_000_000, 0, "gpt-5.4", "2026-07-31T09:00:00.000Z"),
        );
        write_session(root_tmp.path(), "sess-invalid-json", &content);
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        let res = ingest_dir(&gdb, root_tmp.path()).await;

        assert_eq!(res.coverage.state, CoverageState::Partial);
        assert_eq!(res.coverage.sessions, 1);
        assert_eq!(res.rows_changed, 1);
    }

    #[tokio::test]
    async fn ingest_dir_checkpoint_missing_total_is_partial() {
        let root_tmp = TempDir::new().unwrap();
        write_session(
            root_tmp.path(),
            "sess-missing-total",
            r#"{"type":"session.usage_checkpoint","data":{},"timestamp":"2026-07-31T09:00:00.000Z"}"#,
        );
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        let res = ingest_dir(&gdb, root_tmp.path()).await;

        assert_eq!(res.coverage.state, CoverageState::Partial);
        assert_eq!(res.coverage.sessions, 0);
    }
}
