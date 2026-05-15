//! Compiler / test workflow handlers: `diagnose`, `run_affected_tests`.
//!
//! Bridges raw toolchain output (`cargo check`, `cargo clippy`, `cargo test`)
//! to the code graph, so an agent receives diagnostics and test results
//! already attached to the symbols they affect.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;

use crate::diagnose::{parse_cargo_output, Severity};
use crate::errors::{Result, TokenSaveError};
use crate::tokensave::{is_test_file, TokenSave};
use crate::types::NodeKind;

use super::super::ToolResult;
use super::{truncate_response, unique_file_paths};

/// Maximum tests we'll allow `cargo test` to receive in one call. A loose
/// cap — libtest filters are passed as positional args so very long lists
/// can blow past OS argv limits on some platforms.
const MAX_TESTS_HARD_CAP: usize = 500;

/// Handles `tokensave_diagnose`.
pub(super) async fn handle_diagnose(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let cargo_output =
        args.get("cargo_output")
            .and_then(|v| v.as_str())
            .ok_or(TokenSaveError::Config {
                message: "missing required parameter: cargo_output".to_string(),
            })?;

    let severity_filter = args
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let include_callers = args
        .get("include_callers")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let max_diagnostics = args
        .get("max_diagnostics")
        .and_then(serde_json::Value::as_u64)
        .map_or(50_usize, |v| v.min(500) as usize);

    let mut diagnostics: Vec<_> = parse_cargo_output(cargo_output)
        .into_iter()
        .filter(|d| match severity_filter {
            "error" => d.severity == Severity::Error,
            "warning" => d.severity == Severity::Warning,
            _ => true,
        })
        .collect();
    let total = diagnostics.len();
    diagnostics.truncate(max_diagnostics);

    let mut items: Vec<Value> = Vec::with_capacity(diagnostics.len());
    let mut touched: HashSet<String> = HashSet::new();

    for d in &diagnostics {
        touched.insert(d.file.clone());

        let node = cg.node_at_location(&d.file, d.line).await?;
        let callers_json = if include_callers {
            match &node {
                Some(n) => {
                    let callers = cg.get_callers(&n.id, 1).await.unwrap_or_default();
                    let trimmed: Vec<Value> = callers
                        .into_iter()
                        .take(5)
                        .map(|(caller, _)| {
                            touched.insert(caller.file_path.clone());
                            json!({
                                "node_id": caller.id,
                                "name": caller.name,
                                "kind": caller.kind.as_str(),
                                "file": caller.file_path,
                                "line": caller.start_line,
                            })
                        })
                        .collect();
                    Value::Array(trimmed)
                }
                None => Value::Array(vec![]),
            }
        } else {
            Value::Null
        };

        items.push(json!({
            "severity": severity_string(d.severity),
            "code": d.code,
            "message": d.message,
            "file": d.file,
            "line": d.line,
            "column": d.column,
            "node": node.as_ref().map(|n| json!({
                "node_id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "qualified_name": n.qualified_name,
                "start_line": n.start_line,
                "end_line": n.end_line,
            })),
            "callers": callers_json,
        }));
    }

    let mapped = items.iter().filter(|i| !i["node"].is_null()).count();
    let body = json!({
        "diagnostics_parsed": total,
        "diagnostics_returned": items.len(),
        "mapped_to_node": mapped,
        "unmapped": items.len() - mapped,
        "truncated": total > items.len(),
        "diagnostics": items,
    });
    let formatted = serde_json::to_string_pretty(&body).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched.into_iter().collect(),
    })
}

fn severity_string(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
        Severity::Help => "help",
    }
}

