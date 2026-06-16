//! Missed-opportunity analyzer over ingested Claude Code transcript turns.
//!
//! Scans the `turns` table and flags turns that consisted *only* of
//! file-navigation tools (`Read`, `Grep`, `Glob`) — work a tokensave graph
//! query (`search`, `context`, `callers`, `callees`, `impact`, `outline`,
//! `read`, `node`) could have served far more cheaply. Results are bucketed by
//! the navigation tool that drove the turn, with a deliberately CONSERVATIVE
//! estimate of recoverable input tokens.
//!
//! # Estimation method and assumptions
//!
//! The `turns` table records, per turn, the comma-joined `tool_names` and the
//! `input_tokens` billed for that turn. It does **not** store the byte/char
//! size of the content each navigation tool returned, nor the content of
//! `Bash` commands (only the literal tool name `Bash` is persisted). Two
//! consequences follow, and we stay strictly within what the data supports:
//!
//! 1. Bash-based navigation (`grep`/`find`/`cat`/`rg`) cannot be detected here,
//!    because command text is not in the table. Those turns are simply not
//!    counted — this makes the analyzer a lower bound, never an over-claim.
//! 2. We cannot compute exact recoverable tokens (that would need the size of
//!    the file payload a `Read`/`Grep`/`Glob` pulled into context). Instead we
//!    report the **addressable** input tokens — the full `input_tokens` spent
//!    on replaceable navigation turns — and a clearly-labeled conservative
//!    lower-bound recoverable figure: `addressable * RECOVERABLE_FRACTION`.
//!
//! A turn is "replaceable" only when *every* tool it used is a navigation tool;
//! a turn that also edits, runs Bash, delegates, etc. is left out entirely.
//! This keeps the count conservative and avoids attributing edit-turn cost to
//! navigation.

/// Conservative fraction of a navigation turn's input tokens treated as
/// recoverable by a graph query.
///
/// A navigation turn's `input_tokens` covers the system prompt, prior
/// conversation, and tool-result payloads carried in context — not just the
/// freshly-read file. A graph query returns a compact slice instead of whole
/// files, but cannot shrink the fixed conversational overhead. We therefore
/// claim only half the addressable input as recoverable. This is intentionally
/// pessimistic: it is a stated lower bound, not a measured value. Changing it
/// only rescales the recoverable column; the addressable figure is exact.
pub const RECOVERABLE_FRACTION: f64 = 0.5;

/// Which tokensave graph query would have replaced a navigation tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavBucket {
    /// `Read` of a file → `outline` / `read` / `node`.
    Read,
    /// `Grep` across files → `search` / `callers` / `callees` / `impact`.
    Grep,
    /// `Glob` file discovery → `files` / `search`.
    Glob,
}

impl NavBucket {
    /// Short stable identifier (used for ordering and machine output).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Grep => "grep",
            Self::Glob => "glob",
        }
    }

    /// The navigation tool name this bucket corresponds to.
    pub fn tool_name(&self) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Grep => "Grep",
            Self::Glob => "Glob",
        }
    }

    /// The tokensave query/queries that would have served the same intent.
    pub fn suggestion(&self) -> &'static str {
        match self {
            Self::Read => "outline / read / node",
            Self::Grep => "search / callers / callees / impact",
            Self::Glob => "files / search",
        }
    }
}

/// The navigation tool names this analyzer recognizes from `tool_names`.
const NAV_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];

/// Per-bucket tally of replaceable navigation turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketStat {
    pub bucket: NavBucket,
    /// Number of replaceable navigation turns attributed to this bucket.
    pub turns: u64,
    /// Sum of `input_tokens` across those turns (the "addressable" total).
    pub addressable_input_tokens: u64,
}

impl BucketStat {
    /// Conservative lower-bound recoverable input tokens for this bucket.
    ///
    /// Defined as `addressable_input_tokens * RECOVERABLE_FRACTION`, rounded
    /// down. See the module-level docs for the assumption behind the fraction.
    pub fn recoverable_input_tokens(&self) -> u64 {
        ((self.addressable_input_tokens as f64) * RECOVERABLE_FRACTION) as u64
    }
}

