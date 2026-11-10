//! Recorded ambiguous call sites (#412).
//!
//! Each record is an `AmbiguousCall` node keyed, like the libSQL primary
//! key, by `(from_node_id, reference_name, line)`. Candidates are stored as
//! a JSON array, as in the libSQL table.
use std::collections::HashSet;

use super::{
    db_error, get_str, get_u32, int, text, under_path, Database, PropMap, PropSource, AMBIGUOUS,
};
use crate::errors::{Result, TokenSaveError};
use crate::resolution::AmbiguityRefKey;
use crate::types::AmbiguousCall;

impl Database {
    /// Replaces the recorded ambiguities for a set of files (#412).
    pub async fn replace_ambiguous_calls(
        &self,
        files: &[String],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        let op = "replace_ambiguous_calls";
        for file in files {
            let ids = self.find("amb_file", file, op)?;
            self.delete_graph_nodes(&ids, op)?;
        }
        self.insert_ambiguous_calls(calls)
    }

    /// Replaces the recorded ambiguities for a set of references (#484):
    /// clears the record of every re-attempted reference, then inserts the
    /// new records.
    pub async fn replace_ambiguous_calls_for_refs(
        &self,
        refs: &[AmbiguityRefKey],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        let op = "replace_ambiguous_calls_for_refs";
        for key in refs {
            for gid in self.find("amb_from", &key.from_node_id, op)? {
                let Some(node) = self.graph_node(gid, op)? else {
                    continue;
                };
                if get_str(&node, "amb_name") == key.reference_name
                    && get_str(&node, "amb_file") == key.file_path
                    && get_u32(&node, "amb_line") == key.line
                {
                    self.delete_graph_nodes(&[gid], op)?;
                }
            }
        }
        self.insert_ambiguous_calls(calls)
    }

    /// Recorded ambiguous calls under `path_prefix`, by file and line, at
    /// most `limit`.
    pub async fn get_ambiguous_calls(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AmbiguousCall>> {
        let mut out: Vec<AmbiguousCall> = self
            .scan_ambiguous("get_ambiguous_calls")?
            .into_iter()
            .filter(|c| path_prefix.is_none_or(|p| under_path(&c.file_path, p)))
            .collect();
        sort_calls(&mut out);
        out.truncate(limit);
        Ok(out)
    }

    /// Node ids named as a candidate by any recorded ambiguity (#412).
    pub async fn ambiguous_candidate_ids(&self) -> Result<HashSet<String>> {
        Ok(self
            .scan_ambiguous("ambiguous_candidate_ids")?
            .into_iter()
            .flat_map(|c| c.candidate_node_ids)
            .collect())
    }

    /// Recorded ambiguous calls that name `node_id` among their candidates,
    /// by file and line.
    pub async fn get_ambiguous_calls_naming(&self, node_id: &str) -> Result<Vec<AmbiguousCall>> {
        let mut out: Vec<AmbiguousCall> = self
            .scan_ambiguous("get_ambiguous_calls_naming")?
            .into_iter()
            .filter(|c| c.candidate_node_ids.iter().any(|id| id == node_id))
            .collect();
        sort_calls(&mut out);
        Ok(out)
    }

    /// Inserts records, replacing any with the same key.
    fn insert_ambiguous_calls(&self, calls: &[AmbiguousCall]) -> Result<()> {
        let op = "replace_ambiguous_calls";
        let mut new = Vec::with_capacity(calls.len());
        for call in calls {
            for gid in self.find("amb_from", &call.from_node_id, op)? {
                let Some(node) = self.graph_node(gid, op)? else {
                    continue;
                };
                if get_str(&node, "amb_name") == call.reference_name
                    && get_u32(&node, "amb_line") == call.line
                {
                    self.delete_graph_nodes(&[gid], op)?;
                }
            }
            let encoded = serde_json::to_string(&call.candidate_node_ids).map_err(|e| {
                TokenSaveError::Database {
                    message: format!("failed to encode ambiguity candidates: {e}"),
                    operation: op.to_string(),
                }
            })?;
            new.push(PropMap::from([
                ("amb_from".into(), text(&call.from_node_id)),
                ("amb_name".into(), text(&call.reference_name)),
                ("amb_file".into(), text(&call.file_path)),
                ("amb_line".into(), int(call.line)),
                ("amb_candidates".into(), text(&encoded)),
            ]));
        }
        if !new.is_empty() {
            self.db
                .batch_create_nodes_with_props(AMBIGUOUS, new)
                .map_err(|e| db_error(op, &e))?;
        }
        Ok(())
    }

    fn scan_ambiguous(&self, operation: &str) -> Result<Vec<AmbiguousCall>> {
        Ok(self
            .scan_label(AMBIGUOUS, operation)?
            .iter()
            .map(|p| ambiguous_from(p.as_ref()))
            .collect())
    }
}

fn ambiguous_from(p: &(impl PropSource + ?Sized)) -> AmbiguousCall {
    AmbiguousCall {
        from_node_id: get_str(p, "amb_from"),
        reference_name: get_str(p, "amb_name"),
        file_path: get_str(p, "amb_file"),
        line: get_u32(p, "amb_line"),
        column: 0,
        candidate_node_ids: serde_json::from_str(&get_str(p, "amb_candidates")).unwrap_or_default(),
    }
}

fn sort_calls(calls: &mut [AmbiguousCall]) {
    calls.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));
}
