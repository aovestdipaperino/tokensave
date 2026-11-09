//! `AugmentCode` (auggie) CLI session accounting parser.
//!
//! Discovers `~/.augment/sessions/<uuid>.json` session files and surfaces the
//! observed subagent USD spend per session as an optional [`CostTurn`].
//!
//! Design constraints (honest Partial coverage, by design):
//! - `subAgentCostUsd` is a real dollar amount, but it is SUBAGENT-only: it is
//!   the spend of subagents the session spawned, not the full session spend. The
//!   main-agent turns are routed through Augment's `Prism` aliasing (`model_id`
//!   values like `butler_a`) with per-node `token_usage` populated on only a
//!   fraction of nodes, so a trustworthy full-session total cannot be sourced.
//!   We therefore surface the subagent USD we can see and keep Augment coverage
//!   [`CoverageState::Partial`], never `Complete`. We do not fabricate a total.
//! - `subAgentCreditsUsed` is present but observed always zero locally, so it is
//!   ignored; the honest unit here is USD.
//! - The stored `model` is the session's configured `agentState.modelId` (a
//!   label, not a per-cost attribution), so Augment stays out of the per-model
//!   USD view; its spend shows in `cost --by-agent` only.
//! - No network or browser auth; all parsing is local and offline.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use walkdir::WalkDir;

use crate::accounting::parser::{parse_timestamp, CoverageState, SourceCoverage};
use crate::types::CostTurn;

/// One `~/.augment/sessions/<uuid>.json` file (only the fields we account for).
#[derive(Debug, Deserialize)]
struct Session {
    #[serde(rename = "sessionId")]
    id: Option<String>,
    created: Option<String>,
    #[serde(rename = "subAgentCostUsd")]
    sub_agent_cost_usd: Option<f64>,
    #[serde(rename = "agentState")]
    agent_state: Option<AgentState>,
}

#[derive(Debug, Deserialize)]
struct AgentState {
    #[serde(rename = "modelId")]
    model_id: Option<String>,
}

/// Recursively discover `*.json` session files directly below `root`, counting
/// `WalkDir` errors. Returns `(files, walk_errors)`, sorted for determinism.
/// Symlinks are not followed.
fn collect_sessions(root: &Path) -> (Vec<PathBuf>, u64) {
    match std::fs::metadata(root) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), 0);
        }
        Err(_) => return (Vec::new(), 1),
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let mut walk_errors: u64 = 0;
    for entry in WalkDir::new(root).follow_links(false) {
        match entry {
            Err(_) => walk_errors += 1,
            Ok(e) => {
                let name = e.file_name().to_str().unwrap_or_default();
                let is_session = Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
                if e.file_type().is_file() && is_session {
                    files.push(e.into_path());
                }
            }
        }
    }
    files.sort();
    (files, walk_errors)
}

/// Recursively discover all `*.json` session files below `root`.
#[cfg(test)]
pub(crate) fn find_session_files(root: &Path) -> Vec<PathBuf> {
    collect_sessions(root).0
}

