// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};

use libsql::params;

use super::connection::Database;
use crate::errors::{Result, TokenSaveError};
use crate::types::*;

mod clear;
mod edges;
mod files;
mod fingerprints;
mod metadata;
mod nodes;
mod search;
mod stats;
mod unresolved;

// ---------------------------------------------------------------------------
// Helper: build SQL placeholder string `?, ?, ?, …` in one allocation.
// ---------------------------------------------------------------------------

/// Returns a SQL placeholder string of `n` anonymous `?` markers separated by
/// `, `. Used to construct `IN ($qmarks)` clauses without allocating one
/// `String` per id (`format!("?{i}")` previously did that).
pub(crate) fn build_qmark_placeholders(n: usize) -> String {
    debug_assert!(n > 0, "build_qmark_placeholders called with n == 0");
    // Each "?, " occupies 3 bytes; the last one drops the trailing ", ".
    let mut s = String::with_capacity(n * 3);
    for i in 0..n {
        if i > 0 {
            s.push_str(", ");
        }
        s.push('?');
    }
    s
}

// ---------------------------------------------------------------------------
// Helper: map a libsql row to domain types (by column index)
// ---------------------------------------------------------------------------

/// Maps a row from the `nodes` table to a `Node`.
///
/// Expected column order: id(0), kind(1), name(2), `qualified_name(3)`,
/// `file_path(4)`, `start_line(5)`, `end_line(6)`, `start_column(7)`, `end_column(8)`,
/// docstring(9), signature(10), visibility(11), `is_async(12)`,
/// branches(13), loops(14), returns(15), `max_nesting(16)`,
/// `unsafe_blocks(17)`, `unchecked_calls(18)`, assertions(19), `updated_at(20)`,
/// `attrs_start_line(21)`, `parent_id(22)`, and the issue #150 health columns
/// `cognitive_complexity(23)`, `distinct_operators(24)`, `distinct_operands(25)`,
/// `total_operators(26)`, `total_operands(27)`.
///
/// The health columns are read tolerantly (`unwrap_or(0)`): older SELECT lists
/// in this file that don't request them simply yield 0, the same way
/// `parent_id` tolerates absence.
pub(crate) fn row_to_node(row: &libsql::Row) -> std::result::Result<Node, libsql::Error> {
    let kind_str = get_string_lossy(row, 1)?;
    let vis_str = get_string_lossy(row, 11)?;
    let is_async_int = row.get::<i64>(12)?;
    let start_line = row.get::<u32>(5)?;
    // Pre-v7 rows may have attrs_start_line == 0 (default); fall back to start_line.
    let attrs_raw = row.get::<u32>(21).unwrap_or(0);
    let attrs_start_line = if attrs_raw == 0 {
        start_line
    } else {
        attrs_raw
    };
    // `parent_id` is column 22 in v9+ SELECT lists. Older SELECTs in this
    // file don't request it; the .ok().flatten() chain swallows the missing-
    // column error and yields None.
    let parent_id = get_opt_string_lossy(row, 22).ok().flatten();

    Ok(Node {
        id: get_string_lossy(row, 0)?,
        kind: NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Function),
        name: get_string_lossy(row, 2)?,
        qualified_name: get_string_lossy(row, 3)?,
        file_path: get_string_lossy(row, 4)?,
        start_line,
        attrs_start_line,
        end_line: row.get::<u32>(6)?,
        start_column: row.get::<u32>(7)?,
        end_column: row.get::<u32>(8)?,
        signature: get_opt_string_lossy(row, 10)?,
        docstring: get_opt_string_lossy(row, 9)?,
        visibility: Visibility::from_str(&vis_str).unwrap_or_default(),
        is_async: is_async_int != 0,
        branches: row.get::<u32>(13)?,
        loops: row.get::<u32>(14)?,
        returns: row.get::<u32>(15)?,
        max_nesting: row.get::<u32>(16)?,
        unsafe_blocks: row.get::<u32>(17)?,
        unchecked_calls: row.get::<u32>(18)?,
        assertions: row.get::<u32>(19)?,
        // Issue #150 health columns (23..=27). Tolerant of absence in older
        // SELECT lists, mirroring the parent_id pattern above.
        cognitive_complexity: row.get::<u32>(23).unwrap_or(0),
        distinct_operators: row.get::<u32>(24).unwrap_or(0),
        distinct_operands: row.get::<u32>(25).unwrap_or(0),
        total_operators: row.get::<u32>(26).unwrap_or(0),
        total_operands: row.get::<u32>(27).unwrap_or(0),
        updated_at: row.get::<u64>(20)?,
        parent_id,
    })
}

