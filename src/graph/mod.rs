/// Graph traversal algorithms for the code graph.
pub mod traversal;

/// Query operations for analyzing the code graph.
pub mod queries;

/// Structural health analysis algorithms.
pub mod health;

/// Git integration helpers for churn analysis.
pub mod git;

pub use queries::{GraphQueryManager, NodeMetrics};
pub use traversal::GraphTraverser;
