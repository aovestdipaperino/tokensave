//! Factory Droid session accounting parser.
//!
//! Discovers `*.settings.json` files under a supplied root directory,
//! parses each file into an optional [`CostTurn`], and exposes a sorted,
//! deterministic file-list function for incremental ingestion.
//!
//! Design constraints:
//! - No USD cost conversion (Droid uses Factory credits, not dollars).
//! - No network or browser auth; all parsing is local and offline.
//! - `inclusiveTokenUsage` is intentionally ignored; only `tokenUsage`
//!   (the exclusive per-session counter) is consumed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use walkdir::WalkDir;

use crate::accounting::parser::{parse_timestamp, CoverageState, SourceCoverage};
use crate::types::CostTurn;

/// Exclusive per-session token counters from a Droid `.settings.json` file.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExclusiveTokenUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    cache_creation_tokens: i64,
    #[serde(default)]
    cache_read_tokens: i64,
    #[serde(default)]
    thinking_tokens: i64,
    /// Factory credit units consumed in the session. `None` when absent.
    factory_credits: Option<i64>,
}

/// Raw Droid session settings as stored in `<session-id>.settings.json`.
///
/// `inclusiveTokenUsage` is deliberately absent from this struct so serde
/// silently skips it; only the exclusive per-session `tokenUsage` is read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DroidSettings {
    /// Stable session-start timestamp written by the provider lock mechanism.
    /// This is the authoritative timestamp in real `*.settings.json` files.
    /// Optional because Factory init files (all-zero / absent tokenUsage) are
    /// written before the provider lock is acquired and legitimately omit it.
    #[serde(rename = "providerLockTimestamp")]
    provider_lock_timestamp: Option<String>,
    model: Option<String>,
    token_usage: Option<ExclusiveTokenUsage>,
}

/// Recursively discover `*.settings.json` files below `root`, counting any
/// `WalkDir` errors (e.g. permission-denied on a subdirectory).
///
/// Returns `(files, walk_errors)` where `walk_errors` is the number of
/// directory entries that could not be read.  The file list is sorted for
/// determinism.  Symlinks are not followed.
fn collect_settings(root: &Path) -> (Vec<PathBuf>, u64) {
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
                    && e.file_name()
                        .to_str()
                        .is_some_and(|n| n.ends_with(".settings.json"))
                {
                    files.push(e.into_path());
                }
            }
        }
    }
    files.sort();
    (files, walk_errors)
}

/// Recursively discover all `*.settings.json` files below `root`.
///
/// Returns a sorted, deterministic list of paths. If `root` does not exist or
/// cannot be read, returns an empty `Vec`.
#[cfg(test)]
pub(crate) fn find_settings_files(root: &Path) -> Vec<PathBuf> {
    collect_settings(root).0
}