/// Parse a single `AugmentCode` session file at `path`.
///
/// # Return value
///
/// | Condition | Result |
/// |-----------|--------|
/// | File unreadable or not a JSON object | `Err(())` |
/// | No positive `subAgentCostUsd` (no observed subagent spend) | `Ok(None)` |
/// | Subagent spend present; `created` absent or unparseable | `Err(())` |
/// | Subagent spend present; `created` present and parseable | `Ok(Some(turn))` |
///
/// The emitted turn carries `agent = "augment"`, the real subagent USD in
/// `cost_usd`, `credits = None`, and zero token fields (main-agent tokens are
/// not sourced). `model` is the session's `agentState.modelId` label, or the
/// session id's routing state is unknown so it falls back to `"routed"`.
pub(crate) fn parse_session_file(path: &Path) -> Result<Option<CostTurn>, ()> {
    let file = std::fs::File::open(path).map_err(|_| ())?;
    let session: Session =
        serde_json::from_reader(std::io::BufReader::new(file)).map_err(|_| ())?;

    // No observed subagent spend: not accountable, and counting a $0 row would
    // be a phantom. This mirrors how Copilot drops a no-AIU checkpoint.
    let cost_usd = session.sub_agent_cost_usd.unwrap_or(0.0);
    if cost_usd <= 0.0 {
        return Ok(None);
    }

    let timestamp = session
        .created
        .as_deref()
        .and_then(parse_timestamp)
        .ok_or(())?;

    // Session id from the field, falling back to the file stem.
    let session_id = session
        .id
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .ok_or(())?;

    let model = session
        .agent_state
        .and_then(|a| a.model_id)
        .unwrap_or_else(|| "routed".to_string());

    let message_id = format!("augment:{session_id}");

    Ok(Some(CostTurn {
        message_id,
        project_hash: "unknown".to_string(),
        session_id,
        model,
        timestamp,
        input_tokens: 0,
        output_tokens: 0,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        cost_usd,
        category: "unclassified".to_string(),
        tool_names: String::new(),
        agent: "augment".to_string(),
        credits: None,
    }))
}

/// Result of an `AugmentCode` session directory ingestion.
pub(crate) struct AugmentIngestResult {
    pub coverage: SourceCoverage,
    pub rows_changed: u64,
}

