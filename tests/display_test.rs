use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokensave::display::{
    format_bytes, format_number, format_relative_time, format_token_count, print_status_header,
    print_status_table,
};
use tokensave::types::GraphStats;

// ── format_token_count ──────────────────────────────────────────────────────

#[test]
fn test_format_token_count_zero() {
    assert_eq!(format_token_count(0), "0");
}

#[test]
fn test_format_token_count_small() {
    assert_eq!(format_token_count(42), "42");
    assert_eq!(format_token_count(999), "999");
}

#[test]
fn test_format_token_count_thousands() {
    assert_eq!(format_token_count(1_000), "1.0k");
    assert_eq!(format_token_count(1_500), "1.5k");
    assert_eq!(format_token_count(45_300), "45.3k");
    assert_eq!(format_token_count(999_999), "1000.0k");
}

#[test]
fn test_format_token_count_millions() {
    assert_eq!(format_token_count(1_000_000), "1.0M");
    assert_eq!(format_token_count(1_200_000), "1.2M");
    assert_eq!(format_token_count(123_456_789), "123.5M");
}

// ── format_bytes ────────────────────────────────────────────────────────────

#[test]
fn test_format_bytes_small() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1023), "1023 B");
}

#[test]
fn test_format_bytes_kilobytes() {
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1_536), "1.5 KB");
    assert_eq!(format_bytes(1_048_575), "1024.0 KB");
}

#[test]
fn test_format_bytes_megabytes() {
    assert_eq!(format_bytes(1_048_576), "1.0 MB");
    assert_eq!(format_bytes(838_860_800), "800.0 MB");
}

#[test]
fn test_format_bytes_gigabytes() {
    assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    assert_eq!(format_bytes(2_684_354_560), "2.5 GB");
}

// ── format_number ───────────────────────────────────────────────────────────

#[test]
fn test_format_number_no_commas() {
    assert_eq!(format_number(0), "0");
    assert_eq!(format_number(1), "1");
    assert_eq!(format_number(999), "999");
}

#[test]
fn test_format_number_with_commas() {
    assert_eq!(format_number(1_000), "1,000");
    assert_eq!(format_number(12_345), "12,345");
    assert_eq!(format_number(243_302), "243,302");
    assert_eq!(format_number(1_000_000), "1,000,000");
    assert_eq!(format_number(1_234_567_890), "1,234,567,890");
}

// ── format_relative_time ────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn test_format_relative_time_never() {
    assert_eq!(format_relative_time(0), "never");
}

#[test]
fn test_format_relative_time_seconds_ago() {
    let ts = now_secs() - 30;
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("s ago"),
        "expected '...s ago', got '{result}'"
    );
}

#[test]
fn test_format_relative_time_minutes_ago() {
    let ts = now_secs() - 300; // 5 minutes
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("m ago"),
        "expected '...m ago', got '{result}'"
    );
}

#[test]
fn test_format_relative_time_hours_ago() {
    let ts = now_secs() - 7200; // 2 hours
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("h ago"),
        "expected '...h ago', got '{result}'"
    );
}

#[test]
fn test_format_relative_time_days_ago() {
    let ts = now_secs() - 172_800; // 2 days
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("d ago"),
        "expected '...d ago', got '{result}'"
    );
}

#[test]
fn test_format_relative_time_boundary_59s() {
    let ts = now_secs() - 59;
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("s ago"),
        "59s should still be seconds, got '{result}'"
    );
}

#[test]
fn test_format_relative_time_boundary_60s() {
    // 60 seconds = 1 minute
    let ts = now_secs() - 60;
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("m ago"),
        "60s should be minutes, got '{result}'"
    );
}

#[test]
fn test_format_relative_time_boundary_3599s() {
    let ts = now_secs() - 3599;
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("m ago"),
        "3599s should be minutes, got '{result}'"
    );
}

#[test]
fn test_format_relative_time_boundary_3600s() {
    let ts = now_secs() - 3600;
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("h ago"),
        "3600s should be hours, got '{result}'"
    );
}

#[test]
fn test_format_relative_time_boundary_86399s() {
    let ts = now_secs() - 86399;
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("h ago"),
        "86399s should be hours, got '{result}'"
    );
}

#[test]
fn test_format_relative_time_boundary_86400s() {
    let ts = now_secs() - 86400;
    let result = format_relative_time(ts);
    assert!(
        result.ends_with("d ago"),
        "86400s should be days, got '{result}'"
    );
}

