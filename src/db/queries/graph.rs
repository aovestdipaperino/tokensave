//! Graph-analysis reads backing `GraphQueryManager`: dead-code candidates,
//! `GDScript` override liveness and file-level dependency edges.
use std::collections::{HashMap, HashSet};

use super::{get_opt_string_lossy, get_string_lossy};
use crate::db::Database;
use crate::errors::{Result, TokenSaveError};
use crate::types::{Node, NodeKind, Visibility};

/// Godot virtual callbacks that the engine invokes on `Object`, `Node`,
/// `CanvasItem`, `Control`, `Resource` and the physics bodies. A `.gd`
/// method with one of these names has no caller in project code (#598).
const GODOT_ENGINE_VIRTUALS: &[&str] = &[
    "_init",
    "_static_init",
    "_ready",
    "_enter_tree",
    "_exit_tree",
    "_process",
    "_physics_process",
    "_input",
    "_unhandled_input",
    "_unhandled_key_input",
    "_shortcut_input",
    "_gui_input",
    "_notification",
    "_draw",
    "_to_string",
    "_get",
    "_set",
    "_get_property_list",
    "_validate_property",
    "_property_can_revert",
    "_property_get_revert",
    "_get_configuration_warnings",
    "_integrate_forces",
    "_has_point",
    "_get_minimum_size",
    "_make_custom_tooltip",
    "_get_drag_data",
    "_can_drop_data",
    "_drop_data",
    "_structured_text_parser",
    "_setup_local_to_scene",
    "_run",
];

fn row_to_node_dead_code(row: &libsql::Row) -> std::result::Result<Node, libsql::Error> {
    let kind_str = get_string_lossy(row, 1)?;
    let vis_str = get_string_lossy(row, 11)?;
    let is_async_int = row.get::<i64>(12)?;
    let start_line = row.get::<u32>(5)?;
    // Pre-v7 rows lack attrs_start_line; fall back to start_line. The dead-code
    // SELECT below does not include the new column, so attempts to read at 21
    // will always error here — keep the fallback path explicit.
    let attrs_raw = row.get::<u32>(21).unwrap_or(0);
    let attrs_start_line = if attrs_raw == 0 {
        start_line
    } else {
        attrs_raw
    };

    Ok(Node {
        id: get_string_lossy(row, 0)?,
        kind: NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Function),
        name: get_string_lossy(row, 2)?,
        qualified_name: get_string_lossy(row, 3)?,
        file_path: get_string_lossy(row, 4)?,
        start_line,
        attrs_start_line,
        end_line: row.get::<u32>(6)?,
        start_column: row.get::<u32>(7)?,
        end_column: row.get::<u32>(8)?,
        signature: get_opt_string_lossy(row, 10)?,
        docstring: get_opt_string_lossy(row, 9)?,
        visibility: Visibility::from_str(&vis_str).unwrap_or_default(),
        is_async: is_async_int != 0,
        branches: row.get::<u32>(13)?,
        loops: row.get::<u32>(14)?,
        returns: row.get::<u32>(15)?,
        max_nesting: row.get::<u32>(16)?,
        unsafe_blocks: row.get::<u32>(17)?,
        unchecked_calls: row.get::<u32>(18)?,
        assertions: row.get::<u32>(19)?,
        // The dead-code SELECT does not request the issue #150 health columns;
        // they are irrelevant to dead-code analysis, so default them to 0.
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: row.get::<u64>(20)?,
        parent_id: None,
    })
}

