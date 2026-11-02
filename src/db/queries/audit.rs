//! Whole-index scans behind the bare-name edge audit (`edge_audit`, #536).
//!
//! Each scan streams its rows into a callback so the audit holds only the
//! maps it builds, never a copy of the table.
use crate::db::Database;
use crate::errors::{Result, TokenSaveError};

/// One edge, flattened with the source file and the target node facts the
/// audit's reachability predicate needs.
pub(crate) struct EdgeAuditRow {
    pub(crate) src_file: String,
    pub(crate) dst_id: String,
    pub(crate) dst_name: String,
    pub(crate) dst_file: String,
    pub(crate) dst_kind: String,
    pub(crate) dst_line: i64,
    pub(crate) dst_parent: Option<String>,
}

impl Database {
    /// Calls `f(file_path, name)` for every `use` node.
    pub(crate) async fn for_each_use_node(&self, mut f: impl FnMut(String, String)) -> Result<()> {
        self.stream_audit_rows(
            "SELECT file_path, name FROM nodes WHERE kind = 'use'",
            |row| {
                f(
                    row.get(0).unwrap_or_default(),
                    row.get(1).unwrap_or_default(),
                );
            },
        )
        .await
    }

    /// Calls `f(name, count)` with how many symbols (nodes other than files
    /// and `use` nodes) share each name.
    pub(crate) async fn for_each_symbol_name_count(
        &self,
        mut f: impl FnMut(String, i64),
    ) -> Result<()> {
        self.stream_audit_rows(
            "SELECT name, COUNT(*) FROM nodes WHERE kind <> 'file' AND kind <> 'use' GROUP BY name",
            |row| {
                f(
                    row.get(0).unwrap_or_default(),
                    row.get(1).unwrap_or_default(),
                );
            },
        )
        .await
    }

    /// Calls `f(id, name)` for every node.
    pub(crate) async fn for_each_node_name(&self, mut f: impl FnMut(String, String)) -> Result<()> {
        self.stream_audit_rows("SELECT id, name FROM nodes", |row| {
            f(
                row.get(0).unwrap_or_default(),
                row.get(1).unwrap_or_default(),
            );
        })
        .await
    }

    /// Calls `f` for every edge whose source and target nodes both exist.
    pub(crate) async fn for_each_edge_audit_row(
        &self,
        mut f: impl FnMut(EdgeAuditRow),
    ) -> Result<()> {
        self.stream_audit_rows(
            "SELECT s.file_path, n.id, n.name, n.file_path, n.kind, n.start_line, n.parent_id \
             FROM edges e \
             JOIN nodes n ON n.id = e.target \
             JOIN nodes s ON s.id = e.source",
            |row| {
                f(EdgeAuditRow {
                    src_file: row.get(0).unwrap_or_default(),
                    dst_id: row.get(1).unwrap_or_default(),
                    dst_name: row.get(2).unwrap_or_default(),
                    dst_file: row.get(3).unwrap_or_default(),
                    dst_kind: row.get(4).unwrap_or_default(),
                    dst_line: row.get(5).unwrap_or_default(),
                    dst_parent: row.get::<Option<String>>(6).unwrap_or_default(),
                });
            },
        )
        .await
    }

    async fn stream_audit_rows(&self, sql: &str, mut f: impl FnMut(&libsql::Row)) -> Result<()> {
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("edge audit query failed: {e}"),
                operation: "edge_audit".to_string(),
            })?;
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("edge audit read failed: {e}"),
            operation: "edge_audit".to_string(),
        })? {
            f(&row);
        }
        Ok(())
    }
}
