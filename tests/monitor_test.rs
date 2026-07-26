use std::path::Path;
use tempfile::TempDir;
use tokensave::accounting::CoverageState;
use tokensave::monitor::{cost_panel_lines, CostCache};

/// Helper: write an entry to a specific mmap dir.
fn write(dir: &Path, project: &Path, prefix: &str, tool: &str, delta: u64, before: u64) {
    tokensave::monitor::write_entry_to(dir, project, prefix, tool, delta, before);
}

/// Helper: open reader at a specific mmap dir.
fn reader(dir: &Path) -> tokensave::monitor::MmapReader {
    tokensave::monitor::MmapReader::open_at(dir).unwrap()
}

#[test]
fn test_write_and_read_entry() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(
        dir.path(),
        project.path(),
        "tokensave",
        "tokensave_context",
        63_102,
        290_000,
    );

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 1);

    let entry = r.entry(0).unwrap();
    assert_eq!(entry.prefix, "tokensave");
    assert_eq!(entry.tool_name, "tokensave_context");
    assert_eq!(entry.delta, 63_102);
    assert_eq!(entry.before, 290_000);
    assert!(entry.timestamp > 0);
    assert!(!entry.project.is_empty());
}

#[test]
fn test_ring_buffer_wraps() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    for i in 0..260u64 {
        write(
            dir.path(),
            project.path(),
            "tokensave",
            "tokensave_search",
            i + 1,
            i * 10,
        );
    }

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 260);

    // Slot 0 should now have entry 256 (index 256, delta=257)
    let entry = r.entry(0).unwrap();
    assert_eq!(entry.delta, 257);

    // Slot 3 should have entry 259 (index 259, delta=260)
    let entry = r.entry(3).unwrap();
    assert_eq!(entry.delta, 260);
}

#[test]
fn test_write_entry_accumulates() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(
        dir.path(),
        project.path(),
        "tokensave",
        "tokensave_context",
        100,
        500,
    );
    write(
        dir.path(),
        project.path(),
        "tokensave",
        "tokensave_search",
        50,
        200,
    );

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 2);

    let e0 = r.entry(0).unwrap();
    let e1 = r.entry(1).unwrap();
    assert_eq!(e0.delta + e1.delta, 150);
}

#[test]
fn test_entry_label_format() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(
        dir.path(),
        project.path(),
        "tokensave",
        "tokensave_context",
        42,
        100,
    );

    let r = reader(dir.path());
    let entry = r.entry(0).unwrap();
    let label = entry.label();
    assert!(label.starts_with("tokensave - "), "got: {label}");
    assert!(label.ends_with(" - tokensave_context"), "got: {label}");
}

#[test]
fn test_tool_name_truncation() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    let long_name = "a".repeat(100);
    write(dir.path(), project.path(), "tokensave", &long_name, 42, 100);

    let r = reader(dir.path());
    let entry = r.entry(0).unwrap();
    // Should be truncated to 31 chars (32 bytes with null)
    assert_eq!(entry.tool_name.len(), 31);
}

#[test]
fn test_multiple_projects() {
    let dir = TempDir::new().unwrap();
    let project_a = TempDir::with_prefix("alpha").unwrap();
    let project_b = TempDir::with_prefix("bravo").unwrap();

    write(
        dir.path(),
        project_a.path(),
        "tokensave",
        "tokensave_context",
        100,
        500,
    );
    write(
        dir.path(),
        project_b.path(),
        "tokensave",
        "tokensave_search",
        200,
        600,
    );

    let r = reader(dir.path());
    assert_eq!(r.write_idx(), 2);

    let e0 = r.entry(0).unwrap();
    let e1 = r.entry(1).unwrap();
    assert_ne!(e0.project, e1.project);
}

#[test]
fn test_different_prefixes() {
    let dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    write(
        dir.path(),
        project.path(),
        "tokensave",
        "tokensave_context",
        100,
        500,
    );
    write(
        dir.path(),
        project.path(),
        "othertool",
        "do_stuff",
        200,
        600,
    );

    let r = reader(dir.path());
    let e0 = r.entry(0).unwrap();
    let e1 = r.entry(1).unwrap();
    assert_eq!(e0.prefix, "tokensave");
    assert_eq!(e1.prefix, "othertool");
}

// ── cost_panel_lines tests ────────────────────────────────────────────

fn base_cache() -> CostCache {
    CostCache {
        today_cost: 0.0,
        week_cost: 0.0,
        tokens_saved: 0,
        efficiency_pct: 0.0,
        top_model: String::new(),
        top_model_cost: 0.0,
        today_credits: None,
        week_credits: None,
        week_droid_tokens: 0,
        droid_state: CoverageState::Absent,
        last_refresh: std::time::Instant::now(),
    }
}

/// Droid credits and raw tokens render as lowercase compact notation (2.9k, 24.6k).
#[test]
fn droid_credits_and_tokens_formatted_compact_lowercase() {
    let cache = CostCache {
        today_credits: Some(2_900),
        week_credits: Some(2_900),
        week_droid_tokens: 24_600,
        droid_state: CoverageState::Complete,
        ..base_cache()
    };
    let lines = cost_panel_lines(&cache);
    let droid_line = &lines[1];
    assert!(
        droid_line.contains("2.9k"),
        "expected 2.9k in droid line, got: {droid_line}"
    );
    assert!(
        droid_line.contains("24.6k"),
        "expected 24.6k in droid line, got: {droid_line}"
    );
}

