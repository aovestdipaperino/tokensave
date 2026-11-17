//! Index facts on the output of a `grep`, `rg` or `git grep` command (#688).
//!
//! `tokensave grep COMMAND...` runs the search command unchanged. Each
//! `path:line:` hit keeps its bytes and gains a suffix after a tab, shown
//! here as `<TAB>`:
//!
//! ```text
//! src/cli.rs:24:    let b = get_string(args);<TAB>#ts [in run, 18-40] [calls get_string, src/args.rs:310-322]
//! ```
//!
//! The parts of this module:
//!
//! - [`classify_search`] reads the command arguments. It decides whether the
//!   output has hits and which names the search is about.
//! - [`split_terminator`] keeps the line terminator apart from the text.
//! - [`Index`] opens the index read-only and returns the facts for a line.

mod hit;
mod index;
mod plan;

pub use hit::split_terminator;
pub use index::Index;
pub use index::IndexState;
pub use plan::classify_search;
pub use plan::pattern_tokens;
pub use plan::AnnotatePlan;
pub use plan::SearchMode;
pub use plan::SearchPlan;
pub use plan::Tool;
pub use plan::UsageError;

/// The text between a hit and its index facts.
pub const FACTS_MARKER: &str = "\t#ts ";
