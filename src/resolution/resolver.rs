// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};

use rayon::prelude::*;

use crate::db::Database;
use crate::types::*;

/// Names that are too common to resolve across files reliably.
/// These are standard library types, trait methods, and ubiquitous constructors
/// that create false edges when matched by name alone.
const CROSS_FILE_BLOCKLIST: &[&str] = &[
    // Rust std types / prelude
    "Result",
    "Option",
    "String",
    "Vec",
    "Box",
    "Arc",
    "Rc",
    "Ok",
    "Err",
    "Some",
    "None",
    // Ubiquitous trait methods
    "fmt",
    "format",
    "display",
    "to_string",
    "clone",
    "clone_from",
    "default",
    "from",
    "into",
    "try_from",
    "try_into",
    "new",
    "build",
    "builder",
    "parse",
    "from_str",
    "eq",
    "ne",
    "cmp",
    "partial_cmp",
    "hash",
    "next",
    "iter",
    "into_iter",
    "drop",
    "deref",
    "deref_mut",
    "as_ref",
    "as_mut",
    "borrow",
    "borrow_mut",
    "read",
    "write",
    "flush",
    "close",
    "len",
    "is_empty",
    "contains",
    "push",
    "pop",
    "insert",
    "remove",
    "get",
    "unwrap",
    "expect",
    "map",
    "and_then",
    "or_else",
    "unwrap_or",
    // Common test/assertion names
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    // Common patterns matched across files
    "run",
    "start",
    "stop",
    "init",
    "setup",
    // Stdlib method names that collide with user-defined functions
    "status",
    "modified",
    "output",
    "exists",
    "join",
    "display",
    "to_owned",
    "collect",
    "filter",
    "find",
    "take",
    "skip",
    "count",
    "sum",
    "max",
    "min",
    "sort",
    "extend",
    "chain",
    "zip",
    "enumerate",
    "flatten",
    "open",
    "create",
    "metadata",
    "canonicalize",
    "spawn",
    "wait",
    "send",
    "recv",
    "lock",
    "try_lock",
];

/// Returns the trailing "simple" name of a possibly-qualified reference:
/// the last segment after the final `::` (Rust/C++/PHP path) or `.`
/// (Python/TS/JS/Java receiver call). `Self::watermark_band` -> `watermark_band`,
/// `obj.render_to_png` -> `render_to_png`, `plain` -> `plain`.
pub fn simple_ref_name(name: &str) -> &str {
    let after_path = name.rsplit("::").next().unwrap_or(name);
    after_path.rsplit('.').next().unwrap_or(after_path)
}

fn ruby_constant_name(node: &Node) -> &str {
    let mut name = node.qualified_name.as_str();
    while let Some(unqualified) = name
        .strip_prefix(&node.file_path)
        .and_then(|name| name.strip_prefix("::"))
    {
        name = unqualified;
    }
    name
}

fn split_ruby_receiver_call(reference_name: &str) -> Option<(&str, &str)> {
    let separators = ["&.", ".", "::"];
    let (index, separator) = separators
        .iter()
        .filter_map(|separator| {
            reference_name.rfind(separator).and_then(|index| {
                if *separator == "." && reference_name[..index].ends_with('&') {
                    None
                } else {
                    Some((index, *separator))
                }
            })
        })
        .max_by_key(|(index, _)| *index)?;
    let receiver = &reference_name[..index];
    let method_name = &reference_name[index + separator.len()..];
    (!receiver.is_empty() && !method_name.is_empty()).then_some((receiver, method_name))
}

/// Removes redundant bare-name Go call edges left beside an import-path
/// selector resolution (#153 Bug 1).
///
/// The Go extractor emits two refs for a selector call `pkg.Fn()`: the selector
/// `pkg.Fn` and a bare-name sibling `Fn`, both at the same call position. Once
/// `pkg.Fn` resolves through its in-scope import path (`go-selector-import`),
/// the sibling is redundant; left in, it falls back to a name-keyed tie-break
/// and dumps a phantom edge onto whichever same-named definition wins. Dropping
/// the sibling makes a package-qualified call contribute exactly one edge — the
/// correct one — without touching genuine bare calls or receiver-method
/// fallbacks, whose qualifier is not a known import.
fn suppress_go_selector_bare_siblings(resolved: &mut Vec<ResolvedRef>) {
    // Sites where a selector resolved by import path, keyed on the exact call
    // position plus the callee's bare name (the selector's trailing segment) —
    // precisely the identity the sibling bare-name ref carries.
    let suppressed: HashSet<(&str, &str, u32, u32, &str)> = resolved
        .iter()
        .filter(|r| r.resolved_by == "go-selector-import")
        .filter_map(|r| {
            let bare = r.original.reference_name.rsplit('.').next()?;
            Some((
                r.original.from_node_id.as_str(),
                r.original.file_path.as_str(),
                r.original.line,
                r.original.column,
                bare,
            ))
        })
        .collect();
    if suppressed.is_empty() {
        return;
    }
    // A bare-name ref (no `.`) sitting on a suppressed site is the phantom
    // sibling; everything else — including the selector ref itself — is kept.
    let keep: Vec<bool> = resolved
        .iter()
        .map(|r| {
            r.original.reference_name.contains('.')
                || !suppressed.contains(&(
                    r.original.from_node_id.as_str(),
                    r.original.file_path.as_str(),
                    r.original.line,
                    r.original.column,
                    r.original.reference_name.as_str(),
                ))
        })
        .collect();
    drop(suppressed);
    let mut idx = 0;
    resolved.retain(|_| {
        let k = keep[idx];
        idx += 1;
        k
    });
}

/// `resolved_by` tag of a `GDScript` call resolved through its receiver's type.
const GDSCRIPT_TYPED: &str = "gdscript-typed-receiver";

/// `resolved_by` for the trailing segment of a dotted receiver call
/// (`recv.method`), where nothing is known about the receiver.
const SIMPLE_NAME_MATCH: &str = "simple-name-match";

/// `resolved_by` for the trailing segment of a `::` path (`module::name`,
/// `Self::name`) whose full path matched no qualified name. Kept apart from
/// [`SIMPLE_NAME_MATCH`] (#544): a path names a declaration, while a dotted
/// receiver may be a value of any type, so the two fallbacks are not equally
/// trustworthy.
const PATH_TAIL_MATCH: &str = "path-tail-match";

/// `GDScript` is deliberately absent from [`lang_from_path`]: a `.gd` call into
/// a godot-cpp method (#269) relies on the cross-language confidence that an
/// unknown language gets, so the tag would drop those edges.
///
/// Case-insensitive, to agree with the SQL side, which selects `.gd` rows with
/// `LIKE '%.gd'` (ASCII case-insensitive in `SQLite`).
pub fn is_gdscript(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("gd"))
}

/// `resolved_by` tag of a C# call resolved through its receiver's type (#642).
const CSHARP_TYPED: &str = "csharp-typed-receiver";

/// True for a C# source path. Case-insensitive, like [`is_gdscript`].
pub fn is_csharp(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("cs"))
}

/// True when a file's `calls` refs holding `::` are typed-receiver type
/// expressions (`Type[::step]*::method`): `GDScript` (#597) and C# (#642).
pub fn has_typed_receiver_refs(path: &str) -> bool {
    is_gdscript(path) || is_csharp(path)
}

fn is_typed_receiver_tag(tag: &str) -> bool {
    tag == GDSCRIPT_TYPED || tag == CSHARP_TYPED
}

/// A call site: caller, file, line, column, and the method's bare name.
type CallSite<'r> = (&'r str, &'r str, u32, u32, &'r str);

/// Call sites where a typed-receiver ref resolved, keyed like the sibling
/// refs and ambiguity records they make redundant. The column is part of the
/// key: the typed ref and its sibling share the call node's position, while a
/// different same-named call on the same line (`given.subscribe(subscribe(1))`)
/// does not, and must keep its own edge or ambiguity record.
fn gdscript_typed_sites(resolved: &[ResolvedRef]) -> HashSet<CallSite<'_>> {
    resolved
        .iter()
        .filter(|r| is_typed_receiver_tag(&r.resolved_by))
        .map(|r| {
            (
                r.original.from_node_id.as_str(),
                r.original.file_path.as_str(),
                r.original.line,
                r.original.column,
                simple_ref_name(&r.original.reference_name),
            )
        })
        .collect()
}

/// Drops the receiver-qualified sibling of a `GDScript` call whose typed ref
/// resolved (#597).
///
/// The extractor records both `recv.method` and `Type::method` for a typed
/// receiver. Once the typed one resolves, the name-based sibling can only
/// agree with it (the same edge, collapsed by the unique index) or disagree
/// by binding a same-named method the receiver's type does not have — for
/// instance one in the caller's own file, which scoring favours. Either way
/// the typed answer is the one to keep.
fn suppress_gdscript_typed_siblings(resolved: &mut Vec<ResolvedRef>) {
    let keep: Vec<bool> = {
        let sites = gdscript_typed_sites(resolved);
        if sites.is_empty() {
            return;
        }
        resolved
            .iter()
            .map(|r| {
                is_typed_receiver_tag(&r.resolved_by)
                    || r.original.reference_kind != EdgeKind::Calls
                    || !sites.contains(&(
                        r.original.from_node_id.as_str(),
                        r.original.file_path.as_str(),
                        r.original.line,
                        r.original.column,
                        simple_ref_name(&r.original.reference_name),
                    ))
            })
            .collect()
    };
    let mut idx = 0;
    resolved.retain(|_| {
        let k = keep[idx];
        idx += 1;
        k
    });
}

/// The class a `GDScript` declaration names after `extends`, when it is a class
/// name rather than a `"res://..."` path. Read from the class node's
/// signature, which the extractor writes as `class_name X extends Y` or
/// `class X extends Y:`.
fn gdscript_extends(signature: &str) -> Option<&str> {
    let (_, rest) = signature.split_once(" extends ")?;
    let base = rest
        .trim()
        .split(|c: char| c.is_whitespace() || c == ':')
        .next()?;
    is_gdscript_ident(base).then_some(base)
}

/// The declared return type of a `GDScript` function signature
/// (`func f(...) -> T`), when it is a class name. `void` has no members.
fn gdscript_return_type(signature: &str) -> Option<&str> {
    let (_, ty) = signature.rsplit_once("->")?;
    let ty = ty.trim().trim_end_matches(':').trim();
    (is_gdscript_ident(ty) && ty != "void").then_some(ty)
}

