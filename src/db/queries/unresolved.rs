//! Unresolved-reference queries.
use super::*;

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
                (from_node_id, reference_name, reference_kind, line, col, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    uref.from_node_id.as_str(),
                    uref.reference_name.as_str(),
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
            .prepare("INSERT INTO unresolved_refs (from_node_id,reference_name,reference_kind,line,col,file_path) VALUES (?1,?2,?3,?4,?5,?6)")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;

        for uref in refs {
            stmt.execute(params![
                uref.from_node_id.as_str(),
                uref.reference_name.as_str(),
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
                    reference_kind: EdgeKind::from_str(&row.get::<String>(3).unwrap_or_default())
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
