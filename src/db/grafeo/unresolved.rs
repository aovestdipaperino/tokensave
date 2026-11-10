//! Unresolved references awaiting resolution.
//!
//! Each reference is an `UnresolvedRef` node. `ref_seq` numbers them in
//! insertion order, standing in for the libSQL autoincrement id that
//! [`Database::get_unresolved_refs_after`] pages by.
use std::sync::atomic::Ordering;

use grafeo::Value;

use super::{
    db_error, ends_with_ci, get_i64, get_str, get_u32, int, text, Database, PropMap, PropSource,
    UNRESOLVED,
};
use crate::errors::Result;
use crate::types::{EdgeKind, UnresolvedRef};

impl Database {
    /// The typed-receiver call refs (#597, #642): `calls` refs from `.gd`
    /// and `.cs` files whose name holds a `::` type expression.
    pub async fn get_gdscript_typed_refs(&self) -> Result<Vec<UnresolvedRef>> {
        Ok(self
            .scan_unresolved("get_gdscript_typed_refs")?
            .into_iter()
            .map(|(_, r)| r)
            .filter(|r| {
                r.reference_kind == EdgeKind::Calls
                    && (ends_with_ci(&r.file_path, ".gd") || ends_with_ci(&r.file_path, ".cs"))
                    && r.reference_name.contains("::")
            })
            .collect())
    }

    /// Inserts a single unresolved reference.
    pub async fn insert_unresolved_ref(&self, uref: &UnresolvedRef) -> Result<()> {
        self.insert_unresolved_refs(std::slice::from_ref(uref))
            .await
    }

    /// Inserts a batch of unresolved references.
    pub async fn insert_unresolved_refs(&self, refs: &[UnresolvedRef]) -> Result<()> {
        if refs.is_empty() {
            return Ok(());
        }
        let props: Vec<PropMap> = refs
            .iter()
            .map(|uref| {
                let seq = self.ref_seq.fetch_add(1, Ordering::SeqCst) + 1;
                unresolved_props(uref, seq)
            })
            .collect();
        self.db
            .batch_create_nodes_with_props(UNRESOLVED, props)
            .map_err(|e| db_error("insert_unresolved_refs", &e))?;
        Ok(())
    }

    /// Returns all unresolved references, in insertion order.
    pub async fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>> {
        Ok(self
            .scan_unresolved("get_unresolved_refs")?
            .into_iter()
            .map(|(_, r)| r)
            .collect())
    }

    /// How many unresolved references the graph holds.
    pub async fn count_unresolved_refs(&self) -> Result<usize> {
        let rows = self.query_rows(
            &format!("MATCH (r:{UNRESOLVED}) RETURN count(r)"),
            "count_unresolved_refs",
        )?;
        Ok(rows
            .first()
            .and_then(|row| row.first())
            .and_then(Value::as_int64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0))
    }

    /// Up to `limit` references whose sequence number is above `after_id`,
    /// in sequence order, each paired with that number (#482).
    pub async fn get_unresolved_refs_after(
        &self,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<(i64, UnresolvedRef)>> {
        let mut page: Vec<(i64, UnresolvedRef)> = self
            .scan_unresolved("get_unresolved_refs_after")?
            .into_iter()
            .filter(|(seq, _)| *seq > after_id)
            .collect();
        page.sort_by_key(|(seq, _)| *seq);
        page.truncate(limit);
        Ok(page)
    }

    /// The references recorded from any of `source_ids`.
    pub async fn get_unresolved_refs_for_sources(
        &self,
        source_ids: &[String],
    ) -> Result<Vec<UnresolvedRef>> {
        let op = "get_unresolved_refs_for_sources";
        let mut out = Vec::new();
        for source in source_ids {
            for gid in self.find("ref_from", source, op)? {
                if let Some(node) = self.graph_node(gid, op)? {
                    out.push(unresolved_from(&node));
                }
            }
        }
        Ok(out)
    }

    /// Removes all unresolved references.
    pub async fn clear_unresolved_refs(&self) -> Result<()> {
        self.db
            .execute(&format!("MATCH (r:{UNRESOLVED}) DETACH DELETE r"))
            .map_err(|e| db_error("clear_unresolved_refs", &e))?;
        Ok(())
    }

    /// Every reference with its sequence number, in insertion order.
    pub(super) fn scan_unresolved(&self, operation: &str) -> Result<Vec<(i64, UnresolvedRef)>> {
        Ok(self
            .scan_label(UNRESOLVED, operation)?
            .iter()
            .map(|p| {
                (
                    get_i64(p.as_ref(), "ref_seq").unwrap_or(0),
                    unresolved_from(p.as_ref()),
                )
            })
            .collect())
    }
}

fn unresolved_props(uref: &UnresolvedRef, seq: i64) -> PropMap {
    PropMap::from([
        ("ref_from".into(), text(&uref.from_node_id)),
        ("ref_name".into(), text(&uref.reference_name)),
        ("ref_kind".into(), text(uref.reference_kind.as_str())),
        ("ref_line".into(), int(uref.line)),
        ("ref_col".into(), int(uref.column)),
        ("ref_file_path".into(), text(&uref.file_path)),
        ("ref_seq".into(), int(seq)),
    ])
}

fn unresolved_from(p: &(impl PropSource + ?Sized)) -> UnresolvedRef {
    UnresolvedRef {
        from_node_id: get_str(p, "ref_from"),
        reference_name: get_str(p, "ref_name"),
        reference_kind: EdgeKind::from_str(&get_str(p, "ref_kind")).unwrap_or(EdgeKind::Uses),
        line: get_u32(p, "ref_line"),
        column: get_u32(p, "ref_col"),
        file_path: get_str(p, "ref_file_path"),
    }
}