/// Reads a text column as String, replacing invalid UTF-8 bytes with U+FFFD.
/// This prevents crashes when source files with non-UTF-8 encoding (e.g. Latin-1)
/// have their signatures or docstrings stored in the database.
///
/// libsql's `get::<String>()` panics on Blob values via `unreachable!()`, so we
/// must read as `Value` first and convert.
pub(crate) fn get_string_lossy(
    row: &libsql::Row,
    idx: i32,
) -> std::result::Result<String, libsql::Error> {
    let val = row.get::<libsql::Value>(idx)?;
    match val {
        libsql::Value::Text(s) => Ok(s),
        libsql::Value::Blob(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        libsql::Value::Null => Ok(String::new()),
        libsql::Value::Integer(i) => Ok(i.to_string()),
        libsql::Value::Real(f) => Ok(f.to_string()),
    }
}

/// Like `get_string_lossy` but for nullable columns.
pub(crate) fn get_opt_string_lossy(
    row: &libsql::Row,
    idx: i32,
) -> std::result::Result<Option<String>, libsql::Error> {
    let val = row.get::<libsql::Value>(idx)?;
    match val {
        libsql::Value::Null => Ok(None),
        libsql::Value::Text(s) => Ok(Some(s)),
        libsql::Value::Blob(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        libsql::Value::Integer(i) => Ok(Some(i.to_string())),
        libsql::Value::Real(f) => Ok(Some(f.to_string())),
    }
}

/// Maps a row from the `edges` table to an `Edge`.
///
/// Expected column order: source(0), target(1), kind(2), line(3).
pub(crate) fn row_to_edge(row: &libsql::Row) -> std::result::Result<Edge, libsql::Error> {
    let kind_str = row.get::<String>(2)?;
    let line = row.get::<Option<u32>>(3)?;

    Ok(Edge {
        source: row.get::<String>(0)?,
        target: row.get::<String>(1)?,
        kind: EdgeKind::from_str(&kind_str).unwrap_or(EdgeKind::Uses),
        line,
    })
}

/// Maps a row from the `files` table to a `FileRecord`.
///
/// Expected column order: path(0), `content_hash(1)`, size(2), `modified_at(3)`,
/// `indexed_at(4)`, `node_count(5)`.
pub(crate) fn row_to_file(row: &libsql::Row) -> std::result::Result<FileRecord, libsql::Error> {
    Ok(FileRecord {
        path: row.get::<String>(0)?,
        content_hash: row.get::<String>(1)?,
        size: row.get::<u64>(2)?,
        modified_at: row.get::<i64>(3)?,
        indexed_at: row.get::<i64>(4)?,
        node_count: row.get::<u32>(5)?,
    })
}

/// Maps a row from the `unresolved_refs` table to an `UnresolvedRef`.
///
/// Expected column order: `from_node_id(0)`, `reference_name(1)`,
/// `reference_kind(2)`, line(3), col(4), `file_path(5)`.
pub(crate) fn row_to_unresolved_ref(
    row: &libsql::Row,
) -> std::result::Result<UnresolvedRef, libsql::Error> {
    let kind_str = row.get::<String>(2)?;

    Ok(UnresolvedRef {
        from_node_id: row.get::<String>(0)?,
        reference_name: row.get::<String>(1)?,
        reference_kind: EdgeKind::from_str(&kind_str).unwrap_or(EdgeKind::Uses),
        line: row.get::<u32>(3)?,
        column: row.get::<u32>(4)?,
        file_path: row.get::<String>(5)?,
    })
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Converts `Option<String>` to a `libsql::Value` for use in params.
pub(crate) fn opt_str(opt: Option<&str>) -> libsql::Value {
    match opt {
        Some(s) => libsql::Value::Text(s.to_string()),
        None => libsql::Value::Null,
    }
}

/// Appends a SQL-safe single-quoted string to `buf`, escaping `'` as `''`.
pub(crate) fn push_quoted(buf: &mut String, s: &str) {
    buf.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            buf.push_str("''");
        } else {
            buf.push(ch);
        }
    }
    buf.push('\'');
}

/// Appends a SQL-safe quoted string or NULL for Option<String>.
pub(crate) fn push_opt_quoted(buf: &mut String, opt: Option<&str>) {
    match opt {
        Some(s) => push_quoted(buf, s),
        None => buf.push_str("NULL"),
    }
}

/// Appends an integer literal to the buffer.
pub(crate) fn push_int(buf: &mut String, val: i64) {
    use std::fmt::Write;
    let _ = write!(buf, "{val}");
}

/// Collects all rows from a `Rows` iterator into a `Vec<T>` using the given
/// row-mapping function.
pub(crate) async fn collect_rows<T>(
    rows: &mut libsql::Rows,
    map_fn: fn(&libsql::Row) -> std::result::Result<T, libsql::Error>,
    operation: &str,
) -> Result<Vec<T>> {
    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read row: {e}"),
        operation: operation.to_string(),
    })? {
        items.push(map_fn(&row).map_err(|e| TokenSaveError::Database {
            message: format!("failed to map row: {e}"),
            operation: operation.to_string(),
        })?);
    }
    Ok(items)
}

