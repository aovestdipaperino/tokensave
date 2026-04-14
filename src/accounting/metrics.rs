//! Aggregation and summary queries for token accounting.

use crate::display::CostRow;
use crate::global_db::GlobalDb;

/// Full cost summary with breakdowns.
pub struct CostSummary {
    pub total_cost: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub by_model: Vec<(String, f64, u64)>,    // (model, cost, total_tokens)
    pub by_category: Vec<(String, f64, u64)>,  // (category, cost, turn_count)
    pub tokens_saved: u64,
    pub efficiency_ratio: f64,
}

/// Quick cost summary for the `tokensave status` header row.
/// Returns `None` if no accounting data exists.
pub async fn quick_cost_summary(
    gdb: &GlobalDb,
    tokens_saved: u64,
    global_tokens_saved: u64,
) -> Option<CostRow> {
    let now = now_epoch();
    let today_start = today_start_epoch(now);
    let week_start = now.saturating_sub(7 * 86400);

    let today_cost = gdb.total_cost_since(today_start).await?;
    let week_cost = gdb.total_cost_since(week_start).await?;
    let week_consumed = gdb.total_tokens_since(week_start).await.unwrap_or(0);

    // Don't show the row if there's no meaningful data
    if today_cost < 0.001 && week_cost < 0.001 {
        return None;
    }

    let total_saved = tokens_saved + global_tokens_saved;
    let efficiency_pct = if total_saved + week_consumed > 0 {
        (total_saved as f64 / (total_saved + week_consumed) as f64) * 100.0
    } else {
        0.0
    };

    Some(CostRow {
        today_cost,
        week_cost,
        efficiency_pct,
    })
}

/// Build a full cost summary for a given time range.
pub async fn cost_summary(
    gdb: &GlobalDb,
    since: u64,
    tokens_saved: u64,
) -> Option<CostSummary> {
    let total_cost = gdb.total_cost_since(since).await?;
    let (total_input, total_output, total_cache_read) =
        gdb.token_breakdown_since(since).await.unwrap_or((0, 0, 0));
    let by_model = gdb.cost_by_model_since(since).await;
    let by_category = gdb.cost_by_category_since(since).await;

    let total_consumed = total_input + total_output;
    let efficiency_ratio = if tokens_saved + total_consumed > 0 {
        tokens_saved as f64 / (tokens_saved + total_consumed) as f64
    } else {
        0.0
    };

    Some(CostSummary {
        total_cost,
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_cache_read_tokens: total_cache_read,
        by_model,
        by_category,
        tokens_saved,
        efficiency_ratio,
    })
}

/// Parse a range string into a unix timestamp for "since".
pub fn parse_range(range: &str) -> u64 {
    let now = now_epoch();
    match range {
        "today" => today_start_epoch(now),
        "7d" => now.saturating_sub(7 * 86400),
        "30d" => now.saturating_sub(30 * 86400),
        "month" => month_start_epoch(now),
        "all" => 0,
        _ => now.saturating_sub(7 * 86400), // default to 7d
    }
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Start of today (midnight UTC).
fn today_start_epoch(now: u64) -> u64 {
    now - (now % 86400)
}

/// Start of the current calendar month (UTC).
/// Uses 30 days as an approximation to avoid pulling in chrono.
fn month_start_epoch(now: u64) -> u64 {
    now.saturating_sub(30 * 86400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range() {
        let now = now_epoch();
        let today = parse_range("today");
        assert!(today <= now);
        assert!(now - today < 86400);

        let week = parse_range("7d");
        assert!(now - week >= 7 * 86400 - 1);
        assert!(now - week <= 7 * 86400 + 1);

        assert_eq!(parse_range("all"), 0);
    }

    #[test]
    fn test_today_start() {
        // Use a value that's exactly at midnight UTC (divisible by 86400)
        let midnight = (1_713_100_800 / 86400) * 86400;
        assert_eq!(today_start_epoch(midnight), midnight);
        assert_eq!(today_start_epoch(midnight + 3600), midnight);
        assert_eq!(today_start_epoch(midnight + 86399), midnight);
    }
}
