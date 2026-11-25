//! Tool-name tokens stored in the `turns.tool_names` column.
//!
//! `tool_names` is a comma-joined list of the tools a turn used. A `Bash` use
//! that is a code search is stored with a suffix so `tokensave discover` can
//! count it without keeping the command text (#1118).

/// Claude Code's shell tool name.
pub const BASH_TOOL: &str = "Bash";

/// A `Bash` use whose command a graph query could have answered.
pub const BASH_SEARCH_TOOL: &str = "Bash:grep";

/// Prefix of a `Bash` search that `discover` excludes; the exclusion code
/// ([`crate::hooks::BashSearchExclusion::as_str`]) follows it.
pub const BASH_SEARCH_EXCLUDED_PREFIX: &str = "Bash:grep-skip:";
