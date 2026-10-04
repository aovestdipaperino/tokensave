//! Graph-based symbol rename (`tokensave_rename`, #568).
//!
//! The rename sites are the symbol's own name token plus every incoming
//! reference the graph records, each located to an exact byte span by
//! re-parsing the file with tree-sitter. Every site carries a confidence
//! class derived from how the resolver bound it (`edges.resolved_by`, #544).
//!
//! This is *graph-based, not binding-aware*: tokensave resolves references by
//! name, scope hints and imports, not by each language's binding rules. The
//! classes say how far each site can be trusted; only `exact` sites are
//! edited unless the caller opts in to `heuristic` ones, and `ambiguous` and
//! `text_only` sites are never edited.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tree_sitter::{Language, Parser, Tree};

use crate::errors::{Result, TokenSaveError};
use crate::types::{EdgeKind, FileKind, Node, NodeKind, ResolvedBy, UnresolvedRef};

use super::TokenSave;

/// How far a rename site can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameConfidence {
    /// The resolver bound this reference to the symbol by a qualified path,
    /// a typed receiver, an import, or a name nothing else carries, and the
    /// name token was located unambiguously.
    Exact,
    /// A name-based fallback bound it, or its token could not be told apart
    /// from another same-named token on the line. Edited only with
    /// `allow_heuristic`.
    Heuristic,
    /// A call the resolver could not decide between this symbol and others.
    /// Listed, never edited.
    Ambiguous,
    /// A mention the graph does not link to the symbol: a comment, a string,
    /// a doc, or an identifier the graph did not resolve here. Listed for
    /// review, never edited.
    TextOnly,
}

impl RenameConfidence {
    /// The class name as reported in the plan.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RenameConfidence::Exact => "exact",
            RenameConfidence::Heuristic => "heuristic",
            RenameConfidence::Ambiguous => "ambiguous",
            RenameConfidence::TextOnly => "text_only",
        }
    }
}

/// One place the old name appears.
#[derive(Debug, Clone, Serialize)]
pub struct RenameSite {
    /// Index-relative path of the file.
    pub file: String,
    /// 1-based line of the name token.
    pub line: u32,
    /// 1-based byte column where the token starts, or 0 when it could not be
    /// located.
    pub column: u32,
    /// 1-based byte column one past the token's end, or 0 when unlocated.
    pub end_column: u32,
    /// Trust class.
    pub confidence: RenameConfidence,
    /// What the site is: `definition`, `override`, `impl`, an edge kind
    /// (`calls`, `uses`, …), `ambiguous_call`, or for text-only mentions the
    /// context (`comment`, `string`, `code`, `text`).
    pub kind: String,
    /// The resolver path that produced the edge, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<&'static str>,
    /// Qualified name of the referencing symbol, for edge sites.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Why the site is not exact, when it is not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Byte range of the name token in the file as read, when located.
    #[serde(skip)]
    pub byte_range: Option<(usize, usize)>,
}

/// Everything `tokensave_rename` knows before it edits anything.
#[derive(Debug, Clone)]
pub struct RenamePlan {
    /// The symbol being renamed.
    pub target: Node,
    /// Its current name.
    pub old_name: String,
    /// The requested name, when one was given.
    pub new_name: Option<String>,
    /// Every site, in file then position order.
    pub sites: Vec<RenameSite>,
    /// Text-only mentions found beyond the listing cap.
    pub text_only_omitted: usize,
    /// Of those, unlinked identifiers in code. They are not listed, but they
    /// gate an apply exactly like the listed ones.
    pub unlinked_code_omitted: usize,
    /// Indexed files that mention the old name but could not be classified
    /// (too large to parse, or unreadable). They gate an apply: a reference
    /// in one of them would be left stale without anyone being told.
    pub unscanned: Vec<String>,
    /// Conditions that make the rename impossible regardless of flags
    /// (invalid name, collision, file changed since indexing).
    pub blockers: Vec<String>,
    /// Advisory notes.
    pub warnings: Vec<String>,
    /// File contents as read, keyed by index-relative path.
    sources: HashMap<String, String>,
    /// Resolved absolute path per file.
    abs_paths: HashMap<String, PathBuf>,
    /// Root-relative path for reindexing, when the file is under the index.
    index_paths: HashMap<String, Option<String>>,
}

/// The result of applying a plan.
#[derive(Debug, Clone, Serialize)]
pub struct RenameOutcome {
    /// Whether the files were written.
    pub applied: bool,
    /// Why nothing was written, when it was not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
    /// The sites that blocked the edit.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocking_sites: Vec<RenameSite>,
    /// `(file, sites edited)` for every written file.
    pub files_changed: Vec<(String, usize)>,
    /// Located sites that were left alone.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<RenameSite>,
    /// Advisory notes (failed reindex, …).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Mentions listed in a plan before the rest are only counted.
const TEXT_ONLY_LIST_CAP: usize = 200;

/// Files larger than this are not scanned for text-only mentions.
const TEXT_SCAN_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// How many lines past a reference's recorded start its name token may sit
/// (`obj\n    .method()` records the line of `obj`).
const REF_LOOKAHEAD_LINES: usize = 3;

/// Edge kinds whose source spells the target's name at the edge's line.
/// `contains`, `annotates` and `documents` are structural, not references.
const REFERENCE_KINDS: &[EdgeKind] = &[
    EdgeKind::Calls,
    EdgeKind::Uses,
    EdgeKind::Implements,
    EdgeKind::Extends,
    EdgeKind::Instantiates,
    EdgeKind::TypeOf,
    EdgeKind::Returns,
    EdgeKind::Receives,
    EdgeKind::DerivesMacro,
    EdgeKind::Reopens,
];

/// Validates `name` as an identifier in the languages tokensave indexes:
/// a letter, `_` or `$`, then letters, digits, `_` or `$`.
///
/// Keywords are not rejected here; an edit that turns a name into a keyword
/// fails the post-edit parse check instead, for every language at once.
///
/// # Errors
/// Returns a message describing why `name` is not an identifier.
pub fn validate_identifier(name: &str) -> std::result::Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("new_name is empty".to_string());
    };
    if !(first.is_alphabetic() || first == '_' || first == '$') {
        return Err(format!(
            "new_name '{name}' is not an identifier: it must start with a letter, '_' or '$'"
        ));
    }
    if let Some(bad) = chars.find(|c| !(c.is_alphanumeric() || *c == '_' || *c == '$')) {
        return Err(format!(
            "new_name '{name}' is not an identifier: '{bad}' is not allowed"
        ));
    }
    Ok(())
}

