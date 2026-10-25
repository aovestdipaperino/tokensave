//! Whole-database clearing.
use super::*;

// ---------------------------------------------------------------------------
// Clear
// ---------------------------------------------------------------------------

impl Database {
    /// Removes all data from every table.
    pub async fn clear(&self) -> Result<()> {
        self.conn()
            .execute_batch(
                "DELETE FROM vectors;
                 DELETE FROM executable_body_fts;
                 DELETE FROM trait_dispatch_callers;
                 DELETE FROM unresolved_refs;
                 DELETE FROM edges;
                 DELETE FROM nodes;
                 DELETE FROM files;",
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to clear database: {e}"),
                operation: "clear".to_string(),
            })?;
        self.trait_dispatch_callers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }
}
