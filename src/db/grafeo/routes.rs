//! Reads and writes behind Rails route dispatch. The dispatch decision is
//! shared with the libSQL backend in `crate::db::rails_routes`.
use std::collections::{BTreeSet, HashMap, HashSet};

use grafeo::store::Direction;

use super::{db_error, get_i64, Database};
use crate::db::rails_routes::{
    constant_suffixes, is_ruby_file, ruby_constant, Evidence, OwnerMethods, Owners, RouteRow,
};
use crate::errors::Result;
use crate::extraction::rails_support::{
    RAILS_ROUTES_UNSUPPORTED_PREFIX, RUBY_VISIBILITY_REFERENCE_PREFIX,
};
use crate::types::{Edge, EdgeKind, NodeKind, ResolvedBy};

impl Database {
    /// Marks route dispatch for a rebuild when the index holds routes or a
    /// previous rebuild recorded any.
    pub(crate) async fn invalidate_rails_routes(&self) -> Result<()> {
        let op = "rails_routes";
        let routes_exist = !self.find("kind", NodeKind::Route.as_str(), op)?.is_empty()
            || self.scan_unresolved(op)?.iter().any(|(_, r)| {
                r.reference_name
                    .starts_with(RAILS_ROUTES_UNSUPPORTED_PREFIX)
            });
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

    /// Rebuilds declared route dispatch when it is marked pending.
    pub async fn rebuild_rails_routes(&self) -> Result<()> {
        let op = "rails_routes";
        if self.get_metadata("rails_routes_pending").await?.as_deref() != Some("1") {
            return Ok(());
        }
        let start = std::time::Instant::now();
        self.graph_changed();
        for route in self.find("kind", NodeKind::Route.as_str(), op)? {
            let adjacent = self
                .db
                .get_node_edges(route, Direction::Outgoing, &[])
                .map_err(|e| db_error(op, &e))?;
            for (_, eid) in adjacent {
                let Some(edge) = self.db.get_edge(eid).map_err(|e| db_error(op, &e))? else {
                    continue;
                };
                if get_i64(&edge, "resolved_by") == Some(ResolvedBy::RailsRoute.code()) {
                    self.db.delete_edge(eid).map_err(|e| db_error(op, &e))?;
                }
            }
        }
        let plan = self.rails_route_plan().await?;
        let links: Vec<Edge> = plan
            .links
            .into_iter()
            .map(|(route, action, line)| Edge {
                source: route,
                target: action,
                kind: EdgeKind::Calls,
                line: Some(line),
                resolved_by: Some(ResolvedBy::RailsRoute),
            })
            .collect();
        self.insert_edges(&links).await?;
        self.set_metadata("rails_route_counts", &plan.counts)
            .await?;
        self.set_metadata("rails_routes_pending", "0").await?;
        self.set_metadata(
            "rails_route_duration_ms",
            &start.elapsed().as_millis().to_string(),
        )
        .await
    }

    /// Route nodes with the `calls` reference naming their target, by id.
    pub(in crate::db) async fn route_rows(&self) -> Result<Vec<RouteRow>> {
        let op = "rails_routes";
        let mut routes = Vec::new();
        for node in self.symbols_where("kind", NodeKind::Route.as_str(), op)? {
            for gid in self.find("ref_from", &node.id, op)? {
                let Some(reference) = self.graph_node(gid, op)? else {
                    continue;
                };
                if super::get_str(&reference, "ref_kind") != EdgeKind::Calls.as_str() {
                    continue;
                }
                routes.push(RouteRow {
                    id: node.id.clone(),
                    file: node.file_path.clone(),
                    line: node.start_line,
                    target: super::get_str(&reference, "ref_name"),
                });
            }
        }
        routes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(routes)
    }

    /// `uses` evidence references named `name`, or starting with it when it
    /// ends in `:`.
    pub(in crate::db) async fn rails_evidence(&self, name: &str) -> Result<Vec<Evidence>> {
        let prefix = name.ends_with(':');
        Ok(self
            .scan_unresolved("rails_routes")?
            .into_iter()
            .map(|(_, r)| r)
            .filter(|r| {
                r.reference_kind == EdgeKind::Uses
                    && if prefix {
                        r.reference_name.starts_with(name)
                    } else {
                        r.reference_name == name
                    }
            })
            .map(|r| Evidence {
                value: r
                    .reference_name
                    .strip_prefix(name)
                    .unwrap_or(&r.reference_name)
                    .to_string(),
                file: r.file_path,
                from: r.from_node_id,
                line: r.line,
            })
            .collect())
    }

    /// Ruby classes and modules by full constant, as `(id, kind)`.
    pub(in crate::db) async fn ruby_owners(&self, constants: &BTreeSet<&str>) -> Result<Owners> {
        let op = "rails_routes";
        let mut owners = Owners::new();
        for name in constant_suffixes(constants) {
            for node in self.symbols_where("name", &name, op)? {
                if !matches!(node.kind, NodeKind::Class | NodeKind::Module)
                    || !is_ruby_file(&node.file_path)
                {
                    continue;
                }
                if let Some(constant) = ruby_constant(&node.qualified_name, &node.file_path) {
                    if constants.contains(constant.as_str()) {
                        owners
                            .entry(constant)
                            .or_default()
                            .push((node.id, node.kind.as_str().to_string()));
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
        for owner in owner_ids {
            for node in self.symbols_where("parent_id", owner, "rails_routes")? {
                if matches!(node.kind, NodeKind::Method | NodeKind::SingletonMethod) {
                    methods.entry(owner.clone()).or_default().push((
                        node.id,
                        node.name,
                        node.kind.as_str().to_string(),
                        node.visibility.as_str().to_string(),
                    ));
                }
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
        for owner in owner_ids {
            for reference in self
                .get_unresolved_refs_for_sources(std::slice::from_ref(owner))
                .await?
            {
                if reference.reference_kind != EdgeKind::Uses {
                    continue;
                }
                if let Some(name) = reference
                    .reference_name
                    .strip_prefix(RUBY_VISIBILITY_REFERENCE_PREFIX)
                {
                    hidden
                        .entry(owner.clone())
                        .or_default()
                        .insert(name.to_string());
                }
            }
        }
        Ok(hidden)
    }
}