#[test]
fn test_format_relative_time_future_timestamp() {
    // Timestamp in the future — saturating_sub should yield 0 delta → "0s ago"
    let ts = now_secs() + 1000;
    let result = format_relative_time(ts);
    assert_eq!(result, "0s ago");
}

// ── helpers for status table tests ──────────────────────────────────────────

fn sample_stats() -> GraphStats {
    let mut nodes_by_kind = HashMap::new();
    nodes_by_kind.insert("function".to_string(), 100);
    nodes_by_kind.insert("struct".to_string(), 20);
    nodes_by_kind.insert("method".to_string(), 50);

    let mut files_by_language = HashMap::new();
    files_by_language.insert("Rust".to_string(), 30);
    files_by_language.insert("Go".to_string(), 10);

    GraphStats {
        node_count: 170,
        edge_count: 300,
        file_count: 40,
        db_size_bytes: 1_048_576,
        nodes_by_kind,
        files_by_language,
        last_sync_at: 1000,
        last_full_sync_at: 500,
        edges_by_kind: HashMap::new(),
        last_updated: 1000,
        total_source_bytes: 2_000_000,
    }
}

fn empty_stats() -> GraphStats {
    GraphStats {
        node_count: 0,
        edge_count: 0,
        file_count: 0,
        db_size_bytes: 0,
        nodes_by_kind: HashMap::new(),
        files_by_language: HashMap::new(),
        last_sync_at: 0,
        last_full_sync_at: 0,
        edges_by_kind: HashMap::new(),
        last_updated: 0,
        total_source_bytes: 0,
    }
}

fn many_kinds_stats() -> GraphStats {
    let mut nodes_by_kind = HashMap::new();
    for kind in &[
        "function",
        "struct",
        "method",
        "enum",
        "trait",
        "impl",
        "const",
        "static",
        "type_alias",
        "field",
        "macro",
        "use",
        "class",
        "interface",
        "constructor",
        "module",
    ] {
        nodes_by_kind.insert(kind.to_string(), 10);
    }

    let mut files_by_language = HashMap::new();
    files_by_language.insert("Rust".to_string(), 50);
    files_by_language.insert("Go".to_string(), 30);
    files_by_language.insert("Java".to_string(), 20);
    files_by_language.insert("Python".to_string(), 15);
    files_by_language.insert("TypeScript".to_string(), 10);

    GraphStats {
        node_count: 160,
        edge_count: 500,
        file_count: 125,
        db_size_bytes: 5_242_880,
        nodes_by_kind,
        files_by_language,
        last_sync_at: now_secs() - 120,
        last_full_sync_at: now_secs() - 86400,
        edges_by_kind: HashMap::new(),
        last_updated: now_secs(),
        total_source_bytes: 10_000_000,
    }
}

// ── print_status_table ──────────────────────────────────────────────────────

#[test]
fn test_print_status_table_no_flags_no_worldwide() {
    let stats = sample_stats();
    // Should not panic
    print_status_table(&stats, 50_000, None, None, &[]);
}

#[test]
fn test_print_status_table_with_flags() {
    let stats = sample_stats();
    let flags = vec![
        "\u{1f1fa}\u{1f1f8}".to_string(),
        "\u{1f1ec}\u{1f1e7}".to_string(),
    ];
    print_status_table(&stats, 50_000, None, None, &flags);
}

#[test]
fn test_print_status_table_with_worldwide() {
    let stats = sample_stats();
    print_status_table(&stats, 50_000, None, Some(10_000_000), &[]);
}

#[test]
fn test_print_status_table_with_global_tokens() {
    let stats = sample_stats();
    print_status_table(&stats, 50_000, Some(200_000), None, &[]);
}

#[test]
fn test_print_status_table_with_all_options() {
    let stats = sample_stats();
    let flags = vec![
        "\u{1f1fa}\u{1f1f8}".to_string(),
        "\u{1f1e9}\u{1f1ea}".to_string(),
        "\u{1f1ef}\u{1f1f5}".to_string(),
    ];
    print_status_table(&stats, 100_000, Some(500_000), Some(50_000_000), &flags);
}

#[test]
fn test_print_status_table_empty_stats() {
    let stats = empty_stats();
    // Empty stats with file_count=0 and node_count=0 should satisfy debug_assert
    print_status_table(&stats, 0, None, None, &[]);
}

