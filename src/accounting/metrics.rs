//! Aggregation and summary queries for token accounting.

use crate::accounting::parser::{CoverageState, SourceCoverage};
use crate::display::{format_token_count, CostRow};
use crate::global_db::{AgentCostSummary, GlobalDb};

/// Full cost summary with breakdowns.
pub struct CostSummary {
    pub total_cost: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_write_tokens: u64,
    pub by_model: Vec<(String, f64, u64)>, // (model, cost, total_tokens)
    pub by_category: Vec<(String, f64, u64)>, // (category, cost, turn_count)
    pub tokens_saved: u64,
    pub efficiency_ratio: f64,
    pub by_agent: Vec<AgentCostSummary>,
}

/// Quick cost summary for the `tokensave status` header row.
/// Returns `None` if no accounting data exists.
pub async fn quick_cost_summary(
    gdb: &GlobalDb,
    tokens_saved: u64,
    global_tokens_saved: u64,
) -> Option<CostRow> {
    quick_cost_summary_with_droid_presence(gdb, tokens_saved, global_tokens_saved, true).await
}

/// Build a quick summary while excluding stale Droid rows when no Droid source exists.
pub async fn quick_cost_summary_with_droid_presence(
    gdb: &GlobalDb,
    tokens_saved: u64,
    global_tokens_saved: u64,
    droid_present: bool,
) -> Option<CostRow> {
    let now = now_epoch();
    let today_start = today_start_epoch(now);
    let week_start = now.saturating_sub(7 * 86400);

    let today_cost = gdb.total_cost_since(today_start).await?;
    let week_cost = gdb.total_cost_since(week_start).await?;
    let by_agent = gdb.cost_by_agent_since(week_start).await;
    let week_consumed = consumed_tokens(&by_agent, droid_present);

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
pub async fn cost_summary(gdb: &GlobalDb, since: u64) -> Option<CostSummary> {
    cost_summary_with_droid_presence(gdb, since, true).await
}

/// Build a full summary while excluding stale Droid rows when no Droid source exists.
///
/// `tokens_saved` is read from the savings ledger for the same `since` the rest
/// of the summary uses, rather than taken as an argument (#473). It used to be
/// passed in, and every caller passed `global_tokens_saved()` — a lifetime,
/// all-projects counter — which put a constant inside a range-scoped payload:
/// the savings figure for `today` equalled the one for `all`, and
/// `efficiency_ratio` moved across ranges only because a fixed numerator was
/// being divided by a growing denominator. Deriving it here from `since` makes
/// the two halves of the summary share one scope by construction, and makes the
/// figure agree with `tokensave gain`, which reads the same ledger.
pub async fn cost_summary_with_droid_presence(
    gdb: &GlobalDb,
    since: u64,
    droid_present: bool,
) -> Option<CostSummary> {
    let tokens_saved = gdb.sum_savings(None, since as i64).await.saved_tokens;
    let total_cost = gdb.total_cost_since(since).await?;
    let (total_input, total_output, total_cache_read, total_cache_write) = gdb
        .token_breakdown_since(since)
        .await
        .unwrap_or((0, 0, 0, 0));
    let by_model = gdb.cost_by_model_since(since).await;
    let by_category = gdb.cost_by_category_since(since).await;
    let mut by_agent = gdb.cost_by_agent_since(since).await;
    if !droid_present {
        by_agent.retain(|summary| summary.agent != "droid");
    }

    let all_consumed = consumed_tokens(&by_agent, droid_present);
    let efficiency_ratio = if tokens_saved + all_consumed > 0 {
        tokens_saved as f64 / (tokens_saved + all_consumed) as f64
    } else {
        0.0
    };

    Some(CostSummary {
        total_cost,
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_cache_read_tokens: total_cache_read,
        total_cache_write_tokens: total_cache_write,
        by_model,
        by_category,
        tokens_saved,
        efficiency_ratio,
        by_agent,
    })
}

pub(crate) fn consumed_tokens(by_agent: &[AgentCostSummary], droid_present: bool) -> u64 {
    by_agent
        .iter()
        .filter(|summary| droid_present || summary.agent != "droid")
        .fold(0, |total, summary| {
            total
                .saturating_add(summary.input_tokens)
                .saturating_add(summary.output_tokens)
        })
}

/// Format a human-readable coverage string when a Droid source exists.
///
/// Returns an empty string when Droid is absent so existing Claude-only output
/// remains unchanged.
pub fn format_coverage(coverage: &[SourceCoverage], by_agent: &[AgentCostSummary]) -> String {
    let claude_state = coverage
        .iter()
        .find(|c| c.agent == "claude")
        .map_or(CoverageState::Absent, |c| c.state);

    let droid_state = coverage
        .iter()
        .find(|c| c.agent == "droid")
        .map_or(CoverageState::Absent, |c| c.state);
    if droid_state == CoverageState::Absent {
        return String::new();
    }

    let claude_text = format!(
        "Claude {}",
        match claude_state {
            CoverageState::Complete => "complete",
            CoverageState::Partial => "partial",
            CoverageState::Absent => "absent",
        }
    );
    let usage = by_agent.iter().find(|a| a.agent == "droid");
    let raw = usage.map(|u| {
        u.input_tokens
            .saturating_add(u.output_tokens)
            .saturating_add(u.cache_write_tokens)
            .saturating_add(u.cache_read_tokens)
    });
    let droid_text = match droid_state {
        CoverageState::Absent => unreachable!("handled above"),
        CoverageState::Partial => match (usage, raw) {
            (Some(u), Some(r)) if r > 0 => match u.credits {
                Some(credits) => format!(
                    "Droid {} credits, {} raw tokens (partial, session-start buckets)",
                    format_token_count(credits),
                    format_token_count(r)
                ),
                None => format!(
                    "Droid credits n/a, {} raw tokens (partial, session-start buckets)",
                    format_token_count(r)
                ),
            },
            _ => "Droid partial; no usage in range".to_string(),
        },
        CoverageState::Complete => match (usage, raw) {
            (Some(u), Some(r)) if r > 0 => match u.credits {
                Some(credits) => format!(
                    "Droid {} credits, {} raw tokens (observed locally, session-start buckets)",
                    format_token_count(credits),
                    format_token_count(r)
                ),
                None => format!(
                    "Droid credits n/a, {} raw tokens (observed locally, session-start buckets)",
                    format_token_count(r)
                ),
            },
            _ => "Droid complete; no usage in range".to_string(),
        },
    };
    format!("Coverage: {claude_text}; {droid_text}")
}

/// Parse a range string into a unix timestamp for "since".
pub fn parse_range(range: &str) -> u64 {
    let now = now_epoch();
    match range {
        "today" => today_start_epoch(now),
        "30d" => now.saturating_sub(30 * 86400),
        "month" => month_start_epoch(now),
        "all" => 0,
        _ => now.saturating_sub(7 * 86400),
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::global_db::AgentCostSummary;

    fn make_agent(
        agent: &str,
        input: u64,
        output: u64,
        cw: u64,
        cr: u64,
        credits: Option<u64>,
    ) -> AgentCostSummary {
        AgentCostSummary {
            agent: agent.to_string(),
            cost_usd: 0.0,
            input_tokens: input,
            output_tokens: output,
            cache_write_tokens: cw,
            cache_read_tokens: cr,
            credits,
            turns: 1,
        }
    }

    #[test]
    fn format_coverage_complete_with_credits_and_usage() {
        let coverage = vec![
            SourceCoverage {
                agent: "claude",
                state: CoverageState::Complete,
                sessions: 5,
            },
            SourceCoverage {
                agent: "droid",
                state: CoverageState::Complete,
                sessions: 3,
            },
        ];
        // raw = 20000 + 3000 + 1000 + 600 = 24600 → "24.6k"; credits = 2900 → "2.9k"
        let by_agent = vec![
            make_agent("claude", 100, 20, 5, 7, None),
            make_agent("droid", 20000, 3000, 1000, 600, Some(2900)),
        ];
        let got = format_coverage(&coverage, &by_agent);
        let expected = "Coverage: Claude complete; Droid 2.9k credits, 24.6k raw tokens (observed locally, session-start buckets)";
        assert_eq!(got, expected);
    }

    #[test]
    fn format_coverage_complete_credits_none() {
        // Complete + usage but credits field is None → "Droid credits n/a, ..."
        let coverage = vec![
            SourceCoverage {
                agent: "claude",
                state: CoverageState::Complete,
                sessions: 5,
            },
            SourceCoverage {
                agent: "droid",
                state: CoverageState::Complete,
                sessions: 3,
            },
        ];
        // raw = 20000 + 3000 + 1000 + 600 = 24600 → "24.6k"; credits = None
        let by_agent = vec![make_agent("droid", 20000, 3000, 1000, 600, None)];
        let got = format_coverage(&coverage, &by_agent);
        let expected = "Coverage: Claude complete; Droid credits n/a, 24.6k raw tokens (observed locally, session-start buckets)";
        assert_eq!(got, expected);
    }

    #[test]
    fn format_coverage_partial_credits_na() {
        let coverage = vec![
            SourceCoverage {
                agent: "claude",
                state: CoverageState::Partial,
                sessions: 2,
            },
            SourceCoverage {
                agent: "droid",
                state: CoverageState::Partial,
                sessions: 1,
            },
        ];
        // raw = 400 + 50 + 10 + 20 = 480
        let by_agent = vec![make_agent("droid", 400, 50, 10, 20, None)];
        let got = format_coverage(&coverage, &by_agent);
        let expected = "Coverage: Claude partial; Droid credits n/a, 480 raw tokens (partial, session-start buckets)";
        assert_eq!(got, expected);
    }

    #[test]
    fn format_coverage_partial_preserves_known_range_credits() {
        let coverage = vec![
            SourceCoverage {
                agent: "claude",
                state: CoverageState::Complete,
                sessions: 1,
            },
            SourceCoverage {
                agent: "droid",
                state: CoverageState::Partial,
                sessions: 1,
            },
        ];
        let by_agent = vec![make_agent("droid", 24_383, 73, 0, 123, Some(2_945))];

        let got = format_coverage(&coverage, &by_agent);

        assert_eq!(
            got,
            "Coverage: Claude complete; Droid 2.9k credits, 24.6k raw tokens (partial, session-start buckets)"
        );
    }

    #[test]
    fn format_coverage_partial_no_usage() {
        // Partial coverage but no usage entries in range
        let coverage = vec![
            SourceCoverage {
                agent: "claude",
                state: CoverageState::Partial,
                sessions: 2,
            },
            SourceCoverage {
                agent: "droid",
                state: CoverageState::Partial,
                sessions: 1,
            },
        ];
        let got = format_coverage(&coverage, &[]);
        let expected = "Coverage: Claude partial; Droid partial; no usage in range";
        assert_eq!(got, expected);
    }

    #[test]
    fn format_coverage_droid_absent() {
        let coverage = vec![
            SourceCoverage {
                agent: "claude",
                state: CoverageState::Complete,
                sessions: 1,
            },
            SourceCoverage {
                agent: "droid",
                state: CoverageState::Absent,
                sessions: 0,
            },
        ];
        let got = format_coverage(&coverage, &[]);
        assert!(got.is_empty(), "{got}");
    }

    #[test]
    fn format_coverage_droid_no_usage_in_range() {
        let coverage = vec![
            SourceCoverage {
                agent: "claude",
                state: CoverageState::Complete,
                sessions: 1,
            },
            SourceCoverage {
                agent: "droid",
                state: CoverageState::Complete,
                sessions: 3,
            },
        ];
        let got = format_coverage(&coverage, &[]);
        assert_eq!(
            got,
            "Coverage: Claude complete; Droid complete; no usage in range"
        );
    }

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