/// Parse a single Droid session settings file at `path`.
///
/// # Return value
///
/// | Condition | Result |
/// |-----------|--------|
/// | File unreadable or not valid JSON | `Err(())` |
/// | `tokenUsage` absent, or all counters normalize to zero | `Ok(None)` |
/// | Nonzero usage; `providerLockTimestamp` absent or unparseable | `Err(())` |
/// | Nonzero usage; `providerLockTimestamp` present and parseable | `Ok(Some(turn))` |
///
/// Negative token counters are clamped to zero. `thinkingTokens` is folded
/// into `output_tokens`. `cache_creation_tokens` and `cache_read_tokens` are
/// preserved in their own fields. `cost_usd` is always `0.0`. `credits` holds
/// the exact `factoryCredits` value when it is non-negative; negative values
/// are treated as absent.
pub(crate) fn parse_settings_file(path: &Path) -> Result<Option<CostTurn>, ()> {
    let contents = std::fs::read_to_string(path).map_err(|_| ())?;
    let settings: DroidSettings = serde_json::from_str(&contents).map_err(|_| ())?;

    // Session id: strip the `.settings.json` suffix from the filename.
    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or(())?;
    let session_id = file_name
        .strip_suffix(".settings.json")
        .ok_or(())?
        .to_string();

    // Project hash: the immediate parent directory name, or "unknown".
    let project_hash = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let Some(usage) = settings.token_usage else {
        return Ok(None);
    };

    // Clamp negative counters to zero.
    let input_tokens = usage.input_tokens.max(0) as u64;
    let output_tokens_raw = usage.output_tokens.max(0) as u64;
    let cache_write_tokens = usage.cache_creation_tokens.max(0) as u64;
    let cache_read_tokens = usage.cache_read_tokens.max(0) as u64;
    let thinking_tokens = usage.thinking_tokens.max(0) as u64;

    // Fold thinkingTokens into output_tokens.
    let output_tokens = output_tokens_raw.saturating_add(thinking_tokens);

    // All-zero check after normalization.
    // Factory init files (no providerLockTimestamp, all-zero counters) reach
    // here; returning Ok(None) keeps them out of parse_errors and preserves
    // Complete/Absent coverage when mixed with valid sessions.
    if input_tokens == 0 && output_tokens == 0 && cache_write_tokens == 0 && cache_read_tokens == 0
    {
        return Ok(None);
    }

    // Nonzero usage: providerLockTimestamp is required to anchor the turn in
    // time. Files without it cannot be bucketed honestly → Err(()).
    let timestamp = settings
        .provider_lock_timestamp
        .and_then(|ts| parse_timestamp(&ts))
        .ok_or(())?;

    // Credits: exact value, negative treated as absent.
    let credits = usage
        .factory_credits
        .and_then(|c| if c >= 0 { Some(c as u64) } else { None });

    let model = settings.model.unwrap_or_else(|| "unknown".to_string());
    let message_id = format!("droid:{session_id}");

    Ok(Some(CostTurn {
        message_id,
        project_hash,
        session_id,
        model,
        timestamp,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
        cost_usd: 0.0,
        category: "unclassified".to_string(),
        tool_names: String::new(),
        agent: "droid".to_string(),
        credits,
    }))
}

/// Result of a Droid session directory ingestion.
pub(crate) struct DroidIngestResult {
    pub coverage: SourceCoverage,
    pub rows_changed: u64,
}

/// Deduplicate turns by `message_id` using monotonic semantics.
///
/// A candidate replaces the stored turn only when every token counter is ≥ the
/// stored value and credits do not regress.  Missing candidate credits preserve
/// known stored credits.  The output is sorted by `message_id` for
/// determinism.
fn dedupe_turns(turns: Vec<CostTurn>) -> Vec<CostTurn> {
    let mut map: HashMap<String, CostTurn> = HashMap::new();

    for candidate in turns {
        match map.get(&candidate.message_id) {
            None => {
                map.insert(candidate.message_id.clone(), candidate);
            }
            Some(stored) => {
                let tokens_dominate = candidate.input_tokens >= stored.input_tokens
                    && candidate.output_tokens >= stored.output_tokens
                    && candidate.cache_write_tokens >= stored.cache_write_tokens
                    && candidate.cache_read_tokens >= stored.cache_read_tokens;

                let credits_ok = match (candidate.credits, stored.credits) {
                    (Some(c), Some(s)) => c >= s,
                    _ => true,
                };

                if tokens_dominate && credits_ok {
                    let preserved_credits = if candidate.credits.is_none() {
                        stored.credits
                    } else {
                        candidate.credits
                    };
                    let mut winner = candidate;
                    winner.credits = preserved_credits;
                    map.insert(winner.message_id.clone(), winner);
                }
            }
        }
    }

    let mut result: Vec<CostTurn> = map.into_values().collect();
    result.sort_by(|a, b| a.message_id.cmp(&b.message_id));
    result
}

