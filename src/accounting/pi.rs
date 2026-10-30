//! Pi session accounting parser.
//!
//! Reads `~/.pi/agent/sessions/**/*.jsonl`, one file per session, and turns
//! each assistant message that reports usage into a row of the `turns` table.
//! Pi already prices every message (`usage.cost.total` is the amount the
//! provider charged), so no embedded pricing table is involved and the stored
//! cost is the billed one rather than an estimate.
//!
//! Incremental like the Claude parser: each file's byte offset and mtime are
//! saved after a clean read, and a resumed run continues where the last one
//! stopped. Pi message ids are only unique within a session, so the stored
//! `message_id` is prefixed with the session id.

use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;
use walkdir::WalkDir;

use crate::accounting::classifier;
use crate::accounting::parser::{parse_timestamp, CoverageState, SourceCoverage};
use crate::global_db::GlobalDb;
use crate::types::CostTurn;

/// What one ingestion run of the Pi sessions directory produced.
pub(crate) struct PiIngestResult {
    pub coverage: SourceCoverage,
    pub turns_inserted: u64,
    pub cost_usd: f64,
    pub tokens_consumed: u64,
}

/// Recursively discover `*.jsonl` session files below `root`, sorted.
///
/// Returns `(files, walk_errors)`; symlinks are not followed.
fn find_session_files(root: &Path) -> (Vec<PathBuf>, bool) {
    if !root.is_dir() {
        return (Vec::new(), false);
    }
    let mut files = Vec::new();
    let mut had_errors = false;
    for entry in WalkDir::new(root).follow_links(false) {
        match entry {
            Err(_) => had_errors = true,
            Ok(entry) => {
                let is_session = entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "jsonl");
                if is_session {
                    files.push(entry.into_path());
                }
            }
        }
    }
    files.sort();
    (files, had_errors)
}

/// Maps a Pi tool name to the Claude Code name the shared classifier and the
/// navigation buckets are written in terms of.
///
/// Only the classification uses this; `turns.tool_names` keeps Pi's own
/// names, because the missed-opportunity analyzer is a Claude-only measurement
/// and would otherwise count Pi's reads as if their result sizes were known.
fn classifier_name(pi_tool: &str) -> Option<&'static str> {
    match pi_tool {
        "edit" => Some("Edit"),
        "write" => Some("Write"),
        "read" | "ls" => Some("Read"),
        "grep" => Some("Grep"),
        "find" => Some("Glob"),
        "bash" => Some("Bash"),
        "subagent" | "task" => Some("Agent"),
        _ => None,
    }
}

/// Parses one Pi session line into a turn, or `None` when the line is not an
/// assistant message that reports usage.
///
/// `session_id` scopes the stored `message_id`: Pi's line ids repeat across
/// sessions.
fn parse_line(line: &str, project_hash: &str, session_id: &str) -> Option<CostTurn> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "message" {
        return None;
    }
    let message = value.get("message")?;
    if message.get("role")?.as_str()? != "assistant" {
        return None;
    }
    let line_id = value.get("id")?.as_str()?;

    let usage = message.get("usage")?;
    let input_tokens = usage.get("input").and_then(Value::as_u64).unwrap_or(0);
    let output_tokens = usage
        .get("output")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(usage.get("reasoning").and_then(Value::as_u64).unwrap_or(0));
    let cache_write_tokens = usage.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
    let cache_read_tokens = usage.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
    // A message with no tokens at all is not a billable turn.
    if input_tokens == 0 && output_tokens == 0 && cache_read_tokens == 0 && cache_write_tokens == 0
    {
        return None;
    }

    let cost_usd = usage
        .pointer("/cost/total")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(parse_timestamp)
        .or_else(|| {
            let millis = message.get("timestamp").and_then(Value::as_u64)?;
            Some(millis / 1000)
        })?;

    let model = message
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    let mut tool_names: Vec<&str> = Vec::new();
    let mut bash_commands: Vec<&str> = Vec::new();
    let blocks = message.get("content").and_then(Value::as_array);
    for block in blocks.into_iter().flatten() {
        if block.get("type").and_then(Value::as_str) != Some("toolCall") {
            continue;
        }
        let Some(name) = block.get("name").and_then(Value::as_str) else {
            continue;
        };
        tool_names.push(name);
        if name == "bash" {
            if let Some(command) = block.pointer("/arguments/command").and_then(Value::as_str) {
                bash_commands.push(command);
            }
        }
    }

    let classified: Vec<&str> = tool_names
        .iter()
        .filter_map(|name| classifier_name(name))
        .collect();
    let category = classifier::classify(&classified, &bash_commands);

    Some(CostTurn {
        message_id: format!("pi:{session_id}:{line_id}"),
        project_hash: project_hash.to_string(),
        session_id: session_id.to_string(),
        model,
        timestamp,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
        cost_usd,
        category: category.as_str().to_string(),
        tool_names: tool_names.join(","),
        agent: "pi".to_string(),
        credits: None,
    })
}