impl Database {
    /// Returns the nodes that have no incoming use-site edge and are not
    /// exempted as entry points, test functions or (by default) public or
    /// trait-impl items. `GraphQueryManager::find_dead_code` documents the
    /// exemptions and applies the `GDScript` override filter on top.
    pub async fn find_dead_code_candidates(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
        include_trait_impls: bool,
    ) -> Result<Vec<Node>> {
        let kind_filter = if kinds.is_empty() {
            String::new()
        } else {
            let kind_strs: Vec<String> =
                kinds.iter().map(|k| format!("'{}'", k.as_str())).collect();
            format!(" AND kind IN ({})", kind_strs.join(", "))
        };
        let visibility_filter = if include_public {
            ""
        } else {
            " AND visibility != 'public'"
        };

        // Trait-impl methods (Rust `impl Trait for Type` blocks) are reached
        // implicitly by the compiler — `Display::fmt` via `{}`, `Deref::deref`
        // via auto-deref, `Drop::drop` via drop glue, `From::from` via `?`/
        // `.into()` — so they carry no explicit incoming `calls` edge. They
        // also can't be written `pub` in Rust, so they're stored as `private`
        // and the visibility filter above never excludes them. The result is
        // a flood of false positives (issue #137). Exclude any method whose
        // parent is an `Impl`-kind node that is a *trait* impl (vs an inherent
        // `impl Type` block, whose un-called methods are genuine dead code).
        //
        // We detect a trait impl two ways, OR'd together:
        //   1) the impl node's signature contains ` for ` — i.e. the header is
        //      `impl <Trait> for <Type>`. This is the PRIMARY signal and the
        //      only one that works for the common case, because…
        //   2) …the impl node bears an outgoing `implements` edge. This edge
        //      only materializes when the trait resolves to an indexed node,
        //      which it does NOT for stdlib/external traits (`Display`, `Drop`,
        //      `Deref`, `From`, `AsRef`, …) — precisely the traits in the bug
        //      report. So the edge alone misses almost every real case (mex has
        //      zero `implements` edges); the signature scan is what carries the
        //      fix. The edge is kept as a fallback for multi-line impl headers
        //      where `extract_first_line` truncates before the ` for `.
        //
        // Inherent-impl signatures (`impl Type`) and ObjC `@implementation Foo`
        // blocks contain no ` for `, so they are untouched — inherent dead
        // methods are still reported. Class-based languages attach `implements`
        // to the `Class` node (not an `impl`) and store interface methods
        // `public`, so the visibility filter already covers them. The guarding
        // `parent_id IS NULL OR` is required: `NULL NOT IN (...)` is NULL, not
        // true, which would silently drop every top-level function.
        let trait_impl_filter = if include_trait_impls {
            ""
        } else {
            " AND (parent_id IS NULL OR parent_id NOT IN (
                 SELECT p.id FROM nodes p
                 WHERE p.kind = 'impl'
                   AND (p.signature LIKE '% for %'
                        OR p.id IN (SELECT source FROM edges WHERE kind = 'implements'))
             ))"
        };

        // Only true "use sites" count as evidence that a symbol is alive:
        // `calls`, `implements`, `extends`, `type_of`, `returns`, `receives`,
        // `uses`. Bookkeeping edges (`contains`, `annotates`, `derives_macro`)
        // are emitted by the extractor even for unused code — every Rust
        // function tagged `#[inline]`, every `#[derive(Debug)]` on an unused
        // struct, every annotation_usage adds an incoming edge that is NOT a
        // real reference. Previously we excluded only `contains`, so any
        // attribute on an otherwise-unused function masked it from
        // dead-code detection — which is why the sonium run reported zero
        // dead functions across 5,715. The narrower allowlist below restores
        // the intended semantics: "no real caller / referencer = dead".
        // Test-marker exclusion uses a THREE-step "resolve + pre-join + probe":
        //   1) `collect_test_marker_ids` runs the leading-wildcard `LIKE`
        //      scan exactly once over `kind = 'annotation_usage'`.
        //   2) Those ids land in `temp.test_markers` (PK on `id`).
        //   3) `temp.test_annotated_targets` is built from
        //      `edges WHERE kind='annotates' AND source IN temp.test_markers`
        //      — i.e., "which node ids are annotated by ANY test marker."
        //      ~15 K rows on chromium.
        //   4) The dead-code SELECT then uses
        //      `nodes.id NOT IN (SELECT target FROM temp.test_annotated_targets)`
        //      — a single PK probe per candidate against a small table.
        //
        // History — DO NOT regress this:
        // - The pre-4.14.8 form joined `nodes a ON a.id = e2.source` inside
        //   the correlated `NOT EXISTS` and ran the `a.name LIKE '%::test'`
        //   chain per matching edge. Fast on scirs (0.1 s, 76 K
        //   annotation_usage) but timed out on chromium at the 25 s probe
        //   ceiling, cascade-poisoning every subsequent MCP tool call.
        // - 4.14.8's `WITH test_marker_ids AS (...)` CTE attempt was even
        //   worse: SQLite inlined the single-reference CTE, turning every
        //   dead-candidate row into a full annotation_usage scan. Regressed
        //   scirs from 0.1 s to >60 s, reverted in 4.14.9.
        // - 4.14.12's first attempt (single temp table + correlated
        //   `IN (SELECT id FROM temp.test_markers)` inside `NOT EXISTS`)
        //   also failed on chromium: SQLite picked `idx_edges_unique
        //   (source, target, kind)` for the subquery and iterated every
        //   marker as the outer driver for every candidate
        //   (~13K markers × ~134K candidates ≈ 1.7B probes). Timed out.
        // - The current pre-join form (two temp tables, indexed `NOT IN`)
        //   measures 0.75 s on chromium and 0.6 s on scirs. The
        //   wildcard scan still runs exactly once per call; the per-row
        //   probe is now against a ~15K-row indexed lookup table, not a
        //   correlated subquery the optimiser can re-shape.
        let result = self
            .find_dead_code_candidates_inner(visibility_filter, &kind_filter, trait_impl_filter)
            .await;

        // Always drop both temp tables, even on the error path, so a
        // failed query does not leak rows to the next caller on the same
        // connection. Best-effort: a drop failure shouldn't mask the
        // original error.
        let _ = self.drop_test_annotated_targets_temp_table().await;
        let _ = self.drop_test_marker_temp_table().await;
        result
    }

    /// Body of `find_dead_code_candidates`. Split out so the caller can wrap
    /// it in a guaranteed `drop_*_temp_table()` even on the error path.
    async fn find_dead_code_candidates_inner(
        &self,
        visibility_filter: &str,
        kind_filter: &str,
        trait_impl_filter: &str,
    ) -> Result<Vec<Node>> {
        let marker_ids = self.collect_test_marker_ids().await?;
        self.populate_test_marker_temp_table(&marker_ids).await?;
        self.populate_test_annotated_targets_temp_table().await?;
        let godot_virtuals = GODOT_ENGINE_VIRTUALS
            .iter()
            .map(|n| format!("'{n}'"))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path, start_line, end_line,
                    start_column, end_column, docstring, signature, visibility,
                    is_async, branches, loops, returns, max_nesting, unsafe_blocks,
                    unchecked_calls, assertions, updated_at, attrs_start_line
             FROM nodes
             WHERE kind != 'route'
             AND name != 'main'
             AND name != '_bind_methods'
             AND name NOT LIKE 'test%'
             -- Go package-level `init` is invoked implicitly by the runtime at
             -- package load and can never be called explicitly, so it carries
             -- no incoming `calls` edge — the same reason `main`, `test*`, and
             -- Rust trait-impl methods are exempt. Scoped to `.go` so a callable
             -- function named `init` in another language is still checked (#346).
             AND NOT (name = 'init' AND file_path LIKE '%.go')
             -- Godot engine virtuals (`_ready`, `_process`, ...) are called by the
             -- engine, never by project code, so they have no incoming edge either.
             -- Scoped to `.gd` for the same reason as Go `init` (#598).
             AND NOT (file_path LIKE '%.gd' AND name IN ({godot_virtuals}))
             {visibility_filter}
             {kind_filter}
             {trait_impl_filter}
             AND NOT EXISTS (
                 SELECT 1 FROM edges
                 WHERE target = nodes.id
                 AND kind IN ('calls', 'implements', 'extends', 'type_of', 'returns', 'receives', 'uses')
             )
             AND id NOT IN (SELECT target FROM temp.test_annotated_targets)"
        );

        let mut rows = self
            .conn()
            .query(&sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to find dead code: {e}"),
                operation: "find_dead_code".to_string(),
            })?;

        let mut dead = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: "find_dead_code".to_string(),
        })? {
            let node = row_to_node_dead_code(&row)?;
            dead.push(node);
        }
        Ok(dead)
    }

    /// `GDScript` methods that override a base-class method which is called or
    /// used (#598).
    ///
    /// A base class calling `_fields()` on `self` resolves to the base
    /// declaration, yet for a subclass instance it is the override that runs.
    /// The override has no edge of its own, and adding a synthetic one would
    /// make `callers`/`callees` claim a call the source does not make, so the
    /// override is exempted here instead. Inheritance comes from resolved
    /// `extends` edges, followed transitively, so a grandchild override of a
    /// called method is live too; an override of a base method nothing
    /// reaches stays dead, like the base method itself.
    pub async fn gdscript_live_overrides(&self) -> Result<HashSet<String>> {
        let sql = "WITH RECURSIVE ancestry(cls, base) AS (
                 SELECT e.source, e.target FROM edges e
                 WHERE e.kind = 'extends'
                   AND e.source IN (SELECT id FROM nodes
                                    WHERE file_path LIKE '%.gd'
                                      AND kind IN ('class', 'inner_class'))
                 UNION
                 SELECT a.cls, e.target FROM ancestry a
                 JOIN edges e ON e.source = a.base AND e.kind = 'extends'
             )
             SELECT DISTINCT m.id FROM ancestry a
             JOIN nodes m ON m.parent_id = a.cls AND m.kind IN ('function', 'method')
             JOIN nodes b ON b.parent_id = a.base AND b.name = m.name
                         AND b.kind IN ('function', 'method')
             WHERE EXISTS (SELECT 1 FROM edges c
                           WHERE c.target = b.id AND c.kind IN ('calls', 'uses'))";
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to find live GDScript overrides: {e}"),
                operation: "gdscript_live_overrides".to_string(),
            })?;
        let mut live = HashSet::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: "gdscript_live_overrides".to_string(),
        })? {
            live.insert(row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read override id: {e}"),
                operation: "gdscript_live_overrides".to_string(),
            })?);
        }
        Ok(live)
    }
}

