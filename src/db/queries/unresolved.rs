//! Unresolved-reference queries.
use super::*;

// ---------------------------------------------------------------------------
// Unresolved reference operations
// ---------------------------------------------------------------------------

impl Database {
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
