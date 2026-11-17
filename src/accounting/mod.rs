//! Token accounting: cost observability from agent session transcripts.
//!
//! Parses `~/.claude/projects/**/*.jsonl` (Claude Code, priced from the
//! embedded model table), `~/.pi/agent/sessions/**/*.jsonl` (Pi, which
//! reports its own billed cost per message), and Factory Droid session
//! settings, classifies each turn, and stores results in the global database
//! for fast aggregate queries.

mod augment;
pub mod classifier;
mod codex;
mod copilot;
pub mod discover;
mod droid;
pub mod metrics;
pub mod parser;
mod pi;
pub mod pricing;

/// Agents metered in native credit units rather than USD.
///
/// Droid (Factory credits) and Copilot (nano-AIU) have no dollar figure, so
/// their USD cost cell is blank in tables, `null` in JSON, and empty in CSV,
/// never `$0.00`. Claude and Codex are USD-metered (real tokens times model
/// price) and are deliberately absent here. This single predicate replaces the
/// per-surface `agent == "droid" || agent == "copilot"` checks so a new agent is
/// classified in exactly one place. See `.overlay/notes/adding-a-model.md`.
pub fn is_credits_only(agent: &str) -> bool {
    matches!(agent, "droid" | "copilot")
}

pub use classifier::TaskCategory;
pub use copilot::format_aiu as copilot_format_aiu;
pub use discover::{analyze, DiscoverReport};
pub use metrics::{format_coverage, quick_cost_summary, CostSummary};
pub use parser::{
    ingest, ingest_claude_only, ingest_pi_only, CoverageState, IngestStats, SourceCoverage,
};