#[test]
fn test_print_status_table_many_node_kinds() {
    let stats = many_kinds_stats();
    // 16 node kinds should exercise column wrapping
    print_status_table(&stats, 1_000_000, Some(5_000_000), Some(100_000_000), &[]);
}

#[test]
fn test_print_status_table_zero_tokens() {
    let stats = sample_stats();
    print_status_table(&stats, 0, None, None, &[]);
}

#[test]
fn test_print_status_table_large_token_values() {
    let stats = sample_stats();
    print_status_table(
        &stats,
        999_999_999,
        Some(1_000_000_000),
        Some(50_000_000_000),
        &[],
    );
}

#[test]
fn test_print_status_table_no_source_bytes() {
    let mut stats = sample_stats();
    stats.total_source_bytes = 0;
    print_status_table(&stats, 10_000, None, None, &[]);
}

#[test]
fn test_print_status_table_no_languages() {
    let mut stats = sample_stats();
    stats.files_by_language.clear();
    print_status_table(&stats, 10_000, None, None, &[]);
}

#[test]
fn test_print_status_table_many_flags() {
    let stats = sample_stats();
    // 30 flags — exceeds MAX_DISPLAY_FLAGS (25), should trigger truncation with "..."
    let flags: Vec<String> = (0..30)
        .map(|_| "\u{1f1fa}\u{1f1f8}".to_string())
        .collect();
    print_status_table(&stats, 50_000, None, None, &flags);
}

#[test]
fn test_print_status_table_single_node_kind() {
    let mut stats = empty_stats();
    stats.node_count = 5;
    stats.file_count = 5;
    stats.nodes_by_kind.insert("function".to_string(), 5);
    print_status_table(&stats, 100, None, None, &[]);
}

#[test]
fn test_print_status_table_recent_sync_times() {
    let mut stats = sample_stats();
    stats.last_sync_at = now_secs() - 5;
    stats.last_full_sync_at = now_secs() - 3600;
    print_status_table(&stats, 10_000, None, None, &[]);
}

// ── print_status_header ─────────────────────────────────────────────────────

#[test]
fn test_print_status_header_no_flags_no_worldwide() {
    let stats = sample_stats();
    print_status_header(&stats, 50_000, None, None, &[]);
}

#[test]
fn test_print_status_header_with_flags() {
    let stats = sample_stats();
    let flags = vec![
        "\u{1f1fa}\u{1f1f8}".to_string(),
        "\u{1f1ec}\u{1f1e7}".to_string(),
    ];
    print_status_header(&stats, 50_000, None, None, &flags);
}

#[test]
fn test_print_status_header_with_worldwide() {
    let stats = sample_stats();
    print_status_header(&stats, 50_000, None, Some(10_000_000), &[]);
}

#[test]
fn test_print_status_header_with_global_tokens() {
    let stats = sample_stats();
    print_status_header(&stats, 50_000, Some(200_000), None, &[]);
}

#[test]
fn test_print_status_header_with_all_options() {
    let stats = sample_stats();
    let flags = vec![
        "\u{1f1fa}\u{1f1f8}".to_string(),
        "\u{1f1e9}\u{1f1ea}".to_string(),
        "\u{1f1ef}\u{1f1f5}".to_string(),
    ];
    print_status_header(&stats, 100_000, Some(500_000), Some(50_000_000), &flags);
}

#[test]
fn test_print_status_header_empty_stats() {
    let stats = empty_stats();
    print_status_header(&stats, 0, None, None, &[]);
}

#[test]
fn test_print_status_header_many_node_kinds() {
    let stats = many_kinds_stats();
    print_status_header(&stats, 1_000_000, Some(5_000_000), Some(100_000_000), &[]);
}

#[test]
fn test_print_status_header_many_flags() {
    let stats = sample_stats();
    let flags: Vec<String> = (0..30)
        .map(|_| "\u{1f1fa}\u{1f1f8}".to_string())
        .collect();
    print_status_header(&stats, 50_000, None, None, &flags);
}

#[test]
fn test_print_status_header_never_synced() {
    let mut stats = sample_stats();
    stats.last_sync_at = 0;
    stats.last_full_sync_at = 0;
    print_status_header(&stats, 10_000, None, None, &[]);
}
