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

/// Variable declaration prefixes understood by Claude's Bash search advice.
pub(super) const JS_VARIABLE_DECLARATION_KEYWORDS: &[&str] = &["const", "let", "var"];

/// Restores native Grep redirects for Claude Code sessions that require them.
pub(super) const CLAUDE_STRICT_GREP_ENV: &str = "TOKENSAVE_CLAUDE_STRICT_GREP";

// C# keywords that can appear alone in text searches. Contextual keywords
// may also name symbols, so an unqualified search in C# stays native.
pub(super) const CSHARP_SEARCH_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "async",
    "await",
    "base",
    "bool",
    "break",
    "by",
    "byte",
    "case",
    "catch",
    "char",
    "checked",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "delegate",
    "do",
    "double",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "false",
    "finally",
    "fixed",
    "float",
    "for",
    "foreach",
    "from",
    "get",
    "global",
    "goto",
    "group",
    "if",
    "implicit",
    "in",
    "init",
    "int",
    "interface",
    "internal",
    "into",
    "is",
    "join",
    "let",
    "lock",
    "long",
    "nameof",
    CSHARP_NAMESPACE_KEYWORD,
    "new",
    "null",
    "object",
    "operator",
    "orderby",
    "out",
    "override",
    "params",
    "partial",
    "private",
    "protected",
    "public",
    "readonly",
    "record",
    "ref",
    "required",
    "return",
    "sbyte",
    "sealed",
    "select",
    "set",
    "short",
    "sizeof",
    "stackalloc",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "uint",
    "ulong",
    "unchecked",
    "unsafe",
    "ushort",
    "using",
    "value",
    "var",
    "virtual",
    "void",
    "volatile",
    "when",
    "where",
    "while",
    "with",
    "yield",
];
pub(super) const CSHARP_SEARCH_TYPES: &[&str] = &["cs", "csharp"];

pub(super) const CSHARP_NAMESPACE_KEYWORD: &str = "namespace";

pub(super) const POWERSHELL_MAX_COMMAND_BYTES: usize = 8 * 1024;
pub(super) const POWERSHELL_SEARCH_NAMES: &[&str] = &["Select-String", "sls"];
pub(super) const POWERSHELL_SHELL_NAMES: &[&str] =
    &["powershell", "powershell.exe", "pwsh", "pwsh.exe"];
/// GNU grep short switches that take no argument and can precede `-r` or `-R` in a cluster.
pub(super) const GREP_NO_VALUE_FLAGS: &str = "EFGPVabchilnorsvwxzHLRZqI";