/// The declared type of a `GDScript` member variable signature
/// (`[@annotation] var name: T [= v]`), when it is a class name. `:=` infers
/// the type from the initializer, which the signature line does not type.
fn gdscript_field_type(signature: &str) -> Option<&str> {
    let (_, rest) = signature.split_once("var ")?;
    let rest = rest.trim_start();
    let after_name = rest.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_');
    let after_colon = after_name.trim_start().strip_prefix(':')?;
    if after_colon.starts_with('=') {
        return None;
    }
    let after_colon = after_colon.trim_start();
    let end = after_colon
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(after_colon.len());
    let (ty, rest) = after_colon.split_at(end);
    // `Array[T]` and `Outer.Inner` are not a plain class lookup.
    let plain = !rest.starts_with('[') && !rest.starts_with('.');
    (plain && is_gdscript_ident(ty)).then_some(ty)
}

fn is_gdscript_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Whether `path` is an ERB or Slim template, indexed as Ruby.
fn is_ruby_template(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("erb") || ext.eq_ignore_ascii_case("slim"))
}

/// Whether a Ruby callable is something a view template can call bare.
/// See `try_ruby_template_helper_match` for the rationale.
fn is_ruby_template_helper(node: &Node) -> bool {
    if lang_from_path(&node.file_path) != "ruby" {
        return false;
    }
    match node.kind {
        NodeKind::Function => true,
        NodeKind::Method => {
            if node
                .file_path
                .split('/')
                .any(|segment| segment == "helpers")
            {
                return true;
            }
            // Qualified names start with the file path, sometimes twice.
            let mut scope = node.qualified_name.as_str();
            while let Some(rest) = scope
                .strip_prefix(node.file_path.as_str())
                .and_then(|rest| rest.strip_prefix("::"))
            {
                scope = rest;
            }
            let mut segments: Vec<&str> = scope.split("::").collect();
            segments.pop();
            segments
                .iter()
                .any(|s| s.ends_with("Helper") || *s == "ApplicationController")
        }
        _ => false,
    }
}

/// Callable node kinds a `GDScript` method lookup accepts.
fn is_gdscript_callable(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function | NodeKind::Method | NodeKind::Constructor
    )
}

/// The indexed `File` node a relative JS/TS import specifier names, if any.
fn first_indexed_candidate<'n>(
    file_nodes: &HashMap<&str, &'n Node>,
    importer: &str,
    specifier: &str,
) -> Option<&'n Node> {
    super::js_specifier::relative_module_candidates(importer, specifier)
        .iter()
        .find_map(|path| file_nodes.get(path.as_str()).copied())
}

/// Callable node kinds a C# member lookup accepts.
fn is_csharp_callable(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Method | NodeKind::Function | NodeKind::Constructor
    )
}

/// A C# type expression reduced to the class name a lookup can use: no
/// namespace or alias qualifier, type arguments, array rank or nullability.
/// `global::A.B.Writer<T>[]?` -> `Writer`. `None` for a tuple, pointer or
/// anything else that is not a plain name.
pub fn csharp_type_name(raw: &str) -> Option<&str> {
    let s = raw.trim();
    let s = s.strip_prefix("global::").unwrap_or(s);
    let end = s
        .find(|c: char| matches!(c, '<' | '[' | '(' | '?' | '*') || c.is_whitespace())
        .unwrap_or(s.len());
    let s = s[..end].rsplit(['.', ':']).next()?;
    let mut chars = s.chars();
    let ident = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_');
    (ident && s != "var").then_some(s)
}

/// The type written before declaration `name` in a C# signature: a method's
/// return type, a field's or property's type. The name is found at bracket
/// depth 0, so attribute arguments and parameter lists cannot match.
fn csharp_declared_type<'s>(signature: &'s str, name: &str) -> Option<&'s str> {
    let bytes = signature.as_bytes();
    let mut depth = 0i32;
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'<' | b'{' => depth += 1,
            b')' | b']' | b'>' | b'}' => depth -= 1,
            _ => {}
        }
        if depth != 0 || !bytes[i..].starts_with(name.as_bytes()) {
            continue;
        }
        let before_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
        let after_ok = bytes.get(i + name.len()).is_none_or(|c| {
            matches!(c, b'(' | b'<' | b';' | b'=' | b',' | b'{') || c.is_ascii_whitespace()
        });
        if before_ok && after_ok {
            return preceding_type_token(signature.get(..i)?);
        }
    }
    None
}

/// The last whitespace-separated token of `s`, keeping bracketed type
/// arguments whole (`Task<Dictionary<string, int>>`).
fn preceding_type_token(s: &str) -> Option<&str> {
    let s = s.trim_end();
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut start = bytes.len();
    for (i, &c) in bytes.iter().enumerate().rev() {
        match c {
            b')' | b']' | b'>' => depth += 1,
            b'(' | b'[' | b'<' => depth -= 1,
            _ => {}
        }
        if depth == 0 && c.is_ascii_whitespace() {
            break;
        }
        start = i;
    }
    s.get(start..).filter(|t| !t.is_empty())
}

/// The type argument of `Task<T>` / `ValueTask<T>`, the value an `await`
/// produces.
fn unwrap_task(ty: &str) -> Option<&str> {
    if !matches!(csharp_type_name(ty), Some("Task" | "ValueTask")) {
        return None;
    }
    let (_, inner) = ty.split_once('<')?;
    Some(inner.trim_end().strip_suffix('>')?.trim())
}

/// The base types named in a C# type declaration signature
/// (`class A(int x) : Base<T>, IFoo where T : new()` -> `Base`, `IFoo`).
fn csharp_bases(signature: &str) -> Vec<&str> {
    let bytes = signature.as_bytes();
    let mut depth = 0i32;
    let mut colon = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'<' | b'{' => depth += 1,
            b')' | b']' | b'>' | b'}' => depth -= 1,
            b':' if bytes.get(i + 1) == Some(&b':') => i += 1,
            b':' if depth == 0 => {
                colon = Some(i);
                break;
            }
            b'w' if depth == 0
                && bytes[i..].starts_with(b"where")
                && i > 0
                && bytes[i - 1].is_ascii_whitespace() =>
            {
                return Vec::new();
            }
            _ => {}
        }
        i += 1;
    }
    let Some(colon) = colon else {
        return Vec::new();
    };
    let rest = signature.get(colon + 1..).unwrap_or("");
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let rb = rest.as_bytes();
    for (j, &c) in rb.iter().enumerate() {
        match c {
            b'(' | b'[' | b'<' => depth += 1,
            b')' | b']' | b'>' => depth -= 1,
            _ => {}
        }
        let at_where =
            depth == 0 && rb[j..].starts_with(b"where") && j > 0 && rb[j - 1].is_ascii_whitespace();
        if depth == 0 && (c == b',' || at_where) {
            out.extend(rest.get(start..j).and_then(csharp_type_name));
            start = j + 1;
            if at_where {
                return out;
            }
        }
    }
    out.extend(rest.get(start..).and_then(csharp_type_name));
    out
}

/// Infer a coarse language tag from a file path extension.
fn lang_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => "rust",
        "go" => "go",
        "py" | "pyi" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" | "inl" | "ipp" | "tcc" => "cpp",
        "cs" => "csharp",
        "rb" | "rake" | "erb" | "slim" => "ruby",
        "php" => "php",
        "scala" | "sc" => "scala",
        "dart" => "dart",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "sh" | "bash" => "bash",
        "nix" => "nix",
        "tf" | "tfvars" => "terraform",
        "zig" => "zig",
        "proto" => "proto",
        "vhd" | "vhdl" => "vhdl",
        "v" | "vh" | "sv" | "svh" => "systemverilog",
        _ => "unknown",
    }
}

/// A `.cpp` calling into its own `.h` tags differently, and without this the match takes 0.5, under
/// the 0.6 floor in [`Resolver::resolve_all`], and the edge is dropped.
fn same_language_family(a: &str, b: &str) -> bool {
    a == b || matches!((a, b), ("c", "cpp") | ("cpp", "c"))
}

fn is_c_family(lang: &str) -> bool {
    matches!(lang, "c" | "cpp")
}

fn is_header_path(path: &str) -> bool {
    matches!(
        path.rsplit('.').next().unwrap_or(""),
        "h" | "hpp" | "hxx" | "hh" | "inl" | "ipp" | "tcc"
    )
}

/// Count shared path segments between two file paths.
fn path_proximity(a: &str, b: &str) -> i64 {
    let seg_a: Vec<&str> = a.split('/').collect();
    let seg_b: Vec<&str> = b.split('/').collect();
    let shared = seg_a
        .iter()
        .zip(seg_b.iter())
        .take_while(|(x, y)| x == y)
        .count();
    // +5 per shared segment, capped at +40
    (shared as i64 * 5).min(40)
}

/// True if Go source file `file_path` belongs to the package that import path
/// `import_path` points at.
///
/// A Go import path's trailing segments name the package directory: import
/// `example.com/m/internal/foo/jobs` is satisfied by any `.go` file directly
/// under `internal/foo/jobs`. We compare the file's directory segments against
/// the import path's trailing segments (the file's dir must be a suffix of the
/// import path), so a single-module repo whose paths are relative to the module
/// root matches without needing the module prefix.
fn go_file_in_package(file_path: &str, import_path: &str) -> bool {
    // Directory of the candidate file (drop the file name). A file at module
    // root matches only a bare-package import (no slash).
    let Some((dir, _)) = file_path.rsplit_once('/') else {
        return !import_path.contains('/');
    };
    let dir_segs: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let imp_segs: Vec<&str> = import_path.split('/').filter(|s| !s.is_empty()).collect();
    if dir_segs.is_empty() || dir_segs.len() > imp_segs.len() {
        return false;
    }
    // The file's directory segments must be a suffix of the import path.
    dir_segs
        .iter()
        .rev()
        .zip(imp_segs.iter().rev())
        .all(|(d, i)| d == i)
}