/// The reserved words of the language `path` is written in, with its name.
///
/// Some grammars accept a keyword where an identifier goes (tree-sitter-rust
/// parses `fn fn() {}` without an error node), so the post-edit parse check
/// cannot be relied on to catch a rename to a keyword.
fn reserved_words(path: &str) -> Option<(&'static str, &'static [&'static str])> {
    const RUST: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
        "final", "gen", "macro", "override", "priv", "try", "typeof", "unsized", "virtual",
        "yield",
    ];
    const PYTHON: &[&str] = &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ];
    const GO: &[&str] = &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
    ];
    const JAVA: &[&str] = &[
        "abstract",
        "assert",
        "boolean",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "float",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "int",
        "interface",
        "long",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "short",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "void",
        "volatile",
        "while",
        "true",
        "false",
        "null",
    ];
    const JS: &[&str] = &[
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "null",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ];
    const C: &[&str] = &[
        "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
        "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
        "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
        "union", "unsigned", "void", "volatile", "while",
    ];
    const CPP: &[&str] = &[
        "alignas",
        "alignof",
        "asm",
        "auto",
        "bool",
        "break",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "constexpr",
        "const_cast",
        "continue",
        "decltype",
        "default",
        "delete",
        "do",
        "double",
        "dynamic_cast",
        "else",
        "enum",
        "explicit",
        "export",
        "extern",
        "false",
        "float",
        "for",
        "friend",
        "goto",
        "if",
        "inline",
        "int",
        "long",
        "mutable",
        "namespace",
        "new",
        "noexcept",
        "nullptr",
        "operator",
        "private",
        "protected",
        "public",
        "register",
        "reinterpret_cast",
        "return",
        "short",
        "signed",
        "sizeof",
        "static",
        "static_assert",
        "static_cast",
        "struct",
        "switch",
        "template",
        "this",
        "thread_local",
        "throw",
        "true",
        "try",
        "typedef",
        "typeid",
        "typename",
        "union",
        "unsigned",
        "using",
        "virtual",
        "void",
        "volatile",
        "wchar_t",
        "while",
    ];
    const CSHARP: &[&str] = &[
        "abstract",
        "as",
        "base",
        "bool",
        "break",
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
        "goto",
        "if",
        "implicit",
        "in",
        "int",
        "interface",
        "internal",
        "is",
        "lock",
        "long",
        "namespace",
        "new",
        "null",
        "object",
        "operator",
        "out",
        "override",
        "params",
        "private",
        "protected",
        "public",
        "readonly",
        "ref",
        "return",
        "sbyte",
        "sealed",
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
        "virtual",
        "void",
        "volatile",
        "while",
    ];
    const KOTLIN: &[&str] = &[
        "as",
        "break",
        "class",
        "continue",
        "do",
        "else",
        "false",
        "for",
        "fun",
        "if",
        "in",
        "interface",
        "is",
        "null",
        "object",
        "package",
        "return",
        "super",
        "this",
        "throw",
        "true",
        "try",
        "typealias",
        "typeof",
        "val",
        "var",
        "when",
        "while",
    ];
    const SWIFT: &[&str] = &[
        "associatedtype",
        "class",
        "deinit",
        "enum",
        "extension",
        "fileprivate",
        "func",
        "import",
        "init",
        "inout",
        "internal",
        "let",
        "open",
        "operator",
        "private",
        "protocol",
        "public",
        "rethrows",
        "static",
        "struct",
        "subscript",
        "typealias",
        "var",
        "break",
        "case",
        "continue",
        "default",
        "defer",
        "do",
        "else",
        "fallthrough",
        "for",
        "guard",
        "if",
        "in",
        "repeat",
        "return",
        "switch",
        "where",
        "while",
        "as",
        "catch",
        "false",
        "is",
        "nil",
        "super",
        "self",
        "Self",
        "throw",
        "throws",
        "true",
        "try",
    ];
    const RUBY: &[&str] = &[
        "BEGIN", "END", "alias", "and", "begin", "break", "case", "class", "def", "do", "else",
        "elsif", "end", "ensure", "false", "for", "if", "in", "module", "next", "nil", "not", "or",
        "redo", "rescue", "retry", "return", "self", "super", "then", "true", "undef", "unless",
        "until", "when", "while", "yield",
    ];
    const PHP: &[&str] = &[
        "abstract",
        "and",
        "array",
        "as",
        "break",
        "callable",
        "case",
        "catch",
        "class",
        "clone",
        "const",
        "continue",
        "declare",
        "default",
        "do",
        "echo",
        "else",
        "elseif",
        "empty",
        "extends",
        "final",
        "finally",
        "fn",
        "for",
        "foreach",
        "function",
        "global",
        "goto",
        "if",
        "implements",
        "include",
        "instanceof",
        "insteadof",
        "interface",
        "isset",
        "list",
        "match",
        "namespace",
        "new",
        "or",
        "print",
        "private",
        "protected",
        "public",
        "readonly",
        "require",
        "return",
        "static",
        "switch",
        "throw",
        "trait",
        "try",
        "unset",
        "use",
        "var",
        "while",
        "xor",
        "yield",
    ];
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => ("Rust", RUST),
        "py" | "pyi" => ("Python", PYTHON),
        "go" => ("Go", GO),
        "java" => ("Java", JAVA),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => ("JavaScript", JS),
        "c" | "h" => ("C", C),
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => ("C++", CPP),
        "cs" => ("C#", CSHARP),
        "kt" | "kts" => ("Kotlin", KOTLIN),
        "swift" => ("Swift", SWIFT),
        "rb" | "rake" => ("Ruby", RUBY),
        "php" => ("PHP", PHP),
        _ => return None,
    })
}

/// The identifier a reference name ends in: `crate::a::b` → `b`,
/// `self.run` → `run`, `Type::step()::m` → `m`.
fn last_segment(reference_name: &str) -> &str {
    let trimmed = reference_name.trim_end_matches("()");
    trimmed
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .next()
        .unwrap_or(trimmed)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80
}

/// Byte offsets of every whole-word occurrence of `name` in `source`.
fn word_occurrences(source: &str, name: &str) -> Vec<usize> {
    let bytes = source.as_bytes();
    source
        .match_indices(name)
        .map(|(i, _)| i)
        .filter(|&i| {
            let before = i == 0 || !is_ident_byte(bytes[i - 1]);
            let end = i + name.len();
            let after = end >= bytes.len() || !is_ident_byte(bytes[end]);
            before && after
        })
        .collect()
}

/// Whether `kind` names a comment or string node in some grammar.
fn is_comment_or_string_kind(kind: &str) -> Option<&'static str> {
    if kind.contains("comment") {
        Some("comment")
    } else if kind.contains("string") || kind.contains("heredoc") || kind == "char_literal" {
        Some("string")
    } else {
        None
    }
}

/// Counts error and missing nodes in a tree.
fn error_count(tree: &Tree) -> usize {
    let mut count = 0;
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            count += 1;
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return count;
            }
        }
    }
}

fn parse(language: &Language, source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).ok()?;
    parser.parse(source, None)
}

/// A file read for the plan, with what the plan needs to locate tokens.
struct FileText {
    source: String,
    tree: Option<Tree>,
    line_starts: Vec<usize>,
    /// Byte ranges of code tokens spelling the old name, in order. From the
    /// syntax tree when there is a grammar, else every whole-word match.
    tokens: Vec<(usize, usize)>,
}

