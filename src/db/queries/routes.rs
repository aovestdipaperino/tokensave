//! libSQL reads and writes behind Rails route dispatch. The dispatch
//! decision itself is shared with the other backends in
//! `crate::db::rails_routes`.
use std::collections::BTreeSet;

use super::*;
use crate::db::rails_routes::{
    constant_suffixes, is_ruby_file, ruby_constant, Evidence, OwnerMethods, Owners, RouteRow,
};
use crate::extraction::rails_support::{
    RAILS_ROUTES_UNSUPPORTED_PREFIX, RUBY_VISIBILITY_REFERENCE_PREFIX,
};

/// Bound parameters per `IN (...)` batch, well under SQLite's limit.
const BATCH: usize = 500;

fn route_error(error: &libsql::Error) -> TokenSaveError {
    TokenSaveError::Database {
        message: format!("Rails routes: {error}"),
        operation: "rails_routes".into(),
    }
}

fn text(value: String) -> libsql::Value {
    libsql::Value::Text(value)
}

/// The upper bound of the `reference_name` range that starts with `prefix`,
/// so a prefix scan can use the `reference_name` index.
fn prefix_end(prefix: &str) -> String {
    let mut end = prefix.to_string();
    if let Some(last) = end.pop() {
        end.push(char::from_u32(last as u32 + 1).unwrap_or(last));
    }
    end
}

impl Database {
    pub(crate) async fn invalidate_rails_routes(&self) -> Result<()> {
        // Metadata shares the transaction connection, so unlocked writes can be rolled back by another task.
        let _guard = self.lock_writes().await;
        let mut rows = self
            .conn()
            .query(
                "SELECT EXISTS (SELECT 1 FROM nodes WHERE kind = 'route')
                    OR EXISTS (SELECT 1 FROM unresolved_refs WHERE reference_name >= ?1 AND reference_name < ?2)",
                params![
                    RAILS_ROUTES_UNSUPPORTED_PREFIX,
                    prefix_end(RAILS_ROUTES_UNSUPPORTED_PREFIX)
                ],
            )
            .await
            .map_err(|error| route_error(&error))?;
        let routes_exist = match rows.next().await.map_err(|error| route_error(&error))? {
            Some(row) => row.get::<i64>(0).map_err(|error| route_error(&error))? != 0,
            None => false,
        };
        if routes_exist
            || self
                .get_metadata("rails_route_counts")
                .await?
                .as_deref()
                .is_some_and(|counts| counts != "{}")
        {
            self.set_metadata("rails_routes_pending", "1").await?;
        }
        Ok(())
    }

    /// Rebuild only declared route dispatch, including links lost after controller reindexing.
    pub async fn rebuild_rails_routes(&self) -> Result<()> {
        let _guard = self.lock_writes().await;
        if self.get_metadata("rails_routes_pending").await?.as_deref() != Some("1") {
            return Ok(());
        }
        let tx_span = super::super::profiled::transaction_span();
        let start = std::time::Instant::now();
        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|error| route_error(&error))?;
        let result = self.rebuild_rails_routes_inner(start).await;
        match result {
            Ok(()) => {
                if let Err(error) = self.conn().execute("COMMIT", ()).await {
                    let _ = self.conn().execute("ROLLBACK", ()).await;
                    return Err(route_error(&error));
                }
                tx_span.finish();
                Ok(())
            }
            Err(error) => {
                let _ = self.conn().execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }

    async fn rebuild_rails_routes_inner(&self, start: std::time::Instant) -> Result<()> {
        // By source, so the delete uses the edges source index.
        self.conn()
            .execute(
                "DELETE FROM edges WHERE resolved_by = ?1
                    AND source IN (SELECT id FROM nodes WHERE kind = 'route')",
                params![ResolvedBy::RailsRoute.code()],
            )
            .await
            .map_err(|error| route_error(&error))?;
        let plan = self.rails_route_plan().await?;
        for (route, action, line) in &plan.links {
            self.conn().execute("INSERT INTO edges(source,target,kind,line,resolved_by) VALUES (?1,?2,'calls',?3,?4)", params![route.as_str(), action.as_str(), i64::from(*line), ResolvedBy::RailsRoute.code()]).await.map_err(|error| route_error(&error))?;
        }
        self.set_metadata("rails_route_counts", &plan.counts)
            .await?;
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO metadata(key,value) VALUES ('rails_routes_pending','0')",
                (),
            )
            .await
            .map_err(|error| route_error(&error))?;

        self.set_metadata(
            "rails_route_duration_ms",
            &start.elapsed().as_millis().to_string(),
        )
        .await?;
        Ok(())
    }

