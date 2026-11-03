//! Graph-analysis queries: dead-code candidates, file adjacency and the
//! cross-file import scan.
//!
//! The analysis built on these rows lives in `crate::graph`; this module
//! only reads the index.
use super::*;
use crate::profile::DbOp;

/// One resolved, cross-file `uses` edge from an import statement, as read by
/// [`Database::get_cross_file_import_rows`].
#[derive(Debug, Clone)]
pub struct ImportEdgeRow {
    /// File containing the import statement.
    pub file: String,
    /// Zero-based line of the `Use` node.
    pub line: u32,
    /// Name of the `Use` node (the imported path).
    pub imported: String,
    /// Source text of the statement, when stored.
    pub statement: Option<String>,
    /// File of the node the import resolved to.
    pub resolved_file: String,
    /// Kind of the `Use` node's parent; `"file"` when it has none.
    pub parent_kind: String,
}

/// Direction of the edges read by [`Database::get_edge_endpoint_ids`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeEndpoint {
    /// Targets of edges whose source is in the given set.
    Targets,
    /// Sources of edges whose target is in the given set.
    Sources,
}

impl Database {
    /// Returns nodes with no incoming use-site edge, the raw candidates for
    /// dead-code analysis.
    ///
    /// `godot_virtuals` are method names exempted in `.gd` files. The caller
    /// applies the remaining, non-SQL filters.
    pub async fn find_dead_code_candidates(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
        include_trait_impls: bool,
        godot_virtuals: &[&str],
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
        // Test-marker exclusion uses a two-step "resolve + filter in SQL":
        //   1) `collect_test_marker_ids` runs the leading-wildcard `LIKE`
        //      scan exactly once over `kind = 'annotation_usage'`.
        //   2) The marker ids go to the dead-code SELECT as ONE bound
        //      parameter, a JSON array built with `serde_json`, and the
        //      SELECT drops test-annotated candidates with
        //      `id NOT IN (SELECT target FROM edges WHERE kind = 'annotates'
        //      AND source IN (SELECT value FROM json_each(?)))`.
        //      The subquery is uncorrelated, so SQLite plans it as a
        //      `LIST SUBQUERY`: it walks the JSON array once, probes
        //      `idx_edges_source_kind` per marker, and builds a transient
        //      in-memory index of the targets that each candidate probes.
        //      That is the same shape as the old two-temp-table form,
        //      without writing anything: selected graphs are opened with
        //      `PRAGMA query_only = 1`, and the transient index is not a
        //      database write. A single parameter also avoids SQLite's
        //      bound-parameter limit, so there is no chunking.
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
        // - The next pre-join form (two temp tables, indexed `NOT IN`)
        //   measured 0.75 s on chromium and 0.6 s on scirs. The
        //   wildcard scan ran exactly once per call; the per-row
        //   probe was against a ~15K-row indexed lookup table, not a
        //   correlated subquery the optimiser can re-shape.
        // - The two-temp-table form above wrote `temp.test_markers` and
        //   `temp.test_annotated_targets`, which fails on selected graphs
        //   (opened with `PRAGMA query_only = 1`) and let concurrent
        //   read-only calls on the shared connection drop or refill each
        //   other's tables.
        // - The next form collected the targets into an in-memory `HashSet`
        //   (chunked SELECTs of 500 marker ids) and skipped matching rows
        //   while reading the main query. It was read-only safe but ~11%
        //   slower per call: rows SQL used to filter out (~2,200 of ~2,970
        //   candidates on a 24 K-node project) were read and decoded, then
        //   discarded. Filtering in SQL with `json_each` restores that.
        let marker_ids = self.collect_test_marker_ids().await?;
        // Always bind the array, even when empty: `NOT IN` over an empty
        // set is true, so every candidate is kept.
        let marker_ids_json =
            serde_json::to_string(&marker_ids).map_err(|e| TokenSaveError::Database {
                message: format!("failed to encode test marker ids: {e}"),
                operation: "find_dead_code".to_string(),
            })?;
        drop(marker_ids);
        let godot_virtuals = godot_virtuals
            .iter()
            .map(|n| format!("'{n}'"))
            .collect::<Vec<_>>()
            .join(", ");

        // The SELECT list stops at `attrs_start_line(21)`: `row_to_node`
        // reads the absent `parent_id` as `None` and the absent health
        // columns as 0, which dead-code analysis does not use.
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
             AND id NOT IN (
                 SELECT target FROM edges
                 WHERE kind = 'annotates'
                 AND source IN (SELECT value FROM json_each(?1))
             )"
        );