impl FileText {
    fn new(path: &str, source: String, name: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(source.match_indices('\n').map(|(i, _)| i + 1));
        let tree = crate::extraction::ts_provider::language_for_path(path)
            .and_then(|lang| parse(&lang, &source));
        let tokens = match &tree {
            Some(tree) => code_tokens(tree, &source, name),
            None => word_occurrences(&source, name)
                .into_iter()
                .map(|i| (i, i + name.len()))
                .collect(),
        };
        Self {
            source,
            tree,
            line_starts,
            tokens,
        }
    }

    fn has_grammar(&self) -> bool {
        self.tree.is_some()
    }

    /// Byte offset of a 0-based `(row, byte column)` point, clamped.
    fn offset(&self, row: u32, column: u32) -> usize {
        let Some(&start) = self.line_starts.get(row as usize) else {
            return self.source.len();
        };
        (start + column as usize).min(self.source.len())
    }

    /// Byte offset where 0-based `row` starts, or the end of the file.
    fn line_start(&self, row: usize) -> usize {
        self.line_starts
            .get(row)
            .copied()
            .unwrap_or(self.source.len())
    }

    /// 0-based row and byte column of `offset`.
    fn position(&self, offset: usize) -> (usize, usize) {
        let row = self.line_starts.partition_point(|&s| s <= offset) - 1;
        (row, offset - self.line_starts[row])
    }

    /// The first name token starting in `[from, until)`.
    fn first_token_in(&self, from: usize, until: usize) -> Option<(usize, usize)> {
        let i = self.tokens.partition_point(|&(s, _)| s < from);
        self.tokens.get(i).copied().filter(|&(s, _)| s < until)
    }

    /// Every name token starting in `[from, until)`.
    fn tokens_in(&self, from: usize, until: usize) -> Vec<(usize, usize)> {
        let i = self.tokens.partition_point(|&(s, _)| s < from);
        self.tokens[i..]
            .iter()
            .copied()
            .take_while(|&(s, _)| s < until)
            .collect()
    }

    /// Classifies a whole-word mention at `[start, end)`.
    fn mention_context(&self, start: usize, end: usize) -> &'static str {
        let Some(tree) = &self.tree else {
            return "text";
        };
        let mut node = tree
            .root_node()
            .descendant_for_byte_range(start, end)
            .unwrap_or_else(|| tree.root_node());
        loop {
            if let Some(ctx) = is_comment_or_string_kind(node.kind()) {
                return ctx;
            }
            match node.parent() {
                Some(parent) => node = parent,
                None => break,
            }
        }
        if self.tokens.binary_search(&(start, end)).is_ok() {
            "code"
        } else {
            "text"
        }
    }
}

/// Leaf tokens spelling `name` outside comments and strings.
fn code_tokens(tree: &Tree, source: &str, name: &str) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut cursor = tree.walk();
    let mut suppressed_depth: Option<usize> = None;
    let mut depth = 0usize;
    loop {
        let node = cursor.node();
        if suppressed_depth.is_none() && is_comment_or_string_kind(node.kind()).is_some() {
            suppressed_depth = Some(depth);
        }
        if suppressed_depth.is_none()
            && node.child_count() == 0
            && bytes.get(node.start_byte()..node.end_byte()) == Some(name.as_bytes())
        {
            out.push((node.start_byte(), node.end_byte()));
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if suppressed_depth == Some(depth) {
                suppressed_depth = None;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                out.sort_unstable();
                out.dedup();
                return out;
            }
            depth -= 1;
        }
    }
}

/// Node kinds that cannot be renamed through their name.
fn is_unrenameable(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::File
            | NodeKind::Route
            | NodeKind::Doc
            | NodeKind::Use
            | NodeKind::Impl
            | NodeKind::AnnotationUsage
            | NodeKind::Include
            | NodeKind::Export
            | NodeKind::GoPackage
            | NodeKind::Package
            | NodeKind::ScalaPackage
            | NodeKind::KotlinPackage
    )
}

fn is_callable(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function
            | NodeKind::Method
            | NodeKind::AbstractMethod
            | NodeKind::StructMethod
            | NodeKind::SingletonMethod
    )
}

fn is_type_like(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Struct
            | NodeKind::Enum
            | NodeKind::Union
            | NodeKind::Trait
            | NodeKind::Class
            | NodeKind::Interface
            | NodeKind::TypeAlias
    )
}

/// Builds a unified diff of `old` → `new`, which differ only within lines
/// (a rename never adds or removes a line).
fn unified_diff(path: &str, old: &str, new: &str, context: usize) -> String {
    use std::fmt::Write;
    let old_lines: Vec<&str> = old.split('\n').collect();
    let new_lines: Vec<&str> = new.split('\n').collect();
    if old_lines.len() != new_lines.len() {
        return format!("--- a/{path}\n+++ b/{path}\n(line count changed; diff omitted)\n");
    }
    let changed: Vec<usize> = (0..old_lines.len())
        .filter(|&i| old_lines[i] != new_lines[i])
        .collect();
    if changed.is_empty() {
        return String::new();
    }
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    let mut i = 0;
    while i < changed.len() {
        let start = changed[i].saturating_sub(context);
        let mut end = (changed[i] + context).min(old_lines.len() - 1);
        let mut j = i + 1;
        while j < changed.len() && changed[j] <= end + context + 1 {
            end = (changed[j] + context).min(old_lines.len() - 1);
            j += 1;
        }
        let count = end - start + 1;
        let _ = writeln!(out, "@@ -{},{count} +{},{count} @@", start + 1, start + 1);
        for line in start..=end {
            if old_lines[line] == new_lines[line] {
                let _ = writeln!(out, " {}", old_lines[line]);
            } else {
                let _ = writeln!(out, "-{}", old_lines[line]);
                let _ = writeln!(out, "+{}", new_lines[line]);
            }
        }
        i = j;
    }
    out
}

