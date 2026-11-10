//! Side records: node fingerprints (#83) and session memory (decisions and
//! touched code areas).
use super::search::FtsQuery;
use super::{
    db_error, get_i64, get_opt_str, get_str, get_u32, int, text, Database, PropMap, PropSource,
    CODE_AREA, DECISION, FINGERPRINT,
};
use crate::db::fts_query::to_fts_match_query;
use crate::db::model::StoredFingerprint;
use crate::errors::{Result, TokenSaveError};
use crate::types::{CodeAreaRecord, DecisionRecord};

impl Database {
    /// Upserts a fingerprint for a node, replacing any existing one.
    pub async fn upsert_fingerprint(
        &self,
        node_id: &str,
        fp: &crate::redundancy::Fingerprint,
    ) -> Result<()> {
        let op = "upsert_fingerprint";
        let existing = self.find("fp_node", node_id, op)?;
        self.delete_graph_nodes(&existing, op)?;
        let props = PropMap::from([
            ("fp_node".into(), text(node_id)),
            ("fp_ast".into(), text(&fp.ast_hash)),
            ("fp_cfg".into(), text(&fp.cfg_hash)),
            ("fp_call_seq".into(), text(&fp.call_seq_hash)),
            ("fp_shingles".into(), text(&fp.shingles_to_string())),
            (
                "fp_tokens".into(),
                int(i64::try_from(fp.body_tokens).unwrap_or(i64::MAX)),
            ),
            ("fp_source".into(), text(&fp.source_hash)),
        ]);
        self.db
            .create_node_with_props(&[FINGERPRINT], props)
            .map_err(|e| db_error(op, &e))?;
        Ok(())
    }

    /// Fetches the fingerprint of one node.
    pub async fn get_fingerprint(&self, node_id: &str) -> Result<Option<StoredFingerprint>> {
        let op = "get_fingerprint";
        let Some(gid) = self.find("fp_node", node_id, op)?.into_iter().next() else {
            return Ok(None);
        };
        Ok(self.graph_node(gid, op)?.map(|n| fingerprint_from(&n)))
    }

    /// Fetches every fingerprint.
    pub async fn get_all_fingerprints(&self) -> Result<Vec<StoredFingerprint>> {
        Ok(self
            .scan_label(FINGERPRINT, "get_all_fingerprints")?
            .iter()
            .map(|p| fingerprint_from(p.as_ref()))
            .collect())
    }

    /// Inserts a decision recorded at `created_at`. Returns its id.
    pub async fn insert_decision(
        &self,
        text_value: &str,
        reason: Option<&str>,
        files: &[String],
        tags: &[String],
        created_at: i64,
    ) -> Result<i64> {
        let op = "record_decision";
        let json = |values: &[String], what: &str| {
            serde_json::to_string(values).map_err(|e| TokenSaveError::Database {
                message: format!("record_decision {what} serialization failed: {e}"),
                operation: op.to_string(),
            })
        };
        let id = self.next_record_id(DECISION, "dec_id", op)?;
        let mut props = PropMap::from([
            ("dec_id".into(), int(id)),
            ("dec_text".into(), text(text_value)),
            ("dec_created_at".into(), int(created_at)),
            ("dec_files".into(), text(&json(files, "files")?)),
            ("dec_tags".into(), text(&json(tags, "tags")?)),
        ]);
        if let Some(reason) = reason {
            props.insert("dec_reason".into(), text(reason));
        }
        self.db
            .create_node_with_props(&[DECISION], props)
            .map_err(|e| db_error(op, &e))?;
        Ok(id)
    }

