//! File-record queries.
use super::*;

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

impl Database {
    /// Inserts or replaces a file record.
    /// Batch upserts multiple file records using raw SQL for throughput.
    pub async fn upsert_files(&self, files: &[FileRecord]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin: {e}"),
                operation: "upsert_files".to_string(),
            })?;

        let stmt = self.conn()
            .prepare("INSERT OR REPLACE INTO files (path,content_hash,size,modified_at,indexed_at,node_count) VALUES (?1,?2,?3,?4,?5,?6)")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "upsert_files".to_string(),
            })?;

        for file in files {
            stmt.execute(params![
                file.path.as_str(),
                file.content_hash.as_str(),
                file.size as i64,
                file.modified_at,
                file.indexed_at,
                i64::from(file.node_count),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to upsert file: {e}"),
                operation: "upsert_files".to_string(),
            })?;
            stmt.reset();
        }

        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit: {e}"),
                operation: "upsert_files".to_string(),
            })?;
        Ok(())
    }

    pub async fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO files
                (path, content_hash, size, modified_at, indexed_at, node_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    file.path.as_str(),
                    file.content_hash.as_str(),
                    file.size as i64,
                    file.modified_at,
                    file.indexed_at,
                    i64::from(file.node_count),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to upsert file: {e}"),
                operation: "upsert_file".to_string(),
            })?;
        Ok(())
    }

    /// Retrieves a file record by path, returning `None` if not found.
    pub async fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count
                 FROM files WHERE path = ?1",
                params![path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file: {e}"),
                operation: "get_file".to_string(),
            })?;

        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read file row: {e}"),
            operation: "get_file".to_string(),
        })? {
            Some(row) => {
                let file = row_to_file(&row).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to map file row: {e}"),
                    operation: "get_file".to_string(),
                })?;
                Ok(Some(file))
            }
            None => Ok(None),
        }
    }

    /// Returns all file records.
    pub async fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count FROM files",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all files: {e}"),
                operation: "get_all_files".to_string(),
            })?;

        collect_rows(&mut rows, row_to_file, "get_all_files").await
    }

    /// Deletes a file record and cascades to delete its nodes first.
    pub async fn delete_file(&self, path: &str) -> Result<()> {
        self.delete_nodes_by_file(path).await?;
        self.conn()
            .execute("DELETE FROM files WHERE path = ?1", params![path])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete file: {e}"),
                operation: "delete_file".to_string(),
            })?;
        Ok(())
    }
}