/// Ingest `AugmentCode` session files from `root` (`~/.augment/sessions`).
///
/// Coverage rules (Augment is never `Complete`, by design):
/// - [`CoverageState::Absent`]: no readable sessions and no errors.
/// - [`CoverageState::Partial`]: at least one readable session or any
///   parse/read/walk failure.
pub(crate) async fn ingest_dir(
    gdb: &crate::global_db::GlobalDb,
    root: &Path,
) -> AugmentIngestResult {
    let (files, walk_errors) = collect_sessions(root);

    if files.is_empty() && walk_errors == 0 {
        return AugmentIngestResult {
            coverage: SourceCoverage {
                agent: "augment",
                state: CoverageState::Absent,
                sessions: 0,
            },
            rows_changed: 0,
        };
    }

    let mut parse_errors: u64 = 0;
    let mut readable_sessions: u64 = 0;
    let mut turns: Vec<CostTurn> = Vec::new();

    for file in &files {
        match parse_session_file(file) {
            Err(()) => parse_errors += 1,
            Ok(None) => readable_sessions += 1,
            Ok(Some(turn)) => {
                readable_sessions += 1;
                turns.push(turn);
            }
        }
    }

    let sessions = turns.len() as u64;

    // Readable sessions keep cached snapshots attributable even when their
    // current cost field is empty; observed subagent spend is never Complete.
    let state = if readable_sessions > 0 || parse_errors > 0 || walk_errors > 0 {
        CoverageState::Partial
    } else {
        CoverageState::Absent
    };

    let mut rows_changed: u64 = 0;
    for turn in &turns {
        match gdb.upsert_augment_turn(turn).await {
            Ok(true) => rows_changed += 1,
            Ok(false) => {}
            Err(error) => {
                eprintln!("[tokensave] Augment accounting snapshot failed: {error}");
            }
        }
    }

    AugmentIngestResult {
        coverage: SourceCoverage {
            agent: "augment",
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

    fn write_session(root: &Path, name: &str, content: &str) -> PathBuf {
        fs::create_dir_all(root).unwrap();
        let path = root.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    fn session_json(session_id: &str, cost: &str, created: &str, model: &str) -> String {
        format!(
            r#"{{"sessionId":"{session_id}","created":"{created}","modified":"{created}","subAgentCostUsd":{cost},"subAgentCreditsUsed":0,"agentState":{{"modelId":"{model}"}},"chatHistory":[]}}"#
        )
    }

    #[test]
    fn subagent_usd_parses_to_turn() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "s1.json",
            &session_json(
                "sess-a",
                "0.6676",
                "2026-05-09T12:56:30.050Z",
                "claude-haiku-4-5",
            ),
        );
        let turn = parse_session_file(&path).unwrap().unwrap();
        assert_eq!(turn.agent, "augment");
        assert_eq!(turn.message_id, "augment:sess-a");
        assert_eq!(turn.session_id, "sess-a");
        assert_eq!(turn.model, "claude-haiku-4-5");
        assert_eq!(turn.credits, None);
        assert_eq!(turn.input_tokens, 0);
        assert_eq!(turn.output_tokens, 0);
        assert!((turn.cost_usd - 0.6676).abs() < 1e-12);
    }

    #[test]
    fn zero_subagent_cost_returns_none() {
        // A session with no observed subagent spend must not become a $0 phantom.
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "s2.json",
            &session_json("sess-b", "0", "2026-05-09T12:56:30.050Z", "butler_a"),
        );
        assert!(parse_session_file(&path).unwrap().is_none());
    }

    #[test]
    fn missing_subagent_cost_field_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "s3.json",
            r#"{"sessionId":"sess-c","created":"2026-05-09T12:56:30.050Z","agentState":{"modelId":"butler_a"}}"#,
        );
        assert!(parse_session_file(&path).unwrap().is_none());
    }

    #[test]
    fn spend_missing_created_is_err() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "s4.json",
            r#"{"sessionId":"sess-d","subAgentCostUsd":1.25,"agentState":{"modelId":"butler_a"}}"#,
        );
        assert!(parse_session_file(&path).is_err());
    }

    #[test]
    fn routed_alias_recorded_when_no_agent_state_model() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(
            tmp.path(),
            "s5.json",
            r#"{"sessionId":"sess-e","created":"2026-05-09T12:56:30.050Z","subAgentCostUsd":2.5}"#,
        );
        let turn = parse_session_file(&path).unwrap().unwrap();
        assert_eq!(turn.model, "routed");
        assert!((turn.cost_usd - 2.5).abs() < 1e-12);
    }

    #[test]
    fn non_object_json_is_err_when_read() {
        let tmp = TempDir::new().unwrap();
        let path = write_session(tmp.path(), "s6.json", "[1,2,3]");
        assert!(parse_session_file(&path).is_err());
    }

    #[test]
    fn discovery_is_sorted_and_json_only() {
        let tmp = TempDir::new().unwrap();
        write_session(tmp.path(), "b.json", "{}");
        write_session(tmp.path(), "a.json", "{}");
        write_session(tmp.path(), "notes.txt", "x");
        let files = find_session_files(tmp.path());
        assert_eq!(files.len(), 2);
        assert!(files[0].to_string_lossy().ends_with("a.json"));
        assert!(files[1].to_string_lossy().ends_with("b.json"));
    }

    #[test]
    fn missing_root_returns_empty() {
        let (files, errors) = collect_sessions(Path::new("/no/such/augment/root"));
        assert!(files.is_empty());
        assert_eq!(errors, 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn inaccessible_parent_reports_partial_coverage() {
        use std::os::unix::fs::PermissionsExt;

        let root_tmp = TempDir::new().unwrap();
        let parent = root_tmp.path().join("inaccessible");
        let root = parent.join("sessions");
        fs::create_dir_all(&root).unwrap();

        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        fs::set_permissions(&parent, fs::Permissions::from_mode(0o0)).unwrap();
        let result = ingest_dir(&gdb, &root).await;
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(result.coverage.state, CoverageState::Partial);
        assert_eq!(result.coverage.sessions, 0);
        assert_eq!(result.rows_changed, 0);
    }

    #[tokio::test]
    async fn ingest_dir_absent_when_root_missing() {
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let res = ingest_dir(&gdb, Path::new("/no/such/augment/root")).await;
        assert_eq!(res.coverage.state, CoverageState::Absent);
        assert_eq!(res.coverage.sessions, 0);
        assert_eq!(res.rows_changed, 0);
    }

    #[tokio::test]
    async fn usd_snapshots_grow_without_replay_regression_or_bucket_movement() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("sessions");
        let gdb = crate::global_db::GlobalDb::open_at(&tmp.path().join("global.db"))
            .await
            .unwrap();
        for (index, (cost, changed, expected)) in [
            ("12.34", 1, 12.34),
            ("20.00", 1, 20.00),
            ("20.00", 0, 20.00),
            ("9.00", 0, 20.00),
        ]
        .into_iter()
        .enumerate()
        {
            let created = if index == 0 {
                "2026-05-09T12:56:30.050Z"
            } else {
                "2026-05-10T12:56:30.050Z"
            };
            write_session(
                &root,
                "growing.json",
                &session_json("growing", cost, created, "butler_a"),
            );
            let result = ingest_dir(&gdb, &root).await;
            assert_eq!(result.rows_changed, changed);
            assert_eq!(result.coverage.state, CoverageState::Partial);
            assert!((gdb.total_cost_since(0).await.unwrap() - expected).abs() < 1e-12);
            let next_day = parse_timestamp("2026-05-10T00:00:00Z").unwrap();
            assert!(gdb.total_cost_since(next_day).await.unwrap().abs() < 1e-12);
        }
    }

    #[tokio::test]
    async fn ingest_dir_partial_when_subagent_spend_present() {
        let root_tmp = TempDir::new().unwrap();
        write_session(
            root_tmp.path(),
            "s1.json",
            &session_json(
                "sess-1",
                "0.5",
                "2026-05-09T12:56:30.050Z",
                "claude-opus-4-8",
            ),
        );
        // A zero-spend session contributes no row.
        write_session(
            root_tmp.path(),
            "s2.json",
            &session_json("sess-2", "0", "2026-05-09T13:56:30.050Z", "butler_a"),
        );
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();
        let res = ingest_dir(&gdb, root_tmp.path()).await;
        // Never Complete: observed spend is subagent-only.
        assert_eq!(res.coverage.state, CoverageState::Partial);
        assert_eq!(res.coverage.sessions, 1);
        assert_eq!(res.rows_changed, 1);
    }

    #[tokio::test]
    async fn readable_zero_or_missing_snapshot_preserves_cached_spend() {
        let root_tmp = TempDir::new().unwrap();
        let session_path = root_tmp.path().join("s1.json");
        write_session(
            root_tmp.path(),
            "s1.json",
            &session_json("sess-1", "1.25", "2026-05-09T12:56:30.050Z", "butler_a"),
        );
        let db_tmp = TempDir::new().unwrap();
        let db_path = db_tmp.path().join(".tokensave").join("global.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let gdb = crate::global_db::GlobalDb::open_at(&db_path).await.unwrap();

        assert_eq!(
            ingest_dir(&gdb, root_tmp.path()).await.coverage.state,
            CoverageState::Partial
        );

        for snapshot in [
            session_json("sess-1", "0", "2026-05-09T12:56:30.050Z", "butler_a"),
            r#"{"sessionId":"sess-1","created":"2026-05-09T12:56:30.050Z","agentState":{"modelId":"butler_a"}}"#.to_string(),
        ] {
            fs::write(&session_path, snapshot).unwrap();
            let result = ingest_dir(&gdb, root_tmp.path()).await;
            assert_eq!(result.coverage.state, CoverageState::Partial);

            let summary = crate::accounting::metrics::cost_summary_with_source_presence(
                &gdb,
                0,
                false,
                false,
                result.coverage.state != CoverageState::Absent,
            )
            .await
            .unwrap();
            assert!((summary.total_cost - 1.25).abs() < 1e-12);
        }
    }
}