        let mut rows = self
            .profiled_query(DbOp::FindDeadCode, &sql, params![marker_ids_json])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to find dead code: {e}"),
                operation: "find_dead_code".to_string(),
            })?;
        collect_rows(&mut rows, row_to_node, "find_dead_code").await
    }

    /// `GDScript` methods that override a base-class method which is called or
    /// used (#598).
    ///
    /// Inheritance comes from resolved `extends` edges, followed
    /// transitively, so a grandchild override of a called method is included.
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
            .profiled_query(DbOp::GdscriptLiveOverrides, sql, ())
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

    /// Returns the distinct far endpoints of `calls`/`uses` edges touching
    /// `node_ids`: their targets ([`EdgeEndpoint::Targets`]) or their sources
    /// ([`EdgeEndpoint::Sources`]).
    pub async fn get_edge_endpoint_ids(
        &self,
        node_ids: &[String],
        endpoint: EdgeEndpoint,
    ) -> Result<Vec<String>> {
        if node_ids.is_empty() {
            return Ok(Vec::new());
        }
        // The op names and error messages keep the wording of the
        // `GraphQueryManager` callers they were moved from.
        let (select_col, filter_col, op, operation, what) = match endpoint {
            EdgeEndpoint::Targets => (
                "target",
                "source",
                DbOp::GetFileDependencies,
                "get_file_dependencies",
                ("file dependencies", "target id"),
            ),
            EdgeEndpoint::Sources => (
                "source",
                "target",
                DbOp::GetFileDependents,
                "get_file_dependents",
                ("file dependents", "source id"),
            ),
        };

        let placeholders: Vec<String> = (1..=node_ids.len()).map(|i| format!("?{i}")).collect();
        let kind_filter = "('uses', 'calls')";
        let sql = format!(
            "SELECT DISTINCT e.{select_col} FROM edges e \
             WHERE e.{filter_col} IN ({}) AND e.kind IN {kind_filter}",
            placeholders.join(", ")
        );
        let param_values: Vec<libsql::Value> = node_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();

        let mut rows = self
            .profiled_query(op, &sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query {}: {e}", what.0),
                operation: operation.to_string(),
            })?;

        let mut ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read {}: {e}", what.1),
            operation: operation.to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    /// Returns the distinct `(source file, target file)` pairs joined by a
    /// `calls` or `uses` edge between two different files.
    pub async fn get_cross_file_edge_pairs(&self) -> Result<Vec<(String, String)>> {
        let sql = "SELECT DISTINCT n1.file_path AS src_file, n2.file_path AS tgt_file \
                   FROM edges e \
                   JOIN nodes n1 ON e.source = n1.id \
                   JOIN nodes n2 ON e.target = n2.id \
                   WHERE e.kind IN ('calls', 'uses') \
                   AND n1.file_path != n2.file_path";

        let mut rows = self
            .profiled_query(DbOp::BuildFileAdjacency, sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file adjacency: {e}"),
                operation: "build_file_adjacency".to_string(),
            })?;

        let mut pairs = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read adjacency row: {e}"),
            operation: "build_file_adjacency".to_string(),
        })? {
            let src: String = row.get(0).unwrap_or_default();
            let tgt: String = row.get(1).unwrap_or_default();
            pairs.push((src, tgt));
        }
        Ok(pairs)
    }

    /// Returns every resolved `uses` edge from a `Use` node to a node in a
    /// different file, for the module import graph (#334).
    pub async fn get_cross_file_import_rows(&self) -> Result<Vec<ImportEdgeRow>> {
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

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read import row: {e}"),
            operation: "build_module_import_graph".to_string(),
        })? {
            out.push(ImportEdgeRow {
                file: row.get(0).unwrap_or_default(),
                line: row.get(1).unwrap_or(0),
                imported: row.get(2).unwrap_or_default(),
                statement: row.get(3).ok(),
                resolved_file: row.get(4).unwrap_or_default(),
                parent_kind: row.get(5).unwrap_or_else(|_| "file".to_string()),
            });
        }
        Ok(out)
    }
}