    /// Returns up to `limit` decisions, newest first. `query` is matched
    /// against the text and reason as the libSQL full-text table does; a
    /// query with no searchable terms matches every decision.
    pub async fn get_decisions(
        &self,
        query: Option<&str>,
        since: Option<i64>,
        limit: i64,
    ) -> Result<Vec<DecisionRecord>> {
        let op = "session_recall";
        let fts = query
            .and_then(to_fts_match_query)
            .map(|q| FtsQuery::parse(&q));
        let mut out = Vec::new();
        for p in self.scan_label(DECISION, op)? {
            let p = p.as_ref();
            let record = decision_from(p, op)?;
            if since.is_some_and(|ts| record.created_at < ts) {
                continue;
            }
            if let Some(fts) = &fts {
                let reason = record.reason.as_deref().unwrap_or_default();
                if !fts.matches_any(&[&record.text, reason]) {
                    continue;
                }
            }
            out.push(record);
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        // SQLite treats a negative LIMIT as no limit.
        if let Ok(limit) = usize::try_from(limit) {
            out.truncate(limit);
        }
        Ok(out)
    }

    /// Inserts a code area touched at `touched_at`, or bumps its touch count.
    /// A `None` description keeps the stored one.
    pub async fn upsert_code_area(
        &self,
        path: &str,
        description: Option<&str>,
        touched_at: i64,
    ) -> Result<()> {
        let op = "record_code_area";
        if let Some(gid) = self.find("area_path", path, op)?.into_iter().next() {
            let count = self
                .graph_node(gid, op)?
                .map_or(0, |n| get_i64(&n, "area_count").unwrap_or(0));
            let mut updates = vec![
                ("area_touched", int(touched_at)),
                ("area_count", int(count + 1)),
            ];
            if let Some(description) = description {
                updates.push(("area_desc", text(description)));
            }
            for (key, value) in updates {
                self.db
                    .set_node_property(gid, key, value)
                    .map_err(|e| db_error(op, &e))?;
            }
            return Ok(());
        }
        let id = self.next_record_id(CODE_AREA, "area_id", op)?;
        let mut props = PropMap::from([
            ("area_id".into(), int(id)),
            ("area_path".into(), text(path)),
            ("area_touched".into(), int(touched_at)),
            ("area_count".into(), int(1)),
        ]);
        if let Some(description) = description {
            props.insert("area_desc".into(), text(description));
        }
        self.db
            .create_node_with_props(&[CODE_AREA], props)
            .map_err(|e| db_error(op, &e))?;
        Ok(())
    }

    /// Returns up to `limit` code areas, most recently touched first.
    pub async fn get_code_areas(&self, limit: i64) -> Result<Vec<CodeAreaRecord>> {
        let mut out: Vec<CodeAreaRecord> = self
            .scan_label(CODE_AREA, "list_code_areas")?
            .iter()
            .map(|p| {
                let p = p.as_ref();
                CodeAreaRecord {
                    id: get_i64(p, "area_id").unwrap_or(0),
                    path: get_str(p, "area_path"),
                    description: get_opt_str(p, "area_desc"),
                    last_touched_at: get_i64(p, "area_touched").unwrap_or(0),
                    touch_count: get_u32(p, "area_count"),
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.last_touched_at
                .cmp(&a.last_touched_at)
                .then(b.id.cmp(&a.id))
        });
        if let Ok(limit) = usize::try_from(limit) {
            out.truncate(limit);
        }
        Ok(out)
    }

    /// One more than the largest `id_property` among `label` records, as an
    /// autoincrement id would be.
    fn next_record_id(&self, label: &str, id_property: &str, operation: &str) -> Result<i64> {
        Ok(self
            .scan_label(label, operation)?
            .iter()
            .filter_map(|p| get_i64(p.as_ref(), id_property))
            .max()
            .unwrap_or(0)
            + 1)
    }
}

fn fingerprint_from(p: &(impl PropSource + ?Sized)) -> StoredFingerprint {
    StoredFingerprint {
        node_id: get_str(p, "fp_node"),
        ast_hash: get_str(p, "fp_ast"),
        cfg_hash: get_str(p, "fp_cfg"),
        call_seq_hash: get_str(p, "fp_call_seq"),
        shingles: crate::redundancy::Fingerprint::shingles_from_string(&get_str(p, "fp_shingles")),
        body_tokens: get_u32(p, "fp_tokens"),
        source_hash: get_str(p, "fp_source"),
    }
}

fn decision_from(p: &(impl PropSource + ?Sized), operation: &str) -> Result<DecisionRecord> {
    let json_err = |e: serde_json::Error| TokenSaveError::Database {
        message: format!("session_recall JSON parse failed: {e}"),
        operation: operation.to_string(),
    };
    Ok(DecisionRecord {
        id: get_i64(p, "dec_id").unwrap_or(0),
        text: get_str(p, "dec_text"),
        reason: get_opt_str(p, "dec_reason"),
        created_at: get_i64(p, "dec_created_at").unwrap_or(0),
        files: serde_json::from_str(&get_str(p, "dec_files")).map_err(json_err)?,
        tags: serde_json::from_str(&get_str(p, "dec_tags")).map_err(json_err)?,
    })
}