/// Resolves unresolved references into concrete edges by matching them against
/// known nodes loaded from the database.
///
/// Caches are built once at construction time by loading all nodes from the
/// database and indexing them by `name` and `qualified_name`.
pub struct ReferenceResolver<'a> {
    #[allow(dead_code)]
    db: &'a Database,
    /// Nodes grouped by their short name.
    ///
    /// Keys and values borrow from the caller's node slice rather than owning
    /// copies: on a large graph the previous `HashMap<String, Vec<Node>>`
    /// held a second full copy of every node, and the qualified-name cache a
    /// third, which is what drove `serve` to multi-GiB RSS peaks (#253).
    name_cache: HashMap<&'a str, Vec<&'a Node>>,
    /// Nodes grouped by their qualified name.
    qualified_name_cache: HashMap<&'a str, Vec<&'a Node>>,
    /// Nodes keyed by their stable graph ID.
    node_id_cache: HashMap<&'a str, &'a Node>,
    /// Ruby constant bindings keyed by their exact lexical path.
    ruby_constant_bindings: HashMap<&'a str, Vec<&'a Node>>,
    /// Suffix index: maps every `::suffix` of a qualified name to the full
    /// qualified name(s). Enables O(1) suffix lookups instead of scanning
    /// the entire `qualified_name_cache`. Both sides borrow from the nodes'
    /// `qualified_name` strings — a deep path such as `a::b::c::d` previously
    /// allocated one full copy of the name per `::` segment (#253).
    suffix_cache: HashMap<&'a str, Vec<&'a str>>,
    /// All known symbol names (short + qualified + suffixes) for pre-filtering.
    known_names: HashSet<&'a str>,
    /// Maps `file_path` to the set of qualified names imported by that file.
    /// Built from Use nodes. Used to prefer candidates that the caller imports.
    import_index: HashMap<String, HashSet<String>>,
    /// Maps `file_path` to that Go file's in-scope import qualifiers
    /// (`qualifier` -> full import path). Built from Go Use nodes. Used to
    /// disambiguate a selector call `qualifier.Name` to the package directory
    /// the qualifier refers to, so same-named packages don't collide (#149
    /// Bug 1).
    go_import_qualifiers: HashMap<String, HashMap<String, String>>,
    /// `File` nodes keyed by their path, for binding a relative JS/TS import
    /// specifier to the file it names (#647).
    file_nodes: HashMap<&'a str, &'a Node>,
}

/// References paired with their position in the slice `resolve_all` was given,
/// so a failure can be reported by index rather than by an owned copy (#483).
type IndexedRefs<'r> = Vec<(usize, &'r UnresolvedRef)>;

impl<'a> ReferenceResolver<'a> {
    /// Creates a resolver from pre-loaded nodes.
    pub fn from_nodes(db: &'a Database, all_nodes: &'a [Node]) -> Self {
        let mut name_cache: HashMap<&'a str, Vec<&'a Node>> = HashMap::new();
        let mut qualified_name_cache: HashMap<&'a str, Vec<&'a Node>> = HashMap::new();
        let mut node_id_cache: HashMap<&'a str, &'a Node> = HashMap::new();
        let mut ruby_constant_bindings: HashMap<&'a str, Vec<&'a Node>> = HashMap::new();
        let mut suffix_cache: HashMap<&'a str, Vec<&'a str>> = HashMap::new();
        let mut file_nodes: HashMap<&'a str, &'a Node> = HashMap::new();

        for node in all_nodes {
            node_id_cache.insert(node.id.as_str(), node);
            if node.kind == NodeKind::File {
                file_nodes.insert(node.file_path.as_str(), node);
            }
            // Skip Use nodes — they represent import statements, not definitions.
            // Including them causes false cross-file edges when two files share
            // the same `use std::path::Path` import.
            if node.kind == NodeKind::Use {
                continue;
            }
            name_cache.entry(node.name.as_str()).or_default().push(node);
            let qn = node.qualified_name.as_str();
            qualified_name_cache.entry(qn).or_default().push(node);
            // Build suffix index: for "a::b::c", index "b::c" and "c"
            // (but not the full name — that's in qualified_name_cache already)
            let mut pos = 0;
            while let Some(idx) = qn[pos..].find("::") {
                let suffix = &qn[pos + idx + 2..];
                if !suffix.is_empty() {
                    suffix_cache.entry(suffix).or_default().push(qn);
                }
                pos += idx + 2;
            }

            if lang_from_path(&node.file_path) == "ruby"
                && matches!(
                    node.kind,
                    NodeKind::Class | NodeKind::Module | NodeKind::Const
                )
            {
                let constant_name = ruby_constant_name(node);
                ruby_constant_bindings
                    .entry(constant_name)
                    .or_default()
                    .push(node);
            }
        }

        // Deduplicate suffix entries
        for entries in suffix_cache.values_mut() {
            entries.sort_unstable();
            entries.dedup();
        }

        // Build known_names set for pre-filtering unresolvable refs. Borrows
        // the map keys rather than cloning every one of them (#253).
        let mut known_names: HashSet<&'a str> = HashSet::new();
        known_names.extend(name_cache.keys().copied());
        known_names.extend(qualified_name_cache.keys().copied());
        known_names.extend(suffix_cache.keys().copied());

        // Build import index: for each Use node, record which qualified names
        // the file imports. The Use node's `name` is the import path (e.g.
        // "crate::types::*", "std::path::Path"). We index the last segment.
        let mut import_index: HashMap<String, HashSet<String>> = HashMap::new();
        for node in all_nodes {
            if node.kind == NodeKind::Use {
                // The name field contains the full use path.
                // Extract the imported name (last segment after ::).
                let imported = node.name.rsplit("::").next().unwrap_or(&node.name);
                if imported != "*" {
                    import_index
                        .entry(node.file_path.clone())
                        .or_default()
                        .insert(imported.to_string());
                }
                // A relative JS/TS import names a file, not a symbol. Record
                // the file it resolves to (by its path, which cannot collide
                // with an identifier) so the reachability gate sees that the
                // importer can reach that file's exports (#647).
                if let Some(target) =
                    first_indexed_candidate(&file_nodes, &node.file_path, &node.name)
                {
                    import_index
                        .entry(node.file_path.clone())
                        .or_default()
                        .insert(target.file_path.clone());
                }
            }
        }

        // Build the Go selector-qualifier map: for each Go Use node, record the
        // in-scope qualifier (alias, or the package identifier derived from the
        // import path — handling `/vN` versioned paths) -> the full import path.
        let mut go_import_qualifiers: HashMap<String, HashMap<String, String>> = HashMap::new();
        for node in all_nodes {
            if node.kind != NodeKind::Use || lang_from_path(&node.file_path) != "go" {
                continue;
            }
            // A Go Use node `name` is `<path>` or `<path> as <alias>`.
            let path = node
                .name
                .split_once(" as ")
                .map_or(node.name.as_str(), |(p, _)| p)
                .trim();
            let Some(qualifier) = crate::go_import::import_identifier(&node.name) else {
                continue;
            };
            // Blank (`_`) / dot (`.`) imports derive no usable qualifier — skip.
            if qualifier == "_" || qualifier == "." {
                continue;
            }
            go_import_qualifiers
                .entry(node.file_path.clone())
                .or_default()
                .insert(qualifier, path.to_string());
        }

        Self {
            db,
            name_cache,
            qualified_name_cache,
            node_id_cache,
            ruby_constant_bindings,
            suffix_cache,
            known_names,
            import_index,
            go_import_qualifiers,
            file_nodes,
        }
    }

    /// Attempts to resolve a single unresolved reference.
    ///
    /// Resolution strategies are tried in order:
    /// 1. **Qualified name match** -- if the reference contains `::`, try
    ///    matching against qualified names of known nodes (confidence 0.95).
    /// 2. **Exact name match** -- look up the reference name in the name cache.
    ///    A single match yields confidence 0.9; multiple matches are scored via
    ///    `find_best_matches`; a lone winner gets confidence 0.7, a tie none.
    ///
    /// Returns `None` if no strategy can resolve the reference.
    pub fn resolve_one(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        // Skip `Uses` edges whose reference name is a stdlib, external crate,
        // or wildcard import path. These create false cross-file edges when
        // two files both `use std::path::Path` — the resolver matches the name
        // against nodes in the other file instead of recognizing it as a shared
        // external import.
        if uref.reference_kind == EdgeKind::Uses {
            let name = &uref.reference_name;
            if name.starts_with("std::")
                || name.starts_with("core::")
                || name.starts_with("alloc::")
                || name.starts_with("serde")
                || name.starts_with("tokio::")
                || name.starts_with("rayon::")
                || name.starts_with("clap::")
                || name.starts_with("glob::")
                || name.starts_with("libsql::")
                || name.starts_with("sha2::")
                || name.starts_with("tree_sitter::")
                || name.starts_with("serde_json::")
                || name.starts_with("toml::")
                || name.starts_with("tempfile::")
                || name.starts_with("dirs::")
                || name.starts_with("bincode::")
                || name.contains("::*")
            {
                return None;
            }
        }

        // A relative JS/TS import specifier names a file. Bind it to that
        // file's `File` node, applying TypeScript's `.js` -> `.ts` mapping, or
        // to nothing: its trailing dotted segment (`js` in `./hash.js`) is not
        // a symbol name, so the name-based strategies below can only produce a
        // phantom edge (#647).
        if uref.reference_kind == EdgeKind::Uses
            && super::js_specifier::is_js_family_importer(&uref.file_path)
            && super::js_specifier::is_relative_specifier(&uref.reference_name)
        {
            return first_indexed_candidate(
                &self.file_nodes,
                &uref.file_path,
                &uref.reference_name,
            )
            .map(|target| ResolvedRef {
                original: uref.clone(),
                target_node_id: target.id.clone(),
                confidence: 0.95,
                resolved_by: ResolvedBy::RelativeImport.as_str().to_string(),
            });
        }

        // GDScript typed-receiver calls (#597) carry the receiver's static
        // type, so they resolve through that class or not at all. The
        // receiver-qualified `recv.method` ref the extractor records at the
        // same site keeps today's name-based behaviour for everything else.
        if uref.reference_kind == EdgeKind::Calls
            && is_gdscript(&uref.file_path)
            && uref.reference_name.contains("::")
        {
            return self.try_gdscript_typed_match(uref);
        }

        // C# typed-receiver calls (#642), same contract as GDScript's.
        if uref.reference_kind == EdgeKind::Calls
            && is_csharp(&uref.file_path)
            && uref.reference_name.contains("::")
        {
            return self.try_csharp_typed_match(uref);
        }

        // Ruby receiver-qualified calls use only positive receiver and
        // singleton-definition evidence. Unsupported or ambiguous shapes stay
        // unresolved instead of falling back to the trailing method name.
        if uref.reference_kind == EdgeKind::Calls
            && lang_from_path(&uref.file_path) == "ruby"
            && (uref.reference_name.contains('.') || uref.reference_name.contains("::"))
        {
            return self.try_ruby_receiver_match(uref);
        }

        // Terraform `Uses` references are canonical declaration addresses, not
        // receiver calls. Preserve the whole dotted name so `var.region` cannot
        // fall back to an unrelated `region` attribute.
        if uref.reference_kind == EdgeKind::Uses
            && lang_from_path(&uref.file_path) == "terraform"
            && uref.reference_name.contains('.')
        {
            return self.try_exact_name_match(uref);
        }

        // Strategy 1: qualified name match (`::`-separated paths, e.g. Rust's
        // `Type::method`, `Self::method`, C++ `Class::method`, PHP `A::b`).
        if uref.reference_name.contains("::") {
            if let Some(resolved) = self.try_qualified_match(uref) {
                return Some(resolved);
            }
            // Fall through to try exact name match with the simple name
            let simple_name = uref
                .reference_name
                .rsplit("::")
                .next()
                .unwrap_or(&uref.reference_name);
            if let Some(resolved) =
                self.try_exact_name_match_simple(uref, simple_name, false, PATH_TAIL_MATCH)
            {
                return Some(resolved);
            }
            return None;
        }

        // Strategy 1b: dotted receiver call (`recv.method`). The Python / TS /
        // JS extractors emit the full callee text (`obj.method`) with no
        // separate bare-name ref, so a method call never resolves without this
        // fallback to the trailing segment. (Rust/Go already emit a bare-name
        // ref alongside, so this is harmless there — the duplicate edge is
        // collapsed by the unique edge index.)
        if uref.reference_name.contains('.') {
            // Go selector disambiguation (#149 Bug 1): if the leading qualifier
            // is a known import qualifier in this file, resolve `qualifier.Name`
            // against the package directory that import points at. This keeps
            // same-named packages (`internal/foo/jobs`, `internal/bar/jobs`,
            // both `package jobs`) from collapsing onto a single name-keyed
            // target. A qualifier that is NOT a known import is a receiver
            // variable for a method call; that falls through to the bare-name
            // behavior below, unchanged.
            if let Some(resolved) = self.try_go_selector_match(uref) {
                return Some(resolved);
            }
            let simple_name = uref
                .reference_name
                .rsplit('.')
                .next()
                .unwrap_or(&uref.reference_name);
            // Only a call has a receiver; a dotted base type or type reference
            // (`App.Data.IProducer`) is a namespace path and may name a type in
            // the referrer's own scope (#643).
            if simple_name != uref.reference_name
                && uref.reference_kind == EdgeKind::Calls
                && is_csharp(&uref.file_path)
            {
                return self.try_csharp_receiver_fallback(uref, simple_name);
            }
            if simple_name != uref.reference_name {
                if let Some(resolved) =
                    self.try_exact_name_match_simple(uref, simple_name, true, SIMPLE_NAME_MATCH)
                {
                    return Some(resolved);
                }
            }
            return None;
        }

        // A bare name in an ERB/Slim template binds to view helpers only.
        if uref.reference_kind == EdgeKind::Calls && is_ruby_template(&uref.file_path) {
            return self.try_ruby_template_helper_match(uref);
        }

        // Strategy 2: exact name match
        self.try_exact_name_match(uref)
    }

