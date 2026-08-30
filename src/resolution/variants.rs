// Rust guideline compliant 2026-06-17
//! Build-variant edge propagation.
//!
//! Conditionally-compiled code defines the *same logical symbol* more than
//! once, one definition per mutually-exclusive build configuration:
//!
//! - **Rust** — `#[cfg(target_os = "macos")] fn f()` next to
//!   `#[cfg(not(target_os = "macos"))] fn f()` in the same module. Both share
//!   one `qualified_name`.
//! - **Go** — `f()` declared in `foo_linux.go` and again in `foo_windows.go`
//!   within one package (filename suffix or `//go:build` constraint). These
//!   live in *different files*, so their `qualified_name`s differ; the package
//!   (directory) plus the function name is what they share.
//!
//! The name-based resolver binds a call site to exactly one of the variants,
//! leaving the others with zero incoming edges — so dead-code analysis reports
//! the inactive-platform definition as dead (#141) even though deleting it
//! breaks that platform's build.
//!
//! This pass groups callable nodes into build-variant sets and, when a call
//! lands on any member, replicates that `calls` edge to every sibling so the
//! whole set is seen as reachable. It is a recall-improving over-approximation
//! deliberately scoped to genuine build variants, not arbitrary name clashes.

use std::collections::{HashMap, HashSet};

use crate::types::{Edge, EdgeKind, Node, NodeKind};

/// Callable node kinds a `Calls` edge can target.
fn is_callable(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function
            | NodeKind::Method
            | NodeKind::SingletonMethod
            | NodeKind::StructMethod
            | NodeKind::Constructor
            | NodeKind::AbstractMethod
    )
}

/// Directory portion of a path (the Go "package" proxy). `a/b/c.go` -> `a/b`.
fn parent_dir(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// Returns additional `calls` edges propagating a call that landed on one
/// build-config variant of a symbol to all its sibling variants. Idempotent:
/// edges that already exist are skipped, and the caller's unique edge index
/// collapses any that slip through.
/// `edges` need only contain the `Annotates` and `Calls` edges of the graph.
///
/// Those are the only two kinds read: `Annotates` to find the `#[cfg]`-gated
/// nodes, `Calls` to index what is called and which pairs already exist. The
/// sync paths rely on this and pass a kind-filtered slice rather than the whole
/// edge table (#418), so a change here that starts reading a third kind must
/// widen those queries too — `tests/resolution_test.rs` pins the equivalence.
pub fn propagate_variant_edges(nodes: &[Node], edges: &[Edge]) -> Vec<Edge> {
    let node_by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Rust: node ids carrying a `#[cfg]` / `#[cfg_attr]` attribute, i.e. the
    // target of an `annotates` edge from a `cfg`-named annotation_usage node.
    // Without this guard, two distinct trait impls that happen to share a
    // qualified_name (`From<A>`/`From<B>` both named `from`) would be fused.
    let mut cfg_gated: HashSet<&str> = HashSet::new();
    for e in edges {
        if e.kind == EdgeKind::Annotates {
            if let Some(src) = node_by_id.get(e.source.as_str()) {
                if src.kind == NodeKind::AnnotationUsage
                    && (src.name == "cfg" || src.name == "cfg_attr")
                {
                    cfg_gated.insert(e.target.as_str());
                }
            }
        }
    }

    // Group callable nodes into build-variant sets by a language-specific key.
    let mut groups: HashMap<String, Vec<&str>> = HashMap::new();
    for n in nodes {
        if !is_callable(&n.kind) {
            continue;
        }
        let ext = std::path::Path::new(&n.file_path).extension();
        if ext.is_some_and(|e| e.eq_ignore_ascii_case("rs")) {
            // Rust variants share a qualified_name and are both cfg-gated.
            if cfg_gated.contains(n.id.as_str()) {
                groups
                    .entry(format!("rs\u{1}{}", n.qualified_name))
                    .or_default()
                    .push(n.id.as_str());
            }
        } else if ext.is_some_and(|e| e.eq_ignore_ascii_case("go")) && n.kind == NodeKind::Function
        {
            // Go package-level funcs: same package directory + name. Two such
            // declarations can only coexist under build constraints (the
            // compiler forbids redeclaration otherwise), so >=2 members across
            // the package is itself the build-variant signal.
            groups
                .entry(format!(
                    "go\u{1}{}\u{1}{}",
                    parent_dir(&n.file_path),
                    n.name
                ))
                .or_default()
                .push(n.id.as_str());
        }
    }

    emit_variant_edges(&groups, edges)
}

/// Node kinds that can be a build variant, as SQL string literals.
///
/// Kept beside [`is_callable`] so the in-memory and SQL paths cannot drift:
/// a kind added to one must be added to the other, and
/// `callable_kinds_match_is_callable` fails if they disagree.
pub const CALLABLE_KIND_NAMES: &[&str] = &[
    "function",
    "method",
    "singleton_method",
    "struct_method",
    "constructor",
    "abstract_method",
];

/// Build the variant groups from the SQL candidate rows (#481).
///
/// The Rust rows arrive already keyed and already gated on `cfg`, so they pass
/// straight through. The Go rows are a superset — filtered on a repeated
/// *name* because SQLite has no `dirname` — so the exact `(directory, name)`
/// grouping happens here, matching what the whole-graph pass builds.
///
/// Groups with a single member are dropped: they cannot propagate anything,
/// and dropping them here is what keeps the follow-up edge query small.
pub fn variant_groups_from_candidates<'a>(
    rust: &'a [(String, String)],
    go: &'a [(String, String, String)],
) -> HashMap<String, Vec<&'a str>> {
    let mut groups: HashMap<String, Vec<&'a str>> = HashMap::new();
    for (key, id) in rust {
        groups.entry(key.clone()).or_default().push(id.as_str());
    }
    for (file_path, name, id) in go {
        groups
            .entry(format!("go\u{1}{}\u{1}{}", parent_dir(file_path), name))
            .or_default()
            .push(id.as_str());
    }
    groups.retain(|_, members| members.len() >= 2);
    groups
}

