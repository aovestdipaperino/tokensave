/// Graph traversal algorithms for the code graph.
pub mod traversal;

/// Query operations for analyzing the code graph.
pub mod queries;

pub use queries::{GraphQueryManager, NodeMetrics};
pub use traversal::GraphTraverser;