    /// Resolves a bare call in an ERB or Slim template.
    ///
    /// A template's bare names are mostly not calls the template source can
    /// prove: partial locals (`render "row", item: x` makes `item` a local the
    /// partial never binds), controller-assigned variables and the view
    /// context's own methods. Matching them by name alone binds a partial's
    /// `item` to whatever `def item` the project happens to have. A view
    /// calls helpers, so only helper-shaped targets are candidates: a method
    /// in a `helpers/` directory (Rails' `app/helpers`, engines included), a
    /// method of a `*Helper` module, a method of `ApplicationController`
    /// (where `helper_method` exposures conventionally live), a top-level
    /// `def` (a private method of `Object`, callable everywhere), or a method
    /// defined in the template itself.
    fn try_ruby_template_helper_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        let blocklisted = CROSS_FILE_BLOCKLIST.contains(&uref.reference_name.as_str());
        let candidates: Vec<&Node> = self
            .name_cache
            .get(uref.reference_name.as_str())?
            .iter()
            .copied()
            .filter(|n| kind_compatible(uref, &n.kind))
            .filter(|n| {
                if n.file_path == uref.file_path {
                    return true;
                }
                !blocklisted && is_ruby_template_helper(n)
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        resolve_from_filtered_named(
            uref,
            &candidates,
            "ruby-template-helper",
            &self.import_index,
            false,
            &self.node_id_cache,
        )
    }

    /// Returns true if a reference name could plausibly resolve to a known symbol.
    fn is_known_name(&self, name: &str) -> bool {
        self.known_names.contains(name)
    }

    /// Whether `uref` is a relative JS/TS import whose specifier names an
    /// indexed file. Such a ref is never a known *name* (#647).
    fn is_relative_import(&self, uref: &UnresolvedRef) -> bool {
        uref.reference_kind == EdgeKind::Uses
            && first_indexed_candidate(&self.file_nodes, &uref.file_path, &uref.reference_name)
                .is_some()
    }

    /// Every key the name pre-filter admits, for the equivalence test that
    /// pins `resolution::touched::index_keys` to this index's real shape (#484).
    pub fn known_names(&self) -> &HashSet<&'a str> {
        &self.known_names
    }

    /// Resolves a batch of unresolved references in parallel, returning a
    /// summary of the results.
    ///
    /// Pre-filters references whose name doesn't exist in the graph at all,
    /// turning hopeless lookups into O(1) hash checks.
    pub fn resolve_all(&self, refs: &[UnresolvedRef]) -> ResolutionResult {
        let total = refs.len();
        let (mut resolved, mut ambiguous, unresolved) = self.resolve_batch_inner(refs);
        self.finalize_ambiguous(&resolved, &mut ambiguous);
        self.finalize_resolved(&mut resolved);
        let resolved_count = resolved.len();
        ResolutionResult {
            resolved,
            unresolved,
            total,
            resolved_count,
            ambiguous,
        }
    }

    /// Resolve one batch of references, without the cross-batch finishing step.
    ///
    /// For callers streaming the reference table rather than materialising it
    /// (#482). Every reference resolves independently against the whole index
    /// — `resolve_batch_inner` is a `par_iter().map(resolve_one)` — so the
    /// index stays global and only the *input* is chunked. That is why this is
    /// safe where chunking the node slice is not: a chunked name index would
    /// silently lose targets defined outside the chunk, but a chunked input
    /// cannot lose anything.
    ///
    /// The caller must run [`Self::finalize_resolved`] once over the
    /// accumulated results before creating edges.
    pub fn resolve_batch(&self, refs: &[UnresolvedRef]) -> (Vec<ResolvedRef>, Vec<AmbiguousCall>) {
        let (resolved, ambiguous, _) = self.resolve_batch_inner(refs);
        (resolved, ambiguous)
    }

    /// The one step that cannot be done per batch.
    ///
    /// A Go selector call emits both a selector ref and a bare-name sibling at
    /// the same site; once the selector resolves via its import path the
    /// sibling only adds a phantom name-tie edge (#153 Bug 1). Both members of
    /// such a pair come from the same call site and so from the same file, but
    /// nothing guarantees they land in the same batch, so this runs once over
    /// the accumulated set.
    pub fn finalize_resolved(&self, resolved: &mut Vec<ResolvedRef>) {
        suppress_go_selector_bare_siblings(resolved);
        suppress_gdscript_typed_siblings(resolved);
    }

    /// The ambiguity half of [`Self::finalize_resolved`], run once over the
    /// accumulated results for the same reason.
    ///
    /// A `GDScript` call whose typed ref resolved is not ambiguous, even though
    /// its receiver-qualified sibling tied on the bare method name (#597).
    /// Left in, the record would list the losing same-named methods as
    /// candidates, and `dead_code` treats an ambiguity candidate as referenced.
    pub fn finalize_ambiguous(&self, resolved: &[ResolvedRef], ambiguous: &mut Vec<AmbiguousCall>) {
        let sites = gdscript_typed_sites(resolved);
        if sites.is_empty() {
            return;
        }
        ambiguous.retain(|a| {
            !sites.contains(&(
                a.from_node_id.as_str(),
                a.file_path.as_str(),
                a.line,
                a.column,
                simple_ref_name(&a.reference_name),
            ))
        });
    }

    /// Resolve `refs`, returning the resolved edges, the ambiguity records, and
    /// the input positions that did not resolve.
    ///
    /// Ambiguity is derived before the Go suppression, as it always was: a
    /// suppressed sibling is a resolution that is deliberately dropped, not a
    /// failure to explain.
    fn resolve_batch_inner(
        &self,
        refs: &[UnresolvedRef],
    ) -> (Vec<ResolvedRef>, Vec<AmbiguousCall>, Vec<u32>) {
        // Partition into resolvable (name exists in graph) and hopeless.
        //
        // A qualified/dotted ref (`Self::method`, `Type::method`, `obj.method`)
        // rarely matches a known name *verbatim* — `Self::watermark_band` is
        // not a node name, qualified name, or suffix — so the literal-name
        // check alone dropped every such ref into `hopeless` before
        // `resolve_one` (which strips the prefix and matches the simple name)
        // ever ran. That silently lost all `Self::`/`Type::` and Python/TS
        // dotted-method call edges (#141). Also admit a ref when its trailing
        // simple name is known.
        //
        // Carried with their input positions, so a reference that fails can be
        // reported by index instead of by an owned copy (#483).
        let (candidates, hopeless): (IndexedRefs<'_>, IndexedRefs<'_>) =
            refs.iter().enumerate().partition(|(_, uref)| {
                self.is_known_name(&uref.reference_name)
                    || self.is_known_name(simple_ref_name(&uref.reference_name))
                    || self.is_relative_import(uref)
            });

        let results: Vec<_> = candidates
            .par_iter()
            .map(|(i, uref)| (*i, *uref, self.resolve_one(uref)))
            .collect();

        let mut resolved = Vec::new();
        // Borrowed, not cloned. This used to build a `Vec<UnresolvedRef>` by
        // cloning every reference that failed — ~160,000 owned records per
        // sync on tokensave's own tree, several `String`s each, to populate a
        // field nothing in the product reads (#483).
        let mut failed: IndexedRefs<'_> = hopeless;
        for (i, uref, res) in results {
            match res {
                Some(r) if r.confidence >= 0.6 => resolved.push(r),
                Some(_) | None => failed.push((i, uref)), // below confidence floor or unresolved
            }
        }

        // Why each remaining ref failed, where the reason was a tie (#412).
        let ambiguous: Vec<AmbiguousCall> = failed
            .iter()
            .filter_map(|(_, uref)| self.explain_ambiguity(uref))
            .collect();

        // Input order, which the old field was not: it listed the references
        // rejected by the name pre-filter first, then those that failed
        // resolution, so its order depended on the partition rather than on
        // the caller's slice.
        let mut unresolved: Vec<u32> = failed
            .iter()
            .map(|(i, _)| u32::try_from(*i).unwrap_or(u32::MAX))
            .collect();
        unresolved.sort_unstable();

        (resolved, ambiguous, unresolved)
    }