impl RenamePlan {
    /// Number of sites per class, in class order.
    #[must_use]
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts: BTreeMap<&'static str, usize> = [
            RenameConfidence::Exact,
            RenameConfidence::Heuristic,
            RenameConfidence::Ambiguous,
            RenameConfidence::TextOnly,
        ]
        .into_iter()
        .map(|c| (c.as_str(), 0))
        .collect();
        for site in &self.sites {
            *counts.entry(site.confidence.as_str()).or_default() += 1;
        }
        *counts.entry("text_only").or_default() += self.text_only_omitted;
        counts
    }

    /// Sites whose presence refuses an apply without `allow_heuristic`:
    /// heuristic and ambiguous sites, and identifiers in code the graph does
    /// not link to the symbol (a missed reference would be left stale).
    #[must_use]
    pub fn non_exact_sites(&self) -> Vec<&RenameSite> {
        self.sites
            .iter()
            .filter(|s| match s.confidence {
                RenameConfidence::Exact => false,
                RenameConfidence::Heuristic | RenameConfidence::Ambiguous => true,
                RenameConfidence::TextOnly => s.kind == "code",
            })
            .collect()
    }

    /// Reasons an apply is gated that no listed site carries: unlinked
    /// identifiers past the listing cap, and files that mention the name but
    /// could not be checked.
    #[must_use]
    pub fn unchecked_reasons(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.unlinked_code_omitted > 0 {
            out.push(format!(
                "unlisted: {} more unlinked identifier(s) past the listing cap",
                self.unlinked_code_omitted
            ));
        }
        if !self.unscanned.is_empty() {
            out.push(format!(
                "{} file(s) mention the name but could not be checked: {}",
                self.unscanned.len(),
                self.unscanned.join(", ")
            ));
        }
        out
    }

    /// Sites the edit would change.
    fn editable(&self, allow_heuristic: bool) -> Vec<&RenameSite> {
        self.sites
            .iter()
            .filter(|s| {
                s.byte_range.is_some()
                    && (s.confidence == RenameConfidence::Exact
                        || (allow_heuristic && s.confidence == RenameConfidence::Heuristic))
            })
            .collect()
    }

    /// New contents per file for the editable sites, or `None` without a
    /// new name.
    fn edited_sources(&self, allow_heuristic: bool) -> Option<BTreeMap<String, (String, usize)>> {
        let new_name = self.new_name.as_deref()?;
        let mut by_file: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        for site in self.editable(allow_heuristic) {
            if let Some(range) = site.byte_range {
                by_file.entry(site.file.clone()).or_default().push(range);
            }
        }
        let mut out = BTreeMap::new();
        for (file, mut ranges) in by_file {
            let Some(source) = self.sources.get(&file) else {
                continue;
            };
            ranges.sort_unstable();
            ranges.dedup();
            let mut edited = source.clone();
            for &(start, end) in ranges.iter().rev() {
                edited.replace_range(start..end, new_name);
            }
            out.insert(file, (edited, ranges.len()));
        }
        Some(out)
    }

    /// Unified diff of the edits the plan would apply with these flags, one
    /// entry per file. Empty without a new name.
    #[must_use]
    pub fn file_diffs(&self, allow_heuristic: bool) -> Vec<String> {
        let Some(edited) = self.edited_sources(allow_heuristic) else {
            return Vec::new();
        };
        edited
            .iter()
            .filter_map(|(file, (new, _))| {
                let old = self.sources.get(file)?;
                Some(unified_diff(file, old, new, 2))
            })
            .filter(|d| !d.is_empty())
            .collect()
    }
}

impl TokenSave {
    /// Resolves the symbol to rename from a node id or a symbol locator
    /// (name or qualified name, resolved like `tokensave_replace_symbol`).
    ///
    /// # Errors
    /// Returns an error for an unknown or ambiguous symbol locator.
    pub async fn rename_target(
        &self,
        node_id: Option<&str>,
        symbol: Option<&str>,
    ) -> Result<Option<Node>> {
        if let Some(id) = node_id {
            return self.get_node(id).await;
        }
        let Some(symbol) = symbol else {
            return Err(TokenSaveError::Config {
                message: "missing required parameter: node_id or symbol".to_string(),
            });
        };
        super::query::resolve_symbol_for_edit(self, symbol)
            .await
            .map(Some)
    }