/// Droid line must never contain a dollar sign.
#[test]
fn droid_line_contains_no_dollar_sign() {
    let cache = CostCache {
        today_credits: Some(100),
        week_credits: Some(500),
        week_droid_tokens: 10_000,
        droid_state: CoverageState::Complete,
        ..base_cache()
    };
    let lines = cost_panel_lines(&cache);
    let droid_line = &lines[1];
    assert!(
        !droid_line.contains('$'),
        "droid line must not contain '$': {droid_line}"
    );
}

/// Panel has_activity returns true when only credits are present (no USD).
#[test]
fn has_activity_true_for_droid_only_credits() {
    let cache = CostCache {
        today_credits: None,
        week_credits: Some(500),
        week_droid_tokens: 0,
        droid_state: CoverageState::Complete,
        ..base_cache()
    };
    assert!(
        cache.has_activity(),
        "expected has_activity=true when week_credits > 0"
    );
}

/// Panel has_activity returns true when only raw droid tokens are present.
#[test]
fn has_activity_true_for_droid_only_tokens() {
    let cache = CostCache {
        week_droid_tokens: 1000,
        droid_state: CoverageState::Partial,
        ..base_cache()
    };
    assert!(
        cache.has_activity(),
        "expected has_activity=true when week_droid_tokens > 0"
    );
}

/// Panel has_activity returns false when everything is zero.
#[test]
fn has_activity_false_when_all_zero() {
    let cache = base_cache();
    assert!(
        !cache.has_activity(),
        "expected has_activity=false for empty cache"
    );
}

/// Partial coverage renders 'credits n/a' when week_credits is None.
#[test]
fn droid_partial_coverage_renders_credits_na() {
    let cache = CostCache {
        week_droid_tokens: 5_000,
        droid_state: CoverageState::Partial,
        ..base_cache() // week_credits: None
    };
    let lines = cost_panel_lines(&cache);
    let droid_line = &lines[1];
    assert!(
        droid_line.contains("credits n/a"),
        "partial line should contain 'credits n/a': {droid_line}"
    );
    assert!(
        droid_line.contains("partial"),
        "partial line should contain 'partial': {droid_line}"
    );
    assert!(
        !droid_line.contains('$'),
        "partial droid line must not contain '$': {droid_line}"
    );
}

/// Regression: global Partial state + valid window credits renders credits (not n/a)
/// and includes "partial source" in the parenthetical.
#[test]
fn partial_global_with_valid_window_credits_renders_credits_and_partial_source() {
    let cache = CostCache {
        today_credits: Some(300),
        week_credits: Some(1_500),
        week_droid_tokens: 8_000,
        droid_state: CoverageState::Partial,
        ..base_cache()
    };
    let lines = cost_panel_lines(&cache);
    let droid_line = &lines[1];
    assert!(
        !droid_line.contains("credits n/a"),
        "should render actual credits when week_credits is Some: {droid_line}"
    );
    assert!(
        droid_line.contains("1.5k"),
        "should show week credit count: {droid_line}"
    );
    assert!(
        droid_line.contains("partial source"),
        "should label partial source in parenthetical: {droid_line}"
    );
    assert!(
        !droid_line.contains('$'),
        "droid line must not contain '$': {droid_line}"
    );
}

/// Regression: global Partial state + missing window credits (None) remains 'credits n/a'.
#[test]
fn partial_global_with_missing_window_credits_renders_na() {
    let cache = CostCache {
        week_droid_tokens: 3_000,
        droid_state: CoverageState::Partial,
        ..base_cache() // week_credits: None
    };
    let lines = cost_panel_lines(&cache);
    let droid_line = &lines[1];
    assert!(
        droid_line.contains("credits n/a"),
        "should render 'credits n/a' when week_credits is None: {droid_line}"
    );
}

/// Absent coverage renders the Claude-local fallback message.
#[test]
fn droid_absent_renders_claude_local_message() {
    let cache = base_cache();
    let lines = cost_panel_lines(&cache);
    let droid_line = &lines[1];
    assert!(
        droid_line.contains("Droid absent"),
        "absent line should contain 'Droid absent': {droid_line}"
    );
}

/// Top-priced model shows 'n/a' when no model is set.
#[test]
fn efficiency_line_shows_na_when_no_model() {
    let cache = base_cache();
    let lines = cost_panel_lines(&cache);
    let eff_line = &lines[2];
    assert!(
        eff_line.contains("n/a"),
        "efficiency line should show n/a when no top model: {eff_line}"
    );
}

/// Narrow-width truncation keeps lines within the character limit.
#[test]
fn narrow_width_truncation_respects_limit() {
    use tokensave::monitor::truncate_to_chars;
    let long_line = "  USD spent: $1.23 today | $5.67 7d    Saved: 100.0k";
    let truncated = truncate_to_chars(long_line, 20);
    assert_eq!(
        truncated.chars().count(),
        20,
        "truncated to 20 chars: {truncated:?}"
    );
}

/// Truncation returns original when within limit.
#[test]
fn truncation_noop_when_within_limit() {
    use tokensave::monitor::truncate_to_chars;
    let line = "short";
    let truncated = truncate_to_chars(line, 80);
    assert_eq!(truncated, line);
}