/// Handles `tokensave_run_affected_tests`.
pub(super) async fn handle_run_affected_tests(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let explicit_paths: Option<Vec<String>> = args.get("changed_paths").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
    });
    let profile = args
        .get("profile")
        .and_then(|v| v.as_str())
        .unwrap_or("debug")
        .to_string();
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(300);
    let max_tests = args
        .get("max_tests")
        .and_then(serde_json::Value::as_u64)
        .map_or(100_usize, |v| (v as usize).min(MAX_TESTS_HARD_CAP));

    let project_root = cg.project_root().to_path_buf();

    // 1) Resolve changed paths — explicit list, or fall back to `git diff`.
    let changed_paths = match explicit_paths {
        Some(p) => p,
        None => git_changed_paths(&project_root).await,
    };
    if changed_paths.is_empty() {
        return Ok(empty_result("no changed files detected"));
    }

    // 2) Find tests that cover those paths via the graph: for every source
    // node in a changed file, walk callers and keep the ones that look like
    // tests.
    let mut test_targets: HashMap<String, Vec<String>> = HashMap::new();
    let mut covered_sources: HashSet<String> = HashSet::new();
    for path in &changed_paths {
        let nodes = cg.get_nodes_by_file(path).await.unwrap_or_default();
        for node in nodes {
            if !matches!(node.kind, NodeKind::Function | NodeKind::Method) {
                continue;
            }
            let callers = cg.get_callers(&node.id, 3).await.unwrap_or_default();
            let caller_ids: Vec<String> = callers.iter().map(|(n, _)| n.id.clone()).collect();
            let test_annotated = cg
                .get_test_annotated_node_ids(&caller_ids)
                .await
                .unwrap_or_default();
            for (caller, _) in callers {
                if !is_test_file(&caller.file_path) && !test_annotated.contains(&caller.id) {
                    continue;
                }
                if !matches!(caller.kind, NodeKind::Function | NodeKind::Method) {
                    continue;
                }
                test_targets
                    .entry(caller.name.clone())
                    .or_default()
                    .push(node.id.clone());
                covered_sources.insert(node.id.clone());
            }
        }
    }

    if test_targets.is_empty() {
        return Ok(empty_result(&format!(
            "no tests cover the changed paths ({} file(s))",
            changed_paths.len()
        )));
    }

    let mut test_names: Vec<String> = test_targets.keys().cloned().collect();
    test_names.sort();
    let total_tests = test_names.len();
    test_names.truncate(max_tests);

    // 3) Run cargo test --no-fail-fast with each test name as a libtest
    // filter. We use `--` to pass them through.
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&project_root)
        .arg("test")
        .arg("--no-fail-fast");
    if profile == "release" {
        cmd.arg("--release");
    }
    cmd.arg("--");
    for name in &test_names {
        cmd.arg(name);
    }
    cmd.kill_on_drop(true);

    let run = cmd.output();
    let output = match timeout(Duration::from_secs(timeout_secs), run).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return Ok(error_result(&format!("failed to spawn cargo test: {e}")));
        }
        Err(_) => {
            return Ok(error_result(&format!(
                "cargo test timed out after {timeout_secs}s"
            )));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let results = parse_libtest_output(&stdout);

    let passed = results.iter().filter(|(_, ok)| *ok).count();
    let failed = results.iter().filter(|(_, ok)| !*ok).count();
    let touched_files: Vec<String> = unique_file_paths(changed_paths.iter().map(String::as_str));

    let body = json!({
        "exit_code": output.status.code(),
        "passed": passed,
        "failed": failed,
        "total_observed": results.len(),
        "dispatched_tests": test_names,
        "truncated": total_tests > test_names.len(),
        "results": results
            .iter()
            .map(|(name, ok)| {
                // libtest emits `module::path::test_fn` while the graph keys
                // by the short fn name. Look up by both so we resolve either
                // shape.
                let short = name.rsplit("::").next().unwrap_or(name);
                let covers = test_targets
                    .get(name)
                    .or_else(|| test_targets.get(short))
                    .cloned()
                    .unwrap_or_default();
                json!({
                    "test": name,
                    "passed": ok,
                    "covers_source_ids": covers,
                })
            })
            .collect::<Vec<_>>(),
        "stderr_tail": tail(&stderr, 2000),
        "stdout_tail": tail(&stdout, 2000),
    });

    let formatted = serde_json::to_string_pretty(&body).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Wraps a short status message in a normal `ToolResult`.
fn empty_result(message: &str) -> ToolResult {
    ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                "passed": 0, "failed": 0, "results": [], "note": message
            })).unwrap_or_default() }]
        }),
        touched_files: vec![],
    }
}

fn error_result(message: &str) -> ToolResult {
    ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                "passed": 0, "failed": 0, "results": [], "error": message
            })).unwrap_or_default() }]
        }),
        touched_files: vec![],
    }
}

/// Returns the last `n` characters of `s`, trimmed to a char boundary.
fn tail(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut start = s.len() - n;
    while !s.is_char_boundary(start) && start < s.len() {
        start += 1;
    }
    s[start..].to_string()
}

/// Returns files changed in the working tree relative to HEAD (`git diff
/// --name-only HEAD`). Empty on git failure — `cargo test` against zero
/// affected tests is reported as a no-op above.
async fn git_changed_paths(project_root: &std::path::Path) -> Vec<String> {
    let Ok(output) = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(project_root)
        .output()
        .await
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Parses libtest stdout for `test <name> ... ok` / `... FAILED` lines.
/// Returns `(test_name, passed)` pairs. Robust to colour codes by trimming
/// the common ANSI reset prefix.
fn parse_libtest_output(stdout: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for raw in stdout.lines() {
        let line = raw.trim_start_matches("\u{1b}[0m").trim();
        let Some(rest) = line.strip_prefix("test ") else {
            continue;
        };
        // Skip the "running N tests" / summary lines, which start with "test " followed
        // by something other than a test name (e.g. "test result:").
        if rest.starts_with("result:") {
            continue;
        }
        let Some((name, status)) = rest.rsplit_once(" ... ") else {
            continue;
        };
        let status = status.trim();
        let passed = match status {
            "ok" => true,
            "FAILED" | "failed" => false,
            // Skip "ignored", "bench", incomplete lines, etc.
            _ => continue,
        };
        out.push((name.trim().to_string(), passed));
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_libtest_pass_and_fail() {
        let stdout = "\
running 3 tests
test foo ... ok
test bar ... FAILED
test baz ... ignored
test result: FAILED. 1 passed; 1 failed; 1 ignored
";
        let results = parse_libtest_output(stdout);
        assert_eq!(results, vec![("foo".into(), true), ("bar".into(), false)]);
    }

    #[test]
    fn tail_handles_short_input() {
        assert_eq!(tail("hello", 100), "hello");
        assert_eq!(tail("0123456789", 4), "6789");
    }
}
