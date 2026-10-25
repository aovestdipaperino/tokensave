//! Module-level import graph, cycles, and cut simulation.
//!
//! `tokensave_circular` answers dependency questions at the *file* level using
//! `calls`/`uses` edges. Planning a decomposition needs a different unit and a
//! different edge (#334): which packages form an import cycle, how many import
//! statements actually hold a pair of them together, and whether deleting one
//! of those dependencies would break the cycle or leave everything still
//! mutually reachable.
//!
//! Symbol edges are a poor proxy for that. One `from mod_b import X` may produce
//! zero `calls` edges or fifty depending on how the name is used, so counting
//! calls says nothing about how many statements a refactor would have to touch.
//!
//! No new edge kind was needed. Every extractor already emits a `Use` node per
//! import statement, carrying the imported path, the statement's line, and its
//! source text, and the resolver already binds that node to the symbol it names.
//! The import graph is therefore a projection of data the index already holds.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::db::Database;
use crate::errors::{Result, TokenSaveError};

/// One import statement, as written in the source.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImportSite {
    /// File containing the import statement.
    pub file: String,
    /// 1-based line of the statement.
    pub line: u32,
    /// The imported path, as spelled in the source.
    pub imported: String,
    /// The statement text, when the extractor recorded it.
    pub statement: Option<String>,
    /// File the import resolved to.
    pub resolved_file: String,
    /// True when the import sits inside a function body rather than at module
    /// level. A lazy import is usually there *because* of a cycle, and costs
    /// much less to remove than a module-level one.
    pub lazy: bool,
    /// True when the statement is syntactically type-only (`import type`).
    ///
    /// Only languages that mark this in the statement itself are detected.
    /// Python's `if TYPE_CHECKING:` guard is a property of the enclosing block,
    /// which the index does not currently record, so those read as ordinary
    /// imports rather than being silently guessed at.
    pub type_only: bool,
}

/// A directed dependency between two modules, and the statements creating it.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleDependency {
    pub from: String,
    pub to: String,
    /// Every import statement holding this dependency together. Its length is
    /// the number of edits a refactor removing the dependency would need.
    pub sites: Vec<ImportSite>,
}

/// The module-level import graph.
#[derive(Debug, Clone, Default)]
pub struct ModuleImportGraph {
    /// Dependencies keyed by `(from, to)`.
    edges: HashMap<(String, String), Vec<ImportSite>>,
    /// Every module seen, including leaves with no dependencies.
    modules: HashSet<String>,
}

impl ModuleImportGraph {
    /// Adjacency map over modules, for the SCC routines.
    #[must_use]
    pub fn adjacency(&self) -> HashMap<String, HashSet<String>> {
        let mut adj: HashMap<String, HashSet<String>> = self
            .modules
            .iter()
            .map(|module| (module.clone(), HashSet::new()))
            .collect();
        for (from, to) in self.edges.keys() {
            adj.entry(from.clone()).or_default().insert(to.clone());
        }
        adj
    }

    /// All dependencies, sorted by `(from, to)` so output is stable.
    #[must_use]
    pub fn dependencies(&self) -> Vec<ModuleDependency> {
        let mut out: Vec<ModuleDependency> = self
            .edges
            .iter()
            .map(|((from, to), sites)| {
                let mut sites = sites.clone();
                sites.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
                ModuleDependency {
                    from: from.clone(),
                    to: to.clone(),
                    sites,
                }
            })
            .collect();
        out.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
        out
    }

    /// Module groups that are mutually reachable, sorted for stable output.
    #[must_use]
    pub fn cycles(&self) -> Vec<Vec<String>> {
        let adj = self.adjacency();
        let mut cycles: Vec<Vec<String>> = super::scc::tarjan_scc(&adj)
            .into_iter()
            .filter(|scc| super::scc::is_cyclic_scc(scc, &adj))
            .collect();
        for cycle in &mut cycles {
            cycle.sort_unstable();
        }
        cycles.sort();
        cycles
    }

    /// Cycles that would remain if the `from -> to` dependency were removed.
    ///
    /// This is the question that decides whether a proposed cut is worth
    /// making: a cut that leaves every module still mutually reachable buys
    /// nothing, and nothing short of recomputing the components can tell the
    /// two apart.
    #[must_use]
    pub fn cycles_without(&self, from: &str, to: &str) -> Vec<Vec<String>> {
        let mut adj = self.adjacency();
        if let Some(targets) = adj.get_mut(from) {
            targets.remove(to);
        }
        let mut cycles: Vec<Vec<String>> = super::scc::tarjan_scc(&adj)
            .into_iter()
            .filter(|scc| super::scc::is_cyclic_scc(scc, &adj))
            .collect();
        for cycle in &mut cycles {
            cycle.sort_unstable();
        }
        cycles.sort();
        cycles
    }
}

