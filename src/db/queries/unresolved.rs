//! Unresolved-reference queries.
use super::*;
use crate::profile::DbOp;
use crate::resolution::simple_ref_name;

// ---------------------------------------------------------------------------
// Unresolved reference operations
// ---------------------------------------------------------------------------

impl Database {
    /// The typed-receiver call refs (`Bus::again()::subscribe`, #597;
    /// `Factory::Create()::Write`, #642): `calls` refs from `.gd` and `.cs`
    /// files whose name holds a `::` type expression. A small slice of the table, read before an incremental
    /// resolution to find the call sites whose existing edges may be stale.
    pub async fn get_gdscript_typed_refs(&self) -> Result<Vec<UnresolvedRef>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT from_node_id, reference_name, line, col, file_path
                 FROM unresolved_refs
                 WHERE reference_kind = 'calls'
                   AND (file_path LIKE '%.gd' OR file_path LIKE '%.cs')
                   AND instr(reference_name, '::') > 0",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query typed-receiver refs: {e}"),
                operation: "get_gdscript_typed_refs".to_string(),
            })?;
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            out.push(UnresolvedRef {
                from_node_id: row.get::<String>(0).unwrap_or_default(),
                reference_name: row.get::<String>(1).unwrap_or_default(),
                reference_kind: EdgeKind::Calls,
                line: row.get::<u32>(2).unwrap_or(0),
                column: row.get::<u32>(3).unwrap_or(0),
                file_path: row.get::<String>(4).unwrap_or_default(),
            });
        }
        Ok(out)
    }

    /// Inserts a single unresolved reference.
    pub async fn insert_unresolved_ref(&self, uref: &UnresolvedRef) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO unresolved_refs
                (from_node_id, reference_name, reference_leaf, reference_kind, line, col, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    uref.from_node_id.as_str(),
                    uref.reference_name.as_str(),
                    simple_ref_name(&uref.reference_name),
                    uref.reference_kind.as_str(),
                    i64::from(uref.line),
                    i64::from(uref.column),
                    uref.file_path.as_str(),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert unresolved ref: {e}"),
                operation: "insert_unresolved_ref".to_string(),
            })?;
        Ok(())
    }

    /// Inserts a batch of unresolved references using a prepared statement.
    pub async fn insert_unresolved_refs(&self, refs: &[UnresolvedRef]) -> Result<()> {
        crate::profile::measure_write(
            DbOp::InsertUnresolvedRefs,
            refs.len(),
            self.insert_unresolved_refs_unprofiled(refs),
        )
        .await
    }

    /// Unprofiled body of [`Self::insert_unresolved_refs`].
    async fn insert_unresolved_refs_unprofiled(&self, refs: &[UnresolvedRef]) -> Result<()> {
        if refs.is_empty() {
            return Ok(());
        }

        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;

        let stmt = self.conn()
            .prepare("INSERT INTO unresolved_refs (from_node_id,reference_name,reference_leaf,reference_kind,line,col,file_path) VALUES (?1,?2,?3,?4,?5,?6,?7)")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;

        for uref in refs {
            stmt.execute(params![
                uref.from_node_id.as_str(),
                uref.reference_name.as_str(),
                simple_ref_name(&uref.reference_name),
                uref.reference_kind.as_str(),
                i64::from(uref.line),
                i64::from(uref.column),
                uref.file_path.as_str(),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert unresolved ref: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;
            stmt.reset();
        }

        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;
        Ok(())
    }

    /// Returns all unresolved references.
    pub async fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query unresolved refs: {e}"),
                operation: "get_unresolved_refs".to_string(),
            })?;

        collect_rows(&mut rows, row_to_unresolved_ref, "get_unresolved_refs").await
    }

    /// How many unresolved references the graph holds.
    ///
    /// Cheap gate for the resolution block, which used to decide by loading
    /// the whole table and testing `is_empty()` (#482).
    pub async fn count_unresolved_refs(&self) -> Result<usize> {
        let mut rows = self
            .conn()
            .query("SELECT COUNT(*) FROM unresolved_refs", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to count unresolved refs: {e}"),
                operation: "count_unresolved_refs".to_string(),
            })?;
        let count = rows
            .next()
            .await
            .ok()
            .flatten()
            .and_then(|r| r.get::<i64>(0).ok())
            .unwrap_or(0);
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// One page of unresolved references, ordered by their autoincrement `id`,
    /// paired with that id so the caller can continue (#482).
    ///
    /// `get_unresolved_refs` materialises the whole table: 189,446 records and
    /// +74.6 MiB on tokensave's own tree, on every sync including a one-line
    /// edit, and the second largest allocation a sync makes. Resolution does
    /// not need them all at once — each reference is resolved independently
    /// against the whole name index — so the input can be paged even though
    /// the index cannot.
    ///
    /// Keyset rather than `LIMIT`/`OFFSET`: an offset scan re-walks the rows it
    /// already skipped, so paging the whole table would be quadratic in the
    /// number of pages. `id` is `INTEGER PRIMARY KEY AUTOINCREMENT`, so it is
    /// the rowid and the walk is an index scan from where the last page ended.
    pub async fn get_unresolved_refs_after(
        &self,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<(i64, UnresolvedRef)>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs
                 WHERE id > ?1
                 ORDER BY id
                 LIMIT ?2",
                params![after_id, i64::try_from(limit).unwrap_or(i64::MAX)],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to page unresolved refs: {e}"),
                operation: "get_unresolved_refs_after".to_string(),
            })?;

        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let Ok(id) = row.get::<i64>(0) else { continue };
            // Column 0 is the id, so the shared mapper's offsets are shifted;
            // build the record here rather than teaching it two layouts.
            out.push((
                id,
                UnresolvedRef {
                    from_node_id: row.get::<String>(1).unwrap_or_default(),
                    reference_name: row.get::<String>(2).unwrap_or_default(),
                    reference_kind: row
                        .get::<String>(3)
                        .unwrap_or_default()
                        .parse::<EdgeKind>()
                        .unwrap_or(EdgeKind::Calls),
                    line: row.get::<u32>(4).unwrap_or(0),
                    column: row.get::<u32>(5).unwrap_or(0),
                    file_path: row.get::<String>(6).unwrap_or_default(),
                },
            ));
        }
        Ok(out)
    }

    /// The raw references recorded from any of `source_ids`, with their
    /// columns.
    ///
    /// The edge a reference resolved to keeps only its line; the column lives
    /// here. `tokensave_rename` reads it back to find which identifier on the
    /// line an edge came from. Batched to stay under `SQLite`'s bound
    /// parameter limit.
    pub async fn get_unresolved_refs_for_sources(
        &self,
        source_ids: &[String],
    ) -> Result<Vec<UnresolvedRef>> {
        let mut out = Vec::new();
        for chunk in source_ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs WHERE from_node_id IN ({})",
                placeholders.join(", ")
            );
            let values: Vec<libsql::Value> = chunk
                .iter()
                .map(|id| libsql::Value::Text(id.clone()))
                .collect();
            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query unresolved refs by source: {e}"),
                    operation: "get_unresolved_refs_for_sources".to_string(),
                })?;
            out.extend(
                collect_rows(
                    &mut rows,
                    row_to_unresolved_ref,
                    "get_unresolved_refs_for_sources",
                )
                .await?,
            );
        }
        Ok(out)
    }

    /// Every reference recorded at the given call sites, `(from_node_id,
    /// line)`, so a re-resolution can treat a whole site as one unit (#630).
    ///
    /// A site's members may resolve to different nodes sharing the simple
    /// name, so a re-attempted member's cleanup must not delete a sibling's
    /// edge; fetching the members is what lets the pass delete and rewrite
    /// the site together, as the `GDScript` path already did (#597). Bounded by
    /// the number of sites handed to it.
    pub async fn get_unresolved_refs_at_sites(
        &self,
        sites: &[(String, u32)],
    ) -> Result<Vec<UnresolvedRef>> {
        let mut out = Vec::new();
        for (source, line) in sites {
            let mut rows = self
                .conn()
                .query(
                    "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                     FROM unresolved_refs
                     WHERE from_node_id = ?1 AND line = ?2",
                    params![source.as_str(), i64::from(*line)],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query refs at a call site: {e}"),
                    operation: "get_unresolved_refs_at_sites".to_string(),
                })?;
            out.extend(
                collect_rows(
                    &mut rows,
                    row_to_unresolved_ref,
                    "get_unresolved_refs_at_sites",
                )
                .await?,
            );
        }
        Ok(out)
    }

    /// Extracted call references whose trailing name is `name`, together with
    /// the resolver outcome recorded at each site (#687).
    ///
    /// The `reference_leaf` equality uses its dedicated index. The previous
    /// prototype searched `reference_name LIKE '%Name'`, which necessarily
    /// scanned every extracted reference in a large repository.
    pub async fn get_call_references_by_name(
        &self,
        name: &str,
        limit: usize,
    ) -> Result<(usize, Vec<CallReference>)> {
        let op = "get_call_references_by_name";
        let mut count_rows = self
            .conn()
            .query(
                "SELECT COUNT(*) FROM unresolved_refs
                 WHERE reference_kind = 'calls' AND reference_leaf = ?1",
                params![name],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to count call references by name: {e}"),
                operation: op.to_string(),
            })?;
        let total = count_rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read call-reference count: {e}"),
                operation: op.to_string(),
            })?
            .and_then(|row| row.get::<i64>(0).ok())
            .and_then(|count| usize::try_from(count).ok())
            .unwrap_or(0);
        drop(count_rows);

        let mut rows = self
            .conn()
            .query(
                "WITH selected AS (
                     SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                     FROM unresolved_refs
                     WHERE reference_kind = 'calls' AND reference_leaf = ?1
                     ORDER BY file_path, line, col, id
                     LIMIT ?2
                 )
                 SELECT r.from_node_id, r.reference_name, r.line, r.col, r.file_path,
                        t.id, t.qualified_name, a.candidate_node_ids
                 FROM selected r
                 LEFT JOIN edges e
                   ON e.source = r.from_node_id AND e.line = r.line AND e.kind = 'calls'
                 LEFT JOIN nodes t ON t.id = e.target AND t.name = ?1
                 LEFT JOIN ambiguous_calls a
                   ON a.from_node_id = r.from_node_id
                  AND a.reference_name = r.reference_name
                  AND a.line = r.line
                 ORDER BY r.file_path, r.line, r.col, t.qualified_name",
                params![name, i64::try_from(limit).unwrap_or(i64::MAX)],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query call references by name: {e}"),
                operation: op.to_string(),
            })?;

        let mut out: Vec<CallReference> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read call-reference row: {e}"),
            operation: op.to_string(),
        })? {
            let reference = UnresolvedRef {
                from_node_id: get_string_lossy(&row, 0).unwrap_or_default(),
                reference_name: get_string_lossy(&row, 1).unwrap_or_default(),
                reference_kind: EdgeKind::Calls,
                line: row.get::<i64>(2).unwrap_or(0) as u32,
                column: row.get::<i64>(3).unwrap_or(0) as u32,
                file_path: get_string_lossy(&row, 4).unwrap_or_default(),
            };
            let same_site = out.last().is_some_and(|item| {
                item.reference.from_node_id == reference.from_node_id
                    && item.reference.reference_name == reference.reference_name
                    && item.reference.line == reference.line
                    && item.reference.column == reference.column
                    && item.reference.file_path == reference.file_path
            });
            if !same_site {
                let encoded = get_string_lossy(&row, 7).unwrap_or_default();
                out.push(CallReference {
                    reference,
                    resolved_targets: Vec::new(),
                    ambiguous_candidate_ids: serde_json::from_str(&encoded).unwrap_or_default(),
                });
            }
            let Some(item) = out.last_mut() else { continue };
            if let (Some(id), Some(qualified_name)) = (
                row.get::<Option<String>>(5).ok().flatten(),
                row.get::<Option<String>>(6).ok().flatten(),
            ) {
                if !item.resolved_targets.iter().any(|(seen, _)| seen == &id) {
                    item.resolved_targets.push((id, qualified_name));
                }
            }
        }
        Ok((total, out))
    }

    /// Removes all unresolved references.
    pub async fn clear_unresolved_refs(&self) -> Result<()> {
        self.conn()
            .execute("DELETE FROM unresolved_refs", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to clear unresolved refs: {e}"),
                operation: "clear_unresolved_refs".to_string(),
            })?;
        Ok(())
    }
}
