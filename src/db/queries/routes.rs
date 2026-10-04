use std::collections::BTreeSet;

use super::*;
use crate::extraction::rails_support::{
    camelize, parse_route_target, pluralize, route_file, route_root, underscore, RouteFile,
    RAILS_API_ONLY_REFERENCE, RAILS_ISOLATE_NAMESPACE_PREFIX, RAILS_ROUTES_DRAW_PREFIX,
    RAILS_ROUTES_ENGINE_PREFIX, RAILS_ROUTES_UNSUPPORTED_PREFIX, RUBY_CUSTOM_INFLECTIONS_REFERENCE,
    RUBY_VISIBILITY_REFERENCE_PREFIX,
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

/// `Admin::NotesController` from a Ruby class or module's qualified name.
fn ruby_constant(qualified_name: &str, file_path: &str) -> Option<String> {
    let prefix = format!("{file_path}::");
    let rest = qualified_name.strip_prefix(&prefix)?;
    let rest = rest.strip_prefix(&prefix).unwrap_or(rest);
    Some(rest.trim_start_matches(':').to_string())
}

fn is_ruby_file(path: &str) -> bool {
    [".rb", ".rake", ".erb", ".slim"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

struct RouteRow {
    id: String,
    file: String,
    line: u32,
    target: String,
}

struct Evidence {
    file: String,
    from: String,
    value: String,
    line: u32,
}

/// Where a route file's relative targets live: the module its routes are
/// drawn into, and whether its application is API-only.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Context {
    module: String,
    api_only: bool,
}

/// `draw` calls by `(root, name)`, as `(file, line, module)` of each call.
type DrawSites = HashMap<(String, String), Vec<(String, u32, String)>>;

/// A routes file may draw an engine's routes and the application's in
/// separate blocks, so engines are kept per block as `(first, last, engine)` rows.
struct Contexts {
    engines: HashMap<String, Vec<(u32, u32, String)>>,
    engine_modules: HashMap<String, std::result::Result<String, &'static str>>,
    draws: DrawSites,
    api_only_roots: HashSet<String>,
}

impl Contexts {
    /// The context of a declaration at `line` of `file`.
    fn resolve(
        &self,
        file: &str,
        line: u32,
        visiting: &mut Vec<String>,
    ) -> std::result::Result<Context, &'static str> {
        let engine = self.engines.get(file).and_then(|blocks| {
            blocks
                .iter()
                .find(|(first, last, _)| (*first..=*last).contains(&line))
                .map(|(_, _, engine)| engine)
        });
        match route_file(file) {
            Some(RouteFile::Main { root }) => match engine {
                Some(engine) => Ok(Context {
                    module: self
                        .engine_modules
                        .get(engine)
                        .cloned()
                        .unwrap_or(Err("missing-engine"))?,
                    api_only: false,
                }),
                None => Ok(Context {
                    module: String::new(),
                    api_only: self.api_only_roots.contains(root),
                }),
            },
            Some(RouteFile::Drawn { root, name }) => {
                if visiting.iter().any(|f| f == file) {
                    return Err("not-drawn");
                }
                let Some(sites) = self.draws.get(&(root.to_string(), name.to_string())) else {
                    return Err("not-drawn");
                };
                visiting.push(file.to_string());
                let mut found = BTreeSet::new();
                let mut failure = None;
                for (site, site_line, module) in sites {
                    match self.resolve(site, *site_line, visiting) {
                        Ok(context) => {
                            found.insert(Context {
                                module: join_module(&context.module, module),
                                api_only: context.api_only,
                            });
                        }
                        Err(status) => failure = Some(status),
                    }
                }
                visiting.pop();
                if let Some(status) = failure {
                    return Err(status);
                }
                let mut found = found.into_iter();
                match (found.next(), found.next()) {
                    (Some(context), None) => Ok(context),
                    (Some(_), Some(_)) => Err("ambiguous-draw"),
                    (None, _) => Err("not-drawn"),
                }
            }
            _ => Err("not-drawn"),
        }
    }
}

fn join_module(outer: &str, inner: &str) -> String {
    [outer, inner]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// A route's `(constant, action)`, or why it cannot be linked.
type RouteStatus<'a> = std::result::Result<(String, String), &'a str>;

#[derive(Default)]
struct ActionCandidates {
    owner_found: bool,
    conflicting_kind: bool,
    unresolved_visibility: bool,
    targets: Vec<(String, String, String)>,
}

impl Database {
    pub async fn rails_route_status(&self) -> Result<Option<serde_json::Value>> {
        let counts = self
            .get_metadata("rails_route_counts")
            .await?
            .unwrap_or_else(|| "{}".into());
        let counts: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&counts)
            .map_err(|e| TokenSaveError::Config {
                message: format!("invalid stored route counts: {e}"),
            })?;
        let mut diagnostics: Vec<String> = self
            .rails_evidence(RAILS_ROUTES_UNSUPPORTED_PREFIX)
            .await?
            .into_iter()
            .map(|e| format!("{}:{}: unsupported route declaration", e.file, e.line + 1))
            .collect();
        diagnostics.sort();
        let pending = self.get_metadata("rails_routes_pending").await?.as_deref() == Some("1");
        if counts.is_empty() && diagnostics.is_empty() && !pending {
            return Ok(None);
        }
        Ok(Some(serde_json::json!({
            "counts_by_status": counts,
            "pending": pending,
            "extraction_diagnostics": diagnostics,
            "duration_ms": self.get_metadata("rails_route_duration_ms").await?.and_then(|value| value.parse::<u64>().ok()),
        })))
    }

    pub(crate) async fn invalidate_rails_routes(&self) -> Result<()> {
        // Metadata shares the transaction connection, so unlocked writes can be rolled back by another task.
        let _guard = self.write_lock.lock().await;
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
        let _guard = self.write_lock.lock().await;
        if self.get_metadata("rails_routes_pending").await?.as_deref() != Some("1") {
            return Ok(());
        }
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
                Ok(())
            }
            Err(error) => {
                let _ = self.conn().execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }

    async fn rebuild_rails_routes_inner(&self, start: std::time::Instant) -> Result<()> {
        let custom_inflections = !self
            .rails_evidence(RUBY_CUSTOM_INFLECTIONS_REFERENCE)
            .await?
            .is_empty();
        // By source, so the delete uses the edges source index.
        self.conn()
            .execute(
                "DELETE FROM edges WHERE resolved_by = ?1
                    AND source IN (SELECT id FROM nodes WHERE kind = 'route')",
                params![ResolvedBy::RailsRoute.code()],
            )
            .await
            .map_err(|error| route_error(&error))?;
        let routes = self.route_rows().await?;
        let contexts = self.route_contexts().await?;
        let mut status_by_route: Vec<(&RouteRow, RouteStatus<'_>)> = Vec::new();
        for route in &routes {
            let context = contexts.resolve(&route.file, route.line, &mut Vec::new());
            let resolved = match (parse_route_target(&route.target), context) {
                (None, _) => continue,
                (_, Err(status)) => Err(status),
                (Some(_), Ok(_)) if custom_inflections => Err("custom-inflections"),
                (Some(target), Ok(context)) if target.unless_api_only && context.api_only => {
                    Err("api-only")
                }
                (Some(target), Ok(context)) => Ok((
                    if target.absolute || context.module.is_empty() {
                        target.constant.to_string()
                    } else {
                        format!("{}::{}", camelize(&context.module), target.constant)
                    },
                    target.action.to_string(),
                )),
            };
            status_by_route.push((route, resolved));
        }
        let constants: BTreeSet<&str> = status_by_route
            .iter()
            .filter_map(|(_, r)| r.as_ref().ok().map(|(c, _)| c.as_str()))
            .collect();
        let owners = self.ruby_owners(&constants).await?;
        let owner_ids: Vec<String> = owners
            .values()
            .flatten()
            .map(|(id, _)| id.clone())
            .collect();
        let methods = self.owner_methods(&owner_ids).await?;
        let hidden = self.owner_visibility_evidence(&owner_ids).await?;

        let mut counts = serde_json::Map::new();
        for (route, resolved) in status_by_route {
            let status = match resolved {
                Err(status) => status,
                Ok((constant, action)) => {
                    let mut candidates = ActionCandidates::default();
                    for (owner, kind) in owners.get(&constant).into_iter().flatten() {
                        candidates.owner_found = true;
                        candidates.conflicting_kind |= kind == "module";
                        candidates.unresolved_visibility |= hidden
                            .get(owner)
                            .is_some_and(|names| names.contains(&action) || names.contains("*"));
                        for (id, name, kind, visibility) in methods.get(owner).into_iter().flatten()
                        {
                            if *name == action {
                                candidates.targets.push((
                                    id.clone(),
                                    kind.clone(),
                                    visibility.clone(),
                                ));
                            }
                        }
                    }
                    let ActionCandidates {
                        owner_found,
                        conflicting_kind,
                        unresolved_visibility,
                        targets,
                    } = candidates;
                    if !owner_found {
                        "missing-controller"
                    } else if conflicting_kind || targets.len() > 1 {
                        "ambiguous-action"
                    } else if targets.is_empty() {
                        "missing-action"
                    } else if targets[0].1 != "method" || targets[0].2 != "public" {
                        "non-public-instance-action"
                    } else if unresolved_visibility {
                        "unresolved-visibility"
                    } else {
                        self.conn().execute("INSERT INTO edges(source,target,kind,line,resolved_by) VALUES (?1,?2,'calls',?3,?4)", params![route.id.as_str(), targets[0].0.as_str(), i64::from(route.line), ResolvedBy::RailsRoute.code()]).await.map_err(|error| route_error(&error))?;
                        "resolved"
                    }
                }
            };
            let count = counts.entry(status).or_insert(serde_json::json!(0));
            *count = serde_json::json!(count.as_u64().unwrap_or(0) + 1);
        }
        let counts = serde_json::to_string(&counts).map_err(|e| TokenSaveError::Config {
            message: e.to_string(),
        })?;
        self.set_metadata("rails_route_counts", &counts).await?;
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

    /// Why renaming `target` would leave a route pointing at the old name.
    ///
    /// Route targets are string literals in `config/routes.rb` that cannot
    /// be edited safely, so any action or controller a route may name is
    /// refused: by exact target, by action name on any controller or module
    /// (inherited and concern actions), and by the words of declarations the
    /// extractor skipped.
    pub(crate) async fn rails_route_rename_blocker(&self, target: &Node) -> Result<Option<String>> {
        if !is_ruby_file(&target.file_path) {
            return Ok(None);
        }
        let reason = "route target literals cannot be renamed safely yet";
        let routes = self.route_rows().await?;
        let unsupported = self.rails_evidence(RAILS_ROUTES_UNSUPPORTED_PREFIX).await?;
        if routes.is_empty() && unsupported.is_empty() {
            return Ok(None);
        }
        // A skipped `resources :notes` routes its actions without naming them,
        // so a controller's own name counts as a mention of its actions.
        let mentions = |words: &[&str]| -> Option<String> {
            unsupported.iter().find_map(|e| {
                let found = e.value.split(' ').find_map(|w| {
                    let plural = pluralize(w);
                    words
                        .iter()
                        .find(|word| **word == w || **word == plural)
                        .copied()
                })?;
                Some(format!(
                    "{}:{}: {reason}; a route declaration tokensave could not read mentions `{found}`",
                    e.file,
                    e.line + 1
                ))
            })
        };
        let controller_word = |name: &str| {
            let name = name.rsplit("::").next().unwrap_or(name);
            underscore(name.strip_suffix("Controller").unwrap_or(name))
        };
        match target.kind {
            NodeKind::Method => {
                let Some(parent) = target.parent_id.as_deref() else {
                    return Ok(None);
                };
                let Some(owner) = self.get_node_by_id(parent).await? else {
                    return Ok(None);
                };
                if !(owner.kind == NodeKind::Module
                    || owner.kind == NodeKind::Class && owner.name.ends_with("Controller"))
                {
                    return Ok(None);
                }
                if let Some(route) = routes.iter().find(|route| {
                    parse_route_target(&route.target).is_some_and(|t| t.action == target.name)
                }) {
                    return Ok(Some(format!(
                        "{}:{}: {reason}; a route dispatches to `#{}`",
                        route.file,
                        route.line + 1,
                        target.name
                    )));
                }
                Ok(mentions(&[
                    target.name.as_str(),
                    &controller_word(&owner.name),
                ]))
            }
            NodeKind::Class | NodeKind::Module => {
                let name = target.name.rsplit("::").next().unwrap_or(&target.name);
                let contexts = self.route_contexts().await?;
                for route in &routes {
                    let Some(route_target) = parse_route_target(&route.target) else {
                        continue;
                    };
                    let mut constant = route_target.constant.to_string();
                    if let Ok(context) = contexts.resolve(&route.file, route.line, &mut Vec::new())
                    {
                        if !route_target.absolute && !context.module.is_empty() {
                            constant = format!("{}::{constant}", camelize(&context.module));
                        }
                    }
                    if constant.split("::").any(|s| s.eq_ignore_ascii_case(name)) {
                        return Ok(Some(format!(
                            "{}:{}: {reason}; a route dispatches to `{constant}`",
                            route.file,
                            route.line + 1
                        )));
                    }
                }
                Ok(mentions(&[controller_word(name).as_str()]))
            }
            _ => Ok(None),
        }
    }

    async fn route_rows(&self) -> Result<Vec<RouteRow>> {
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
    async fn rails_evidence(&self, name: &str) -> Result<Vec<Evidence>> {
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

    async fn route_contexts(&self) -> Result<Contexts> {
        let mut engines: HashMap<String, Vec<(u32, u32, String)>> = HashMap::new();
        for e in self.rails_evidence(RAILS_ROUTES_ENGINE_PREFIX).await? {
            let Some((engine, last)) = e.value.split_once('|') else {
                continue;
            };
            let Ok(last) = last.parse::<u32>() else {
                continue;
            };
            engines
                .entry(e.file)
                .or_default()
                .push((e.line, last, engine.to_string()));
        }
        let engine_constants: BTreeSet<&str> = engines
            .values()
            .flatten()
            .map(|(_, _, engine)| engine.as_str())
            .collect();
        let engine_classes = self.ruby_owners(&engine_constants).await?;
        let isolation: Vec<Evidence> = self.rails_evidence(RAILS_ISOLATE_NAMESPACE_PREFIX).await?;
        let mut engine_modules = HashMap::new();
        for engine in &engine_constants {
            let classes: Vec<&String> = engine_classes
                .get(*engine)
                .into_iter()
                .flatten()
                .filter(|(_, kind)| kind == "class")
                .map(|(id, _)| id)
                .collect();
            let namespaces: BTreeSet<&str> = isolation
                .iter()
                .filter(|e| classes.contains(&&e.from))
                .map(|e| e.value.as_str())
                .collect();
            let module = if classes.is_empty() {
                Err("missing-engine")
            } else {
                match namespaces.len() {
                    0 => Ok(String::new()),
                    1 => Ok(namespaces.iter().map(|n| underscore(n)).collect()),
                    _ => Err("ambiguous-engine"),
                }
            };
            engine_modules.insert((*engine).to_string(), module);
        }
        let mut draws = DrawSites::new();
        for e in self.rails_evidence(RAILS_ROUTES_DRAW_PREFIX).await? {
            let (Some(root), Some((name, module))) = (route_root(&e.file), e.value.split_once('|'))
            else {
                continue;
            };
            draws
                .entry((root.to_string(), name.to_string()))
                .or_default()
                .push((e.file.clone(), e.line, module.to_string()));
        }
        let api_only_roots = self
            .rails_evidence(RAILS_API_ONLY_REFERENCE)
            .await?
            .into_iter()
            .filter_map(|e| match route_file(&e.file) {
                Some(RouteFile::Application { root }) => Some(root.to_string()),
                _ => None,
            })
            .collect();
        Ok(Contexts {
            engines,
            engine_modules,
            draws,
            api_only_roots,
        })
    }

    /// Ruby classes and modules by full constant, as `(id, kind)`. A compact
    /// definition (`class Admin::NotesController`) keeps the `::` in its
    /// name, so every suffix of each constant is looked up by name.
    async fn ruby_owners(
        &self,
        constants: &BTreeSet<&str>,
    ) -> Result<HashMap<String, Vec<(String, String)>>> {
        let mut names = BTreeSet::new();
        for constant in constants {
            let segments: Vec<&str> = constant.split("::").collect();
            for start in 0..segments.len() {
                names.insert(segments[start..].join("::"));
            }
        }
        let names: Vec<String> = names.into_iter().collect();
        let mut owners: HashMap<String, Vec<(String, String)>> = HashMap::new();
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
    async fn owner_methods(
        &self,
        owner_ids: &[String],
    ) -> Result<HashMap<String, Vec<(String, String, String, String)>>> {
        let mut methods: HashMap<String, Vec<(String, String, String, String)>> = HashMap::new();
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
    async fn owner_visibility_evidence(
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
    fn ruby_constants_strip_file_prefixes() {
        assert_eq!(
            ruby_constant("a.rb::a.rb::Admin::NotesController", "a.rb").as_deref(),
            Some("Admin::NotesController")
        );
        assert_eq!(
            ruby_constant("a.rb::NotesController", "a.rb").as_deref(),
            Some("NotesController")
        );
        assert_eq!(prefix_end("rails-routes-draw:"), "rails-routes-draw;");
    }
}
