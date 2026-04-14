//! Token accounting: cost observability from Claude Code session transcripts.
//!
//! Parses `~/.claude/projects/**/*.jsonl` files, classifies each API turn,
//! computes dollar cost via embedded model pricing, and stores results in
//! the global database for fast aggregate queries.

pub mod classifier;
pub mod metrics;
pub mod parser;
pub mod pricing;

pub use classifier::TaskCategory;
pub use metrics::{quick_cost_summary, CostSummary};
pub use parser::ingest;