    /// Converts a slice of resolved references into graph edges.
    ///
    /// Duplicates (same source, target, kind and line) collapse to the one
    /// with the strongest provenance, which is the row the unique edge index
    /// then keeps (#544).
    pub fn create_edges(&self, resolved: &[ResolvedRef]) -> Vec<Edge> {
        let mut edges: Vec<Edge> = resolved
            .iter()
            .map(|r| Edge {
                source: r.original.from_node_id.clone(),
                target: r.target_node_id.clone(),
                kind: self.edge_kind_for(r),
                line: Some(r.original.line),
                resolved_by: ResolvedBy::from_name(&r.resolved_by),
            })
            .collect();
        edges.sort_unstable_by(|a, b| {
            (
                &a.source,
                &a.target,
                a.kind.as_str(),
                &a.line,
                a.provenance_key(),
            )
                .cmp(&(
                    &b.source,
                    &b.target,
                    b.kind.as_str(),
                    &b.line,
                    b.provenance_key(),
                ))
        });
        edges.dedup_by(|a, b| {
            a.source == b.source && a.target == b.target && a.kind == b.kind && a.line == b.line
        });
        edges
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// The edge kind to store for a resolved reference.
    ///
    /// A C# base list (`class A : X, IY`) cannot syntactically distinguish a
    /// base class from an interface, so the extractor records a class's
    /// first base as `Extends`. Once the target is known, an `Extends` that
    /// lands on an interface is really an `Implements`, which is what
    /// `tokensave_implementations` reads (#643).
    fn edge_kind_for(&self, r: &ResolvedRef) -> EdgeKind {
        let kind = r.original.reference_kind;
        if kind == EdgeKind::Extends
            && lang_from_path(&r.original.file_path) == "csharp"
            && self
                .node_id_cache
                .get(r.target_node_id.as_str())
                .is_some_and(|n| n.kind == NodeKind::Interface)
        {
            return EdgeKind::Implements;
        }
        kind
    }

    /// Strategy 1: try matching the reference name against qualified names.
    fn try_qualified_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        // Direct lookup first
        if let Some(candidates) = self.qualified_name_cache.get(uref.reference_name.as_str()) {
            if let Some(node) = candidates.iter().find(|n| kind_compatible(uref, &n.kind)) {
                return Some(ResolvedRef {
                    original: uref.clone(),
                    target_node_id: node.id.clone(),
                    confidence: 0.95,
                    resolved_by: "qualified-match".to_string(),
                });
            }
        }

        // Suffix match via pre-built suffix index — O(1) lookup instead of
        // scanning the entire qualified_name_cache.
        if let Some(full_names) = self.suffix_cache.get(uref.reference_name.as_str()) {
            for full_name in full_names {
                if let Some(candidates) = self.qualified_name_cache.get(full_name) {
                    if let Some(node) = candidates.iter().find(|n| kind_compatible(uref, &n.kind)) {
                        return Some(ResolvedRef {
                            original: uref.clone(),
                            target_node_id: node.id.clone(),
                            confidence: 0.95,
                            resolved_by: "qualified-match".to_string(),
                        });
                    }
                }
            }
        }

        None
    }

    /// Go selector resolution (#149 Bug 1): resolve `qualifier.Name` by mapping
    /// `qualifier` to its import path (via the file's in-scope imports), then
    /// picking the candidate named `Name` whose file lives in that import's
    /// package directory.
    ///
    /// Returns `None` when `qualifier` is not a known import in this file (it is
    /// then treated as a receiver variable and resolved by the bare-name
    /// fallback) or when no candidate's directory matches the import path.
    fn try_go_selector_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        if lang_from_path(&uref.file_path) != "go" {
            return None;
        }
        let (qualifier, name) = uref.reference_name.split_once('.')?;
        // Only single-level selectors (`pkg.Fn`) carry a package qualifier; a
        // chained selector (`a.b.c`) is field/method access on a receiver.
        if name.contains('.') {
            return None;
        }
        let import_path = self
            .go_import_qualifiers
            .get(&uref.file_path)?
            .get(qualifier)?;

