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
fn simple_ref_name(name: &str) -> &str {
    let after_path = name.rsplit("::").next().unwrap_or(name);
    after_path.rsplit('.').next().unwrap_or(after_path)
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
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "scala" | "sc" => "scala",
        "dart" => "dart",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "sh" | "bash" => "bash",
        "nix" => "nix",
        "zig" => "zig",
        "proto" => "proto",
        _ => "unknown",
    }
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
    name_cache: HashMap<String, Vec<Node>>,
    /// Nodes grouped by their qualified name.
    qualified_name_cache: HashMap<String, Vec<Node>>,
    /// Suffix index: maps every `::suffix` of a qualified name to the full
    /// qualified name(s). Enables O(1) suffix lookups instead of scanning
    /// the entire `qualified_name_cache`.
    suffix_cache: HashMap<String, Vec<String>>,
    /// All known symbol names (short + qualified + suffixes) for pre-filtering.
    known_names: HashSet<String>,
    /// Maps `file_path` to the set of qualified names imported by that file.
    /// Built from Use nodes. Used to prefer candidates that the caller imports.
    import_index: HashMap<String, HashSet<String>>,
    /// Maps `file_path` to that Go file's in-scope import qualifiers
    /// (`qualifier` -> full import path). Built from Go Use nodes. Used to
    /// disambiguate a selector call `qualifier.Name` to the package directory
    /// the qualifier refers to, so same-named packages don't collide (#149
    /// Bug 1).
    go_import_qualifiers: HashMap<String, HashMap<String, String>>,
}

