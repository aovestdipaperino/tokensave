//! JSONL session parser for Claude Code transcripts.
//!
//! Reads `~/.claude/projects/**/*.jsonl`, extracts assistant turns with
//! model/usage/tool data, and inserts them into the `turns` table via
//! `GlobalDb`. Uses offset tracking for incremental re-parsing.

use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::accounting::classifier;
use crate::accounting::pricing;
use crate::global_db::GlobalDb;

/// A single parsed turn ready for DB insertion.
pub struct CostTurn {
    pub message_id: String,
    pub project_hash: String,
    pub session_id: String,
    pub model: String,
    pub timestamp: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
    pub category: String,
    pub tool_names: String,
}

/// Find all JSONL session files under `~/.claude/projects/`.
fn find_session_files() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let projects_dir = home.join(".claude").join("projects");
    if !projects_dir.is_dir() {
        return Vec::new();
    }

    let mut files = Vec::new();
    collect_jsonl_files(&projects_dir, &mut files, 0);
    files
}

/// Recursively collect .jsonl files, with a depth limit to avoid runaway traversal.
fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out, depth + 1);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

/// Extract project hash and session ID from a JSONL file path.
/// Path pattern: `~/.claude/projects/<project-hash>/<session-id>.jsonl`
/// or `~/.claude/projects/<project-hash>/<session-id>/subagents/<agent>.jsonl`
fn extract_path_parts(path: &Path) -> (String, String) {
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Find "projects" in the path and take the next component as project_hash
    let projects_idx = components.iter().position(|c| *c == "projects");
    let project_hash = projects_idx
        .and_then(|i| components.get(i + 1))
        .unwrap_or(&"unknown")
        .to_string();

    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    (project_hash, session_id)
}

/// Parse a single JSONL line into a `CostTurn`, if it's an assistant message
/// with usage data.
fn parse_line(line: &str, project_hash: &str, session_id: &str) -> Option<CostTurn> {
    let v: Value = serde_json::from_str(line).ok()?;

    // Only process assistant messages
    if v.get("type")?.as_str()? != "assistant" {
        return None;
    }

    let msg = v.get("message")?;
    let message_id = msg.get("id")?.as_str()?;
    let model = msg.get("model")?.as_str()?;

    let usage = msg.get("usage")?;
    let input_tokens = usage.get("input_tokens")?.as_u64().unwrap_or(0);
    let output_tokens = usage.get("output_tokens")?.as_u64().unwrap_or(0);
    let cache_write_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // Parse timestamp from the outer object (ISO 8601)
    let timestamp = parse_timestamp(v.get("timestamp")?.as_str()?)?;

    // Extract tool names and bash commands for classification
    let content = msg.get("content").and_then(|c| c.as_array());
    let mut tool_names_vec: Vec<String> = Vec::new();
    let mut bash_commands: Vec<String> = Vec::new();

    if let Some(blocks) = content {
        for block in blocks {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                    tool_names_vec.push(name.to_string());
                    if name == "Bash" {
                        if let Some(cmd) = block
                            .get("input")
                            .and_then(|i| i.get("command"))
                            .and_then(|c| c.as_str())
                        {
                            bash_commands.push(cmd.to_string());
                        }
                    }
                }
            }
        }
    }

    // Classify
    let tool_refs: Vec<&str> = tool_names_vec.iter().map(|s| s.as_str()).collect();
    let bash_refs: Vec<&str> = bash_commands.iter().map(|s| s.as_str()).collect();
    let category = classifier::classify(&tool_refs, &bash_refs);

    // Compute cost
    let cost_usd = pricing::cost_of_turn(
        model,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
    );

    Some(CostTurn {
        message_id: message_id.to_string(),
        project_hash: project_hash.to_string(),
        session_id: session_id.to_string(),
        model: model.to_string(),
        timestamp,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
        cost_usd,
        category: category.as_str().to_string(),
        tool_names: tool_names_vec.join(","),
    })
}

/// Parse an ISO 8601 timestamp to unix epoch seconds.
fn parse_timestamp(ts: &str) -> Option<u64> {
    // Handle "2026-04-14T10:32:15.039Z" format
    // Simple parsing without pulling in chrono: split on known positions
    if ts.len() < 19 {
        return None;
    }
    let year: i64 = ts.get(0..4)?.parse().ok()?;
    let month: u64 = ts.get(5..7)?.parse().ok()?;
    let day: u64 = ts.get(8..10)?.parse().ok()?;
    let hour: u64 = ts.get(11..13)?.parse().ok()?;
    let min: u64 = ts.get(14..16)?.parse().ok()?;
    let sec: u64 = ts.get(17..19)?.parse().ok()?;

    // Days from epoch using a simple formula (good enough for 2000-2100)
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let month_days = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        days += i64::from(month_days[m as usize] as u8);
    }
    if month > 2 && is_leap(year) {
        days += 1;
    }
    days += (day as i64) - 1;

    Some((days as u64) * 86400 + hour * 3600 + min * 60 + sec)
}

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// Stats returned by the `ingest` function.
pub struct IngestStats {
    /// Number of new turns inserted.
    pub turns_inserted: u64,
    /// Total cost of the newly-inserted turns.
    pub cost_usd: f64,
    /// Total input + output tokens of the newly-inserted turns.
    pub tokens_consumed: u64,
}

