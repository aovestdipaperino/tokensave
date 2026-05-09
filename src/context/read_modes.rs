// Rust guideline compliant 2025-10-17
//! Mode-aware file reads for `tokensave_read`.
//!
//! Four modes are implemented in 5.0:
//!
//! - `full` — verbatim file content (parity with the raw `Read` tool)
//! - `lines` — explicit byte-range slice (`A-B`, 1-based, inclusive)
//! - `map` — flat list of every top-level symbol in the file, sourced from
//!   the code graph (cheap; no source bytes touched)
//! - `signatures` — `map` filtered to function/type kinds, with the cached
//!   `signature` column included
//!
//! Each function returns the rendered body as a `String`. Token-counting and
//! cache I/O happen one layer up, in the MCP handler.

use serde_json::{json, Value};

use crate::db::Database;
use crate::errors::{Result, TokenSaveError};
use crate::types::{Node, NodeKind};

/// Mode selector for `tokensave_read`. Parsed from the JSON `mode` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    Full,
    Lines,
    Map,
    Signatures,
}

impl ReadMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Lines => "lines",
            Self::Map => "map",
            Self::Signatures => "signatures",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "full" => Some(Self::Full),
            "lines" => Some(Self::Lines),
            "map" => Some(Self::Map),
            "signatures" => Some(Self::Signatures),
            _ => None,
        }
    }
}

/// Inclusive 1-based byte-line range parsed from `"A-B"` (or just `"A"` for a
/// single line). Out-of-range values are clamped at render time.
#[derive(Debug, Clone, Copy)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some((a, b)) = s.split_once('-') {
            let start: u32 = a.trim().parse().ok()?;
            let end: u32 = b.trim().parse().ok()?;
            if start == 0 || end < start {
                return None;
            }
            Some(Self { start, end })
        } else {
            let line: u32 = s.parse().ok()?;
            if line == 0 {
                return None;
            }
            Some(Self {
                start: line,
                end: line,
            })
        }
    }
}

/// Renders the `full` mode body — entire file content as UTF-8 text.
pub fn render_full(source: &str) -> String {
    source.to_string()
}

/// Approximates the token count of a UTF-8 string. Uses the ~4-chars-per-token
/// rule of thumb that holds for English source code; it is not exact, but
/// good enough for the metric tokensave reports back to the caller.
pub fn estimate_tokens(s: &str) -> u32 {
    let chars = s.chars().count();
    chars.div_ceil(4).min(u32::MAX as usize) as u32
}

/// Renders the `lines` mode body — slices `range.start..=range.end` (1-based,
/// inclusive). Out-of-range lines are silently clamped.
pub fn render_lines(source: &str, range: LineRange) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = (range.start.saturating_sub(1)) as usize;
    let end = (range.end as usize).min(lines.len());
    if start >= lines.len() || start >= end {
        return String::new();
    }
    lines[start..end].join("\n")
}

/// Renders the `map` mode body — JSON list of every top-level symbol in the
/// file, sourced from the graph. No source bytes are touched.
///
/// `kinds` is an optional case-insensitive filter on `NodeKind::as_str()`
/// values (e.g. `["function", "struct"]`). When `None` or empty, every kind
/// is included.
pub async fn render_map(db: &Database, file_path: &str, kinds: Option<&[String]>) -> Result<Value> {
    let nodes = fetch_nodes(db, file_path).await?;
    let active_filter: Option<&[String]> = kinds.filter(|k| !k.is_empty());
    let entries: Vec<Value> = nodes
        .iter()
        .filter(|n| match active_filter {
            None => true,
            Some(filter) => {
                let lhs = n.kind.as_str();
                filter.iter().any(|want| want.eq_ignore_ascii_case(lhs))
            }
        })
        .map(|n| {
            json!({
                "kind": n.kind.as_str(),
                "name": n.name,
                "line": n.start_line,
                "end_line": n.end_line,
                "visibility": n.visibility.as_str(),
            })
        })
        .collect();
    Ok(json!({
        "file": file_path,
        "symbol_count": entries.len(),
        "symbols": entries,
    }))
}

/// Renders the `signatures` mode body — `map` filtered to function/type kinds
/// with the cached `signature` string. Skips items without a signature so the
/// result stays compact.
pub async fn render_signatures(db: &Database, file_path: &str) -> Result<Value> {
    let nodes = fetch_nodes(db, file_path).await?;
    let entries: Vec<Value> = nodes
        .iter()
        .filter(|n| is_signature_kind(&n.kind))
        .filter_map(|n| {
            let sig = n.signature.as_deref()?;
            Some(json!({
                "kind": n.kind.as_str(),
                "name": n.name,
                "qualified_name": n.qualified_name,
                "line": n.start_line,
                "end_line": n.end_line,
                "visibility": n.visibility.as_str(),
                "signature": sig,
                "is_async": n.is_async,
            }))
        })
        .collect();
    Ok(json!({
        "file": file_path,
        "signature_count": entries.len(),
        "signatures": entries,
    }))
}

async fn fetch_nodes(db: &Database, file_path: &str) -> Result<Vec<Node>> {
    db.get_nodes_by_file(file_path)
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("read_modes: failed to load nodes for {file_path}: {e}"),
            operation: "read_modes::fetch_nodes".to_string(),
        })
}

/// Kinds whose `signature` column carries useful information for the
/// `signatures` mode. Excludes plain identifiers, modules, and string-literal
/// nodes whose "signature" would be redundant with the name.
fn is_signature_kind(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function
            | NodeKind::Method
            | NodeKind::Struct
            | NodeKind::Trait
            | NodeKind::Interface
            | NodeKind::Enum
            | NodeKind::Class
            | NodeKind::TypeAlias
            | NodeKind::Const
            | NodeKind::Static
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode_known_values() {
        assert_eq!(ReadMode::parse("full"), Some(ReadMode::Full));
        assert_eq!(ReadMode::parse("lines"), Some(ReadMode::Lines));
        assert_eq!(ReadMode::parse("map"), Some(ReadMode::Map));
        assert_eq!(ReadMode::parse("signatures"), Some(ReadMode::Signatures));
        assert_eq!(ReadMode::parse("nope"), None);
    }

    #[test]
    fn parse_line_range_pair() {
        let r = LineRange::parse("3-5").unwrap();
        assert_eq!(r.start, 3);
        assert_eq!(r.end, 5);
    }

    #[test]
    fn parse_line_range_single() {
        let r = LineRange::parse("7").unwrap();
        assert_eq!(r.start, 7);
        assert_eq!(r.end, 7);
    }

    #[test]
    fn parse_line_range_invalid() {
        assert!(LineRange::parse("0").is_none());
        assert!(LineRange::parse("5-3").is_none());
        assert!(LineRange::parse("a-b").is_none());
    }

    #[test]
    fn render_lines_clamps_out_of_range() {
        let src = "alpha\nbeta\ngamma\n";
        let r = LineRange { start: 2, end: 99 };
        assert_eq!(render_lines(src, r), "beta\ngamma");
    }

    #[test]
    fn render_lines_single_line() {
        let src = "alpha\nbeta\ngamma\n";
        let r = LineRange { start: 2, end: 2 };
        assert_eq!(render_lines(src, r), "beta");
    }

    #[test]
    fn render_lines_empty_when_past_end() {
        let src = "alpha\nbeta\n";
        let r = LineRange { start: 5, end: 8 };
        assert_eq!(render_lines(src, r), "");
    }

    #[test]
    fn render_full_returns_input() {
        let src = "hello\nworld\n";
        assert_eq!(render_full(src), src);
    }
}