    /// Collects every rename site of `target` and classifies it.
    ///
    /// Reads nothing but the graph and the files the sites live in (plus
    /// any indexed file that mentions the name, for text-only mentions).
    /// `root_override` retargets reading to another tree with the same
    /// layout, e.g. a git worktree; see [`Self::resolve_edit_target`].
    ///
    /// # Errors
    /// Returns an error when the graph cannot be read.
    pub async fn plan_rename(
        &self,
        target: Node,
        new_name: Option<&str>,
        root_override: Option<&str>,
    ) -> Result<RenamePlan> {
        let old_name = target.name.clone();
        let mut plan = RenamePlan {
            target: target.clone(),
            old_name: old_name.clone(),
            new_name: new_name.map(str::to_string),
            sites: Vec::new(),
            text_only_omitted: 0,
            unlinked_code_omitted: 0,
            unscanned: Vec::new(),
            blockers: Vec::new(),
            warnings: Vec::new(),
            sources: HashMap::new(),
            abs_paths: HashMap::new(),
            index_paths: HashMap::new(),
        };

        if is_unrenameable(&target.kind) {
            plan.blockers.push(format!(
                "a {} node cannot be renamed through its name; rename the symbol it refers to",
                target.kind.as_str()
            ));
            return Ok(plan);
        }
        if let Err(message) = validate_identifier(&old_name) {
            plan.blockers.push(format!(
                "the symbol's indexed name is not a plain identifier ({message}); \
                 tokensave_rename only renames identifiers"
            ));
            return Ok(plan);
        }
        if let Some(new) = new_name {
            if let Err(message) = validate_identifier(new) {
                plan.blockers.push(message);
            } else if new == old_name {
                plan.blockers
                    .push(format!("new_name is the current name '{old_name}'"));
            }
        }

        if let Some(reason) = self.db.rails_route_rename_blocker(&target).await? {
            plan.blockers.push(reason);
        }

        let mut files: HashMap<String, FileText> = HashMap::new();

        // --- The definitions: the symbol, its overrides, its impl blocks. ---
        let overrides = self.rename_overrides(&target).await?;
        let mut definitions: Vec<(Node, &'static str)> = vec![(target.clone(), "definition")];
        definitions.extend(overrides.into_iter().map(|n| (n, "override")));
        if is_type_like(&target.kind) {
            for node in self.db.get_nodes_by_name(&old_name).await? {
                if node.kind == NodeKind::Impl {
                    definitions.push((node, "impl"));
                }
            }
        }

        for (node, role) in &definitions {
            let Some(text) = self
                .rename_file(
                    &mut files,
                    &mut plan,
                    &node.file_path,
                    &old_name,
                    root_override,
                )
                .await
            else {
                continue;
            };
            let start = text.offset(node.start_line, node.start_column);
            let end = text
                .offset(node.end_line, node.end_column)
                .max(start + old_name.len());
            let located = text.first_token_in(start, end);
            let (confidence, reason) = match (*role, located, text.has_grammar()) {
                (_, None, _) => (
                    RenameConfidence::Heuristic,
                    Some("name token not found in the declaration".to_string()),
                ),
                ("definition", Some(_), true) => (RenameConfidence::Exact, None),
                (_, Some(_), false) => (
                    RenameConfidence::Heuristic,
                    Some("no grammar for this file; located by text".to_string()),
                ),
                ("override", Some(_), true) => (
                    RenameConfidence::Heuristic,
                    Some(
                        "overrides or implements the renamed method; paired by name and \
                         the implements/extends graph"
                            .to_string(),
                    ),
                ),
                (_, Some(_), true) => (
                    RenameConfidence::Heuristic,
                    Some("impl block named after the type; paired by name".to_string()),
                ),
            };
            let site = make_site(
                text,
                &node.file_path,
                located,
                node.start_line,
                confidence,
                (*role).to_string(),
                None,
                None,
                reason,
            );
            plan.sites.push(site);
        }

        // --- Incoming references to every definition. ---
        let definition_ids: Vec<&Node> = definitions
            .iter()
            .filter(|(_, role)| *role != "impl")
            .map(|(n, _)| n)
            .collect();
        for def in &definition_ids {
            let is_primary = def.id == target.id;
            let edges = self.db.get_incoming_edges(&def.id, REFERENCE_KINDS).await?;
            let source_ids: Vec<String> = edges
                .iter()
                .map(|e| e.source.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let refs = self.db.get_unresolved_refs_for_sources(&source_ids).await?;
            let mut refs_by_key: HashMap<(&str, u32, EdgeKind), Vec<&UnresolvedRef>> =
                HashMap::new();
            for r in &refs {
                if last_segment(&r.reference_name) == old_name {
                    refs_by_key
                        .entry((r.from_node_id.as_str(), r.line, r.reference_kind))
                        .or_default()
                        .push(r);
                }
            }
            let sources: HashMap<String, Node> = self
                .db
                .get_nodes_by_ids(&source_ids)
                .await?
                .into_iter()
                .map(|n| (n.id.clone(), n))
                .collect();

            for edge in &edges {
                let Some(source) = sources.get(&edge.source) else {
                    continue;
                };
                // Exact dispatch does not identify an editable action token inside a route target string.
                let route_reference = source.kind == NodeKind::Route
                    || edge.resolved_by == Some(ResolvedBy::RailsRoute);
                let route_reason = "route target literals cannot be renamed safely yet";
                if route_reference {
                    plan.blockers.push(format!(
                        "{}:{}: {route_reason}",
                        source.file_path,
                        edge.line.unwrap_or(source.start_line) + 1
                    ));
                }
                let Some(text) = self
                    .rename_file(
                        &mut files,
                        &mut plan,
                        &source.file_path,
                        &old_name,
                        root_override,
                    )
                    .await
                else {
                    continue;
                };
                let line = edge.line.unwrap_or(source.start_line);
                if route_reference {
                    plan.sites.push(make_site(
                        text,
                        &source.file_path,
                        None,
                        line,
                        RenameConfidence::Heuristic,
                        edge.kind.as_str().to_string(),
                        edge.resolved_by.map(ResolvedBy::as_str),
                        Some(source.qualified_name.clone()),
                        Some(route_reason.to_string()),
                    ));
                    continue;
                }
                let source_end = text.offset(source.end_line, source.end_column);
                let window_end = text
                    .line_start(line as usize + 1 + REF_LOOKAHEAD_LINES)
                    .min(source_end.max(text.line_start(line as usize + 1)));
                let mut tokens: Vec<(usize, usize)> =
                    match refs_by_key.get(&(edge.source.as_str(), line, edge.kind)) {
                        Some(matching) => matching
                            .iter()
                            .filter_map(|r| {
                                text.first_token_in(text.offset(r.line, r.column), window_end)
                            })
                            .collect(),
                        None => text.tokens_in(
                            text.line_start(line as usize),
                            text.line_start(line as usize + 1),
                        ),
                    };
                tokens.sort_unstable();
                tokens.dedup();

                let provenance = edge.resolved_by;
                let mut confidence = match provenance {
                    Some(r) if r.is_exact() && is_primary => RenameConfidence::Exact,
                    _ => RenameConfidence::Heuristic,
                };
                let mut reason = match provenance {
                    Some(r) if r.is_exact() && is_primary => None,
                    Some(r) if r.is_exact() => {
                        Some("reference to an override of the renamed method".to_string())
                    }
                    Some(r) => Some(format!("resolved by the {} fallback", r.as_str())),
                    None => Some(
                        "resolution provenance not recorded (re-index to record it)".to_string(),
                    ),
                };
                if !text.has_grammar() {
                    confidence = RenameConfidence::Heuristic;
                    reason = Some("no grammar for this file; located by text".to_string());
                }
                if tokens.len() > 1 {
                    confidence = RenameConfidence::Heuristic;
                    reason = Some(format!(
                        "{} occurrences of '{old_name}' on this line; the graph records the \
                         line of the reference, not which occurrence",
                        tokens.len()
                    ));
                }
                if tokens.is_empty() {
                    plan.sites.push(make_site(
                        text,
                        &source.file_path,
                        None,
                        line,
                        RenameConfidence::Heuristic,
                        edge.kind.as_str().to_string(),
                        provenance.map(ResolvedBy::as_str),
                        Some(source.qualified_name.clone()),
                        Some("name token not found at the recorded line".to_string()),
                    ));
                    continue;
                }
                for token in tokens {
                    plan.sites.push(make_site(
                        text,
                        &source.file_path,
                        Some(token),
                        line,
                        confidence,
                        edge.kind.as_str().to_string(),
                        provenance.map(ResolvedBy::as_str),
                        Some(source.qualified_name.clone()),
                        reason.clone(),
                    ));
                }
            }
        }

        // --- Ambiguous calls naming the symbol among their candidates. ---
        for def in &definition_ids {
            for call in self.db.get_ambiguous_calls_naming(&def.id).await? {
                if last_segment(&call.reference_name) != old_name {
                    continue;
                }
                let Some(text) = self
                    .rename_file(
                        &mut files,
                        &mut plan,
                        &call.file_path,
                        &old_name,
                        root_override,
                    )
                    .await
                else {
                    continue;
                };
                let tokens = text.tokens_in(
                    text.line_start(call.line as usize),
                    text.line_start(call.line as usize + 1),
                );
                let reason = Some(format!(
                    "the resolver could not choose between {} candidates for '{}'",
                    call.candidate_node_ids.len(),
                    call.reference_name
                ));
                let from = self
                    .db
                    .get_node_by_id(&call.from_node_id)
                    .await?
                    .map(|n| n.qualified_name);
                if tokens.is_empty() {
                    plan.sites.push(make_site(
                        text,
                        &call.file_path,
                        None,
                        call.line,
                        RenameConfidence::Ambiguous,
                        "ambiguous_call".to_string(),
                        None,
                        from.clone(),
                        reason.clone(),
                    ));
                }
                for token in tokens {
                    plan.sites.push(make_site(
                        text,
                        &call.file_path,
                        Some(token),
                        call.line,
                        RenameConfidence::Ambiguous,
                        "ambiguous_call".to_string(),
                        None,
                        from.clone(),
                        reason.clone(),
                    ));
                }
            }
        }

        // A token claimed by several sites keeps its most trusted class.
        dedup_sites(&mut plan.sites);

        // --- Text-only mentions anywhere in the indexed files. ---
        self.collect_text_only(&mut files, &mut plan, &old_name, root_override)
            .await?;

        // --- Collisions with an existing same-named symbol in scope. ---
        if let Some(new) = new_name.filter(|_| plan.blockers.is_empty()) {
            let site_files: BTreeSet<&str> = plan
                .sites
                .iter()
                .filter(|s| s.confidence != RenameConfidence::TextOnly)
                .map(|s| s.file.as_str())
                .chain(std::iter::once(target.file_path.as_str()))
                .collect();
            let reserved: BTreeSet<&str> = site_files
                .iter()
                .filter_map(|file| reserved_words(file))
                .filter(|(_, words)| words.contains(&new))
                .map(|(language, _)| language)
                .collect();
            if !reserved.is_empty() {
                plan.blockers.push(format!(
                    "new_name '{new}' is not an identifier: it is a reserved word in {}",
                    reserved.into_iter().collect::<Vec<_>>().join(", ")
                ));
            }
            for (node, role) in &definitions {
                if *role == "impl" {
                    continue;
                }
                let siblings = match &node.parent_id {
                    Some(parent) => self.db.get_children_of(parent).await?,
                    None => self.db.get_nodes_by_file(&node.file_path).await?,
                };
                if let Some(clash) = siblings.iter().find(|s| {
                    s.name == new
                        && s.id != node.id
                        && s.parent_id == node.parent_id
                        && !matches!(s.kind, NodeKind::Impl | NodeKind::AnnotationUsage)
                }) {
                    plan.blockers.push(format!(
                        "'{new}' already names {} {} in the same scope ({}:{})",
                        clash.kind.as_str(),
                        clash.qualified_name,
                        clash.file_path,
                        clash.start_line + 1
                    ));
                }
            }
        }

        plan.sites.sort_by(|a, b| {
            (&a.file, a.line, a.column, a.confidence).cmp(&(
                &b.file,
                b.line,
                b.column,
                b.confidence,
            ))
        });
        plan.sources = files.into_iter().map(|(k, v)| (k, v.source)).collect();
        Ok(plan)
    }

    /// Methods that override, or are overridden by, `target`: same-named
    /// callables in scopes linked to its scope by `implements`/`extends`
    /// edges, in either direction.
    async fn rename_overrides(&self, target: &Node) -> Result<Vec<Node>> {
        const MAX_SCOPES: usize = 64;
        if !is_callable(&target.kind) {
            return Ok(Vec::new());
        }
        let Some(parent_id) = target.parent_id.clone() else {
            return Ok(Vec::new());
        };
        let Some(parent) = self.db.get_node_by_id(&parent_id).await? else {
            return Ok(Vec::new());
        };
        if matches!(parent.kind, NodeKind::File | NodeKind::Module) {
            return Ok(Vec::new());
        }
        let kinds = [EdgeKind::Implements, EdgeKind::Extends];
        let mut seen: HashSet<String> = HashSet::from([parent_id.clone()]);
        let mut queue = vec![parent_id];
        while let Some(scope) = queue.pop() {
            if seen.len() >= MAX_SCOPES {
                break;
            }
            let mut next: Vec<String> = self
                .db
                .get_outgoing_edges(&scope, &kinds)
                .await?
                .into_iter()
                .map(|e| e.target)
                .collect();
            next.extend(
                self.db
                    .get_incoming_edges(&scope, &kinds)
                    .await?
                    .into_iter()
                    .map(|e| e.source),
            );
            for id in next {
                if seen.insert(id.clone()) {
                    queue.push(id);
                }
            }
        }
        let scopes: Vec<String> = seen
            .into_iter()
            .filter(|s| Some(s) != target.parent_id.as_ref())
            .collect();
        if scopes.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<Node> = self
            .db
            .get_children_of_many(&scopes)
            .await?
            .into_iter()
            .filter(|n| n.name == target.name && is_callable(&n.kind) && n.id != target.id)
            .collect();
        out.sort_by(|a, b| (&a.file_path, a.start_line).cmp(&(&b.file_path, b.start_line)));
        Ok(out)
    }

    /// Reads and parses `path` once per plan.
    async fn rename_file<'a>(
        &self,
        files: &'a mut HashMap<String, FileText>,
        plan: &mut RenamePlan,
        path: &str,
        name: &str,
        root_override: Option<&str>,
    ) -> Option<&'a FileText> {
        if !files.contains_key(path) {
            let (abs, rel) = self.resolve_edit_target(path, root_override);
            let source = match std::fs::read_to_string(&abs) {
                Ok(source) => source,
                Err(e) => {
                    plan.warnings
                        .push(format!("could not read {}: {e}", abs.display()));
                    return None;
                }
            };
            if root_override.is_none() {
                if let Ok(Some(record)) = self.db.get_file(path).await {
                    if record.content_hash != crate::sync::content_hash(&source) {
                        plan.blockers.push(format!(
                            "{path} changed since it was indexed; sync before renaming"
                        ));
                    }
                }
            }
            plan.abs_paths.insert(path.to_string(), abs);
            plan.index_paths.insert(path.to_string(), rel);
            files.insert(path.to_string(), FileText::new(path, source, name));
        }
        files.get(path)
    }