/// Ingest Droid session settings files from `root`.
///
/// Discovers all `*.settings.json` files, parses them, deduplicates by
/// session ID, and upserts into `gdb`.
///
/// Coverage rules:
/// - [`CoverageState::Absent`]: no files found, or only valid empty/zero
///   settings with no parse failures.
/// - [`CoverageState::Partial`]: any parse/read failure, or any valid
///   session lacks credits.
/// - [`CoverageState::Complete`]: all files parse successfully and every
///   valid session carries credits.
pub(crate) async fn ingest_dir(gdb: &crate::global_db::GlobalDb, root: &Path) -> DroidIngestResult {
    let (files, walk_errors) = collect_settings(root);

    if files.is_empty() && walk_errors == 0 {
        return DroidIngestResult {
            coverage: SourceCoverage {
                agent: "droid",
                state: CoverageState::Absent,
                sessions: 0,
            },
            rows_changed: 0,
        };
    }

    let mut parse_errors: u64 = 0;
    let mut valid_turns: Vec<CostTurn> = Vec::new();

    for file in &files {
        match parse_settings_file(file) {
            Err(()) => parse_errors += 1,
            Ok(None) => {}
            Ok(Some(turn)) => {
                valid_turns.push(turn);
            }
        }
    }

    let deduped = dedupe_turns(valid_turns);
    let sessions = deduped.len() as u64;

    // Check for missing credits on the deduped unique sessions, not raw files,
    // so a stale copied-session file without credits does not falsely degrade
    // coverage when the dominant copy does carry credits.
    let has_partial_credits = deduped.iter().any(|t| t.credits.is_none());

    let state = if parse_errors > 0 || walk_errors > 0 || has_partial_credits {
        CoverageState::Partial
    } else if sessions == 0 {
        CoverageState::Absent
    } else {
        CoverageState::Complete
    };

    let mut rows_changed: u64 = 0;
    for turn in &deduped {
        if gdb.upsert_droid_turn(turn).await {
            rows_changed += 1;
        }
    }

    DroidIngestResult {
        coverage: SourceCoverage {
            agent: "droid",
            state,
            sessions,
        },
        rows_changed,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    /// Exclusive tokenUsage is used even when a conflicting inclusiveTokenUsage
    /// is also present in the JSON (the inclusive object must be ignored).
    #[test]
    fn exclusive_tokens_and_credits_with_inclusive_present() {
        let dir = TempDir::new().unwrap();
        let project_dir = dir.path().join("proj-abc");
        fs::create_dir(&project_dir).unwrap();
        let path = write_file(
            &project_dir,
            "session1.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "model": "claude-opus-4",
                "tokenUsage": {
                    "inputTokens": 1000,
                    "outputTokens": 200,
                    "cacheCreationTokens": 50,
                    "cacheReadTokens": 80,
                    "thinkingTokens": 0,
                    "factoryCredits": 42
                },
                "inclusiveTokenUsage": {
                    "inputTokens": 9999,
                    "outputTokens": 8888
                }
            }"#,
        );
        let turn = parse_settings_file(&path).unwrap().unwrap();
        assert_eq!(turn.input_tokens, 1000);
        assert_eq!(turn.output_tokens, 200);
        assert_eq!(turn.cache_write_tokens, 50);
        assert_eq!(turn.cache_read_tokens, 80);
        assert_eq!(turn.credits, Some(42));
        assert_eq!(turn.agent, "droid");
        assert_eq!(turn.session_id, "session1");
        assert_eq!(turn.project_hash, "proj-abc");
        assert_eq!(turn.message_id, "droid:session1");
        assert_eq!(turn.model, "claude-opus-4");
        assert!(turn.cost_usd.abs() < f64::EPSILON);
        assert_eq!(turn.category, "unclassified");
        assert!(turn.tool_names.is_empty());
    }

    /// `thinkingTokens` is folded into `output_tokens`.
    #[test]
    fn thinking_tokens_folded_into_output() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "tokenUsage": {
                    "inputTokens": 500,
                    "outputTokens": 100,
                    "thinkingTokens": 300
                }
            }"#,
        );
        let turn = parse_settings_file(&path).unwrap().unwrap();
        // 100 output + 300 thinking = 400
        assert_eq!(turn.output_tokens, 400);
        assert_eq!(turn.input_tokens, 500);
    }

    /// Missing tokenUsage → Ok(None).
    #[test]
    fn missing_token_usage_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{"providerLockTimestamp": "2026-04-14T10:00:00.000Z"}"#,
        );
        assert!(parse_settings_file(&path).unwrap().is_none());
    }

    /// All-zero (after normalization) tokenUsage → Ok(None).
    #[test]
    fn all_zero_token_usage_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "tokenUsage": {
                    "inputTokens": 0,
                    "outputTokens": 0,
                    "cacheCreationTokens": 0,
                    "cacheReadTokens": 0,
                    "thinkingTokens": 0
                }
            }"#,
        );
        assert!(parse_settings_file(&path).unwrap().is_none());
    }

    /// Negative counters are clamped to zero; if all clamp to zero → Ok(None).
    #[test]
    fn negative_counters_clamp_to_zero_all_zero_is_none() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "tokenUsage": {
                    "inputTokens": -5,
                    "outputTokens": -3
                }
            }"#,
        );
        assert!(parse_settings_file(&path).unwrap().is_none());
    }

    /// Negative counters are clamped but non-zero ones survive.
    #[test]
    fn negative_counters_clamp_nonzero_survivors_retained() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "tokenUsage": {
                    "inputTokens": 800,
                    "outputTokens": -10
                }
            }"#,
        );
        let turn = parse_settings_file(&path).unwrap().unwrap();
        assert_eq!(turn.input_tokens, 800);
        assert_eq!(turn.output_tokens, 0);
    }

    /// Tokens are retained and credits are None when factoryCredits is absent.
    #[test]
    fn tokens_retained_when_credits_absent() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "tokenUsage": {
                    "inputTokens": 100,
                    "outputTokens": 50
                }
            }"#,
        );
        let turn = parse_settings_file(&path).unwrap().unwrap();
        assert_eq!(turn.input_tokens, 100);
        assert_eq!(turn.output_tokens, 50);
        assert!(turn.credits.is_none());
    }

    /// Malformed JSON returns Err(()).
    #[test]
    fn malformed_json_is_err() {
        let dir = TempDir::new().unwrap();
        let path = write_file(dir.path(), "sess.settings.json", "not json at all");
        assert!(parse_settings_file(&path).is_err());
    }

    /// Missing providerLockTimestamp field returns Err(()).
    #[test]
    fn missing_provider_lock_timestamp_is_err() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{"model": "gpt-4", "tokenUsage": {"inputTokens": 100}}"#,
        );
        assert!(parse_settings_file(&path).is_err());
    }

    /// Regression: settings with no providerLockTimestamp and missing tokenUsage
    /// must return Ok(None), not Err — these are real Factory init files.
    #[test]
    fn no_provider_lock_timestamp_absent_usage_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = write_file(dir.path(), "sess.settings.json", r"{}");
        assert!(
            parse_settings_file(&path).unwrap().is_none(),
            "absent providerLockTimestamp + absent tokenUsage must be Ok(None)"
        );
    }

    /// Regression: settings with no providerLockTimestamp and all-zero tokenUsage
    /// must return Ok(None), not Err.
    #[test]
    fn no_provider_lock_timestamp_all_zero_usage_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{"tokenUsage": {"inputTokens": 0, "outputTokens": 0}}"#,
        );
        assert!(
            parse_settings_file(&path).unwrap().is_none(),
            "absent providerLockTimestamp + all-zero tokenUsage must be Ok(None)"
        );
    }

    /// Regression: `ingest_dir` does not degrade to Partial when Factory init files
    /// (no providerLockTimestamp, zero/absent tokenUsage) are mixed with valid sessions.
    #[tokio::test]
    async fn ingest_dir_empty_init_files_do_not_degrade_coverage() {
        let root = TempDir::new().unwrap();
        let proj = root.path().join("proj");
        fs::create_dir_all(&proj).unwrap();

        // Valid session with credits.
        write_file(
            &proj,
            "valid.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "tokenUsage": {"inputTokens": 100, "outputTokens": 10, "factoryCredits": 100}
            }"#,
        );

        // Factory init files: no providerLockTimestamp, zero/absent tokenUsage.
        write_file(&proj, "init1.settings.json", r"{}");
        write_file(&proj, "init2.settings.json", r#"{"model": "claude"}"#);
        write_file(
            &proj,
            "init3.settings.json",
            r#"{"tokenUsage": {"inputTokens": 0, "outputTokens": 0}}"#,
        );

        let db_tmp = TempDir::new().unwrap();
        let db = open_test_db(&db_tmp).await;

        let result = ingest_dir(&db, root.path()).await;

        assert_eq!(
            result.coverage.state,
            crate::accounting::parser::CoverageState::Complete,
            "empty init files must not degrade coverage to Partial"
        );
        assert_eq!(result.coverage.sessions, 1, "only the valid session counts");
    }

    /// Recursive discovery finds only .settings.json files, sorted.
    #[test]
    fn recursive_discovery_sorted_settings_only() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("a").join("b");
        fs::create_dir_all(&sub).unwrap();

        write_file(dir.path(), "z.settings.json", "{}");
        write_file(dir.path(), "a.settings.json", "{}");
        write_file(dir.path(), "other.json", "{}"); // not .settings.json
        write_file(&sub, "deep.settings.json", "{}");
        write_file(&sub, "ignore.jsonl", "{}"); // wrong extension

        let files = find_settings_files(dir.path());
        // Should find 3 .settings.json files, none others, sorted
        assert_eq!(files.len(), 3);
        let names: Vec<&str> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"a.settings.json"));
        assert!(names.contains(&"z.settings.json"));
        assert!(names.contains(&"deep.settings.json"));
        // Verify sorted order
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
    }

    /// Missing root returns empty list.
    #[test]
    fn missing_root_returns_empty() {
        let files = find_settings_files(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(files.is_empty());
    }

    /// `collect_settings` counts `WalkDir` errors from unreadable directories.
    #[cfg(unix)]
    #[test]
    fn collect_settings_counts_walk_errors() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "ok.settings.json", "{}");

        let locked = dir.path().join("locked");
        fs::create_dir_all(&locked).unwrap();
        write_file(&locked, "hidden.settings.json", "{}");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let (files, errors) = collect_settings(dir.path());

        // Restore permissions before any assertion so TempDir cleanup succeeds.
        let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o755));

        assert!(
            errors > 0,
            "should have walk errors for the locked directory"
        );
        assert_eq!(files.len(), 1, "only the accessible file is returned");
    }

    /// Model defaults to "unknown" when absent.
    #[test]
    fn model_defaults_to_unknown() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{"providerLockTimestamp": "2026-04-14T10:00:00.000Z", "tokenUsage": {"inputTokens": 10}}"#,
        );
        let turn = parse_settings_file(&path).unwrap().unwrap();
        assert_eq!(turn.model, "unknown");
    }

    /// Regression: real schema has `providerLockTimestamp` at the top level and
    /// no `timestamp` field.  Parse must succeed and the `CostTurn` timestamp must
    /// equal `parse_timestamp("providerLockTimestamp" value)`.
    #[test]
    fn real_schema_provider_lock_timestamp_no_top_level_timestamp() {
        let dir = TempDir::new().unwrap();
        let path = write_file(
            dir.path(),
            "sess.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "model": "claude-opus-4",
                "tokenUsage": {
                    "inputTokens": 1000,
                    "outputTokens": 200,
                    "cacheCreationTokens": 50,
                    "cacheReadTokens": 80,
                    "thinkingTokens": 0,
                    "factoryCredits": 42
                }
            }"#,
        );
        let expected_ts = parse_timestamp("2026-04-14T10:00:00.000Z").unwrap();
        let turn = parse_settings_file(&path).unwrap().unwrap();
        assert_eq!(turn.timestamp, expected_ts);
        assert_eq!(turn.input_tokens, 1000);
        assert_eq!(turn.output_tokens, 200);
        assert_eq!(turn.credits, Some(42));
        assert_eq!(turn.agent, "droid");
    }

    // ── ingest_dir tests ─────────────────────────────────────────────────────

    async fn open_test_db(tmp: &TempDir) -> crate::global_db::GlobalDb {
        let db_path = tmp.path().join(".tokensave").join("global.db");
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        crate::global_db::GlobalDb::open_at(&db_path).await.unwrap()
    }

    /// Parent session (exclusive tokenUsage only, inclusive ignored) plus a
    /// separate child session: combined input=130, output=25, credits=1300.
    /// A second ingestion of the same root changes zero rows.
    #[tokio::test]
    async fn ingest_dir_parent_child_sums_and_second_ingestion_noop() {
        let sessions_dir = TempDir::new().unwrap();
        let proj = sessions_dir.path().join("proj1");
        fs::create_dir_all(&proj).unwrap();

        // Parent: tokenUsage (100/20/1000) + inclusiveTokenUsage (must be ignored).
        write_file(
            &proj,
            "parent.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "model": "claude-opus-4",
                "tokenUsage": {
                    "inputTokens": 100,
                    "outputTokens": 20,
                    "cacheCreationTokens": 0,
                    "cacheReadTokens": 0,
                    "thinkingTokens": 0,
                    "factoryCredits": 1000
                },
                "inclusiveTokenUsage": {
                    "inputTokens": 9999,
                    "outputTokens": 8888,
                    "factoryCredits": 99999
                }
            }"#,
        );

        // Child: tokenUsage (30/5/300).
        write_file(
            &proj,
            "child.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T11:00:00.000Z",
                "model": "claude-opus-4",
                "tokenUsage": {
                    "inputTokens": 30,
                    "outputTokens": 5,
                    "cacheCreationTokens": 0,
                    "cacheReadTokens": 0,
                    "thinkingTokens": 0,
                    "factoryCredits": 300
                }
            }"#,
        );

        let db_tmp = TempDir::new().unwrap();
        let db = open_test_db(&db_tmp).await;

        let result = ingest_dir(&db, sessions_dir.path()).await;
        assert_eq!(result.coverage.sessions, 2, "two sessions expected");
        assert_eq!(result.rows_changed, 2, "both rows should be inserted");

        // Sum across both sessions.
        let summaries = db.cost_by_agent_since(0).await;
        let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
        assert_eq!(droid.input_tokens, 130, "100+30 input");
        assert_eq!(droid.output_tokens, 25, "20+5 output");
        assert_eq!(droid.credits, Some(1300), "1000+300 credits");

        // Second ingestion — nothing changes.
        let result2 = ingest_dir(&db, sessions_dir.path()).await;
        assert_eq!(result2.rows_changed, 0, "second pass changes no rows");
    }

    /// A directory containing only malformed JSON gives Partial coverage,
    /// zero sessions, and zero `rows_changed`.
    #[tokio::test]
    async fn ingest_dir_malformed_only_is_partial() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "bad.settings.json", "this is not json");

        let db_tmp = TempDir::new().unwrap();
        let db = open_test_db(&db_tmp).await;

        let result = ingest_dir(&db, dir.path()).await;
        assert_eq!(
            result.coverage.state,
            crate::accounting::parser::CoverageState::Partial,
            "malformed file must give Partial"
        );
        assert_eq!(result.coverage.sessions, 0);
        assert_eq!(result.rows_changed, 0);
    }

    /// Two files sharing the same session ID (copied sessions) are deduped;
    /// the file with the larger snapshot wins.
    #[tokio::test]
    async fn ingest_dir_copied_session_dedupe_picks_larger() {
        let root = TempDir::new().unwrap();
        let dir_a = root.path().join("a");
        let dir_b = root.path().join("b");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();

        let smaller = r#"{
            "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
            "model": "m",
            "tokenUsage": {
                "inputTokens": 100,
                "outputTokens": 20,
                "factoryCredits": 500
            }
        }"#;
        let larger = r#"{
            "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
            "model": "m",
            "tokenUsage": {
                "inputTokens": 200,
                "outputTokens": 40,
                "factoryCredits": 1000
            }
        }"#;

        // Same session ID "sess1" in both dirs.
        write_file(&dir_a, "sess1.settings.json", smaller);
        write_file(&dir_b, "sess1.settings.json", larger);

        let db_tmp = TempDir::new().unwrap();
        let db = open_test_db(&db_tmp).await;

        let result = ingest_dir(&db, root.path()).await;

        assert_eq!(
            result.coverage.sessions, 1,
            "dedupe must yield 1 unique session"
        );
        assert_eq!(result.rows_changed, 1, "only one row inserted");

        let summaries = db.cost_by_agent_since(0).await;
        let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
        assert_eq!(droid.input_tokens, 200, "larger snapshot selected");
        assert_eq!(droid.credits, Some(1000), "larger credits selected");
    }

    /// Regression: stale copied-session file without credits + dominant copy with credits.
    ///
    /// Same session ID appears in two directories: one is a stale/lower snapshot
    /// missing `factoryCredits`, the other is the dominant/larger snapshot that
    /// carries credits.  After deduplication the unique session is complete, so
    /// coverage must be `Complete`, `sessions == 1`, and stored credits exact.
    #[tokio::test]
    async fn ingest_dir_heterogeneous_copies_dedupe_before_coverage() {
        let root = TempDir::new().unwrap();
        let dir_a = root.path().join("dir_a");
        let dir_b = root.path().join("dir_b");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();

        // Stale copy: lower token counts, no factoryCredits.
        let stale = r#"{
            "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
            "model": "claude-opus-4",
            "tokenUsage": {
                "inputTokens": 100,
                "outputTokens": 20
            }
        }"#;

        // Dominant copy: higher token counts, with factoryCredits.
        let dominant = r#"{
            "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
            "model": "claude-opus-4",
            "tokenUsage": {
                "inputTokens": 800,
                "outputTokens": 150,
                "factoryCredits": 750
            }
        }"#;

        // Both files share the same session ID "sessX".
        write_file(&dir_a, "sessX.settings.json", stale);
        write_file(&dir_b, "sessX.settings.json", dominant);

        let db_tmp = TempDir::new().unwrap();
        let db = open_test_db(&db_tmp).await;

        let result = ingest_dir(&db, root.path()).await;

        assert_eq!(
            result.coverage.sessions, 1,
            "dedupe must yield 1 unique session"
        );
        assert_eq!(
            result.coverage.state,
            crate::accounting::parser::CoverageState::Complete,
            "deduped session has credits → coverage must be Complete, not Partial"
        );
        assert_eq!(result.rows_changed, 1, "only one row inserted");

        let summaries = db.cost_by_agent_since(0).await;
        let droid = summaries.iter().find(|s| s.agent == "droid").unwrap();
        assert_eq!(droid.input_tokens, 800, "dominant snapshot selected");
        assert_eq!(
            droid.credits,
            Some(750),
            "credits from dominant copy stored"
        );
    }

    /// A walk error (unreadable subdirectory) causes `ingest_dir` to report Partial coverage.
    #[cfg(unix)]
    #[tokio::test]
    async fn ingest_dir_walk_error_gives_partial_coverage() {
        use std::os::unix::fs::PermissionsExt;

        let root = TempDir::new().unwrap();
        let proj = root.path().join("proj");
        fs::create_dir_all(&proj).unwrap();
        write_file(
            &proj,
            "sess.settings.json",
            r#"{
                "providerLockTimestamp": "2026-04-14T10:00:00.000Z",
                "tokenUsage": {"inputTokens": 100, "outputTokens": 10, "factoryCredits": 100}
            }"#,
        );

        // Locked subdirectory that WalkDir cannot enter.
        let locked = root.path().join("locked");
        fs::create_dir_all(&locked).unwrap();
        write_file(
            &locked,
            "hidden.settings.json",
            r#"{"providerLockTimestamp": "2026-04-14T10:00:00.000Z"}"#,
        );
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let db_tmp = TempDir::new().unwrap();
        let db = open_test_db(&db_tmp).await;

        let result = ingest_dir(&db, root.path()).await;

        // Restore permissions before assertions so cleanup can proceed.
        let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o755));

        assert_eq!(
            result.coverage.state,
            crate::accounting::parser::CoverageState::Partial,
            "walk error must downgrade coverage to Partial"
        );
    }
}