/// Result of analyzing a set of turns for replaceable navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverReport {
    /// Total turns examined (navigation and non-navigation alike).
    pub total_turns: u64,
    /// Per-bucket stats, ranked by addressable input tokens descending.
    pub buckets: Vec<BucketStat>,
}

impl DiscoverReport {
    /// Total replaceable navigation turns across all buckets.
    pub fn total_replaceable_turns(&self) -> u64 {
        self.buckets.iter().map(|b| b.turns).sum()
    }

    /// Total addressable input tokens across all buckets.
    pub fn total_addressable_input_tokens(&self) -> u64 {
        self.buckets
            .iter()
            .map(|b| b.addressable_input_tokens)
            .sum()
    }

    /// Total conservative recoverable input tokens across all buckets.
    pub fn total_recoverable_input_tokens(&self) -> u64 {
        self.buckets
            .iter()
            .map(BucketStat::recoverable_input_tokens)
            .sum()
    }
}

/// Split a stored `tool_names` value (comma-joined) into trimmed names.
fn split_tools(tool_names: &str) -> Vec<&str> {
    tool_names
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Decide whether a turn is "replaceable navigation" and, if so, which bucket.
///
/// A turn qualifies only when it is non-empty and *every* tool it used is a
/// recognized navigation tool. Mixed turns (navigation + edit/Bash/etc.) return
/// `None` so their cost is never attributed to navigation. When several
/// navigation tools appear, the bucket is chosen by a fixed priority
/// (`Grep` > `Glob` > `Read`): a cross-file search is the strongest signal of
/// an opportunity a graph query would have answered, so it wins attribution.
fn classify_nav(tools: &[&str]) -> Option<NavBucket> {
    if tools.is_empty() {
        return None;
    }
    if !tools.iter().all(|t| NAV_TOOLS.contains(t)) {
        return None;
    }
    if tools.contains(&"Grep") {
        Some(NavBucket::Grep)
    } else if tools.contains(&"Glob") {
        Some(NavBucket::Glob)
    } else {
        Some(NavBucket::Read)
    }
}

/// Analyze `(tool_names, input_tokens)` rows into a ranked [`DiscoverReport`].
///
/// Pure function over already-fetched rows: deterministic, no I/O, no LLM. The
/// caller supplies rows via [`crate::global_db::GlobalDb::nav_turns_since`].
pub fn analyze(turns: &[(String, u64)]) -> DiscoverReport {
    let mut read = BucketStat {
        bucket: NavBucket::Read,
        turns: 0,
        addressable_input_tokens: 0,
    };
    let mut grep = BucketStat {
        bucket: NavBucket::Grep,
        turns: 0,
        addressable_input_tokens: 0,
    };
    let mut glob = BucketStat {
        bucket: NavBucket::Glob,
        turns: 0,
        addressable_input_tokens: 0,
    };

    for (tool_names, input_tokens) in turns {
        let tools = split_tools(tool_names);
        if let Some(bucket) = classify_nav(&tools) {
            let stat = match bucket {
                NavBucket::Read => &mut read,
                NavBucket::Grep => &mut grep,
                NavBucket::Glob => &mut glob,
            };
            stat.turns += 1;
            stat.addressable_input_tokens =
                stat.addressable_input_tokens.saturating_add(*input_tokens);
        }
    }

    // Keep only buckets that actually fired, ranked by addressable tokens
    // descending (ties broken by the stable bucket identifier).
    let mut buckets: Vec<BucketStat> = [read, grep, glob]
        .into_iter()
        .filter(|b| b.turns > 0)
        .collect();
    buckets.sort_by(|a, b| {
        b.addressable_input_tokens
            .cmp(&a.addressable_input_tokens)
            .then_with(|| a.bucket.as_str().cmp(b.bucket.as_str()))
    });

    DiscoverReport {
        total_turns: turns.len() as u64,
        buckets,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn t(tools: &str, input: u64) -> (String, u64) {
        (tools.to_string(), input)
    }

    #[test]
    fn empty_input_is_empty_report() {
        let report = analyze(&[]);
        assert_eq!(report.total_turns, 0);
        assert!(report.buckets.is_empty());
        assert_eq!(report.total_recoverable_input_tokens(), 0);
    }

    #[test]
    fn pure_read_turn_is_read_bucket() {
        let report = analyze(&[t("Read", 1000)]);
        assert_eq!(report.buckets.len(), 1);
        let b = &report.buckets[0];
        assert_eq!(b.bucket, NavBucket::Read);
        assert_eq!(b.turns, 1);
        assert_eq!(b.addressable_input_tokens, 1000);
        assert_eq!(b.recoverable_input_tokens(), 500);
    }

    #[test]
    fn grep_wins_over_glob_and_read_when_mixed_navigation() {
        // All-navigation turn with several nav tools attributes to Grep.
        let report = analyze(&[t("Read,Grep,Glob", 2000)]);
        assert_eq!(report.buckets.len(), 1);
        assert_eq!(report.buckets[0].bucket, NavBucket::Grep);
        assert_eq!(report.buckets[0].turns, 1);
    }

    #[test]
    fn glob_wins_over_read() {
        let report = analyze(&[t("Read,Glob", 800)]);
        assert_eq!(report.buckets.len(), 1);
        assert_eq!(report.buckets[0].bucket, NavBucket::Glob);
    }

    #[test]
    fn turn_with_edit_is_not_replaceable() {
        // Navigation mixed with a mutating tool is excluded entirely.
        let report = analyze(&[t("Read,Edit", 5000), t("Grep,Write", 5000)]);
        assert!(report.buckets.is_empty());
        assert_eq!(report.total_replaceable_turns(), 0);
        assert_eq!(report.total_addressable_input_tokens(), 0);
    }

    #[test]
    fn bash_only_turn_is_not_counted() {
        // Bash command content is not stored, so Bash turns are never nav.
        let report = analyze(&[t("Bash", 3000)]);
        assert!(report.buckets.is_empty());
    }

    #[test]
    fn empty_tool_names_conversation_turn_excluded() {
        let report = analyze(&[t("", 1234)]);
        assert_eq!(report.total_turns, 1);
        assert!(report.buckets.is_empty());
    }

    #[test]
    fn buckets_ranked_by_addressable_tokens_descending() {
        let report = analyze(&[
            t("Read", 100),
            t("Read", 100),
            t("Grep", 5000),
            t("Glob", 900),
        ]);
        assert_eq!(report.buckets.len(), 3);
        assert_eq!(report.buckets[0].bucket, NavBucket::Grep);
        assert_eq!(report.buckets[1].bucket, NavBucket::Glob);
        assert_eq!(report.buckets[2].bucket, NavBucket::Read);
        // Read bucket aggregates both read turns.
        assert_eq!(report.buckets[2].turns, 2);
        assert_eq!(report.buckets[2].addressable_input_tokens, 200);
    }

    #[test]
    fn estimate_is_non_negative_and_monotonic() {
        let small = analyze(&[t("Read", 1000)]);
        let large = analyze(&[t("Read", 1000), t("Grep", 4000)]);
        assert!(large.total_recoverable_input_tokens() >= small.total_recoverable_input_tokens());
        // Recoverable never exceeds addressable (fraction <= 1).
        assert!(
            large.total_recoverable_input_tokens() <= large.total_addressable_input_tokens()
        );
        // And is never negative (u64) — trivially true, but assert the bound.
        assert!(small.total_recoverable_input_tokens() <= small.total_addressable_input_tokens());
    }

    #[test]
    fn whitespace_in_tool_names_is_tolerated() {
        let report = analyze(&[t(" Read , Grep ", 1200)]);
        assert_eq!(report.buckets.len(), 1);
        assert_eq!(report.buckets[0].bucket, NavBucket::Grep);
        assert_eq!(report.buckets[0].turns, 1);
    }
}