    /// Lists whole-word mentions of `name` in indexed files that no site
    /// already covers.
    async fn collect_text_only(
        &self,
        files: &mut HashMap<String, FileText>,
        plan: &mut RenamePlan,
        name: &str,
        root_override: Option<&str>,
    ) -> Result<()> {
        let covered: HashSet<(String, usize)> = plan
            .sites
            .iter()
            .filter_map(|s| s.byte_range.map(|(start, _)| (s.file.clone(), start)))
            .collect();
        let mut records = self.db.get_all_files().await?;
        records.sort_by(|a, b| a.path.cmp(&b.path));
        let mut listed = 0usize;
        for record in records {
            if record.kind == FileKind::Artifact && !is_doc_path(&record.path) {
                continue;
            }
            if !files.contains_key(&record.path) {
                let (abs, _) = self.resolve_edit_target(&record.path, root_override);
                let source = match std::fs::read(&abs) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::NotFound {
                            plan.unscanned
                                .push(format!("{} (unreadable: {e})", record.path));
                        }
                        continue;
                    }
                };
                // A cheap byte search first: most files never mention the
                // name, and only those that do are decoded and parsed.
                if !contains_bytes(&source, name.as_bytes()) {
                    continue;
                }
                let Ok(source) = String::from_utf8(source) else {
                    plan.unscanned.push(format!("{} (not UTF-8)", record.path));
                    continue;
                };
                if source.len() as u64 > TEXT_SCAN_MAX_BYTES {
                    plan.unscanned.push(format!(
                        "{} ({} bytes, over the {TEXT_SCAN_MAX_BYTES}-byte scan limit)",
                        record.path,
                        source.len()
                    ));
                    continue;
                }
                plan.abs_paths.insert(record.path.clone(), abs);
                files.insert(
                    record.path.clone(),
                    FileText::new(&record.path, source, name),
                );
            }
            let Some(text) = files.get(&record.path) else {
                continue;
            };
            for start in word_occurrences(&text.source, name) {
                if covered.contains(&(record.path.clone(), start)) {
                    continue;
                }
                let end = start + name.len();
                let context = text.mention_context(start, end);
                // Past the cap a mention is counted rather than listed, but
                // an unlinked identifier still gates an apply.
                if listed >= TEXT_ONLY_LIST_CAP {
                    plan.text_only_omitted += 1;
                    if context == "code" {
                        plan.unlinked_code_omitted += 1;
                    }
                    continue;
                }
                listed += 1;
                let (row, _) = text.position(start);
                plan.sites.push(make_site(
                    text,
                    &record.path,
                    Some((start, end)),
                    row as u32,
                    RenameConfidence::TextOnly,
                    context.to_string(),
                    None,
                    None,
                    Some(match context {
                        "code" => "identifier not linked to this symbol by the graph: a \
                                   missed reference, or a different symbol with the same name"
                            .to_string(),
                        _ => format!("mention in {context}; review by hand"),
                    }),
                ));
            }
        }
        Ok(())
    }

    /// Applies `plan`: all-or-nothing, after a parse check of every edited
    /// file.
    ///
    /// # Errors
    /// Returns an error only for an I/O failure that could not be rolled
    /// back; refusals are reported in the outcome.
    pub async fn apply_rename(
        &self,
        plan: &RenamePlan,
        allow_heuristic: bool,
    ) -> Result<RenameOutcome> {
        let mut outcome = RenameOutcome {
            applied: false,
            refused: None,
            blocking_sites: Vec::new(),
            files_changed: Vec::new(),
            skipped: Vec::new(),
            warnings: Vec::new(),
        };
        if !plan.blockers.is_empty() {
            outcome.refused = Some(plan.blockers.join("; "));
            return Ok(outcome);
        }
        if plan.new_name.is_none() {
            outcome.refused = Some("new_name is required to apply a rename".to_string());
            return Ok(outcome);
        }
        let non_exact = plan.non_exact_sites();
        let unchecked = plan.unchecked_reasons();
        if !allow_heuristic && (!non_exact.is_empty() || !unchecked.is_empty()) {
            let mut reasons = Vec::new();
            if !non_exact.is_empty() || plan.unlinked_code_omitted > 0 {
                reasons.push(format!(
                    "{} site(s) are not exact (heuristic, ambiguous, or an unlinked identifier \
                     in code)",
                    non_exact.len() + plan.unlinked_code_omitted
                ));
            }
            reasons.extend(unchecked.into_iter().filter(|r| !r.starts_with("unlisted")));
            outcome.refused = Some(format!(
                "{}. Review them; pass allow_heuristic=true to edit the heuristic sites too. \
                 Ambiguous and text-only sites are never edited.",
                reasons.join("; ")
            ));
            outcome.blocking_sites = non_exact.into_iter().take(50).cloned().collect();
            return Ok(outcome);
        }
        outcome.skipped = plan
            .sites
            .iter()
            .filter(|s| {
                matches!(
                    s.confidence,
                    RenameConfidence::Heuristic | RenameConfidence::Ambiguous
                ) && !(allow_heuristic
                    && s.confidence == RenameConfidence::Heuristic
                    && s.byte_range.is_some())
            })
            .cloned()
            .collect();

        let Some(edited) = plan.edited_sources(allow_heuristic) else {
            outcome.refused = Some("new_name is required to apply a rename".to_string());
            return Ok(outcome);
        };
        if edited.is_empty() {
            outcome.refused = Some("no editable site was found".to_string());
            return Ok(outcome);
        }

        match commit_edits(&edited, &plan.sources, &plan.abs_paths)? {
            Ok(()) => {}
            Err(refusal) => {
                outcome.refused = Some(refusal);
                return Ok(outcome);
            }
        }

        outcome.applied = true;
        let mut reindex: Vec<String> = Vec::new();
        for (file, (_, count)) in &edited {
            outcome.files_changed.push((file.clone(), *count));
            if let Some(Some(rel)) = plan.index_paths.get(file) {
                reindex.push(rel.clone());
            }
        }
        // All edited files in one pass, then one resolution over what they
        // touched: the path `sync` takes for changed files. Re-indexing them
        // one at a time (`reindex_file`) inserted the callers before the
        // renamed definition existed and resolved nothing, so the callers'
        // edges were lost, and a later `sync` saw matching hashes and never
        // brought them back.
        if !reindex.is_empty() {
            if let Err(e) = self.sync_single_files(&reindex).await {
                outcome.warnings.push(format!(
                    "the files were written but reindexing them failed ({e}); run `tokensave \
                     sync --force` to rebuild the graph"
                ));
            }
        }
        Ok(outcome)
    }
}

