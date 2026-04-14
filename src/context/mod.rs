/// Builds AI-ready context from the code graph.
pub mod builder;

/// Formats task context as Markdown or JSON.
pub mod formatter;

/// Re-ranking of search candidates using structural signals.
pub mod ranking;

pub use builder::{extract_symbols_from_query, ContextBuilder};
pub use formatter::{format_context_as_json, format_context_as_markdown};