        let candidates = self.name_cache.get(name)?;
        let mut matched: Vec<&Node> = candidates
            .iter()
            .copied()
            .filter(|n| kind_compatible(uref, &n.kind))
            .filter(|n| go_file_in_package(&n.file_path, import_path))
            .collect();
        // A single unambiguous match in the imported package is the answer.
        if matched.len() == 1 {
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: matched.remove(0).id.clone(),
                confidence: 0.95,
                resolved_by: "go-selector-import".to_string(),
            });
        }
        // Multiple files in the same package dir define the name — score them,
        // but only among the package-restricted set so a same-named function in
        // a *different* package can never win.
        if matched.len() > 1 {
            // A tie inside one package directory is still a tie: two files in
            // the same package defining the same name are indistinguishable
            // here, so no edge rather than an arbitrary one (#412).
            let winners = Self::find_best_matches(uref, &matched, &self.import_index);
            let [best] = winners.as_slice() else {
                return None;
            };
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: best.id.clone(),
                confidence: 0.9,
                resolved_by: "go-selector-import".to_string(),
            });
        }
        None
    }

    /// Resolve a Ruby call only when its receiver identifies one constant
    /// owner and its target is one explicit singleton-method definition.
    fn try_ruby_receiver_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        let (receiver, method_name) = split_ruby_receiver_call(&uref.reference_name)?;

        let (owners, resolved_by): (Vec<&Node>, &str) = if receiver == "self" {
            let caller = self.node_id_cache.get(uref.from_node_id.as_str())?;
            let owner = match caller.kind {
                NodeKind::Class | NodeKind::Module => *caller,
                NodeKind::SingletonMethod => caller
                    .parent_id
                    .as_deref()
                    .and_then(|id| self.node_id_cache.get(id))?,
                _ => return None,
            };
            (vec![owner], "ruby-self-receiver")
        } else {
            let constant_path = receiver.strip_prefix("::").unwrap_or(receiver);
            let owners = if receiver.starts_with("::") {
                self.ruby_constant_owners_at(constant_path)?
            } else {
                let caller = self.node_id_cache.get(uref.from_node_id.as_str())?;
                self.ruby_lexical_constant_owners(caller, constant_path)?
            };
            (owners, "ruby-constant-receiver")
        };

        let owner_ids: HashSet<&str> = owners.iter().map(|owner| owner.id.as_str()).collect();
        let mut targets = self
            .name_cache
            .get(method_name)?
            .iter()
            .copied()
            .filter(|node| node.kind == NodeKind::SingletonMethod)
            .filter(|node| lang_from_path(&node.file_path) == "ruby")
            .filter(|node| {
                node.parent_id
                    .as_deref()
                    .is_some_and(|parent| owner_ids.contains(parent))
            });
        let target = targets.next()?;
        if targets.next().is_some() {
            return None;
        }

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: target.id.clone(),
            confidence: 0.95,
            resolved_by: resolved_by.to_string(),
        })
    }

    /// Resolve a `GDScript` typed-receiver call `Type[::step]*::method` (#597).
    ///
    /// The extractor writes the receiver's static type as a class name plus
    /// the member steps it went through: `field` reads a member variable's
    /// declared type, `method()` a method's declared return type. Each is
    /// evaluated here against the indexed classes — `class_name` makes every
    /// script class global, so a class is found by name alone — and a member
    /// missing from a class is looked up through its `extends` chain.
    ///
    /// Returns `None` whenever the evidence runs out: an engine or unindexed
    /// class, an untyped step, a method the class does not have. The
    /// receiver-qualified sibling ref then decides, as it did before.
    fn try_gdscript_typed_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        let mut segments = uref.reference_name.split("::");
        let root = segments.next()?;
        let mut steps: Vec<&str> = segments.collect();
        let method = steps.pop()?;

        let mut class = self.gdscript_class(root)?;
        for step in steps {
            let ty = if let Some(name) = step.strip_suffix("()") {
                let callee = self.gdscript_member(class, name, is_gdscript_callable)?;
                gdscript_return_type(callee.signature.as_deref()?)?
            } else {
                let field = self.gdscript_member(class, step, |k| *k == NodeKind::Field)?;
                gdscript_field_type(field.signature.as_deref()?)?
            };
            class = self.gdscript_class(ty)?;
        }

        let target = self.gdscript_member(class, method, is_gdscript_callable)?;
        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: target.id.clone(),
            confidence: 0.95,
            resolved_by: GDSCRIPT_TYPED.to_string(),
        })
    }

    /// The one `GDScript` class named `name`: a `class_name` script class, or
    /// failing that an inner class. Several of either is no evidence.
    fn gdscript_class(&self, name: &str) -> Option<&'a Node> {
        let candidates = self.name_cache.get(name)?;
        let unique = |kind: NodeKind| {
            let mut it = candidates
                .iter()
                .copied()
                .filter(|n| n.kind == kind && is_gdscript(&n.file_path));
            let first = it.next()?;
            it.next().is_none().then_some(first)
        };
        unique(NodeKind::Class).or_else(|| unique(NodeKind::InnerClass))
    }

    /// The member `name` of `class`, declared there or inherited through its
    /// `extends` chain.
    fn gdscript_member(
        &self,
        class: &'a Node,
        name: &str,
        kind_ok: impl Fn(&NodeKind) -> bool,
    ) -> Option<&'a Node> {
        // Bounded so an `extends` cycle (which Godot rejects, but an index of
        // a broken tree can hold) cannot loop.
        const MAX_DEPTH: usize = 32;
        let mut class = class;
        for _ in 0..MAX_DEPTH {
            let qn = format!("{}.{name}", class.qualified_name);
            if let Some(found) = self
                .qualified_name_cache
                .get(qn.as_str())
                .and_then(|nodes| nodes.iter().copied().find(|n| kind_ok(&n.kind)))
            {
                return Some(found);
            }
            let base = gdscript_extends(class.signature.as_deref()?)?;
            class = self.gdscript_class(base)?;
        }
        None
    }

    /// Resolve a C# typed-receiver call `Type[::step]*::Method` (#642).
    ///
    /// Steps are `member` (a field's or property's declared type), `Method()`
    /// (a method's declared return type) and `await Method()` (the same,
    /// unwrapping `Task<T>`/`ValueTask<T>`), each read from the declaration's
    /// signature. A member missing from a type is looked up through its base
    /// list. Several same-named types (partial classes, or one name in two
    /// namespaces) are all searched; distinct targets are scored and a tie is
    /// no answer.
    ///
    /// Returns `None` whenever the evidence runs out (an unindexed type, an
    /// extension method, an ambiguous step); the receiver-qualified sibling
    /// ref then decides.
    fn try_csharp_typed_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        let mut segments = uref.reference_name.split("::");
        let root = segments.next()?;
        let mut steps: Vec<&str> = segments.collect();
        let method = steps.pop()?;

        let mut types = self.csharp_types(root);
        for step in steps {
            let (awaited, step) = match step.strip_prefix("await ") {
                Some(s) => (true, s),
                None => (false, step),
            };
            let members = match step.strip_suffix("()") {
                Some(name) => self.csharp_members(&types, name, is_csharp_callable),
                None => self.csharp_members(&types, step, |k| {
                    matches!(k, NodeKind::Field | NodeKind::CSharpProperty)
                }),
            };
            let mut next: Vec<&str> = members
                .iter()
                .filter_map(|m| {
                    let raw = csharp_declared_type(m.signature.as_deref()?, &m.name)?;
                    let raw = if awaited { unwrap_task(raw)? } else { raw };
                    csharp_type_name(raw)
                })
                .collect();
            next.sort_unstable();
            next.dedup();
            let [ty] = next.as_slice() else {
                return None;
            };
            types = self.csharp_types(ty);
        }

        let mut targets = self.csharp_members(&types, method, is_csharp_callable);
        // Overloads share a qualified name; the first declared stands for them.
        targets.sort_by(|a, b| {
            (a.qualified_name.as_str(), a.start_line)
                .cmp(&(b.qualified_name.as_str(), b.start_line))
        });
        targets.dedup_by(|a, b| a.qualified_name == b.qualified_name);
        let (target_node_id, confidence) = match targets.as_slice() {
            [] => return None,
            [one] => (one.id.clone(), 0.95),
            many => {
                let winners = Self::find_best_matches(uref, many, &self.import_index);
                let [best] = winners.as_slice() else {
                    return None;
                };
                (best.id.clone(), 0.9)
            }
        };
        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id,
            confidence,
            resolved_by: CSHARP_TYPED.to_string(),
        })
    }

    /// Every indexed C# type declaration named `name`.
    fn csharp_types(&self, name: &str) -> Vec<&'a Node> {
        self.name_cache
            .get(name)
            .map(|nodes| {
                nodes
                    .iter()
                    .copied()
                    .filter(|n| {
                        matches!(
                            n.kind,
                            NodeKind::Class
                                | NodeKind::InnerClass
                                | NodeKind::Struct
                                | NodeKind::Interface
                                | NodeKind::Record
                        ) && is_csharp(&n.file_path)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The members named `name` of `types`, declared there or, failing that,
    /// in the nearest base types that declare one.
    fn csharp_members(
        &self,
        types: &[&'a Node],
        name: &str,
        kind_ok: impl Fn(&NodeKind) -> bool,
    ) -> Vec<&'a Node> {
        // Bounded so a base-list cycle in a broken tree cannot loop.
        const MAX_DEPTH: usize = 16;
        let mut seen: HashSet<&str> = types.iter().map(|t| t.id.as_str()).collect();
        let mut frontier: Vec<&'a Node> = types.to_vec();
        for _ in 0..MAX_DEPTH {
            if frontier.is_empty() {
                break;
            }
            let found: Vec<&'a Node> = frontier
                .iter()
                .filter_map(|t| {
                    self.qualified_name_cache
                        .get(format!("{}::{name}", t.qualified_name).as_str())
                })
                .flat_map(|nodes| nodes.iter().copied().filter(|n| kind_ok(&n.kind)))
                .collect();
            if !found.is_empty() {
                return found;
            }
            let mut next = Vec::new();
            for t in &frontier {
                for base in csharp_bases(t.signature.as_deref().unwrap_or("")) {
                    for b in self.csharp_types(base) {
                        if seen.insert(b.id.as_str()) {
                            next.push(b);
                        }
                    }
                }
            }
            frontier = next;
        }
        Vec::new()
    }

    /// The name-based fallback for a C# receiver-qualified call `recv.Method`.
    ///
    /// A receiver that is not `this`/`base` is some other object, so the
    /// caller's own class is no evidence for the target: the same-file bonus
    /// would otherwise bind `refresher.RefreshAsync()` inside
    /// `Coordinator.PollAndRefreshAsync` to `Coordinator.RefreshAsync` (#642,
    /// the C# form of #503). Members of the caller's own type are dropped
    /// from the candidates before the usual scoring.
    fn try_csharp_receiver_fallback(
        &self,
        uref: &UnresolvedRef,
        simple_name: &str,
    ) -> Option<ResolvedRef> {
        let receiver = uref
            .reference_name
            .rsplit_once('.')
            .map_or("", |(recv, _)| recv);
        let own_scope = self
            .node_id_cache
            .get(uref.from_node_id.as_str())
            .and_then(|caller| caller.qualified_name.rsplit_once("::"))
            .map(|(scope, _)| scope);
        let Some(own_scope) = own_scope.filter(|_| !matches!(receiver, "this" | "base")) else {
            return self.try_exact_name_match_simple(uref, simple_name, true, SIMPLE_NAME_MATCH);
        };
        let in_own_scope = |n: &Node| {
            n.qualified_name
                .rsplit_once("::")
                .is_some_and(|(scope, _)| scope == own_scope)
        };
        let compatible: Vec<&Node> = self
            .name_cache
            .get(simple_name)?
            .iter()
            .copied()
            .filter(|n| kind_compatible(uref, &n.kind))
            .collect();
        if !compatible.iter().any(|n| in_own_scope(n)) {
            return self.try_exact_name_match_simple(uref, simple_name, true, SIMPLE_NAME_MATCH);
        }
        if CROSS_FILE_BLOCKLIST.contains(&simple_name) {
            return None;
        }
        let others: Vec<&Node> = compatible
            .into_iter()
            .filter(|n| !in_own_scope(n))
            .collect();
        if others.is_empty() {
            return None;
        }
        resolve_from_filtered_named(
            uref,
            &others,
            SIMPLE_NAME_MATCH,
            &self.import_index,
            true,
            &self.node_id_cache,
        )
    }

    fn ruby_constant_owners_at(&self, constant_path: &str) -> Option<Vec<&Node>> {
        let bindings = self.ruby_constant_bindings.get(constant_path)?;
        bindings
            .iter()
            .all(|node| matches!(node.kind, NodeKind::Class | NodeKind::Module))
            .then(|| bindings.clone())
    }

    fn ruby_lexical_constant_owners(
        &self,
        caller: &Node,
        constant_path: &str,
    ) -> Option<Vec<&Node>> {
        let first_segment = constant_path.split("::").next()?;
        let mut scope = if matches!(caller.kind, NodeKind::Class | NodeKind::Module) {
            Some(caller)
        } else {
            caller
                .parent_id
                .as_deref()
                .and_then(|id| self.node_id_cache.get(id).copied())
        };

        while let Some(node) = scope {
            if matches!(node.kind, NodeKind::Class | NodeKind::Module) {
                let scope_name = ruby_constant_name(node);
                let desired = format!("{scope_name}::{constant_path}");
                if self.ruby_constant_bindings.contains_key(desired.as_str()) {
                    return self.ruby_constant_owners_at(&desired);
                }
                let lexical_head = format!("{scope_name}::{first_segment}");
                if self
                    .ruby_constant_bindings
                    .contains_key(lexical_head.as_str())
                {
                    return None;
                }
            }
            scope = node
                .parent_id
                .as_deref()
                .and_then(|id| self.node_id_cache.get(id).copied());
        }

        self.ruby_constant_owners_at(constant_path)
    }

    /// Strategy 2: exact name match using the name cache.
    fn try_exact_name_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        // Skip cross-file resolution for blocklisted names (too ambiguous).
        if CROSS_FILE_BLOCKLIST.contains(&uref.reference_name.as_str()) {
            // Still allow same-file resolution, but apply the same
            // kind-compatibility filter as the non-blocklist path —
            // otherwise a `Calls` ref to `new()` happily binds to a
            // same-file `struct new` because that's the only same-file
            // node with the name.
            let candidates = self.name_cache.get(uref.reference_name.as_str())?;
            let same_file: Vec<&Node> = candidates
                .iter()
                .copied()
                .filter(|n| n.file_path == uref.file_path)
                .filter(|n| kind_compatible(uref, &n.kind))
                .collect();
            if same_file.len() == 1 {
                return Some(ResolvedRef {
                    original: uref.clone(),
                    target_node_id: same_file[0].id.clone(),
                    confidence: 0.9,
                    resolved_by: "same-file-blocklist".to_string(),
                });
            }
            return None;
        }

        let raw_candidates = self.name_cache.get(uref.reference_name.as_str())?;
        // Filter by node-kind compatibility with the reference kind. An
        // `Implements`/`Extends`/`DerivesMacro` ref like `impl Default for X`
        // must NOT bind to an unrelated node kind (e.g. a local
        // `enum_variant Default`) just because the names match — that
        // poisons `tokensave_rank` and every downstream graph query.
        let kind_filtered: Vec<&Node> = raw_candidates
            .iter()
            .copied()
            .filter(|n| kind_compatible(uref, &n.kind))
            .collect();
        if kind_filtered.is_empty() {
            return None;
        }
        let candidates: &[&Node] = if kind_filtered.len() == raw_candidates.len() {
            raw_candidates
        } else {
            // Cache the filtered subset in a local Vec so the downstream
            // helpers see the same shape. Allocating here only on the
            // shrunk path keeps the happy path zero-copy.
            return resolve_from_filtered(
                uref,
                &kind_filtered,
                &self.import_index,
                &self.node_id_cache,
            );
        };

        if candidates.len() == 1 {
            let ref_lang = lang_from_path(&uref.file_path);
            // Being the only candidate is not evidence. #508 taught the
            // dotted-receiver path this; the bare-name path never learned it,
            // so a production function declaring a local `exe` acquired an edge
            // to a pytest fixture named `exe` purely because the fixture was
            // the only symbol of that name in the project (#522). Scoped by
            // language: see `bare_name_needs_evidence`.
            if bare_name_needs_evidence(ref_lang)
                && !is_plausibly_reachable(
                    uref,
                    candidates[0],
                    &self.import_index,
                    &self.node_id_cache,
                )
            {
                return None;
            }
            let candidate_lang = lang_from_path(&candidates[0].file_path);
            let confidence = if ref_lang != "unknown"
                && candidate_lang != "unknown"
                && !same_language_family(ref_lang, candidate_lang)
            {
                0.5
            } else {
                0.9
            };
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: candidates[0].id.clone(),
                confidence,
                resolved_by: "exact-match".to_string(),
            });
        }

        // Multiple candidates -- score them. A single winner is the answer; a
        // tie is recorded as an ambiguity instead of resolved arbitrarily.
        let winners = Self::find_best_matches(uref, candidates, &self.import_index);
        let [best] = winners.as_slice() else {
            return None;
        };

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: best.id.clone(),
            confidence: 0.7,
            resolved_by: "exact-match-scored".to_string(),
        })
    }

    fn try_exact_name_match_simple(
        &self,
        uref: &UnresolvedRef,
        simple_name: &str,
        require_reachable: bool,
        tag: &str,
    ) -> Option<ResolvedRef> {
        if CROSS_FILE_BLOCKLIST.contains(&simple_name) {
            let candidates = self.name_cache.get(simple_name)?;
            // Same fix as `try_exact_name_match`: filter by kind before
            // returning a same-file blocklisted match.
            let same_file: Vec<&Node> = candidates
                .iter()
                .copied()
                .filter(|n| n.file_path == uref.file_path)
                .filter(|n| kind_compatible(uref, &n.kind))
                .collect();
            if same_file.len() == 1 {
                return Some(ResolvedRef {
                    original: uref.clone(),
                    target_node_id: same_file[0].id.clone(),
                    confidence: 0.9,
                    resolved_by: "same-file-blocklist".to_string(),
                });
            }
            return None;
        }

        let raw_candidates = self.name_cache.get(simple_name)?;
        let kind_filtered: Vec<&Node> = raw_candidates
            .iter()
            .copied()
            .filter(|n| kind_compatible(uref, &n.kind))
            .collect();
        if kind_filtered.is_empty() {
            return None;
        }
        let candidates: &[&Node] = if kind_filtered.len() == raw_candidates.len() {
            raw_candidates
        } else {
            return resolve_from_filtered_named(
                uref,
                &kind_filtered,
                tag,
                &self.import_index,
                require_reachable,
                &self.node_id_cache,
            );
        };

        if candidates.len() == 1 {
            if require_reachable
                && !is_plausibly_reachable(
                    uref,
                    candidates[0],
                    &self.import_index,
                    &self.node_id_cache,
                )
            {
                return None;
            }
            let ref_lang = lang_from_path(&uref.file_path);
            let candidate_lang = lang_from_path(&candidates[0].file_path);
            let confidence = if ref_lang != "unknown"
                && candidate_lang != "unknown"
                && !same_language_family(ref_lang, candidate_lang)
            {
                0.5
            } else {
                0.9
            };
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: candidates[0].id.clone(),
                confidence,
                resolved_by: tag.to_string(),
            });
        }

        let winners = Self::find_best_matches(uref, candidates, &self.import_index);
        let [best] = winners.as_slice() else {
            return None;
        };

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: best.id.clone(),
            confidence: 0.7,
            resolved_by: format!("{tag}-scored"),
        })
    }

    /// Scores one candidate against a reference.
    ///
    /// Scoring heuristics:
    /// - Same file as reference: +100
    /// - Directory proximity (shared path segments): +5 per segment, capped at +40
    /// - Same language: +50, cross-language: -80
    /// - Exported / pub visibility: +10
    /// - Callable kind (function/method) when the ref kind is `Calls`: +25
    /// - Line proximity (same file only): +20 - (`line_distance` / 10)
    /// - Import match (caller imports this name): +30
    ///
    /// Extracted from the match search so ambiguity can be detected by
    /// comparing scores rather than re-deriving them (#378).
    fn score_candidate(
        uref: &UnresolvedRef,
        node: &Node,
        import_index: &HashMap<String, HashSet<String>>,
    ) -> i64 {
        let ref_lang = lang_from_path(&uref.file_path);
        let mut score: i64 = 0;

        // Same file bonus
        if node.file_path == uref.file_path {
            score += 100;

            // Line proximity bonus (same file only)
            let distance = node.start_line.abs_diff(uref.line);
            let proximity = 20_i64.saturating_sub(i64::from(distance) / 10);
            score += proximity.max(0);
        } else {
            // Directory proximity bonus (different files only)
            score += path_proximity(&uref.file_path, &node.file_path);
        }

        // Language matching
        let candidate_lang = lang_from_path(&node.file_path);
        if ref_lang != "unknown" && candidate_lang != "unknown" {
            if same_language_family(ref_lang, candidate_lang) {
                score += 50;
            } else {
                score -= 80;
            }
        }

        // Header declares, source defines, a caller wants the body; small enough that directory
        // proximity still decides between two definitions.
        if is_c_family(ref_lang) && is_c_family(candidate_lang) && !is_header_path(&node.file_path)
        {
            score += 20;
        }

        // Exported / pub bonus
        if node.visibility == Visibility::Pub {
            score += 10;
        }

        // Callable kind bonus for Calls references
        if uref.reference_kind == EdgeKind::Calls
            && matches!(
                node.kind,
                NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::SingletonMethod
                    | NodeKind::StructMethod
                    | NodeKind::Constructor
                    | NodeKind::AbstractMethod
            )
        {
            score += 25;
        }

        // Import match bonus: caller explicitly imports a name that matches
        if let Some(imports) = import_index.get(&uref.file_path) {
            if imports.contains(&node.name) {
                score += 30;
            }
        }

        score
    }

    /// Reports the candidates behind an unresolved reference, when the reason
    /// it went unresolved was a tie rather than an absence.
    ///
    /// Run only over refs that already failed, so it costs one name lookup and
    /// a rescore for a minority of references rather than threading state
    /// through the parallel resolution pass.
    ///
    /// Returns `None` when the name is simply unknown, or when a candidate won
    /// outright (in which case the ref failed for some other reason), so the
    /// record stays limited to genuine ties (#412).
    fn explain_ambiguity(&self, uref: &UnresolvedRef) -> Option<AmbiguousCall> {
        if uref.reference_kind != EdgeKind::Calls {
            return None;
        }
        // A GDScript typed ref that did not resolve is not a name tie: its
        // receiver-qualified sibling at the same site explains any tie (#597).
        if has_typed_receiver_refs(&uref.file_path) && uref.reference_name.contains("::") {
            return None;
        }
        let simple_name = simple_ref_name(&uref.reference_name);
        let raw = self.name_cache.get(simple_name)?;
        // A template's bare call only ever competes among helpers; a tie
        // among unrelated same-named methods is not an ambiguity it has.
        let template_bare = is_ruby_template(&uref.file_path) && simple_name == uref.reference_name;
        let candidates: Vec<&Node> = raw
            .iter()
            .copied()
            .filter(|n| kind_compatible(uref, &n.kind))
            .filter(|n| {
                !template_bare || n.file_path == uref.file_path || is_ruby_template_helper(n)
            })
            .collect();

        let winners = Self::find_best_matches(uref, &candidates, &self.import_index);
        if winners.len() < 2 {
            return None;
        }

        Some(AmbiguousCall {
            from_node_id: uref.from_node_id.clone(),
            reference_name: uref.reference_name.clone(),
            file_path: uref.file_path.clone(),
            line: uref.line,
            column: uref.column,
            // `find_best_matches` already orders by id, so the record is
            // stable across runs.
            candidate_node_ids: winners.into_iter().map(|n| n.id).collect(),
        })
    }

    /// Every candidate tied for the best score.
    ///
    /// Returns the winners rather than *a* winner, because when several
    /// candidates score identically the evidence genuinely does not separate
    /// them and choosing one is a coin flip. Before this the comparison was
    /// `if score > best_score`, so the first candidate the scan reached won —
    /// and "first" is file enumeration order, which made the graph a function
    /// of the filesystem rather than of the source (#378, #412).
    ///
    /// Callers decide what a tie means for them. A single winner resolves
    /// normally; several are recorded as an ambiguity with their candidates,
    /// so the information reaches a reader who can judge it from the source
    /// instead of being discarded or guessed at.
    ///
    /// Ordered by node id so the candidate list itself is stable across runs.
    fn find_best_matches(
        uref: &UnresolvedRef,
        candidates: &[&Node],
        import_index: &HashMap<String, HashSet<String>>,
    ) -> Vec<Node> {
        if candidates.is_empty() {
            return Vec::new();
        }

        let scored: Vec<(i64, &&Node)> = candidates
            .iter()
            .map(|node| (Self::score_candidate(uref, node, import_index), node))
            .collect();
        let Some(best_score) = scored.iter().map(|(score, _)| *score).max() else {
            return Vec::new();
        };

        let mut winners: Vec<Node> = scored
            .into_iter()
            .filter(|(score, _)| *score == best_score)
            .map(|(_, node)| (*node).clone())
            .collect();
        winners.sort_by(|a, b| a.id.cmp(&b.id));
        winners
    }
}