/// Groups a file path into its module name at `depth` path components.
///
/// Depth 1 over `anomaly/alerts/slack.py` gives `anomaly`; depth 2 gives
/// `anomaly/alerts`. A file with fewer components than `depth` is its own
/// module, so a top-level script never silently merges into a package it is
/// not part of.
#[must_use]
pub fn module_of(file_path: &str, depth: usize) -> String {
    let depth = depth.max(1);
    let components: Vec<&str> = file_path.split('/').collect();
    // The last component is the file itself, never a module directory.
    let dirs = components.len().saturating_sub(1);
    if dirs == 0 {
        return file_path.to_string();
    }
    components[..depth.min(dirs)].join("/")
}

/// Does this statement mark itself as type-only?
fn is_type_only(statement: Option<&str>) -> bool {
    let Some(text) = statement else {
        return false;
    };
    let trimmed = text.trim_start();
    // TypeScript/Flow spell it in the statement. Matching on the leading
    // keywords keeps a variable named `type` in an ordinary import from
    // reading as a type-only one.
    trimmed.starts_with("import type ")
        || trimmed.starts_with("export type ")
        || trimmed.starts_with("import type{")
}

impl Database {
    /// Builds the module-level import graph at the given grouping depth.
    ///
    /// Only resolved, cross-file imports contribute: an import of a third-party
    /// package resolves to nothing and is not a dependency between this
    /// project's modules, and a same-module import is not a dependency at all.
    pub async fn build_module_import_graph(&self, depth: usize) -> Result<ModuleImportGraph> {
        // `n1` is the Use node the extractor emitted for the import statement;
        // `n2` is whatever the resolver bound it to. `Contains` is stored as
        // `nodes.parent_id` rather than an edge row, and that parent is what
        // distinguishes a module-level import (parent is the file) from a lazy
        // one written inside a function body.
        let sql = "SELECT DISTINCT n1.file_path, n1.start_line, n1.name, n1.signature, \
                   n2.file_path, COALESCE(parent.kind, 'file') \
                   FROM edges e \
                   JOIN nodes n1 ON e.source = n1.id \
                   JOIN nodes n2 ON e.target = n2.id \
                   LEFT JOIN nodes parent ON parent.id = n1.parent_id \
                   WHERE e.kind = 'uses' AND n1.kind = 'use' \
                   AND n1.file_path != n2.file_path";

        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query import graph: {e}"),
                operation: "build_module_import_graph".to_string(),
            })?;

        let mut graph = ModuleImportGraph::default();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read import row: {e}"),
            operation: "build_module_import_graph".to_string(),
        })? {
            let file: String = row.get(0).unwrap_or_default();
            let line: u32 = row.get(1).unwrap_or(0);
            let imported: String = row.get(2).unwrap_or_default();
            let statement: Option<String> = row.get(3).ok();
            let resolved_file: String = row.get(4).unwrap_or_default();
            let parent_kind: String = row.get(5).unwrap_or_else(|_| "file".to_string());

            let from = module_of(&file, depth);
            let to = module_of(&resolved_file, depth);
            graph.modules.insert(from.clone());
            graph.modules.insert(to.clone());
            if from == to {
                continue;
            }

            let site = ImportSite {
                file,
                line: line.saturating_add(1),
                imported,
                type_only: is_type_only(statement.as_deref()),
                statement,
                resolved_file,
                lazy: parent_kind != "file",
            };
            let sites = graph.edges.entry((from, to)).or_default();
            if !sites.contains(&site) {
                sites.push(site);
            }
        }

        Ok(graph)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn module_of_groups_by_depth() {
        assert_eq!(module_of("anomaly/alerts/slack.py", 1), "anomaly");
        assert_eq!(module_of("anomaly/alerts/slack.py", 2), "anomaly/alerts");
        // Depth beyond the directory nesting stops at the containing directory
        // rather than folding the filename in as if it were a package.
        assert_eq!(module_of("anomaly/alerts/slack.py", 9), "anomaly/alerts");
    }

    #[test]
    fn a_top_level_file_is_its_own_module() {
        // Otherwise every root-level script would collapse into one module and
        // appear to import itself.
        assert_eq!(module_of("setup.py", 1), "setup.py");
        assert_eq!(module_of("setup.py", 3), "setup.py");
    }

    #[test]
    fn depth_zero_is_treated_as_one() {
        assert_eq!(module_of("a/b/c.py", 0), "a");
    }

    #[test]
    fn type_only_needs_the_keyword_not_just_the_word() {
        assert!(is_type_only(Some("import type { Foo } from './foo'")));
        assert!(!is_type_only(Some("import { type_registry } from './x'")));
        assert!(!is_type_only(Some("from typing import TYPE_CHECKING")));
        assert!(!is_type_only(None));
    }
}