/// The session id of a Pi session file: its file stem, without `.jsonl`.
fn session_id_of(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// The project bucket of a Pi session file: Pi slugs the session's working
/// directory into the directory name.
fn project_hash_of(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Ingests every Pi session file below `root`.
///
/// Coverage is `Absent` when there is nothing to read, `Partial` when a walk,
/// read, or line parse failed, and `Complete` otherwise. Individual malformed
/// lines are skipped without failing the file: Pi appends as it goes, so the
/// last line of a live session is routinely incomplete.
pub(crate) async fn ingest_dir(gdb: &GlobalDb, root: &Path) -> PiIngestResult {
    let (files, mut had_errors) = find_session_files(root);

    let mut turns_inserted = 0u64;
    let mut cost_usd = 0.0f64;
    let mut tokens_consumed = 0u64;

    for file_path in &files {
        let path_str = file_path.to_string_lossy().to_string();
        let Ok(metadata) = fs::metadata(file_path) else {
            had_errors = true;
            continue;
        };
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_secs());
        let (previous_offset, previous_mtime) =
            gdb.get_parse_offset(&path_str).await.unwrap_or((0, 0));
        if mtime == previous_mtime && previous_offset > 0 {
            continue;
        }
        let seek_to = if mtime == previous_mtime || (previous_mtime > 0 && mtime > previous_mtime) {
            previous_offset
        } else {
            0
        };

        let Ok(file) = fs::File::open(file_path) else {
            had_errors = true;
            continue;
        };
        let mut reader = BufReader::new(file);
        if seek_to > 0 && reader.seek(SeekFrom::Start(seek_to)).is_err() {
            had_errors = true;
            continue;
        }

        let project_hash = project_hash_of(file_path);
        let session_id = session_id_of(file_path);
        let mut offset = seek_to;
        let mut line = String::new();
        let mut read_failed = false;

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Err(_) => {
                    had_errors = true;
                    read_failed = true;
                    break;
                }
                Ok(bytes) => {
                    // The offset advances past the bytes just read whether or
                    // not the line parses, so a malformed line is skipped once
                    // instead of stalling every later run on it.
                    offset += bytes as u64;
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let Some(turn) = parse_line(trimmed, &project_hash, &session_id) else {
                        continue;
                    };
                    if gdb.insert_turn(&turn).await {
                        turns_inserted += 1;
                        cost_usd += turn.cost_usd;
                        tokens_consumed += turn.input_tokens + turn.output_tokens;
                    }
                }
            }
        }

        if !read_failed {
            gdb.set_parse_offset(&path_str, offset, mtime).await;
        }
    }

    let state = if had_errors {
        CoverageState::Partial
    } else if files.is_empty() {
        CoverageState::Absent
    } else {
        CoverageState::Complete
    };

    PiIngestResult {
        coverage: SourceCoverage {
            agent: "pi",
            state,
            sessions: files.len() as u64,
        },
        turns_inserted,
        cost_usd,
        tokens_consumed,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn open_test_db(tmp: &TempDir) -> GlobalDb {
        let db_path = tmp.path().join(".tokensave").join("global.db");
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        GlobalDb::open_at(&db_path).await.unwrap()
    }

    const ASSISTANT_LINE: &str = r#"{"type":"message","id":"9bc9f5b6","parentId":null,"timestamp":"2026-10-19T17:39:21.296Z","message":{"role":"assistant","content":[{"type":"toolCall","id":"call_1","name":"read","arguments":{"path":"a.md"}}],"provider":"openai","model":"minimax-m3","usage":{"input":40726,"output":70,"cacheRead":78,"cacheWrite":5,"reasoning":29,"totalTokens":40874,"cost":{"input":0.01,"output":0.0001,"cacheRead":0.000005,"cacheWrite":0,"total":0.0123}},"stopReason":"toolUse","timestamp":1792431561321}}"#;

    fn write_session(root: &Path, bucket: &str, name: &str, lines: &[&str]) -> PathBuf {
        let dir = root.join(bucket);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        path
    }

    #[test]
    fn an_assistant_message_becomes_a_priced_turn() {
        let turn = parse_line(ASSISTANT_LINE, "--proj--", "sess-1").unwrap();
        assert_eq!(turn.message_id, "pi:sess-1:9bc9f5b6");
        assert_eq!(turn.agent, "pi");
        assert_eq!(turn.model, "minimax-m3");
        assert_eq!(turn.input_tokens, 40726);
        // Reasoning tokens are part of what the model produced.
        assert_eq!(turn.output_tokens, 70 + 29);
        assert_eq!(turn.cache_read_tokens, 78);
        assert_eq!(turn.cache_write_tokens, 5);
        // Pi's own billed total, not a table lookup.
        assert!((turn.cost_usd - 0.0123).abs() < f64::EPSILON);
        assert_eq!(turn.category, "exploration");
        assert_eq!(turn.tool_names, "read");
        assert!(turn.timestamp > 1_700_000_000);
    }

    #[test]
    fn non_assistant_and_usageless_lines_are_skipped() {
        assert!(parse_line(r#"{"type":"session","id":"x"}"#, "p", "s").is_none());
        let user = r#"{"type":"message","id":"a","message":{"role":"user","content":"hi"}}"#;
        assert!(parse_line(user, "p", "s").is_none());
        let no_usage = r#"{"type":"message","id":"a","message":{"role":"assistant","content":[]}}"#;
        assert!(parse_line(no_usage, "p", "s").is_none());
        let zero_usage = r#"{"type":"message","id":"a","timestamp":"2026-10-19T17:39:21.296Z","message":{"role":"assistant","model":"m","usage":{"input":0,"output":0},"content":[]}}"#;
        assert!(parse_line(zero_usage, "p", "s").is_none());
    }

    #[test]
    fn bash_commands_feed_the_classifier() {
        let line = r#"{"type":"message","id":"b1","timestamp":"2026-10-19T17:39:21.296Z","message":{"role":"assistant","model":"m","usage":{"input":10,"output":2},"content":[{"type":"toolCall","name":"bash","arguments":{"command":"cargo test --workspace"}}]}}"#;
        let turn = parse_line(line, "p", "s").unwrap();
        assert_eq!(turn.category, "testing");
        assert_eq!(turn.tool_names, "bash");
    }

    #[tokio::test]
    async fn ingest_reads_only_new_lines_on_the_second_run() {
        let home = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let gdb = open_test_db(&db_dir).await;
        let root = home.path().join("sessions");

        let path = write_session(&root, "--proj--", "sess.jsonl", &[ASSISTANT_LINE]);
        let first = ingest_dir(&gdb, &root).await;
        assert_eq!(first.turns_inserted, 1);
        assert_eq!(first.coverage.state, CoverageState::Complete);
        assert_eq!(first.coverage.agent, "pi");

        let second = ingest_dir(&gdb, &root).await;
        assert_eq!(second.turns_inserted, 0, "an unchanged file is not re-read");

        // Appending a turn is picked up from the saved offset. The mtime is
        // pushed forward explicitly: two writes inside one second would be
        // indistinguishable to the mtime comparison the parser shares with the
        // Claude source.
        let mut contents = std::fs::read_to_string(&path).unwrap();
        contents.push_str(&ASSISTANT_LINE.replace("9bc9f5b6", "9bc9f5b7"));
        contents.push('\n');
        std::fs::write(&path, contents).unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(later)
            .unwrap();

        let third = ingest_dir(&gdb, &root).await;
        assert_eq!(third.turns_inserted, 1);
        assert!(third.cost_usd > 0.0);
        assert_eq!(third.tokens_consumed, 40726 + 99);
    }

    #[tokio::test]
    async fn a_malformed_line_is_skipped_without_losing_the_file() {
        let home = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let gdb = open_test_db(&db_dir).await;
        let root = home.path().join("sessions");

        write_session(
            &root,
            "--proj--",
            "sess.jsonl",
            &["{\"type\":\"message\",", ASSISTANT_LINE],
        );
        let stats = ingest_dir(&gdb, &root).await;
        assert_eq!(stats.turns_inserted, 1);
        assert_eq!(stats.coverage.state, CoverageState::Complete);
    }

    #[tokio::test]
    async fn an_absent_directory_reports_absent_coverage() {
        let home = TempDir::new().unwrap();
        let db_dir = TempDir::new().unwrap();
        let gdb = open_test_db(&db_dir).await;

        let stats = ingest_dir(&gdb, &home.path().join("sessions")).await;
        assert_eq!(stats.coverage.state, CoverageState::Absent);
        assert_eq!(stats.coverage.sessions, 0);
        assert_eq!(stats.turns_inserted, 0);
    }
}
