//! Phrase tables for the Agent/Task prompt classifier in `hooks.rs`.
//!
//! All entries are lowercase: the classifier lowercases the prompt (ASCII)
//! before matching.

/// Phrases that only describe code research, so they redirect an untyped
/// agent on their own.
pub(super) const CODE_RESEARCH_PHRASES: &[&str] = &[
    "codebase structure",
    "codebase architecture",
    "codebase overview",
    "source files contents",
    "entire codebase",
    "call graph",
    "call path",
    "call chain",
    "symbol relat",
    "symbol lookup",
    "who calls",
    "callers of",
    "callees of",
];

/// Generic reading or exploration phrases. They also describe non-code work
/// (for example "read every listed file" for OCR text, #1078), so they
/// redirect only when the prompt also contains a [`CODE_CONTEXT_WORDS`] word.
pub(super) const GENERIC_EXPLORATION_PHRASES: &[&str] = &[
    "explore",
    "read every",
    "full contents",
    "architecture and structure",
];

/// Whole words that tie a generic exploration phrase to source code. Matched
/// as complete words, so "decode" or "classify" do not count as "code" or
/// "class".
pub(super) const CODE_CONTEXT_WORDS: &[&str] = &[
    "code",
    "codebase",
    "source",
    "sources",
    "src",
    "repo",
    "repository",
    "function",
    "functions",
    "method",
    "methods",
    "symbol",
    "symbols",
    "class",
    "classes",
    "module",
    "modules",
    "struct",
    "structs",
    "crate",
    "crates",
    "package",
    "packages",
    "implementation",
    "implementations",
];

/// Commands and short options recognized by Claude's Bash search advice.
pub(super) const BASH_ADVISORY_GREP_TOOLS: &[&str] = &["grep", "rg", "ag"];
pub(super) const BASH_ADVISORY_GREP_FLAGS: &str = "nrRw";
