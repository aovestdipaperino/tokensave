//! Whole-index scans behind the bare-name resolution audit (#536).
//!
//! Each scan streams rows to a callback instead of collecting them, so the
//! audit keeps only the maps it builds from them.
use super::*;

/// One edge, flattened with the node facts the audit predicate needs.
#[derive(Debug, Clone)]
pub struct AuditEdgeRow {
    /// File of the edge's source node.
    pub src_file: String,
    /// Id of the edge's target node.
    pub dst_id: String,
    /// Name of the target node.
    pub dst_name: String,
    /// File of the target node.
    pub dst_file: String,
    /// Kind of the target node.
    pub dst_kind: String,
    /// Zero-based start line of the target node.
    pub dst_line: i64,
    /// Parent id of the target node.
    pub dst_parent: Option<String>,
}

impl Database {
    /// Runs `sql` and passes each row to `f`, with the audit's error wording.
    async fn audit_scan(&self, sql: &str, mut f: impl FnMut(&libsql::Row)) -> Result<()> {
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

    /// Calls `f(file_path, name)` for every `use` node.
    pub async fn for_each_use_node(&self, mut f: impl FnMut(String, String)) -> Result<()> {
        self.audit_scan(
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

    /// Calls `f(name, count)` for every name shared by symbols that are not
    /// `file` or `use` nodes.
    pub async fn for_each_symbol_name_count(&self, mut f: impl FnMut(String, i64)) -> Result<()> {
        self.audit_scan(
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
    pub async fn for_each_node_name(&self, mut f: impl FnMut(String, String)) -> Result<()> {
        self.audit_scan("SELECT id, name FROM nodes", |row| {
            f(
                row.get(0).unwrap_or_default(),
                row.get(1).unwrap_or_default(),
            );
        })
        .await
    }

    /// Calls `f` for every edge whose source and target nodes both exist.
    pub async fn for_each_audit_edge(&self, mut f: impl FnMut(AuditEdgeRow)) -> Result<()> {
        self.audit_scan(
            "SELECT s.file_path, n.id, n.name, n.file_path, n.kind, n.start_line, n.parent_id \
             FROM edges e \
             JOIN nodes n ON n.id = e.target \
             JOIN nodes s ON s.id = e.source",
            |row| {
                f(AuditEdgeRow {
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
}