/// Ingest all Claude Code session files into the global DB.
/// Uses offset tracking to only parse new lines since the last run.
pub async fn ingest(gdb: &GlobalDb) -> IngestStats {
    let files = find_session_files();
    let mut total_inserted = 0u64;
    let mut total_cost = 0.0f64;
    let mut total_tokens = 0u64;

    for file_path in &files {
        let path_str = file_path.to_string_lossy().to_string();

        // Check file mtime
        let meta = match fs::metadata(file_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check if we've already parsed this file up to this mtime
        let (prev_offset, prev_mtime) = gdb
            .get_parse_offset(&path_str)
            .await
            .unwrap_or((0, 0));

        if mtime == prev_mtime && prev_offset > 0 {
            // File hasn't changed since last parse
            continue;
        }

        let seek_to = if mtime == prev_mtime {
            prev_offset
        } else if prev_mtime > 0 && mtime > prev_mtime {
            // File was appended to -- seek to previous offset
            prev_offset
        } else {
            // File is new or was rewritten -- start from beginning
            0
        };

        let (project_hash, session_id) = extract_path_parts(file_path);

        let f = match fs::File::open(file_path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut reader = BufReader::new(f);

        // Seek to the saved offset
        if seek_to > 0 && reader.seek(SeekFrom::Start(seek_to)).is_err() {
            continue;
        }

        let mut line = String::new();
        let mut current_offset = seek_to;

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(n) => {
                    current_offset += n as u64;
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Some(turn) = parse_line(trimmed, &project_hash, &session_id) {
                        let turn_cost = turn.cost_usd;
                        let turn_tokens = turn.input_tokens + turn.output_tokens;
                        if gdb.insert_turn(&turn).await {
                            total_inserted += 1;
                            total_cost += turn_cost;
                            total_tokens += turn_tokens;
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // Save the new offset
        gdb.set_parse_offset(&path_str, current_offset, mtime).await;
    }

    IngestStats {
        turns_inserted: total_inserted,
        cost_usd: total_cost,
        tokens_consumed: total_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        // 2026-01-01T00:00:00Z
        let ts = parse_timestamp("2026-01-01T00:00:00.000Z");
        assert!(ts.is_some());
        let epoch = ts.unwrap();
        // 2026-01-01 = 56 years from 1970, roughly 20454 days
        assert!(epoch > 1_700_000_000);
        assert!(epoch < 1_800_000_000);
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert!(parse_timestamp("bad").is_none());
        assert!(parse_timestamp("").is_none());
    }

    #[test]
    fn test_extract_path_parts() {
        let path = PathBuf::from(
            "/Users/test/.claude/projects/-Users-test-Code/abc123-session.jsonl",
        );
        let (project, session) = extract_path_parts(&path);
        assert_eq!(project, "-Users-test-Code");
        assert_eq!(session, "abc123-session");
    }

    #[test]
    fn test_parse_line_assistant() {
        let line = r#"{"type":"assistant","message":{"id":"msg_01abc","model":"claude-opus-4-6","role":"assistant","usage":{"input_tokens":1000,"output_tokens":200,"cache_creation_input_tokens":500,"cache_read_input_tokens":800},"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"test.rs"}}]},"timestamp":"2026-04-14T10:00:00.000Z"}"#;
        let turn = parse_line(line, "proj", "sess");
        assert!(turn.is_some());
        let t = turn.unwrap();
        assert_eq!(t.message_id, "msg_01abc");
        assert_eq!(t.model, "claude-opus-4-6");
        assert_eq!(t.input_tokens, 1000);
        assert_eq!(t.output_tokens, 200);
        assert_eq!(t.cache_write_tokens, 500);
        assert_eq!(t.cache_read_tokens, 800);
        assert_eq!(t.category, "coding");
        assert_eq!(t.tool_names, "Edit");
        assert!(t.cost_usd > 0.0);
    }

    #[test]
    fn test_parse_line_user_skipped() {
        let line = r#"{"type":"user","message":{"content":"hello"},"timestamp":"2026-04-14T10:00:00.000Z"}"#;
        assert!(parse_line(line, "proj", "sess").is_none());
    }

    #[test]
    fn test_parse_line_malformed() {
        assert!(parse_line("not json at all", "proj", "sess").is_none());
        assert!(parse_line("{}", "proj", "sess").is_none());
    }
}
