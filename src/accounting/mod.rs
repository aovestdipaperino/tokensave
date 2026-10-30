//! Token accounting: cost observability from agent session transcripts.
//!
//! Parses `~/.claude/projects/**/*.jsonl` (Claude Code, priced from the
//! embedded model table), `~/.pi/agent/sessions/**/*.jsonl` (Pi, which
//! reports its own billed cost per message), and Factory Droid session
//! settings, classifies each turn, and stores results in the global database
//! for fast aggregate queries.

pub mod classifier;
pub mod discover;
mod droid;
pub mod metrics;
pub mod parser;
mod pi;
pub mod pricing;

pub use classifier::TaskCategory;
pub use discover::{analyze, DiscoverReport};
pub use metrics::{format_coverage, quick_cost_summary, CostSummary};
pub use parser::{
    ingest, ingest_claude_only, ingest_pi_only, CoverageState, IngestStats, SourceCoverage,
};