impl Database {
    /// Returns the distinct targets of `calls` and `uses` edges leaving any
    /// of `source_ids`.
    pub async fn get_call_use_target_ids(&self, source_ids: &[String]) -> Result<Vec<String>> {
        self.call_use_endpoint_ids(
            source_ids,
            "SELECT DISTINCT e.target FROM edges e \
             WHERE e.source IN ({}) AND e.kind IN ('uses', 'calls')",
            "get_file_dependencies",
            "file dependencies",
            "target",
        )
        .await
    }

    /// Returns the distinct sources of `calls` and `uses` edges entering any
    /// of `target_ids`.
    pub async fn get_call_use_source_ids(&self, target_ids: &[String]) -> Result<Vec<String>> {
        self.call_use_endpoint_ids(
            target_ids,
            "SELECT DISTINCT e.source FROM edges e \
             WHERE e.target IN ({}) AND e.kind IN ('uses', 'calls')",
            "get_file_dependents",
            "file dependents",
            "source",
        )
        .await
    }

    /// Runs one of the `IN ({})` endpoint templates above with `ids` bound
    /// as `?1..?N`. An empty `ids` returns no rows without querying.
    async fn call_use_endpoint_ids(
        &self,
        ids: &[String],
        template: &str,
        operation: &str,
        what: &str,
        column: &str,
    ) -> Result<Vec<String>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = template.replacen("{}", &placeholders.join(", "), 1);
        let param_values: Vec<libsql::Value> = ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();

        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query {what}: {e}"),
                operation: operation.to_string(),
            })?;

        let mut out: Vec<String> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read {column} id: {e}"),
            operation: operation.to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                out.push(id);
            }
        }
        Ok(out)
    }

    /// Returns the file-level `calls`/`uses` adjacency between distinct
    /// files, keyed by source file.
    ///
    /// When `prefix` is `Some`, it must already end with `/`; only pairs with
    /// both files under it are kept. Rows are filtered while streaming so the
    /// map is the only copy held in memory.
    pub async fn get_cross_file_call_use_adjacency(
        &self,
        prefix: Option<&str>,
    ) -> Result<HashMap<String, HashSet<String>>> {
        let sql = "SELECT DISTINCT n1.file_path AS src_file, n2.file_path AS tgt_file \
                   FROM edges e \
                   JOIN nodes n1 ON e.source = n1.id \
                   JOIN nodes n2 ON e.target = n2.id \
                   WHERE e.kind IN ('calls', 'uses') \
                   AND n1.file_path != n2.file_path";

        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file adjacency: {e}"),
                operation: "build_file_adjacency".to_string(),
            })?;

        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read adjacency row: {e}"),
            operation: "build_file_adjacency".to_string(),
        })? {
            let src: String = row.get(0).unwrap_or_default();
            let tgt: String = row.get(1).unwrap_or_default();

            if let Some(pfx) = prefix {
                if !src.starts_with(pfx) || !tgt.starts_with(pfx) {
                    continue;
                }
            }

            adj.entry(src).or_default().insert(tgt);
        }
        Ok(adj)
    }
}