    pub(in crate::db) async fn route_rows(&self) -> Result<Vec<RouteRow>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT n.id, n.file_path, n.start_line, r.reference_name
                 FROM nodes n JOIN unresolved_refs r ON r.from_node_id = n.id
                 WHERE n.kind = 'route' AND r.reference_kind = 'calls'
                 ORDER BY n.id",
                (),
            )
            .await
            .map_err(|error| route_error(&error))?;
        let mut routes = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
            routes.push(RouteRow {
                id: row.get(0).map_err(|error| route_error(&error))?,
                file: row.get(1).map_err(|error| route_error(&error))?,
                line: row.get(2).map_err(|error| route_error(&error))?,
                target: row.get(3).map_err(|error| route_error(&error))?,
            });
        }
        Ok(routes)
    }

    /// Evidence rows named `name`, or starting with it when it ends in `:`.
    pub(in crate::db) async fn rails_evidence(&self, name: &str) -> Result<Vec<Evidence>> {
        let (sql, params): (&str, Vec<libsql::Value>) = if name.ends_with(':') {
            (
                "SELECT file_path, from_node_id, reference_name, line FROM unresolved_refs
                 WHERE reference_name >= ?1 AND reference_name < ?2 AND reference_kind = 'uses'",
                vec![text(name.into()), text(prefix_end(name))],
            )
        } else {
            (
                "SELECT file_path, from_node_id, reference_name, line FROM unresolved_refs
                 WHERE reference_name = ?1 AND reference_kind = 'uses'",
                vec![text(name.into())],
            )
        };
        let mut rows = self
            .conn()
            .query(sql, libsql::params_from_iter(params))
            .await
            .map_err(|error| route_error(&error))?;
        let mut evidence = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
            let reference: String = row.get(2).map_err(|error| route_error(&error))?;
            evidence.push(Evidence {
                file: row.get(0).map_err(|error| route_error(&error))?,
                from: row.get(1).map_err(|error| route_error(&error))?,
                value: reference
                    .strip_prefix(name)
                    .unwrap_or(&reference)
                    .to_string(),
                line: row.get(3).map_err(|error| route_error(&error))?,
            });
        }
        Ok(evidence)
    }

    /// Ruby classes and modules by full constant, as `(id, kind)`. A compact
    /// definition (`class Admin::NotesController`) keeps the `::` in its
    /// name, so every suffix of each constant is looked up by name.
    pub(in crate::db) async fn ruby_owners(&self, constants: &BTreeSet<&str>) -> Result<Owners> {
        let names: Vec<String> = constant_suffixes(constants).into_iter().collect();
        let mut owners = Owners::new();
        for chunk in names.chunks(BATCH) {
            let sql = format!(
                "SELECT id, kind, qualified_name, file_path FROM nodes
                 WHERE name IN ({}) AND kind IN ('class','module')",
                build_qmark_placeholders(chunk.len())
            );
            let mut rows = self
                .conn()
                .query(
                    &sql,
                    libsql::params_from_iter(chunk.iter().cloned().map(text)),
                )
                .await
                .map_err(|error| route_error(&error))?;
            while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
                let id: String = row.get(0).map_err(|error| route_error(&error))?;
                let kind: String = row.get(1).map_err(|error| route_error(&error))?;
                let qualified_name: String = row.get(2).map_err(|error| route_error(&error))?;
                let file_path: String = row.get(3).map_err(|error| route_error(&error))?;
                if !is_ruby_file(&file_path) {
                    continue;
                }
                if let Some(constant) = ruby_constant(&qualified_name, &file_path) {
                    if constants.contains(constant.as_str()) {
                        owners.entry(constant).or_default().push((id, kind));
                    }
                }
            }
        }
        for list in owners.values_mut() {
            list.sort();
        }
        Ok(owners)
    }

    /// Methods directly defined in each owner, as `(id, name, kind, visibility)`.
    pub(in crate::db) async fn owner_methods(&self, owner_ids: &[String]) -> Result<OwnerMethods> {
        let mut methods = OwnerMethods::new();
        for chunk in owner_ids.chunks(BATCH) {
            let sql = format!(
                "SELECT parent_id, id, name, kind, visibility FROM nodes
                 WHERE parent_id IN ({}) AND kind IN ('method','singleton_method')",
                build_qmark_placeholders(chunk.len())
            );
            let mut rows = self
                .conn()
                .query(
                    &sql,
                    libsql::params_from_iter(chunk.iter().cloned().map(text)),
                )
                .await
                .map_err(|error| route_error(&error))?;
            while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
                let parent: String = row.get(0).map_err(|error| route_error(&error))?;
                methods.entry(parent).or_default().push((
                    row.get(1).map_err(|error| route_error(&error))?,
                    row.get(2).map_err(|error| route_error(&error))?,
                    row.get(3).map_err(|error| route_error(&error))?,
                    row.get(4).map_err(|error| route_error(&error))?,
                ));
            }
        }
        for list in methods.values_mut() {
            list.sort();
        }
        Ok(methods)
    }

    /// Method names each owner's visibility directives may hide.
    pub(in crate::db) async fn owner_visibility_evidence(
        &self,
        owner_ids: &[String],
    ) -> Result<HashMap<String, HashSet<String>>> {
        let mut hidden: HashMap<String, HashSet<String>> = HashMap::new();
        for chunk in owner_ids.chunks(BATCH) {
            let sql = format!(
                "SELECT from_node_id, reference_name FROM unresolved_refs
                 WHERE from_node_id IN ({}) AND reference_kind = 'uses'
                    AND reference_name >= ? AND reference_name < ?",
                build_qmark_placeholders(chunk.len())
            );
            let params = chunk.iter().cloned().map(text).chain([
                text(RUBY_VISIBILITY_REFERENCE_PREFIX.into()),
                text(prefix_end(RUBY_VISIBILITY_REFERENCE_PREFIX)),
            ]);
            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(params))
                .await
                .map_err(|error| route_error(&error))?;
            while let Some(row) = rows.next().await.map_err(|error| route_error(&error))? {
                let owner: String = row.get(0).map_err(|error| route_error(&error))?;
                let name: String = row.get(1).map_err(|error| route_error(&error))?;
                if let Some(name) = name.strip_prefix(RUBY_VISIBILITY_REFERENCE_PREFIX) {
                    hidden.entry(owner).or_default().insert(name.to_string());
                }
            }
        }
        Ok(hidden)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn route_invalidation_waits_for_concurrent_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("routes.db");
        let (db, _) = Database::initialize(&path).await.unwrap();
        db.set_metadata("rails_routes_pending", "0").await.unwrap();
        db.set_metadata("rails_route_counts", r#"{"resolved":1}"#)
            .await
            .unwrap();
        let guard = db.write_lock.lock().await;
        db.conn().execute("BEGIN", ()).await.unwrap();
        let mut writer = Box::pin(db.invalidate_rails_routes());
        let attempted = tokio::time::timeout(Duration::from_millis(50), writer.as_mut()).await;
        db.conn().execute("ROLLBACK", ()).await.unwrap();
        drop(guard);
        assert!(
            attempted.is_err(),
            "invalidation wrote inside another task's transaction"
        );
        writer.await.unwrap();
        drop(db);
        let (db, _) = Database::open(&path).await.unwrap();
        assert_eq!(
            db.get_metadata("rails_routes_pending")
                .await
                .unwrap()
                .as_deref(),
            Some("1")
        );
    }

    #[tokio::test]
    async fn route_rebuild_checks_pending_after_waiting_for_write_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let (db, _) = Database::initialize(&dir.path().join("routes.db"))
            .await
            .unwrap();
        db.set_metadata("rails_routes_pending", "1").await.unwrap();
        let guard = db.write_lock.lock().await;
        db.conn().execute("BEGIN", ()).await.unwrap();
        db.set_metadata("rails_routes_pending", "0").await.unwrap();
        let mut rebuild = Box::pin(db.rebuild_rails_routes());
        let attempted = tokio::time::timeout(Duration::from_millis(50), rebuild.as_mut()).await;
        db.conn().execute("ROLLBACK", ()).await.unwrap();
        drop(guard);
        assert!(
            attempted.is_err(),
            "rebuild used an uncommitted pending flag"
        );
        rebuild.await.unwrap();
        assert_eq!(
            db.get_metadata("rails_routes_pending")
                .await
                .unwrap()
                .as_deref(),
            Some("0")
        );
        assert_eq!(
            db.get_metadata("rails_route_counts")
                .await
                .unwrap()
                .as_deref(),
            Some("{}")
        );
    }

    #[test]
    fn prefix_end_bumps_the_last_character() {
        assert_eq!(prefix_end("rails-routes-draw:"), "rails-routes-draw;");
    }
}
