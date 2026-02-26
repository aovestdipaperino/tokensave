/// Builds AI-ready context from the code graph.
pub mod builder;

/// Formats task context as Markdown or JSON.
pub mod formatter;

pub use builder::{extract_symbols_from_query, ContextBuilder};
pub use formatter::{format_context_as_json, format_context_as_markdown};