/// Writes every edited file, or none of them.
///
/// Each file must first parse with tree-sitter with no more error nodes than
/// it had before the edit; one that does not, or that has no grammar to
/// check it with, refuses the whole set. Files are then written through a
/// temporary sibling and renamed into place, and if any write fails the
/// files already written are restored.
///
/// Returns `Ok(Err(reason))` for a refusal or a rolled-back write, and
/// `Err` only when a rollback itself failed.
fn commit_edits(
    edited: &BTreeMap<String, (String, usize)>,
    originals: &HashMap<String, String>,
    abs_paths: &HashMap<String, PathBuf>,
) -> Result<std::result::Result<(), String>> {
    let mut broken: Vec<String> = Vec::new();
    for (file, (new_source, _)) in edited {
        let Some(language) = crate::extraction::ts_provider::language_for_path(file) else {
            broken.push(format!(
                "{file}: no grammar to verify the edit with tree-sitter"
            ));
            continue;
        };
        let old_source = originals.get(file).map_or("", String::as_str);
        let before = parse(&language, old_source).map_or(usize::MAX, |t| error_count(&t));
        let after = parse(&language, new_source).map_or(usize::MAX, |t| error_count(&t));
        if after > before {
            broken.push(format!(
                "{file}: the edit introduces {} syntax error(s)",
                after.saturating_sub(before).max(1)
            ));
        }
    }
    if !broken.is_empty() {
        return Ok(Err(format!(
            "nothing was written; the edited files would not parse cleanly: {}",
            broken.join("; ")
        )));
    }

    let mut written: Vec<&String> = Vec::new();
    let mut failure: Option<String> = None;
    for (file, (new_source, _)) in edited {
        let Some(abs) = abs_paths.get(file) else {
            failure = Some(format!("{file}: path not resolved"));
            break;
        };
        if let Err(e) = write_atomically(abs, new_source) {
            failure = Some(format!("{}: {e}", abs.display()));
            break;
        }
        written.push(file);
    }
    let Some(failure) = failure else {
        return Ok(Ok(()));
    };
    let mut unrestored = Vec::new();
    for file in written {
        let (Some(abs), Some(original)) = (abs_paths.get(file), originals.get(file)) else {
            continue;
        };
        if write_atomically(abs, original).is_err() {
            unrestored.push(abs.display().to_string());
        }
    }
    if !unrestored.is_empty() {
        return Err(TokenSaveError::Config {
            message: format!(
                "rename failed ({failure}) and these files could not be restored: {}",
                unrestored.join(", ")
            ),
        });
    }
    Ok(Err(format!("write failed, all files restored: {failure}")))
}