/// True when an unresolved-ref's edge kind is structurally compatible
/// with a candidate target node's kind.
///
/// Without this check, the resolver fuzzy-binds `impl Default for X`
/// (an `Implements` ref) to whatever local node happens to share the
/// name `Default` — e.g. a `Token::Default` enum variant in a parser
/// crate. That poisons `tokensave_rank --edge-kind implements`,
/// `tokensave_impls`, and the type-hierarchy tools.
///
/// The compatibility matrix is deliberately conservative: when the
/// edge kind constrains the target shape (`Implements`/`Extends`/
/// `DerivesMacro` must target a trait or interface; `Calls` must
/// target a callable), we enforce it. Everything else stays permissive
/// (e.g. `Uses` accepts any kind because imports cover the full type
/// system).
///
/// A Ruby `Implements` ref (`include`/`prepend`/`extend Mixin`, indexed by
/// the extractor as `NodeKind::Module`) resolves *exclusively* to a
/// `NodeKind::Module` target — never to the shared Trait/Class/etc. list.
/// Ruby itself enforces this: `include SomeClass` raises `TypeError: wrong
/// argument type Class (expected Module)`. Keeping the allowance exclusive
/// (rather than additive to the shared list) also matters when a project
/// indexes both a `class Foo` and a `module Foo` — an additive rule would let
/// `try_qualified_match` bind to whichever sorts first in the suffix index,
/// silently picking the class.
fn kind_compatible(uref: &UnresolvedRef, target_kind: &NodeKind) -> bool {
    match uref.reference_kind {
        EdgeKind::Implements if lang_from_path(&uref.file_path) == "ruby" => {
            matches!(target_kind, NodeKind::Module)
        }
        // A VHDL architecture implements an entity, and an entity is indexed
        // as a `Module` because an instantiation targets it (#344).
        EdgeKind::Implements if lang_from_path(&uref.file_path) == "vhdl" => {
            matches!(target_kind, NodeKind::Module)
        }
        // A VHDL `use` clause names a package. The package body is indexed as
        // an `Impl` with the same name; left permissive, `Uses` would tie
        // between the two and resolve to neither.
        EdgeKind::Uses if lang_from_path(&uref.file_path) == "vhdl" => {
            matches!(target_kind, NodeKind::Package)
        }
        EdgeKind::Implements | EdgeKind::Extends | EdgeKind::DerivesMacro => {
            matches!(
                target_kind,
                NodeKind::Trait
                    | NodeKind::Interface
                    | NodeKind::InterfaceType
                    | NodeKind::Class
                    | NodeKind::InnerClass
                    | NodeKind::AbstractMethod
                    | NodeKind::SealedClass
                    | NodeKind::Annotation
                    | NodeKind::TypeAlias
            )
        }
        // An HDL instantiation names a module or interface and nothing else
        // (#344). Left permissive, `child u_child (...)` would happily bind to
        // any same-named symbol in any language in the index — a vendor cell
        // that is not indexed must produce no edge, not a fabricated one.
        EdgeKind::Instantiates => matches!(
            target_kind,
            NodeKind::Module | NodeKind::Interface | NodeKind::InterfaceType
        ),
        EdgeKind::Calls => matches!(
            target_kind,
            NodeKind::Function
                | NodeKind::Method
                | NodeKind::SingletonMethod
                | NodeKind::StructMethod
                | NodeKind::Constructor
                | NodeKind::AbstractMethod
                | NodeKind::ArrowFunction
                | NodeKind::Procedure
                | NodeKind::Macro
        ),
        // `annotates` names exactly one relation to every consumer:
        // attachment of an annotation/decorator usage to the item it
        // decorates (`get_annotation_sites`, `get_test_annotated_node_ids`,
        // `get_files_with_test_annotations`,
        // `populate_test_annotated_targets_temp_table`). Extractors already
        // emit that edge directly at the usage site — this resolver has no
        // second, distinct relation to express under the same edge kind.
        //
        // `AnnotationUsage` and `Decorator` are both usage-site kinds, not
        // declarations: allowing either as a ref target let a lone-candidate
        // usage resolve to *itself* or to a sibling usage of the same name
        // (96% of `annotates` edges in this repo were this phantom pattern).
        // `Annotation` is a real declaration (Java `@interface`), but no
        // consumer reads a resolver-produced usage → declaration edge as
        // attachment, so binding to it is equally wrong under this kind.
        // A ref that matches nothing simply stays unresolved.
        EdgeKind::Annotates => false,
        // Uses / TypeOf / Returns / Contains / Receives — permissive.
        _ => true,
    }
}

