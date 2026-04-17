//! Status table rendering for the CLI.
//!
//! All the formatting and layout logic for `tokensave status` output,
//! extracted from main.rs to keep the CLI entry point focused on dispatch.

use crate::types::GraphStats;

/// Formats a token count as a compact string (e.g. "1.2M", "45.3k").
pub fn format_token_count(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Formats a UNIX timestamp as a human-readable relative time (e.g. "2m ago", "3d ago").
/// Returns "never" when the timestamp is 0.
pub fn format_relative_time(timestamp: u64) -> String {
    if timestamp == 0 {
        return "never".to_string();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let delta = now.saturating_sub(timestamp);
    if delta < 60 {
        format!("{delta}s ago")
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86400)
    }
}

/// Formats a byte count into a human-readable string (e.g. "798.0 MB").
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Formats a number with comma separators (e.g. 243302 -> "243,302").
pub fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

/// Formats a single table cell with left-aligned label and right-aligned value.
fn format_cell(label: &str, value: &str, width: usize) -> String {
    let content_len = label.len() + value.len();
    let pad = width.saturating_sub(2 + content_len);
    format!(" {}{}{} ", label, " ".repeat(pad), value)
}

/// Builds a horizontal separator line (e.g. ├──┬──┤).
fn table_separator(
    left: char,
    mid: char,
    right: char,
    cell_width: usize,
    num_cols: usize,
) -> String {
    let mut line = String::from(left);
    for i in 0..num_cols {
        line.push_str(&"─".repeat(cell_width));
        line.push(if i < num_cols - 1 { mid } else { right });
    }
    line
}

/// Data for the cost row in the status header.
pub struct CostRow {
    pub today_cost: f64,
    pub week_cost: f64,
    pub efficiency_pct: f64,
}

/// Prints only the header section of the status table (version, tokens, sync times).
/// Optional branch info for the status display.
pub struct BranchInfo {
    pub branch: String,
    pub parent: Option<String>,
    pub is_fallback: bool,
}

pub fn print_status_header(
    stats: &GraphStats,
    tokens_saved: u64,
    global_tokens_saved: Option<u64>,
    worldwide: Option<u64>,
    country_flags: &[String],
    branch_info: Option<&BranchInfo>,
    cost_info: Option<&CostRow>,
) {
    let num_cols = 3;
    let mut sorted_kinds: Vec<_> = stats.nodes_by_kind.iter().collect();
    sorted_kinds.sort_by_key(|(k, _)| (*k).clone());
    let cell_width = compute_cell_width(&sorted_kinds);
    let inner_width = cell_width * num_cols + (num_cols - 1);

    println!("{}", table_separator('╭', '─', '╮', cell_width, num_cols));
    print_version_flags_row(country_flags, inner_width);
    print_tokens_row(tokens_saved, global_tokens_saved, worldwide, inner_width);
    if let Some(ci) = cost_info {
        print_cost_row(ci, inner_width);
    }
    print_sync_row(stats.last_sync_at, stats.last_full_sync_at, inner_width);
    if let Some(bi) = branch_info {
        print_branch_row(bi, inner_width);
    }
    println!("{}", table_separator('╰', '─', '╯', cell_width, num_cols));
}

/// Prints the status output as a compact bordered table.
#[allow(clippy::too_many_arguments)]
pub fn print_status_table(
    stats: &GraphStats,
    tokens_saved: u64,
    global_tokens_saved: Option<u64>,
    worldwide: Option<u64>,
    country_flags: &[String],
    branch_info: Option<&BranchInfo>,
    cost_info: Option<&CostRow>,
    details: bool,
) {
    let num_cols = 3;
    debug_assert!(
        stats.file_count > 0 || stats.node_count == 0,
        "print_status_table: node_count should be 0 when file_count is 0"
    );
    debug_assert!(
        stats.node_count >= stats.file_count || stats.file_count == 0,
        "print_status_table: node_count should be >= file_count"
    );

    let mut sorted_kinds: Vec<_> = stats.nodes_by_kind.iter().collect();
    sorted_kinds.sort_by_key(|(k, _)| (*k).clone());
    let num_kind_rows = sorted_kinds.len().div_ceil(num_cols);

    let cell_width = compute_cell_width(&sorted_kinds);
    let inner_width = cell_width * num_cols + (num_cols - 1);

    println!("{}", table_separator('╭', '─', '╮', cell_width, num_cols));
    print_version_flags_row(country_flags, inner_width);
    print_tokens_row(tokens_saved, global_tokens_saved, worldwide, inner_width);
    if let Some(ci) = cost_info {
        print_cost_row(ci, inner_width);
    }
    print_sync_row(stats.last_sync_at, stats.last_full_sync_at, inner_width);
    if let Some(bi) = branch_info {
        print_branch_row(bi, inner_width);
    }
    println!("{}", table_separator('├', '┬', '┤', cell_width, num_cols));

    let stats_rows = build_stats_rows(stats, num_cols);
    print_table_rows(&stats_rows, cell_width, num_cols);

    if details && !sorted_kinds.is_empty() {
        println!("{}", table_separator('├', '┼', '┤', cell_width, num_cols));
        print_kind_rows(&sorted_kinds, num_kind_rows, num_cols, cell_width);
    }

    println!("{}", table_separator('╰', '┴', '╯', cell_width, num_cols));
}

/// Maximum cell width — caps total table width at 100 columns.
const MAX_CELL_WIDTH: usize = 32;

/// Maximum number of country flags to display in the title row.
/// Derived from MAX_CELL_WIDTH: available = 3*32 = 96, title ~16, gap 2 → 78 cols for flags.
/// Each flag = 3 cols (2 emoji + 1 space), first = 2 → fits 26; use 25 for margin.
const MAX_DISPLAY_FLAGS: usize = 25;

/// Compute cell width from the widest node-kind entry, capped at MAX_CELL_WIDTH.
fn compute_cell_width(sorted_kinds: &[(&String, &u64)]) -> usize {
    let max_kind_len = sorted_kinds
        .iter()
        .map(|(k, _)| k.len())
        .max()
        .unwrap_or(10);
    let max_count_len = sorted_kinds
        .iter()
        .map(|(_, c)| format_number(**c).len())
        .max()
        .unwrap_or(5);
    (max_kind_len + max_count_len + 3).clamp(22, MAX_CELL_WIDTH)
}

/// Print the top title row: version (left) + country flags (right).
fn print_version_flags_row(country_flags: &[String], inner_width: usize) {
    let version = env!("CARGO_PKG_VERSION");
    let daemon_running = crate::daemon::running_daemon_pid().is_some();
    let title = if daemon_running {
        format!("😈 TokenSave v{version}")
    } else {
        format!("   TokenSave v{version}")
    };
    // "😈 " is 3 display columns (2-wide emoji + space) but 6 bytes;
    // "   " is 3 display columns and 3 bytes.
    // Display width = byte len minus the overhead of the emoji bytes.
    let title_display_width = if daemon_running {
        title.len() - 2
    } else {
        title.len()
    };
    let available = inner_width.saturating_sub(2);

    if country_flags.is_empty() {
        let pad = available.saturating_sub(title_display_width);
        println!("│ {}{} │", title, " ".repeat(pad));
        return;
    }

    // Build flags string, capped at MAX_DISPLAY_FLAGS and fitting within available width
    let capped = &country_flags[..country_flags.len().min(MAX_DISPLAY_FLAGS)];
    let has_overflow = country_flags.len() > MAX_DISPLAY_FLAGS;
    let mut flags_str = String::new();
    let mut display_width = 0;
    let flag_width = 2; // emoji flags are 2 columns wide
                        // Reserve space for title + at least 2 spaces gap
    let max_flags_width = available.saturating_sub(title_display_width + 2);
    for (i, flag) in capped.iter().enumerate() {
        let needed = if i == 0 { flag_width } else { 1 + flag_width };
        let more_coming = has_overflow || i + 1 < capped.len();
        let reserve = if more_coming { 2 } else { 0 };
        if display_width + needed + reserve > max_flags_width {
            flags_str.push_str(" …");
            display_width += 2;
            break;
        }
        if i > 0 {
            flags_str.push(' ');
            display_width += 1;
        }
        flags_str.push_str(flag);
        display_width += flag_width;
        if i + 1 == capped.len() && has_overflow {
            flags_str.push_str(" …");
            display_width += 2;
        }
    }

    let pad = available.saturating_sub(title_display_width + display_width);
    println!("│ {}{}{} │", title, " ".repeat(pad), flags_str);
}

/// Print the second title row: token counts right-aligned in green.
fn print_tokens_row(
    tokens_saved: u64,
    global_tokens_saved: Option<u64>,
    worldwide: Option<u64>,
    inner_width: usize,
) {
    let tokens_text = {
        let mut parts = Vec::new();
        match global_tokens_saved {
            Some(global) => {
                parts.push(format!("Project ~{}", format_token_count(tokens_saved)));
                parts.push(format!(
                    "All projects ~{}",
                    format_token_count(tokens_saved + global)
                ));
            }
            None => {
                parts.push(format!("Saved ~{}", format_token_count(tokens_saved)));
            }
        }
        if let Some(ww) = worldwide {
            parts.push(format!("Worldwide ~{}", format_token_count(ww)));
        }
        parts.join("  ")
    };
    let available = inner_width.saturating_sub(2);
    let pad = available.saturating_sub(tokens_text.len());
    println!("│ {}\x1b[32m{}\x1b[0m │", " ".repeat(pad), tokens_text);
}

/// Print the third title row: last sync and full sync timestamps, right-aligned in dim.
fn print_sync_row(last_sync_at: u64, last_full_sync_at: u64, inner_width: usize) {
    let sync_text = format!(
        "Last sync {}  Full sync {}",
        format_relative_time(last_sync_at),
        format_relative_time(last_full_sync_at),
    );
    let available = inner_width.saturating_sub(2);
    let pad = available.saturating_sub(sync_text.len());
    println!("│ {}\x1b[2m{}\x1b[0m │", " ".repeat(pad), sync_text,);
}

fn print_branch_row(info: &BranchInfo, inner_width: usize) {
    let mut text = format!("Branch: {}", info.branch);
    if let Some(ref parent) = info.parent {
        text.push_str(&format!("  (from {parent})"));
    }
    if info.is_fallback {
        text.push_str("  \x1b[33m[fallback]\x1b[0m");
    }
    let available = inner_width.saturating_sub(2);
    // Strip ANSI for length calculation
    let visible_len = text.replace("\x1b[33m", "").replace("\x1b[0m", "").len();
    let pad = available.saturating_sub(visible_len);
    println!("│ {}{} │", " ".repeat(pad), text);
}

/// Print the cost summary row: today's cost, 7-day cost, efficiency ratio.
fn print_cost_row(cost_info: &CostRow, inner_width: usize) {
    let mut parts = Vec::new();
    if cost_info.today_cost >= 0.001 {
        parts.push(format!("Today ${:.2}", cost_info.today_cost));
    }
    if cost_info.week_cost >= 0.001 {
        parts.push(format!("7d ${:.2}", cost_info.week_cost));
    }
    if cost_info.efficiency_pct > 0.0 {
        parts.push(format!("Efficiency {:.0}%", cost_info.efficiency_pct));
    }
    if parts.is_empty() {
        return;
    }
    let text = parts.join("  ");
    let available = inner_width.saturating_sub(2);
    let pad = available.saturating_sub(text.len());
    println!("│ {}\x1b[36m{}\x1b[0m │", " ".repeat(pad), text,);
}

/// Build the stats rows (files/nodes/edges, DB size, languages).
fn build_stats_rows(stats: &GraphStats, num_cols: usize) -> Vec<Vec<(&str, String)>> {
    let mut sorted_langs: Vec<_> = stats.files_by_language.iter().collect();
    sorted_langs.sort_by(|a, b| b.1.cmp(a.1));

    let mut rows: Vec<Vec<(&str, String)>> = vec![vec![
        ("Files", format_number(stats.file_count)),
        ("Nodes", format_number(stats.node_count)),
        ("Edges", format_number(stats.edge_count)),
    ]];

    let mut second_row: Vec<(&str, String)> = vec![("DB Size", format_bytes(stats.db_size_bytes))];
    if stats.total_source_bytes > 0 {
        second_row.push(("Source", format_bytes(stats.total_source_bytes)));
    }
    let mut lang_idx = 0;
    while second_row.len() < num_cols && lang_idx < sorted_langs.len() {
        let (lang, count) = sorted_langs[lang_idx];
        second_row.push((lang.as_str(), format_number(*count)));
        lang_idx += 1;
    }
    while second_row.len() < num_cols {
        second_row.push(("", String::new()));
    }
    rows.push(second_row);

    while lang_idx < sorted_langs.len() {
        let mut row: Vec<(&str, String)> = Vec::new();
        for _ in 0..num_cols {
            if lang_idx < sorted_langs.len() {
                let (lang, count) = sorted_langs[lang_idx];
                row.push((lang.as_str(), format_number(*count)));
                lang_idx += 1;
            } else {
                row.push(("", String::new()));
            }
        }
        rows.push(row);
    }
    rows
}

/// Print rows of label-value pairs in a bordered table.
fn print_table_rows(rows: &[Vec<(&str, String)>], cell_width: usize, num_cols: usize) {
    for row in rows {
        print!("│");
        for (i, (label, value)) in row.iter().enumerate() {
            if label.is_empty() {
                print!("{}", " ".repeat(cell_width));
            } else {
                print!("{}", format_cell(label, value, cell_width));
            }
            print!("{}", if i < num_cols - 1 { "│" } else { "│\n" });
        }
    }
}

/// Print node kinds in column-major order.
fn print_kind_rows(
    sorted_kinds: &[(&String, &u64)],
    num_kind_rows: usize,
    num_cols: usize,
    cell_width: usize,
) {
    for r in 0..num_kind_rows {
        print!("│");
        for c in 0..num_cols {
            let idx = r + c * num_kind_rows;
            if idx < sorted_kinds.len() {
                let (kind, count) = &sorted_kinds[idx];
                print!("{}", format_cell(kind, &format_number(**count), cell_width));
            } else {
                print!("{}", " ".repeat(cell_width));
            }
            print!("{}", if c < num_cols - 1 { "│" } else { "│\n" });
        }
    }
}
