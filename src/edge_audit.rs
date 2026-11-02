//! Audit of bare-name resolution quality over a built index (#536).
//!
//! The impossible-edge metric used to evaluate #533 counted edges crossing
//! from production into `tests/`. That is a proxy, and a lossy one: measured
//! against its own parent, #533 removed 1,216 edges on a 515-file Python
//! project while the metric saw 731. The other 485 were the identical defect
//! landing production → production, where a directory-crossing test cannot see
//! them — which read as 60% precision when the evidence said close to 100%.
//!
//! This counts what the gate actually governs instead: cross-file edges
//! resolved through the bare-name path, in a language where a bare name is not
//! by itself evidence of a binding, whose target is the sole symbol of that
//! name in the index. That population needs no test/production classification,
//! so it is not sensitive to repository layout.
//!
//! It is a **population count, not a phantom count.** Every edge reported here
//! passed `is_plausibly_reachable` at resolution time — the ones that failed it
//! are absent from the index and cannot be counted from the index. The number
//! is read comparatively: index a tree at two commits and diff the counts, the
//! way #533 was measured. A resolution change that removes phantoms shows up
//! as a drop here whether the phantom crossed into `tests/` or not.

use crate::db::{Database, EdgeAuditRow as EdgeRow};
use crate::errors::Result;
use std::collections::{HashMap, HashSet};

/// Languages where a bare name alone is not evidence of a binding, mirroring
/// `resolution::resolver::bare_name_needs_evidence`. Ruby is deliberately
/// absent there and so is absent here; the two lists must not drift.
fn is_gated_path(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or_default();
    matches!(
        ext,
        "py" | "pyi" | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx"
    )
}

fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// Whether `imports` for a file carries evidence of `name`.
///
/// Mirrors the resolver's reading: an entry matches when it equals the name
/// outright or ends with it as a dotted segment, so `import pkg.mod.analyzer`
/// counts as importing `analyzer`.
fn imports_name(imports: &HashSet<String>, name: &str) -> bool {
    imports
        .iter()
        .any(|entry| entry == name || entry.rsplit('.').next() == Some(name))
}

/// One target that many bare-name references collapsed onto.
#[derive(Debug, Clone)]
pub struct HotTarget {
    pub name: String,
    pub file_path: String,
    pub start_line: i64,
    pub kind: String,
    /// How many distinct source files hold an unreachable edge to this target.
    pub source_files: i64,
    pub edges: i64,
}

/// The result of an edge audit over one index.
#[derive(Debug, Clone)]
pub struct EdgeAudit {
    pub total_edges: i64,
    /// Edges whose source file is in a gated language.
    pub gated_edges: i64,
    /// ...of which cross a file boundary.
    pub cross_file: i64,
    /// ...of which also target the sole symbol of that name in the index.
    pub sole_candidate_cross_file: i64,
    /// ...of which the source file has no evidence it can reach. This is the
    /// figure to diff between two commits.
    pub unreachable: i64,
    /// The targets absorbing the most unreachable edges, largest first.
    pub hot_targets: Vec<HotTarget>,
}

/// Runs the audit over `db`.
///
/// `limit` caps how many hot targets are listed.
pub async fn audit(db: &Database, limit: usize) -> Result<EdgeAudit> {
    // Import evidence per file. Imports are graph nodes of kind `use`, which
    // is what lets the gate's predicate be evaluated after the fact rather
    // than only during resolution.
    let mut imports: HashMap<String, HashSet<String>> = HashMap::new();
    db.for_each_use_node(|file, name| {
        imports.entry(file).or_default().insert(name);
    })
    .await?;

    // How many symbols share each name, so "sole candidate" is a lookup.
    let mut name_counts: HashMap<String, i64> = HashMap::new();
    db.for_each_symbol_name_count(|name, count| {
        name_counts.insert(name, count);
    })
    .await?;

    // Parent names, for the "the class that owns the method is the evidence"
    // arm of the resolver's predicate.
    let mut node_name: HashMap<String, String> = HashMap::new();
    db.for_each_node_name(|id, name| {
        node_name.insert(id, name);
    })
    .await?;

    let mut total_edges = 0i64;
    let mut gated_edges = 0i64;
    let mut cross_file = 0i64;
    let mut sole_candidate_cross_file = 0i64;
    let mut unreachable = 0i64;
    let mut per_target: HashMap<String, (EdgeRow, i64, HashSet<String>)> = HashMap::new();

    db.for_each_edge_audit_row(|r| {
        total_edges += 1;

        if !is_gated_path(&r.src_file) {
            return;
        }
        gated_edges += 1;

        if r.src_file == r.dst_file {
            return;
        }
        cross_file += 1;

        if name_counts.get(&r.dst_name).copied().unwrap_or(0) != 1 {
            return;
        }
        sole_candidate_cross_file += 1;

        // The resolver's evidence model, evaluated after the fact: same
        // directory, the name imported, or the owning class imported.
        if dir_of(&r.src_file) == dir_of(&r.dst_file) {
            return;
        }
        let file_imports = imports.get(&r.src_file);
        let reachable = file_imports.is_some_and(|imports| {
            imports_name(imports, &r.dst_name)
                || r.dst_parent
                    .as_deref()
                    .and_then(|id| node_name.get(id))
                    .is_some_and(|parent| imports_name(imports, parent))
        });
        if reachable {
            return;
        }

        unreachable += 1;
        let entry = per_target
            .entry(r.dst_id.clone())
            .or_insert_with(|| (r, 0, HashSet::new()));
        entry.1 += 1;
        entry.2.insert(entry.0.src_file.clone());
    })
    .await?;

    // `src_file` on the stored row is whichever edge landed first, so the
    // distinct-source count is tracked separately rather than read off it.
    let mut hot: Vec<(EdgeRow, i64, HashSet<String>)> = per_target.into_values().collect();
    hot.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.dst_name.cmp(&b.0.dst_name)));
    let hot_targets = hot
        .into_iter()
        .take(limit)
        .map(|(r, edges, files)| HotTarget {
            name: r.dst_name,
            file_path: r.dst_file,
            start_line: r.dst_line,
            kind: r.dst_kind,
            source_files: files.len() as i64,
            edges,
        })
        .collect();

    Ok(EdgeAudit {
        total_edges,
        gated_edges,
        cross_file,
        sole_candidate_cross_file,
        unreachable,
        hot_targets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gating_follows_the_resolver_language_list() {
        for path in [
            "a/b.py", "a/b.pyi", "x.js", "x.jsx", "x.mjs", "x.ts", "x.tsx",
        ] {
            assert!(is_gated_path(path), "{path} should be gated");
        }
        // Rust and Go resolve bare names through a module system the gate
        // cannot see, and Ruby is deliberately excluded in the resolver.
        for path in ["src/main.rs", "m.go", "a.rb", "x.java", "no_extension"] {
            assert!(!is_gated_path(path), "{path} must not be gated");
        }
    }
}