/// Resolution helper used after the kind filter has reduced the
/// candidate list to a strict subset of `name_cache`. Mirrors the
/// single-candidate / multi-candidate branches of
/// `try_exact_name_match` but operates on the borrowed slice.
fn resolve_from_filtered<'a>(
    uref: &UnresolvedRef,
    kind_filtered: &[&Node],
    import_index: &HashMap<String, HashSet<String>>,
    node_by_id: &HashMap<&'a str, &'a Node>,
) -> Option<ResolvedRef> {
    resolve_from_filtered_named(
        uref,
        kind_filtered,
        "exact-match",
        import_index,
        false,
        node_by_id,
    )
}

/// Whether a lone candidate is plausibly visible from the call site.
///
/// This governs the dotted-receiver fallback only — `recv.method()` in a
/// dynamically typed language, where the receiver's type is not tracked and
/// the resolver has nothing to go on but the method name. Being the only
/// symbol in the project with that name is *not* evidence of a match: nothing
/// checked that the call site can reach it (#503, the #378 defect in Python).
///
/// The failure is quiet and it has a direction. Test doubles are deliberately
/// named after the API they stand in for, so a fake logger defines `info` and
/// a faithful fake of a UI toolkit defines `after`, `delete` and `grid` —
/// exactly the names production code calls on untracked receivers. Production
/// code then binds to the test tree, and every consumer of `calls` inherits
/// it: `circular` reports one strongly-connected component spanning production
/// and tests, `dead_code` sees a phantom caller and calls live code reachable,
/// `impact` and `file_dependents` report modules as depending on test files.
///
/// Evidence means one of: the candidate is in the caller's own file; it sits
/// in the same directory, which is one package in every language this path
/// serves; the caller imports its name; or the caller imports the module it
/// lives in. Anything else declines, and a declined reference is simply
/// unresolved — a missing edge degrades an answer, a fabricated one corrupts
/// it.
/// Languages where a bare name alone is not evidence of a binding.
///
/// The reachability gate was built for the dotted-receiver fallback in
/// dynamically typed languages, and its evidence model — same file, same
/// directory, an imported name, an imported module — describes how those
/// languages are written. A blanket rule is measurably wrong: Rust and Go
/// resolve a bare name through a module system the gate cannot see, so
/// gating them declines calls the code plainly makes — **11.4% of every Rust
/// call edge** on a 1,824-file tree, against no reduction in impossible
/// edges, because Rust never had this problem (#522).
///
/// **Ruby is deliberately absent.** It looks like it belongs, and it does not:
/// it has its own resolution paths here (`try_ruby_receiver_match`, the
/// constant-binding table), and gating its bare names drops a legitimate
/// `Implements` edge for a module included from another file — caught by
/// `test_ruby_incremental_sync_graph_matches_full_reindex`. Anything added to
/// this list needs the same before/after measurement Python got, not an
/// argument from resemblance.
fn bare_name_needs_evidence(lang: &str) -> bool {
    matches!(lang, "python" | "javascript" | "typescript")
}

fn is_plausibly_reachable(
    uref: &UnresolvedRef,
    candidate: &Node,
    import_index: &HashMap<String, HashSet<String>>,
    node_by_id: &HashMap<&str, &Node>,
) -> bool {
    if candidate.file_path == uref.file_path {
        return true;
    }

    let dir_of = |path: &str| path.rfind('/').map(|i| path[..i].to_string());
    if dir_of(&candidate.file_path) == dir_of(&uref.file_path) {
        return true;
    }

    let Some(imports) = import_index.get(&uref.file_path) else {
        return false;
    };

    // A relative JS/TS import of the candidate's file, recorded by path when
    // the index was built (#647).
    if imports.contains(&candidate.file_path) {
        return true;
    }

    // The index keys each import on the last `::` segment, which for a Python
    // or JS import is the whole dotted path — `headroom.perf.analyzer` is one
    // key, not three. So an entry matches when it equals the name outright or
    // ends with it as a dotted segment; without the second reading, `import
    // headroom.perf.analyzer` is not recognised as importing `analyzer` and
    // the guard declines calls the file plainly can make.
    let imported = |name: &str| {
        imports
            .iter()
            .any(|entry| entry == name || entry.rsplit('.').next() == Some(name))
    };

    if imported(&candidate.name) {
        return true;
    }

    // The class that owns the method is the evidence a method call actually
    // needs: `from pkg.encoder import Encoder` then `self.enc.encode(x)`
    // imports `Encoder`, never `encode`. Without this the guard would decline
    // most legitimate method calls in the languages it governs — measured at
    // 13% of all call edges on a 992-file Python project, which is far too
    // much recall to trade for the phantoms.
    if let Some(parent) = candidate
        .parent_id
        .as_deref()
        .and_then(|id| node_by_id.get(id))
    {
        if imported(&parent.name) {
            return true;
        }
    }

    // The module the candidate lives in: `pkg/logging.py` is imported as
    // `logging`, and the import index keys on that last segment.
    let module = candidate
        .file_path
        .rsplit('/')
        .next()
        .and_then(|file| file.split('.').next());
    module.is_some_and(imported)
}

fn resolve_from_filtered_named(
    uref: &UnresolvedRef,
    kind_filtered: &[&Node],
    resolved_by: &str,
    import_index: &HashMap<String, HashSet<String>>,
    require_reachable: bool,
    node_by_id: &HashMap<&str, &Node>,
) -> Option<ResolvedRef> {
    if kind_filtered.len() == 1 {
        if require_reachable
            && !is_plausibly_reachable(uref, kind_filtered[0], import_index, node_by_id)
        {
            return None;
        }
        return Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: kind_filtered[0].id.clone(),
            confidence: 0.85,
            resolved_by: resolved_by.to_string(),
        });
    }
    // Multiple kind-compatible candidates: score them like every other
    // multi-candidate path. This used to pick the first candidate in the
    // reference's own file, else the first overall — and "first" is file
    // enumeration order, the last place in the resolver where the graph
    // was a function of the filesystem rather than of the source (#412).
    // A lone winner resolves; a tie resolves to nothing and is reported
    // by `explain_ambiguity` with its candidates.
    let winners = ReferenceResolver::find_best_matches(uref, kind_filtered, import_index);
    let [best] = winners.as_slice() else {
        return None;
    };
    Some(ResolvedRef {
        original: uref.clone(),
        target_node_id: best.id.clone(),
        confidence: 0.65,
        resolved_by: format!("{resolved_by}-scored"),
    })
}
