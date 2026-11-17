//! Context assembly, rendering, ranking, and conditional-read helpers.

/// Builds AI-ready context from the code graph.
// Keep the established public `context::builder` path for caller compatibility.
// Its primary type is also available through the `ContextBuilder` re-export.
pub mod builder;

/// Formats task context as Markdown or JSON.
pub mod formatter;

/// Re-ranking of search candidates using structural signals.
pub mod ranking;

/// Digests and file timestamps for conditional reads and edit tools.
pub mod read_cache;

/// Mode dispatchers (`full`, `lines`, `map`, `signatures`) for `tokensave_read`.
pub mod read_modes;

pub use builder::{extract_symbols_from_query, ContextBuilder};
pub use formatter::{compact_signature, format_context_as_json, format_context_as_markdown};
