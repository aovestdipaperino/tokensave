//! Aggregate statistics.
use std::collections::HashMap;

use super::Database;
use crate::db::language::display_language_for_path;
use crate::errors::Result;
use crate::types::GraphStats;

impl Database {
    /// Returns aggregate statistics about the code graph.
    pub async fn get_stats(&self) -> Result<GraphStats> {
        self.get_stats_with_languages(|_| None).await
    }

    /// Aggregate stats using project dispatch labels where configured.
    pub(crate) async fn get_stats_with_languages(
        &self,
        language_override: impl Fn(&str) -> Option<String>,
    ) -> Result<GraphStats> {
        let op = "get_stats";
        let mut nodes_by_kind: HashMap<String, u64> = HashMap::new();
        let nodes = self.all_symbols(op)?;
        for node in &nodes {
            *nodes_by_kind
                .entry(node.kind.as_str().to_string())
                .or_insert(0) += 1;
        }
        let mut edges_by_kind: HashMap<String, u64> = HashMap::new();
        let edges = self.scan_edges(&[], op)?;
        for edge in &edges {
            *edges_by_kind
                .entry(edge.kind.as_str().to_string())
                .or_insert(0) += 1;
        }
        let files = self.get_all_files().await?;
        let mut files_by_language: HashMap<String, u64> = HashMap::new();
        for file in &files {
            *files_by_language
                .entry(
                    language_override(&file.path)
                        .unwrap_or_else(|| display_language_for_path(&file.path).to_string()),
                )
                .or_insert(0) += 1;
        }
        let metadata_u64 =
            |value: Option<String>| value.and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        Ok(GraphStats {
            node_count: nodes.len() as u64,
            edge_count: edges.len() as u64,
            file_count: files.len() as u64,
            nodes_by_kind,
            edges_by_kind,
            db_size_bytes: self.size().await.unwrap_or(0),
            last_updated: files
                .iter()
                .map(|f| f.indexed_at)
                .max()
                .and_then(|t| u64::try_from(t).ok())
                .unwrap_or(0),
            total_source_bytes: files.iter().map(|f| f.size).sum(),
            files_by_language,
            last_sync_at: metadata_u64(self.get_metadata("last_sync_at").await?),
            last_full_sync_at: metadata_u64(self.get_metadata("last_full_sync_at").await?),
            last_full_index_version: self
                .get_metadata("last_full_index_version")
                .await?
                .unwrap_or_default(),
            last_sync_duration_ms: metadata_u64(self.get_metadata("last_sync_duration_ms").await?),
        })
    }

    /// Returns the most recent `indexed_at` across all files; 0 when there
    /// are none.
    pub async fn last_index_time(&self) -> Result<i64> {
        Ok(self
            .get_all_files()
            .await?
            .iter()
            .map(|f| f.indexed_at)
            .max()
            .unwrap_or(0))
    }
}