impl<'a> ReferenceResolver<'a> {
    /// Creates a resolver from pre-loaded nodes.
    pub fn from_nodes(db: &'a Database, all_nodes: &[Node]) -> Self {
        let mut name_cache: HashMap<String, Vec<Node>> = HashMap::new();
        let mut qualified_name_cache: HashMap<String, Vec<Node>> = HashMap::new();
        let mut suffix_cache: HashMap<String, Vec<String>> = HashMap::new();

        for node in all_nodes {
            // Skip Use nodes — they represent import statements, not definitions.
            // Including them causes false cross-file edges when two files share
            // the same `use std::path::Path` import.
            if node.kind == NodeKind::Use {
                continue;
            }
            name_cache
                .entry(node.name.clone())
                .or_default()
                .push(node.clone());
            let qn = &node.qualified_name;
            qualified_name_cache
                .entry(qn.clone())
                .or_default()
                .push(node.clone());
            // Build suffix index: for "a::b::c", index "b::c" and "c"
            // (but not the full name — that's in qualified_name_cache already)
            let mut pos = 0;
            while let Some(idx) = qn[pos..].find("::") {
                let suffix = &qn[pos + idx + 2..];
                if !suffix.is_empty() {
                    suffix_cache
                        .entry(suffix.to_string())
                        .or_default()
                        .push(qn.clone());
                }
                pos += idx + 2;
            }
        }

        // Deduplicate suffix entries
        for entries in suffix_cache.values_mut() {
            entries.sort_unstable();
            entries.dedup();
        }

        // Build known_names set for pre-filtering unresolvable refs.
        let mut known_names: HashSet<String> = HashSet::new();
        for key in name_cache.keys() {
            known_names.insert(key.clone());
        }
        for key in qualified_name_cache.keys() {
            known_names.insert(key.clone());
        }
        for key in suffix_cache.keys() {
            known_names.insert(key.clone());
        }

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
            suffix_cache,
            known_names,
            import_index,
            go_import_qualifiers,
        }
    }

    /// Attempts to resolve a single unresolved reference.
    ///
    /// Resolution strategies are tried in order:
    /// 1. **Qualified name match** -- if the reference contains `::`, try
    ///    matching against qualified names of known nodes (confidence 0.95).
    /// 2. **Exact name match** -- look up the reference name in the name cache.
    ///    A single match yields confidence 0.9; multiple matches are scored via
    ///    `find_best_match` and the winner gets confidence 0.7.
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
            if let Some(resolved) = self.try_exact_name_match_simple(uref, simple_name) {
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
            if simple_name != uref.reference_name {
                if let Some(resolved) = self.try_exact_name_match_simple(uref, simple_name) {
                    return Some(resolved);
                }
            }
            return None;
        }

        // Strategy 2: exact name match
        self.try_exact_name_match(uref)
    }

    /// Returns true if a reference name could plausibly resolve to a known symbol.
    fn is_known_name(&self, name: &str) -> bool {
        self.known_names.contains(name)
    }

    /// Resolves a batch of unresolved references in parallel, returning a
    /// summary of the results.
    ///
    /// Pre-filters references whose name doesn't exist in the graph at all,
    /// turning hopeless lookups into O(1) hash checks.
    pub fn resolve_all(&self, refs: &[UnresolvedRef]) -> ResolutionResult {
        let total = refs.len();

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
        let (candidates, hopeless): (Vec<_>, Vec<_>) = refs.iter().partition(|uref| {
            self.is_known_name(&uref.reference_name)
                || self.is_known_name(simple_ref_name(&uref.reference_name))
        });

        let results: Vec<_> = candidates
            .par_iter()
            .map(|uref| (*uref, self.resolve_one(uref)))
            .collect();

        let mut resolved = Vec::new();
        let mut unresolved: Vec<UnresolvedRef> = hopeless.into_iter().cloned().collect();
        for (uref, res) in results {
            match res {
                Some(r) if r.confidence >= 0.6 => resolved.push(r),
                Some(_) | None => unresolved.push(uref.clone()), // below confidence floor or unresolved
            }
        }

        let resolved_count = resolved.len();

        ResolutionResult {
            resolved,
            unresolved,
            total,
            resolved_count,
        }
    }

    /// Converts a slice of resolved references into graph edges.
    pub fn create_edges(&self, resolved: &[ResolvedRef]) -> Vec<Edge> {
        resolved
            .iter()
            .map(|r| Edge {
                source: r.original.from_node_id.clone(),
                target: r.target_node_id.clone(),
                kind: r.original.reference_kind,
                line: Some(r.original.line),
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// Strategy 1: try matching the reference name against qualified names.
    fn try_qualified_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        // Direct lookup first
        if let Some(candidates) = self.qualified_name_cache.get(&uref.reference_name) {
            if let Some(node) = candidates
                .iter()
                .find(|n| kind_compatible(uref.reference_kind, &n.kind))
            {
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
        if let Some(full_names) = self.suffix_cache.get(&uref.reference_name) {
            for full_name in full_names {
                if let Some(candidates) = self.qualified_name_cache.get(full_name) {
                    if let Some(node) = candidates
                        .iter()
                        .find(|n| kind_compatible(uref.reference_kind, &n.kind))
                    {
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
            .filter(|n| kind_compatible(uref.reference_kind, &n.kind))
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
            let owned: Vec<Node> = matched.iter().map(|n| (*n).clone()).collect();
            let best = Self::find_best_match(uref, &owned, &self.import_index)?;
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: best.id.clone(),
                confidence: 0.9,
                resolved_by: "go-selector-import".to_string(),
            });
        }
        None
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
            let candidates = self.name_cache.get(&uref.reference_name)?;
            let same_file: Vec<_> = candidates
                .iter()
                .filter(|n| n.file_path == uref.file_path)
                .filter(|n| kind_compatible(uref.reference_kind, &n.kind))
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

        let raw_candidates = self.name_cache.get(&uref.reference_name)?;
        // Filter by node-kind compatibility with the reference kind. An
        // `Implements`/`Extends`/`DerivesMacro` ref like `impl Default for X`
        // must NOT bind to an unrelated node kind (e.g. a local
        // `enum_variant Default`) just because the names match — that
        // poisons `tokensave_rank` and every downstream graph query.
        let kind_filtered: Vec<&Node> = raw_candidates
            .iter()
            .filter(|n| kind_compatible(uref.reference_kind, &n.kind))
            .collect();
        if kind_filtered.is_empty() {
            return None;
        }
        let candidates: &[Node] = if kind_filtered.len() == raw_candidates.len() {
            raw_candidates
        } else {
            // Cache the filtered subset in a local Vec so the downstream
            // helpers see the same shape. Allocating here only on the
            // shrunk path keeps the happy path zero-copy.
            return resolve_from_filtered(uref, &kind_filtered);
        };

        if candidates.len() == 1 {
            let ref_lang = lang_from_path(&uref.file_path);
            let candidate_lang = lang_from_path(&candidates[0].file_path);
            let confidence = if ref_lang != "unknown"
                && candidate_lang != "unknown"
                && ref_lang != candidate_lang
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

        // Multiple candidates -- score them and pick the best.
        let best = Self::find_best_match(uref, candidates, &self.import_index)?;

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: best.id.clone(),
            confidence: 0.7,
            resolved_by: "exact-match".to_string(),
        })
    }

    fn try_exact_name_match_simple(
        &self,
        uref: &UnresolvedRef,
        simple_name: &str,
    ) -> Option<ResolvedRef> {
        if CROSS_FILE_BLOCKLIST.contains(&simple_name) {
            let candidates = self.name_cache.get(simple_name)?;
            // Same fix as `try_exact_name_match`: filter by kind before
            // returning a same-file blocklisted match.
            let same_file: Vec<_> = candidates
                .iter()
                .filter(|n| n.file_path == uref.file_path)
                .filter(|n| kind_compatible(uref.reference_kind, &n.kind))
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
            .filter(|n| kind_compatible(uref.reference_kind, &n.kind))
            .collect();
        if kind_filtered.is_empty() {
            return None;
        }
        let candidates: &[Node] = if kind_filtered.len() == raw_candidates.len() {
            raw_candidates
        } else {
            return resolve_from_filtered_named(uref, &kind_filtered, "simple-name-match");
        };

        if candidates.len() == 1 {
            let ref_lang = lang_from_path(&uref.file_path);
            let candidate_lang = lang_from_path(&candidates[0].file_path);
            let confidence = if ref_lang != "unknown"
                && candidate_lang != "unknown"
                && ref_lang != candidate_lang
            {
                0.5
            } else {
                0.9
            };
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: candidates[0].id.clone(),
                confidence,
                resolved_by: "simple-name-match".to_string(),
            });
        }

        let best = Self::find_best_match(uref, candidates, &self.import_index)?;

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: best.id.clone(),
            confidence: 0.7,
            resolved_by: "simple-name-match".to_string(),
        })
    }

    /// Scores candidate nodes for a reference and returns the best match.
    ///
    /// Scoring heuristics:
    /// - Same file as reference: +100
    /// - Directory proximity (shared path segments): +5 per segment, capped at +40
    /// - Same language: +50, cross-language: -80
    /// - Exported / pub visibility: +10
    /// - Callable kind (function/method) when the ref kind is `Calls`: +25
    /// - Line proximity (same file only): +20 - (`line_distance` / 10)
    /// - Import match (caller imports this name): +30
    fn find_best_match(
        uref: &UnresolvedRef,
        candidates: &[Node],
        import_index: &HashMap<String, HashSet<String>>,
    ) -> Option<Node> {
        if candidates.is_empty() {
            return None;
        }

        let ref_lang = lang_from_path(&uref.file_path);
        let mut best_score = i64::MIN;
        let mut best_node: Option<&Node> = None;

        for node in candidates {
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
                if ref_lang == candidate_lang {
                    score += 50;
                } else {
                    score -= 80;
                }
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

            if score > best_score {
                best_score = score;
                best_node = Some(node);
            }
        }

        best_node.cloned()
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
fn kind_compatible(ref_kind: EdgeKind, target_kind: &NodeKind) -> bool {
    match ref_kind {
        EdgeKind::Implements | EdgeKind::Extends | EdgeKind::DerivesMacro => matches!(
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
        ),
        EdgeKind::Calls => matches!(
            target_kind,
            NodeKind::Function
                | NodeKind::Method
                | NodeKind::StructMethod
                | NodeKind::Constructor
                | NodeKind::AbstractMethod
                | NodeKind::ArrowFunction
                | NodeKind::Procedure
                | NodeKind::Macro
        ),
        EdgeKind::Annotates => matches!(
            target_kind,
            NodeKind::Annotation | NodeKind::Decorator | NodeKind::AnnotationUsage
        ),
        // Uses / TypeOf / Returns / Contains / Receives — permissive.
        _ => true,
    }
}

/// Resolution helper used after the kind filter has reduced the
/// candidate list to a strict subset of `name_cache`. Mirrors the
/// single-candidate / multi-candidate branches of
/// `try_exact_name_match` but operates on the borrowed slice.
fn resolve_from_filtered(uref: &UnresolvedRef, kind_filtered: &[&Node]) -> Option<ResolvedRef> {
    resolve_from_filtered_named(uref, kind_filtered, "exact-match")
}

fn resolve_from_filtered_named(
    uref: &UnresolvedRef,
    kind_filtered: &[&Node],
    resolved_by: &str,
) -> Option<ResolvedRef> {
    if kind_filtered.len() == 1 {
        return Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: kind_filtered[0].id.clone(),
            confidence: 0.85,
            resolved_by: resolved_by.to_string(),
        });
    }
    // Multiple kind-compatible candidates: pick the first one in the
    // same file as the reference if possible, otherwise the first
    // overall. Real scoring (`find_best_match`) wants `&[Node]` and
    // these are `&[&Node]`; rather than re-allocate to satisfy it we
    // accept this coarser heuristic, which still beats the previous
    // behaviour of picking whatever node happened to match by name.
    let pick = kind_filtered
        .iter()
        .find(|n| n.file_path == uref.file_path)
        .copied()
        .or_else(|| kind_filtered.first().copied())?;
    Some(ResolvedRef {
        original: uref.clone(),
        target_node_id: pick.id.clone(),
        confidence: 0.65,
        resolved_by: resolved_by.to_string(),
    })
}