/// Whether `needle` occurs in `haystack`.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// True for documentation files worth scanning for mentions.
fn is_doc_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".md", ".markdown", ".rst", ".txt", ".adoc"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// Writes `contents` to a sibling temporary file and renames it over `path`.
///
/// A symlink is followed: the rename lands on the file it points to, and the
/// link itself is left in place. Renaming over the link would replace it with
/// a plain file and leave the real file unedited.
fn write_atomically(path: &Path, contents: &str) -> std::io::Result<()> {
    let resolved;
    let path = if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        resolved = std::fs::canonicalize(path)?;
        resolved.as_path()
    } else {
        path
    };
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{file_name}.tokensave-rename.tmp"));
    std::fs::write(&tmp, contents)?;
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[allow(clippy::too_many_arguments)]
fn make_site(
    text: &FileText,
    file: &str,
    token: Option<(usize, usize)>,
    fallback_row: u32,
    confidence: RenameConfidence,
    kind: String,
    resolved_by: Option<&'static str>,
    from: Option<String>,
    reason: Option<String>,
) -> RenameSite {
    let (line, column, end_column) = match token {
        Some((start, end)) => {
            let (row, col) = text.position(start);
            (
                row as u32 + 1,
                col as u32 + 1,
                (col + end - start) as u32 + 1,
            )
        }
        None => (fallback_row + 1, 0, 0),
    };
    RenameSite {
        file: file.to_string(),
        line,
        column,
        end_column,
        confidence,
        kind,
        resolved_by,
        from,
        reason,
        byte_range: token,
    }
}

/// Collapses sites sharing a token to the most trusted one. Unlocated sites
/// are kept as they are.
fn dedup_sites(sites: &mut Vec<RenameSite>) {
    sites.sort_by(|a, b| {
        (&a.file, a.byte_range, a.confidence).cmp(&(&b.file, b.byte_range, b.confidence))
    });
    sites.dedup_by(|later, earlier| {
        later.byte_range.is_some()
            && later.file == earlier.file
            && later.byte_range == earlier.byte_range
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn validate_identifier_accepts_and_rejects() {
        assert!(validate_identifier("compute_total").is_ok());
        assert!(validate_identifier("_x9").is_ok());
        assert!(validate_identifier("$el").is_ok());
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("9lives").is_err());
        assert!(validate_identifier("a-b").is_err());
        assert!(validate_identifier("a b").is_err());
        assert!(validate_identifier("a::b").is_err());
    }

    #[test]
    fn last_segment_takes_the_trailing_identifier() {
        assert_eq!(last_segment("crate::utils::helper"), "helper");
        assert_eq!(last_segment("self.run"), "run");
        assert_eq!(last_segment("helper"), "helper");
        assert_eq!(last_segment("Type::step()::method"), "method");
        assert_eq!(last_segment("obj->method"), "method");
    }

    #[test]
    fn word_occurrences_respects_word_boundaries() {
        let src = "foo foo_bar barfoo foo(foo)";
        assert_eq!(word_occurrences(src, "foo"), vec![0, 19, 23]);
    }

    #[test]
    fn code_tokens_skip_comments_and_strings() {
        let src = "// helper here\nfn helper() { let s = \"helper\"; helper(); }\n";
        let text = FileText::new("x.rs", src.to_string(), "helper");
        let starts: Vec<usize> = text.tokens.iter().map(|t| t.0).collect();
        assert_eq!(starts.len(), 2, "{starts:?}");
        assert_eq!(text.mention_context(3, 9), "comment");
        let string_pos = src.find("\"helper\"").unwrap() + 1;
        assert_eq!(text.mention_context(string_pos, string_pos + 6), "string");
    }

    #[test]
    fn unified_diff_shows_changed_lines_with_context() {
        let old = "a\nb\nfoo()\nc\nd\ne\nf\ng\nfoo\n";
        let new = "a\nb\nbar()\nc\nd\ne\nf\ng\nbar\n";
        let diff = unified_diff("x.rs", old, new, 1);
        assert!(diff.starts_with("--- a/x.rs\n+++ b/x.rs\n"));
        assert!(diff.contains("@@ -2,3 +2,3 @@\n b\n-foo()\n+bar()\n c\n"));
        assert!(diff.contains("-foo\n+bar\n"));
    }

    #[test]
    fn error_count_detects_new_errors() {
        let lang = crate::extraction::ts_provider::language_for_path("x.py").unwrap();
        let good = parse(&lang, "def a(self):\n    pass\n").unwrap();
        let bad = parse(&lang, "def class(self):\n    pass\n").unwrap();
        assert_eq!(error_count(&good), 0);
        assert!(error_count(&bad) > 0);
    }

    #[test]
    fn commit_edits_restores_written_files_when_a_later_write_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let a_old = "def greet():\n    pass\n";
        std::fs::write(dir.path().join("a.py"), a_old).unwrap();
        let originals: HashMap<String, String> = [
            ("a.py".to_string(), a_old.to_string()),
            (
                "b.py".to_string(),
                "def greet2():\n    greet()\n".to_string(),
            ),
        ]
        .into();
        // b.py's directory does not exist, so its write fails after a.py's
        // has already landed.
        let abs_paths: HashMap<String, PathBuf> = [
            ("a.py".to_string(), dir.path().join("a.py")),
            ("b.py".to_string(), dir.path().join("missing").join("b.py")),
        ]
        .into();
        let edited: BTreeMap<String, (String, usize)> = [
            (
                "a.py".to_string(),
                ("def salute():\n    pass\n".to_string(), 1),
            ),
            (
                "b.py".to_string(),
                ("def greet2():\n    salute()\n".to_string(), 1),
            ),
        ]
        .into();
        let refusal = commit_edits(&edited, &originals, &abs_paths)
            .unwrap()
            .unwrap_err();
        assert!(refusal.contains("all files restored"), "{refusal}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.py")).unwrap(),
            a_old
        );
    }

    #[test]
    fn commit_edits_writes_nothing_when_one_file_breaks() {
        let dir = tempfile::TempDir::new().unwrap();
        let good_old = "def greet():\n    pass\n";
        let bad_old = "def greet2():\n    greet()\n";
        std::fs::write(dir.path().join("a.py"), good_old).unwrap();
        std::fs::write(dir.path().join("b.py"), bad_old).unwrap();
        let originals: HashMap<String, String> = [
            ("a.py".to_string(), good_old.to_string()),
            ("b.py".to_string(), bad_old.to_string()),
        ]
        .into();
        let abs_paths: HashMap<String, PathBuf> = ["a.py", "b.py"]
            .iter()
            .map(|f| ((*f).to_string(), dir.path().join(f)))
            .collect();
        let edited: BTreeMap<String, (String, usize)> = [
            (
                "a.py".to_string(),
                ("def salute():\n    pass\n".to_string(), 1),
            ),
            (
                "b.py".to_string(),
                ("def class():\n    greet()\n".to_string(), 1),
            ),
        ]
        .into();
        let refusal = commit_edits(&edited, &originals, &abs_paths)
            .unwrap()
            .unwrap_err();
        assert!(refusal.contains("b.py"), "{refusal}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.py")).unwrap(),
            good_old
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("b.py")).unwrap(),
            bad_old
        );

        // Without the broken file the good edit lands.
        let mut edited = edited;
        edited.remove("b.py");
        commit_edits(&edited, &originals, &abs_paths)
            .unwrap()
            .unwrap();
        assert!(std::fs::read_to_string(dir.path().join("a.py"))
            .unwrap()
            .contains("salute"));
        assert!(!dir.path().join(".a.py.tokensave-rename.tmp").exists());
    }
}