/// Emit the propagated edges for pre-computed variant groups.
///
/// Split out of [`propagate_variant_edges`] so the whole-graph caller and the
/// SQL-backed one run identical logic (#481) — the two differ only in how the
/// groups and the candidate edges are obtained, and a second copy of this
/// would be a correctness risk for no benefit.
///
/// `edges` need only contain the `calls` edges whose target is a group member.
/// Both uses are member-targeted: `incoming` is keyed by member, and the
/// duplicate check asks whether `(source, sibling)` already exists, where
/// `sibling` is itself a member of the same group.
pub fn emit_variant_edges<S: std::hash::BuildHasher>(
    groups: &HashMap<String, Vec<&str>, S>,
    edges: &[Edge],
) -> Vec<Edge> {
    // Index existing `calls` edges: which targets are called, and the (src,dst)
    // pairs already present (so we never emit a duplicate).
    let mut incoming: HashMap<&str, Vec<&Edge>> = HashMap::new();
    let mut existing: HashSet<(&str, &str)> = HashSet::new();
    for e in edges {
        if e.kind == EdgeKind::Calls {
            incoming.entry(e.target.as_str()).or_default().push(e);
            existing.insert((e.source.as_str(), e.target.as_str()));
        }
    }

    let mut out = Vec::new();
    let mut emitted: HashSet<(String, String)> = HashSet::new();
    for members in groups.values() {
        if members.len() < 2 {
            continue;
        }
        for &m in members {
            let Some(in_edges) = incoming.get(m) else {
                continue;
            };
            for e in in_edges {
                for &sibling in members {
                    if sibling == m || e.source == sibling {
                        continue; // no self-edge, no re-target onto the source
                    }
                    if existing.contains(&(e.source.as_str(), sibling)) {
                        continue;
                    }
                    if emitted.insert((e.source.clone(), sibling.to_string())) {
                        out.push(Edge {
                            source: e.source.clone(),
                            target: sibling.to_string(),
                            kind: EdgeKind::Calls,
                            line: e.line,
                        });
                    }
                }
            }
        }
    }
    out
}