/// Maps a file path to a human-readable language label used in
/// `GraphStats::files_by_language`. Anything we don't recognise lands in
/// `"Other"`. The label set must stay in sync with the language extractors
/// registered in `crate::extraction::LanguageRegistry`; the test
/// `files_by_language_covers_known_extensions` guards the mapping.
pub(crate) fn display_language_for_path(path: &str) -> &'static str {
    // Special-case extensionless files we still recognise by name.
    let basename = path.rsplit('/').next().unwrap_or(path);
    let lower = basename.to_ascii_lowercase();
    if lower == "dockerfile" || lower.starts_with("dockerfile.") {
        return "Dockerfile";
    }
    if lower == "makefile" {
        return "Makefile";
    }
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "Rust",
        "go" => "Go",
        "py" | "pyi" | "pyx" => "Python",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "scala" | "sc" => "Scala",
        "swift" => "Swift",
        "as" => "ActionScript",
        "c" | "h" => "C",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "C++",
        "cs" => "C#",
        "fs" | "fsi" | "fsx" => "F#",
        "fst" | "fsti" => "F*",
        "rb" => "Ruby",
        "php" => "PHP",
        "dart" => "Dart",
        "lua" => "Lua",
        "pl" | "pm" => "Perl",
        "sh" | "bash" => "Bash",
        "ps1" | "psm1" => "PowerShell",
        "nix" => "Nix",
        "zig" => "Zig",
        "proto" => "Protobuf",
        "toml" => "TOML",
        "sql" => "SQL",
        "r" => "R",
        "jl" => "Julia",
        "ex" | "exs" => "Elixir",
        "erl" | "hrl" => "Erlang",
        "hs" => "Haskell",
        "clj" | "cljs" | "cljc" | "edn" => "Clojure",
        "ml" | "mli" => "OCaml",
        "lean" => "Lean",
        "m" | "mm" => "Objective-C",
        "f" | "f90" | "f95" | "f03" | "f08" | "for" => "Fortran",
        "cbl" | "cob" | "cpy" => "COBOL",
        "pas" | "pp" | "dpr" => "Pascal",
        "vb" => "VB.NET",
        "bas" => "BASIC",
        "bat" | "cmd" => "Batch",
        "glsl" | "vert" | "frag" | "comp" | "geom" | "tesc" | "tese" => "GLSL",
        "qnt" => "Quint",
        _ => "Other",
    }
}

/// Executes a `SELECT label, COUNT(*) ... GROUP BY` query and returns
/// the results as a `HashMap<String, u64>`.
pub(crate) async fn query_kind_counts(
    conn: &libsql::Connection,
    sql: &str,
) -> Result<HashMap<String, u64>> {
    let mut map = HashMap::new();
    let mut rows = conn
        .query(sql, ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to query kind counts: {e}"),
            operation: "get_stats".to_string(),
        })?;
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read kind count row: {e}"),
        operation: "get_stats".to_string(),
    })? {
        let kind: String = row.get(0).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read kind: {e}"),
            operation: "get_stats".to_string(),
        })?;
        let count: i64 = row.get(1).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read count: {e}"),
            operation: "get_stats".to_string(),
        })?;
        if count > 0 {
            map.insert(kind, count as u64);
        }
    }
    Ok(map)
}

/// Executes a scalar query returning a single `i64` value.
pub(crate) async fn query_scalar_i64(
    conn: &libsql::Connection,
    sql: &str,
    operation: &str,
) -> Result<i64> {
    let mut rows = conn
        .query(sql, ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to execute scalar query: {e}"),
            operation: operation.to_string(),
        })?;

    let row = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read scalar row: {e}"),
            operation: operation.to_string(),
        })?
        .ok_or_else(|| TokenSaveError::Database {
            message: "no result from scalar query".to_string(),
            operation: operation.to_string(),
        })?;

    row.get::<i64>(0).map_err(|e| TokenSaveError::Database {
        message: format!("failed to read scalar value: {e}"),
        operation: operation.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::display_language_for_path;

    #[test]
    fn maps_common_extensions_to_named_languages() {
        assert_eq!(display_language_for_path("src/main.rs"), "Rust");
        assert_eq!(display_language_for_path("a/b/foo.py"), "Python");
        assert_eq!(display_language_for_path("foo.pyi"), "Python");
        assert_eq!(display_language_for_path("foo.tsx"), "TypeScript");
        assert_eq!(display_language_for_path("foo.cs"), "C#");
        assert_eq!(display_language_for_path("foo.fst"), "F*");
        assert_eq!(display_language_for_path("foo.fsti"), "F*");
        assert_eq!(display_language_for_path("foo.cpp"), "C++");
        assert_eq!(
            display_language_for_path("com/example/Game.as"),
            "ActionScript"
        );
        assert_eq!(display_language_for_path("Dockerfile"), "Dockerfile");
        assert_eq!(
            display_language_for_path("docker/Dockerfile.prod"),
            "Dockerfile"
        );
        assert_eq!(display_language_for_path("Makefile"), "Makefile");
        assert_eq!(display_language_for_path("readme.txt"), "Other");
        assert_eq!(display_language_for_path("noext"), "Other");
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        assert_eq!(display_language_for_path("Foo.RS"), "Rust");
        assert_eq!(display_language_for_path("Foo.PY"), "Python");
    }
}