/// One resolved cross-file import, as read by
/// [`Database::for_each_cross_file_import`].
pub(crate) struct CrossFileImportRow {
    /// File containing the import statement.
    pub file: String,
    /// Zero-based line of the `Use` node.
    pub start_line: u32,
    /// Imported path as the extractor recorded it.
    pub imported: String,
    /// Source text of the statement, when recorded.
    pub statement: Option<String>,
    /// File of the node the import resolved to.
    pub resolved_file: String,
    /// Kind of the `Use` node's parent; `"file"` for a module-level import.
    pub parent_kind: String,
}

impl Database {
    /// Calls `f` for each resolved `uses` edge from a `Use` node to a node in
    /// another file, streaming rows rather than collecting them.
    pub(crate) async fn for_each_cross_file_import(
        &self,
        mut f: impl FnMut(CrossFileImportRow),
    ) -> Result<()> {
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

        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read import row: {e}"),
            operation: "build_module_import_graph".to_string(),
        })? {
            f(CrossFileImportRow {
                file: row.get(0).unwrap_or_default(),
                start_line: row.get(1).unwrap_or(0),
                imported: row.get(2).unwrap_or_default(),
                statement: row.get(3).ok(),
                resolved_file: row.get(4).unwrap_or_default(),
                parent_kind: row.get(5).unwrap_or_else(|_| "file".to_string()),
            });
        }
        Ok(())
    }
}
