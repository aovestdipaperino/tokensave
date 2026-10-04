#![cfg(feature = "lang-ruby")]

use std::fs;
use tempfile::{tempdir, TempDir};
use tokensave::extraction::{LanguageExtractor, RubyExtractor};
use tokensave::tokensave::TokenSave;
use tokensave::types::{
    Edge, EdgeKind, ExtractionResult, Node, NodeKind, ResolvedBy, UnresolvedRef,
};

fn route_records(result: &ExtractionResult) -> Vec<(&str, &str)> {
    result
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Route)
        .map(|node| {
            let reference = result
                .unresolved_refs
                .iter()
                .find(|r| r.from_node_id == node.id)
                .unwrap();
            assert_eq!(reference.reference_kind, tokensave::types::EdgeKind::Calls);
            assert_eq!(reference.line, node.start_line);
            assert_eq!(reference.column, node.start_column);
            (node.name.as_str(), reference.reference_name.as_str())
        })
        .collect()
}

#[test]
fn rails_route_comments_do_not_change_arguments_or_action_lists() {
    let result = RubyExtractor.extract(
        "config/routes.rb",
        include_str!("fixtures/rails_routes_comments.rb"),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        route_records(&result),
        vec![
            ("GET /notes", "NotesController#show"),
            ("GET /staff/notes", "Admin::NotesController#show"),
            ("GET /notes/:id", "NotesController#show"),
            ("PATCH /notes/:id", "NotesController#update"),
            ("PUT /notes/:id", "NotesController#update"),
        ]
    );
}

#[tokio::test]
async fn rails_route_local_visibility_literals_and_unknown_lists_are_safe() {
    for (directive, status) in [
        ("private 'show'", "non-public-instance-action"),
        ("public 'show'", "resolved"),
        ("names = [:show]; private names", "unresolved-visibility"),
        ("self.private :show", "unresolved-visibility"),
    ] {
        let root = project();
        fs::write(
            root.path().join("app/controllers/notes_controller.rb"),
            format!("class NotesController\n def show; end\n {directive}\nend\n"),
        )
        .unwrap();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.index_all().await.unwrap();
        assert_eq!(
            route_edges(&graph).await.len(),
            usize::from(status == "resolved"),
            "{directive}: {:?}",
            graph.db().rails_route_status().await.unwrap()
        );
        assert_eq!(
            graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"][status],
            1,
            "{directive}"
        );
    }
}

#[tokio::test]
async fn rails_route_backfill_waits_for_indexable_routes() {
    let root = project();
    fs::remove_file(root.path().join("config/routes.rb")).unwrap();
    fs::create_dir_all(root.path().join("config/initializers")).unwrap();
    fs::write(
        root.path().join("config/initializers/inflections.rb"),
        "# Default inflections\n",
    )
    .unwrap();
    fs::write(
        root.path().join("app/controllers/visibility.rb"),
        "class NotesController\n private :show\nend\n",
    )
    .unwrap();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.sync().await.unwrap();
    graph.db().conn().execute_batch("DELETE FROM unresolved_refs WHERE reference_name LIKE 'ruby-visibility:%'; DELETE FROM metadata WHERE key = 'rails_route_references_v3';").await.unwrap();
    drop(graph);
    let graph = TokenSave::open(root.path()).await.unwrap();
    for _ in 0..3 {
        let sync = graph.sync().await.unwrap();
        assert_eq!(sync.files_modified, 0);
        assert_eq!(sync.files_added, 0);
        assert!(graph
            .db()
            .get_metadata("rails_route_references_v3")
            .await
            .unwrap()
            .is_none());
    }
    fs::write(
        root.path().join("config/routes.rb"),
        "Rails.application.routes.draw do\n get '/notes', to: 'notes#show'\nend\n",
    )
    .unwrap();
    let sync = graph.sync().await.unwrap();
    assert_eq!(sync.files_added, 1);
    assert_eq!(sync.files_modified, 3);
    assert!(route_edges(&graph).await.is_empty());
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"]
            ["unresolved-visibility"],
        1
    );
    assert_eq!(
        graph
            .db()
            .get_metadata("rails_route_references_v3")
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    assert_eq!(graph.sync().await.unwrap().files_modified, 0);
}

#[tokio::test]
async fn rails_visibility_backfill_reextracts_unchanged_controller_inputs() {
    let root = project();
    fs::write(
        root.path().join("app/controllers/visibility.rb"),
        "class NotesController\n private :show\nend\n",
    )
    .unwrap();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    graph.db().conn().execute_batch("DELETE FROM unresolved_refs WHERE reference_name LIKE 'ruby-visibility:%'; DELETE FROM metadata WHERE key = 'rails_route_references_v3'; INSERT OR REPLACE INTO metadata(key,value) VALUES ('rails_route_references_v1','1'),('rails_routes_pending','1');").await.unwrap();
    graph.db().rebuild_rails_routes().await.unwrap();
    assert_eq!(route_edges(&graph).await.len(), 1);
    drop(graph);
    let graph = TokenSave::open(root.path()).await.unwrap();
    assert_eq!(graph.sync().await.unwrap().files_modified, 3);
    assert!(route_edges(&graph).await.is_empty());
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"]
            ["unresolved-visibility"],
        1
    );
    assert_eq!(graph.sync().await.unwrap().files_modified, 0);
}

#[tokio::test]
async fn rails_route_reopening_visibility_stays_unresolved_and_clears_on_deletion() {
    for directive in [
        "private :show",
        "protected :show",
        "public :show",
        "private 'show'",
        "private :\"show\"",
        "private(*[:show])",
        "names = [:show]; private names",
        "self.private :show",
        "if true; private :show; end",
        "tap do; private :show; end",
        "private :\"#{'show'}\"",
    ] {
        let root = project();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.index_all().await.unwrap();
        assert_eq!(route_edges(&graph).await.len(), 1);
        let reopening = root.path().join("app/controllers/visibility.rb");
        fs::write(
            &reopening,
            format!("class NotesController\n {directive}\nend\n"),
        )
        .unwrap();
        graph.sync().await.unwrap();
        assert!(route_edges(&graph).await.is_empty(), "{directive}");
        assert_eq!(
            graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"]
                ["unresolved-visibility"],
            1,
            "{directive}"
        );
        let incremental = canonical(&graph).await;
        graph.index_all().await.unwrap();
        assert_eq!(incremental, canonical(&graph).await, "{directive}");
        drop(graph);
        let graph = TokenSave::open(root.path()).await.unwrap();
        assert!(route_edges(&graph).await.is_empty(), "{directive}");
        fs::remove_file(reopening).unwrap();
        graph.sync().await.unwrap();
        assert_eq!(route_edges(&graph).await.len(), 1, "{directive}");
    }
    for directive in [
        "private",
        "def other; end; private :other",
        "def self.other; end; class << self; private :other; end",
        "def self.other; end; private_class_method :other",
        "policy.private :show",
        "Class.new do; def show; end; private :show; end",
        "def self.install; private :show; end",
    ] {
        let root = project();
        fs::write(
            root.path().join("app/controllers/visibility.rb"),
            format!("class NotesController\n {directive}\nend\n"),
        )
        .unwrap();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.index_all().await.unwrap();
        assert_eq!(route_edges(&graph).await.len(), 1, "{directive}");
    }
}

#[test]
fn rails_route_scope_and_resource_records() {
    let source = include_str!("fixtures/rails_routes.rb");
    let result = RubyExtractor.extract("config/routes.rb", source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        route_records(&result),
        vec![
            ("GET /", "NotesController#index"),
            ("GET /notes/:id", "NotesController#show"),
            ("GET /staff/notes", "Backoffice::NotesController#index"),
            ("POST /staff/global", "::NotesController#create"),
            ("GET /api/notes", "V1::NotesController#index"),
            ("GET /notes", "NotesController#index"),
            ("GET /notes/:id", "NotesController#show"),
            ("PATCH /notes/:id", "NotesController#update"),
            ("PUT /notes/:id", "NotesController#update"),
            ("GET /profile", "ProfilesController#show"),
        ]
    );
    let expected_lines = [1, 2, 4, 5, 8, 10, 10, 10, 10, 11];
    for (node, line) in result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Route)
        .zip(expected_lines)
    {
        assert_eq!(node.kind, NodeKind::Route);
        assert!(node.parent_id.is_some());
        assert_eq!(node.start_line, line);
        assert_eq!(node.end_line, line);
        assert!(node.start_column == 2 || node.start_column == 4);
        assert!(node.end_column > node.start_column);
    }
    let other = RubyExtractor.extract("other.rb", source);
    assert!(route_records(&other).is_empty());
    let unrelated = RubyExtractor.extract("myconfig/routes.rb", source);
    assert!(route_records(&unrelated).is_empty());
    // An application in a subdirectory has its own routes file.
    let nested = RubyExtractor.extract("backend/config/routes.rb", source);
    assert_eq!(route_records(&nested), route_records(&result));
}

#[test]
fn rails_routes_refuse_dynamic_contexts_and_non_router_calls() {
    let source = r##"
get "/outside", to: "notes#show"
Other.routes.draw do
  get "/foreign", to: "notes#show"
end
Rails.application.routes.draw do
  get dynamic_path, to: "notes#show"
  get "/dynamic", to: "notes#{suffix}#show"
  scope module: module_name do
    get "/unknown", to: "notes#show"
  end
  if enabled?
    get "/conditional", to: "notes#show"
  end
  helper do
    get "/helper", to: "notes#show"
  end
  get "/safe", to: "notes#show"
end
"##;
    let result = RubyExtractor.extract("config/routes.rb", source);
    assert_eq!(route_records(&result).len(), 1);
    assert_eq!(route_records(&result)[0].0, "GET /safe");
    assert_eq!(
        result
            .errors
            .iter()
            .filter(|e| e.starts_with("Rails routes:"))
            .count(),
        5
    );
    // Each skipped declaration keeps the words a rename must not miss.
    let unsupported: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter_map(|r| r.reference_name.strip_prefix("rails-routes-unsupported:"))
        .collect();
    assert_eq!(unsupported.len(), 5);
    assert!(
        unsupported.contains(&"module notes show to unknown"),
        "{unsupported:?}"
    );
    let malformed = RubyExtractor.extract(
        "config/routes.rb",
        "Rails.application.routes.draw do\nget '/x', to: 'notes#show'\n",
    );
    assert!(route_records(&malformed).is_empty());
}

fn project() -> TempDir {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("config")).unwrap();
    fs::create_dir_all(root.path().join("app/controllers")).unwrap();
    fs::write(
        root.path().join("config/routes.rb"),
        "Rails.application.routes.draw do\n  get '/notes/:id', to: 'notes#show'\nend\n",
    )
    .unwrap();
    fs::write(root.path().join("app/controllers/notes_controller.rb"), "class NotesController < ApplicationController\n  def show\n    load_note()\n  end\n  def load_note; end\nend\n").unwrap();
    root
}

async fn route_edges(graph: &TokenSave) -> Vec<Edge> {
    graph
        .db()
        .get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.resolved_by == Some(ResolvedBy::RailsRoute))
        .collect()
}

#[tokio::test]
async fn rails_route_action_renames_are_blocked_without_editing_router_tokens() {
    for action in ["get", "post", "show"] {
        let root = project();
        let routes = format!(
            "Rails.application.routes.draw do\n get '/sample', to: 'notes#{action}'\nend\n"
        );
        let controller = format!("class NotesController\n def {action}; end\nend\n");
        fs::write(root.path().join("config/routes.rb"), &routes).unwrap();
        fs::write(
            root.path().join("app/controllers/notes_controller.rb"),
            &controller,
        )
        .unwrap();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.index_all().await.unwrap();
        let edges = route_edges(&graph).await;
        assert_eq!(edges.len(), 1);
        let target = graph
            .db()
            .get_node_by_id(&edges[0].target)
            .await
            .unwrap()
            .unwrap();
        let plan = graph
            .plan_rename(target, Some("fetch"), None)
            .await
            .unwrap();
        assert!(
            plan.blockers
                .iter()
                .any(|reason| reason.contains("route target literals")),
            "{action}: {:?}",
            plan.blockers
        );
        let route_site = plan
            .sites
            .iter()
            .find(|site| site.resolved_by == Some("rails-route"))
            .unwrap();
        assert!(route_site.byte_range.is_none(), "{action}: {route_site:?}");
        for allow_heuristic in [false, true] {
            assert!(plan
                .file_diffs(allow_heuristic)
                .iter()
                .all(|diff| !diff.contains("config/routes.rb")));
            let outcome = graph.apply_rename(&plan, allow_heuristic).await.unwrap();
            assert!(!outcome.applied);
            assert!(outcome.refused.unwrap().contains("route target literals"));
            assert_eq!(
                fs::read_to_string(root.path().join("config/routes.rb")).unwrap(),
                routes
            );
            assert_eq!(
                fs::read_to_string(root.path().join("app/controllers/notes_controller.rb"))
                    .unwrap(),
                controller
            );
        }
        graph
            .db()
            .conn()
            .execute(
                "UPDATE edges SET resolved_by = NULL WHERE source = ?1 AND target = ?2",
                libsql::params![edges[0].source.as_str(), edges[0].target.as_str()],
            )
            .await
            .unwrap();
        let without_provenance = graph
            .plan_rename(plan.target.clone(), Some("fetch"), None)
            .await
            .unwrap();
        assert!(without_provenance
            .blockers
            .iter()
            .any(|reason| reason.contains("route target literals")));
        assert!(
            !graph
                .apply_rename(&without_provenance, true)
                .await
                .unwrap()
                .applied
        );
        assert!(without_provenance
            .file_diffs(true)
            .iter()
            .all(|diff| !diff.contains("config/routes.rb")));
    }
}

#[tokio::test]
async fn rails_route_references_bypass_generic_resolution() {
    use tokensave::resolution::ReferenceResolver;

    let root = project();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    let mut nodes = graph.db().get_all_nodes().await.unwrap();
    let route = nodes.iter().find(|n| n.kind == NodeKind::Route).unwrap();
    let reference = graph
        .db()
        .get_unresolved_refs()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.from_node_id == route.id)
        .unwrap();
    assert_eq!(route_edges(&graph).await.len(), 1);

    // Even a colliding generic symbol must not substitute for exact route dispatch.
    let mut collision = nodes.iter().find(|n| n.name == "show").unwrap().clone();
    collision.id = "route-target-collision".into();
    collision.name.clone_from(&reference.reference_name);
    collision
        .qualified_name
        .clone_from(&reference.reference_name);
    nodes.push(collision.clone());
    let resolver = ReferenceResolver::from_nodes(graph.db(), &nodes);
    assert!(resolver.resolve_one(&reference).is_none());

    collision.id = "second-route-target-collision".into();
    nodes.push(collision);
    let resolver = ReferenceResolver::from_nodes(graph.db(), &nodes);
    let result = resolver.resolve_all(&[reference]);
    assert!(result.resolved.is_empty());
    assert!(result.ambiguous.is_empty());
    assert_eq!(result.unresolved.len(), 1);

    let visibility = RubyExtractor.extract(
        "visibility.rb",
        "class NotesController\n private :show\nend\n",
    );
    let reference = visibility
        .unresolved_refs
        .iter()
        .find(|r| r.reference_name.starts_with("ruby-visibility:"))
        .unwrap()
        .clone();
    nodes.extend(visibility.nodes);
    let mut collision = nodes.iter().find(|n| n.name == "show").unwrap().clone();
    collision.id = "visibility-reference-collision".into();
    collision.name.clone_from(&reference.reference_name);
    collision
        .qualified_name
        .clone_from(&reference.reference_name);
    nodes.push(collision);
    let resolver = ReferenceResolver::from_nodes(graph.db(), &nodes);
    assert!(resolver.resolve_one(&reference).is_none());

    let inflections = RubyExtractor.extract(
        "config/initializers/inflections.rb",
        "ActiveSupport::Inflector.inflections(:en) do |inflect|\n inflect.acronym 'API'\nend\n",
    );
    let reference = inflections
        .unresolved_refs
        .iter()
        .find(|reference| reference.reference_name == "ruby-inflections:custom")
        .unwrap()
        .clone();
    nodes.extend(inflections.nodes);
    let mut collision = nodes
        .iter()
        .find(|node| node.name == "show")
        .unwrap()
        .clone();
    collision.id = "inflection-reference-collision".into();
    collision.name.clone_from(&reference.reference_name);
    collision
        .qualified_name
        .clone_from(&reference.reference_name);
    nodes.push(collision);
    let resolver = ReferenceResolver::from_nodes(graph.db(), &nodes);
    assert!(resolver.resolve_one(&reference).is_none());
}

async fn canonical(graph: &TokenSave) -> (Vec<Node>, Vec<Edge>, Vec<UnresolvedRef>) {
    let mut nodes = graph.db().get_all_nodes().await.unwrap();
    for node in &mut nodes {
        node.updated_at = 0;
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut edges = graph.db().get_all_edges().await.unwrap();
    edges.sort_by(|a, b| {
        (&a.source, &a.target, a.kind.as_str(), a.line).cmp(&(
            &b.source,
            &b.target,
            b.kind.as_str(),
            b.line,
        ))
    });
    let mut refs = graph.db().get_unresolved_refs().await.unwrap();
    refs.sort_by(|a, b| {
        (&a.from_node_id, &a.reference_name, a.line, a.column).cmp(&(
            &b.from_node_id,
            &b.reference_name,
            b.line,
            b.column,
        ))
    });
    (nodes, edges, refs)
}

#[test]
fn rails_routes_cover_verbs_filters_and_scope_boundaries() {
    let result = RubyExtractor.extract(
        "config/routes.rb",
        r#"
Rails.application.routes.draw do
  scope module: "admin" do
    scope path: "v1" do
      root to: "notes#index"
      options "/notes", to: "notes#index", format: false
      resources :notes, :messages, only: [:show, :update], except: :show
    end
  end
  delete "/notes/:id", to: "notes#destroy"
  put "/notes/:id", to: "notes#update"
  patch "/notes/:id", to: "notes#update"
end
"#,
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        route_records(&result),
        vec![
            ("GET /v1", "Admin::NotesController#index"),
            ("OPTIONS /v1/notes", "Admin::NotesController#index"),
            ("PATCH /v1/notes/:id", "Admin::NotesController#update"),
            ("PUT /v1/notes/:id", "Admin::NotesController#update"),
            ("PATCH /v1/messages/:id", "Admin::MessagesController#update"),
            ("PUT /v1/messages/:id", "Admin::MessagesController#update"),
            ("DELETE /notes/:id", "NotesController#destroy"),
            ("PUT /notes/:id", "NotesController#update"),
            ("PATCH /notes/:id", "NotesController#update"),
        ]
    );
    let unique: std::collections::HashSet<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Route)
        .map(|n| &n.id)
        .collect();
    assert_eq!(unique.len(), route_records(&result).len());
    let encoded = serde_json::to_string(&result).unwrap();
    let decoded: tokensave::types::ExtractionResult = serde_json::from_str(&encoded).unwrap();
    assert_eq!(result.unresolved_refs, decoded.unresolved_refs);
}

#[test]
fn rails_routes_reject_each_unsupported_static_boundary() {
    for declaration in [
        "get '/x', to: target",
        "get '/x', to: 'notes'",
        "get '/x', to: 'notes#'",
        "get '/x', to: 'notes#show', unknown: true",
        "get '/x', to: 'notes#show', to: 'other#show'",
        "get '/x', to: 'notes#show', defaults: { action: 'other' }",
        "get '/x', to: 'notes#show', on: :member",
        "get :x",
        "get 'search'",
        "get '/a', '/b', to: 'notes#show'",
        "match '/x', to: 'notes#show'",
        "match '/x', to: 'notes#show', via: :head",
        "other.get '/x', to: 'notes#show'",
        "head '/x', to: 'notes#show'",
        "root 'notes#index', 'notes#show'",
        "namespace name do; get '/x', to: 'notes#show'; end",
        "namespace :admin, module: '/other' do; get '/x', to: 'notes#show'; end",
        "scope only: :show do; resources :notes; end",
        "defaults action: 'other' do; get '/x', to: 'notes#show'; end",
        "member do; get :x; end",
        "concerns :commentable",
        "draw dynamic_name",
        "draw '../outside'",
        "resources :notes, :Bad, only: :show",
        "resources :notes, only: :unknown",
        "resources :notes, only: [:show, method_name]",
        "resources :notes, only: :show, except: excluded",
        "resources :notes, only: :show, path: dynamic_path",
        "resources :notes, only: :show, controller: dynamic_controller",
        "resources :notes, shallow: true",
        "resources :notes, concerns: :commentable",
        "resources :notes, only: :show, param: 'a:b'",
        "resources :notes, only: :show, module: '/abs'",
        "resource :profile, only: :index, controller: 'profiles'",
    ] {
        let source = format!("Rails.application.routes.draw do\n {declaration}\n get '/safe', to: 'notes#show'\nend\n");
        let result = RubyExtractor.extract("config/routes.rb", &source);
        assert_eq!(
            route_records(&result).len(),
            1,
            "{declaration}: {:?}",
            route_records(&result)
        );
        assert_eq!(route_records(&result)[0].0, "GET /safe", "{declaration}");
        assert_eq!(
            result
                .nodes
                .iter()
                .filter(|n| n.kind == NodeKind::Route)
                .count(),
            1,
            "{declaration}"
        );
        assert!(!result.errors.is_empty(), "{declaration}");
    }
}

#[tokio::test]
async fn rails_routes_validate_controller_prefixes_after_scope_expansion() {
    let root = project();
    fs::write(root.path().join("app/controllers/prefixed.rb"), "module Api\n class HealthController\n  def ping; end\n end\nend\nmodule Admin\n class HealthController\n  def ping; end\n end\nend\n").unwrap();
    fs::write(root.path().join("config/routes.rb"), "Rails.application.routes.draw do\n scope module: 'Api' do\n  get 'ping', to: 'health#ping'\n end\n namespace 'Admin' do\n  get 'ping', to: 'health#ping'\n end\n scope module: 'api' do\n  get 'ping', to: 'health#ping'\n end\nend\n").unwrap();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    let edges = route_edges(&graph).await;
    assert_eq!(edges.len(), 1);
    assert!(graph
        .get_node(&edges[0].target)
        .await
        .unwrap()
        .unwrap()
        .qualified_name
        .ends_with("::Api::HealthController::ping"));
    let status = graph.db().rails_route_status().await.unwrap().unwrap();
    assert_eq!(
        status["extraction_diagnostics"].as_array().unwrap().len(),
        2
    );
    assert!(status["duration_ms"].is_u64());
    let absolute = RubyExtractor.extract("config/routes.rb", "Rails.application.routes.draw do\n scope module: 'Api' do\n  get 'ping', to: '/health#ping'\n end\nend\n");
    assert!(absolute.errors.is_empty());
    assert_eq!(
        route_records(&absolute),
        vec![("GET /ping", "::HealthController#ping")]
    );
}

#[test]
fn rails_route_inflection_evidence_survives_diagnostic_changes_and_serialization() {
    let mut result = RubyExtractor.extract(
        "config/initializers/inflections.rb",
        "ActiveSupport::Inflector.inflections(:en) do |inflect|\n inflect.acronym 'API'\nend\n",
    );
    let evidence = result
        .unresolved_refs
        .iter()
        .find(|reference| reference.reference_name == "ruby-inflections:custom")
        .unwrap()
        .clone();
    assert_eq!(evidence.reference_kind, EdgeKind::Uses);
    assert!(result
        .nodes
        .iter()
        .any(|node| node.id == evidence.from_node_id && node.kind == NodeKind::File));
    result.errors.clear();
    let serialized = serde_json::to_value(&result).unwrap();
    let decoded: tokensave::types::ExtractionResult =
        serde_json::from_value(serialized.clone()).unwrap();
    assert!(decoded.unresolved_refs.contains(&evidence));
    assert!(decoded.errors.is_empty());
    let mut without_evidence = serialized;
    without_evidence["unresolved_refs"] = serde_json::json!([]);
    let decoded: tokensave::types::ExtractionResult =
        serde_json::from_value(without_evidence).unwrap();
    assert!(decoded.unresolved_refs.is_empty());
    assert!(!RubyExtractor
        .extract(
            "config/initializers/inflections.rb",
            "# ActiveSupport::Inflector.inflections(:en)\n"
        )
        .unresolved_refs
        .iter()
        .any(|reference| reference.reference_name == "ruby-inflections:custom"));
}

#[tokio::test]
async fn rails_route_search_keeps_controller_definitions_visible() {
    let root = project();
    fs::create_dir_all(root.path().join("app/models")).unwrap();
    fs::write(root.path().join("app/models/user.rb"), "class User; end\n").unwrap();
    fs::write(root.path().join("app/controllers/users_controller.rb"), "class UsersController\n def index; end\nend\nmodule Admin\n class UsersController\n  def index; end\n end\nend\n").unwrap();
    let declarations: String = (0..14)
        .map(|i| format!(" get '/users/{i}', to: 'users#index'\n"))
        .collect();
    fs::write(
        root.path().join("config/routes.rb"),
        format!("Rails.application.routes.draw do\n{declarations}end\n"),
    )
    .unwrap();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    let results = graph.search("users", 10).await.unwrap();
    assert_eq!(
        results
            .iter()
            .filter(|r| r.node.kind == NodeKind::Class && r.node.name == "UsersController")
            .count(),
        2
    );
    assert!(results.iter().any(|r| r.node.name == "User"));
    assert!(graph
        .search("GET /users/0", 10)
        .await
        .unwrap()
        .iter()
        .any(|r| r.node.kind == NodeKind::Route && r.node.name == "GET /users/0"));
}

#[tokio::test]
async fn rails_routes_reject_reserved_dispatch_segments_in_complete_paths() {
    for declaration in [
        "get '/:controller/:action', to: 'notes#show'",
        "get '/:controller', to: 'notes#show'",
        "get '/notes/:action', to: 'notes#show'",
        "get '/notes(/:action)', to: 'notes#show'",
        "get '/:controller.:action', to: 'notes#show'",
        "get '/:controller-name', to: 'notes#show'",
        "get '/*controller/:action', to: 'notes#show'",
        "get '/notes/*action', to: 'notes#show'",
        "scope '/:controller' do; get '/:action', to: 'notes#show'; end",
        "scope path: '/(:action)' do; root to: 'notes#show'; end",
        "namespace :admin, path: ':controller' do; get '/notes', to: '/notes#show'; end",
        "scope ':action' do; resources :notes, only: :show; end",
        "resources :notes, only: :show, path: ':controller'",
        "resource :note, only: :show, controller: 'notes', path: '*action'",
    ] {
        let root = project();
        fs::write(
            root.path().join("config/routes.rb"),
            format!("Rails.application.routes.draw do\n {declaration}\nend\n"),
        )
        .unwrap();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.index_all().await.unwrap();
        assert!(route_edges(&graph).await.is_empty(), "{declaration}");
        let status = graph.db().rails_route_status().await.unwrap().unwrap();
        assert!(
            status["counts_by_status"].as_object().unwrap().is_empty(),
            "{declaration}"
        );
        assert!(
            !status["extraction_diagnostics"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{declaration}"
        );
    }
    let result = RubyExtractor.extract("config/routes.rb", "Rails.application.routes.draw do\n get '/:controller_name/:action_id', to: 'notes#show'\n get '/controller/action', to: 'notes#show'\nend\n");
    assert_eq!(route_records(&result).len(), 2);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn rails_route_diagnostics_clear_when_unsupported_only_file_is_deleted() {
    for deletion in ["sync", "stale", "database"] {
        let root = project();
        fs::write(
            root.path().join("config/routes.rb"),
            "Rails.application.routes.draw do\n resources :notes, shallow: true\nend\n",
        )
        .unwrap();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.sync().await.unwrap();
        assert!(route_edges(&graph).await.is_empty());
        assert_ne!(
            graph
                .db()
                .get_metadata("rails_routes_pending")
                .await
                .unwrap()
                .as_deref(),
            Some("1")
        );
        assert!(
            !graph.db().rails_route_status().await.unwrap().unwrap()["extraction_diagnostics"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        fs::remove_file(root.path().join("config/routes.rb")).unwrap();
        match deletion {
            "sync" => {
                graph.sync().await.unwrap();
            }
            "stale" => {
                graph
                    .sync_if_stale(&["config/routes.rb".into()])
                    .await
                    .unwrap();
            }
            _ => {
                graph.db().delete_file("config/routes.rb").await.unwrap();
            }
        }
        assert!(
            graph.db().rails_route_status().await.unwrap().is_none(),
            "{deletion}"
        );
        drop(graph);
        let graph = TokenSave::open(root.path()).await.unwrap();
        assert!(
            graph.db().rails_route_status().await.unwrap().is_none(),
            "{deletion}"
        );
    }
}

#[tokio::test]
async fn rails_route_failures_retry_and_inflection_changes_invalidate_links() {
    let root = project();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.sync().await.unwrap();
    let before = route_edges(&graph).await;
    graph.db().conn().execute_batch("CREATE TRIGGER reject_rails_edges BEFORE INSERT ON edges WHEN NEW.resolved_by = 15 BEGIN SELECT RAISE(ABORT, 'synthetic route failure'); END;").await.unwrap();
    graph
        .db()
        .set_metadata("rails_routes_pending", "1")
        .await
        .unwrap();
    graph.sync().await.unwrap();
    assert_eq!(before, route_edges(&graph).await);
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["pending"],
        true
    );
    graph
        .db()
        .conn()
        .execute_batch("DROP TRIGGER reject_rails_edges;")
        .await
        .unwrap();
    graph.sync().await.unwrap();
    assert_eq!(before, route_edges(&graph).await);
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["pending"],
        false
    );

    fs::create_dir_all(root.path().join("config/initializers")).unwrap();
    fs::write(
        root.path().join("config/initializers/inflections.rb"),
        "ActiveSupport::Inflector.inflections(:en) do |inflect|\n inflect.acronym 'API'\nend\n",
    )
    .unwrap();
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"]
            ["custom-inflections"],
        1
    );
    fs::remove_file(root.path().join("config/initializers/inflections.rb")).unwrap();
    graph.sync().await.unwrap();
    assert_eq!(route_edges(&graph).await.len(), 1);
}

#[tokio::test]
async fn rails_routes_reject_unrelated_singleton_and_inherited_actions() {
    let root = project();
    fs::write(root.path().join("app/controllers/notes_controller.rb"), "class BaseController\n def show; end\nend\nclass NotesController < BaseController\n def self.show; end\nend\nclass OtherController\n def show; end\nend\n").unwrap();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"]
            ["non-public-instance-action"],
        1
    );
    fs::write(
        root.path().join("app/controllers/reopened.rb"),
        "class NotesController\n def show; end\nend\n",
    )
    .unwrap();
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    fs::write(
        root.path().join("app/controllers/notes_controller.rb"),
        "class NotesController; end\n",
    )
    .unwrap();
    graph.sync().await.unwrap();
    assert_eq!(route_edges(&graph).await.len(), 1);
    fs::remove_file(root.path().join("app/controllers/reopened.rb")).unwrap();
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"]
            ["missing-action"],
        1
    );
    fs::remove_file(root.path().join("app/controllers/notes_controller.rb")).unwrap();
    graph.sync().await.unwrap();
    assert_eq!(
        graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"]
            ["missing-controller"],
        1
    );
}

#[tokio::test]
async fn rails_route_reference_backfill_preserves_the_existing_schema() {
    let root = project();
    fs::create_dir_all(root.path().join("config/initializers")).unwrap();
    fs::write(
        root.path().join("config/initializers/inflections.rb"),
        "# Default inflections\n",
    )
    .unwrap();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.sync().await.unwrap();
    graph.db().conn().execute_batch("DELETE FROM nodes WHERE kind = 'route'; DELETE FROM metadata WHERE key LIKE 'rails_route%';").await.unwrap();
    drop(graph);
    let graph = TokenSave::open(root.path()).await.unwrap();
    let sync = graph.sync().await.unwrap();
    assert_eq!(sync.files_modified, 3);
    assert_eq!(route_edges(&graph).await.len(), 1);
    assert_eq!(graph.sync().await.unwrap().files_modified, 0);
    assert_eq!(
        graph
            .db()
            .get_metadata("rails_route_references_v3")
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let mut version = graph
        .db()
        .conn()
        .query("PRAGMA user_version", ())
        .await
        .unwrap();
    assert_eq!(
        version
            .next()
            .await
            .unwrap()
            .unwrap()
            .get::<i64>(0)
            .unwrap(),
        18
    );
    let mut tables = graph.db().conn().query("SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','trigger') AND name LIKE 'rails_%'", ()).await.unwrap();
    assert_eq!(
        tables.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
        0
    );
}

#[tokio::test]
async fn rails_routes_link_exact_actions_and_restore_after_edits() {
    let root = project();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    let edge = route_edges(&graph).await.pop().unwrap();
    let action = graph.get_node(&edge.target).await.unwrap().unwrap();
    let route = graph.get_node(&edge.source).await.unwrap().unwrap();
    assert_eq!(action.name, "show");
    assert_eq!(route.name, "GET /notes/:id");
    assert_eq!(edge.line, Some(1));
    assert!(graph
        .get_callers(&action.id, 2)
        .await
        .unwrap()
        .iter()
        .any(|(n, _)| n.id == route.id));
    assert!(graph
        .get_callees(&route.id, 2)
        .await
        .unwrap()
        .iter()
        .any(|(n, _)| n.name == "load_note"));
    assert!(graph
        .search("notes", 20)
        .await
        .unwrap()
        .iter()
        .any(|r| r.node.kind == NodeKind::Route));
    assert!(graph
        .search("/notes/:id", 20)
        .await
        .unwrap()
        .iter()
        .any(|r| r.node.id == route.id));
    assert!(graph
        .get_impact_radius(&action.id, 2)
        .await
        .unwrap()
        .nodes
        .iter()
        .any(|n| n.id == route.id));
    assert!(graph
        .find_dead_code(&[NodeKind::Route], true, true)
        .await
        .unwrap()
        .is_empty());
    let context = graph
        .build_context(
            &route.name,
            &tokensave::types::BuildContextOptions::default(),
        )
        .await
        .unwrap();
    assert!(
        context
            .entry_points
            .iter()
            .any(|n| n.kind == NodeKind::Route),
        "{:?}",
        context.entry_points
    );
    let rename = graph
        .plan_rename(route.clone(), Some("other"), None)
        .await
        .unwrap();
    assert!(!rename.blockers.is_empty());
    assert!(graph
        .replace_symbol(&route.qualified_name, "changed", None)
        .await
        .is_err());

    graph
        .str_replace(
            "app/controllers/notes_controller.rb",
            "def show",
            "def show # changed",
            None,
        )
        .await
        .unwrap();
    assert_eq!(route_edges(&graph).await.len(), 1);
    fs::write(
        root.path().join("app/controllers/notes_controller.rb"),
        "class NotesController\n  private\n  def show; end\nend\n",
    )
    .unwrap();
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    fs::write(
        root.path().join("app/controllers/notes_controller.rb"),
        "class NotesController\n  def show; end\nend\n",
    )
    .unwrap();
    assert!(!graph
        .sync_if_stale(&["app/controllers/notes_controller.rb".into()])
        .await
        .unwrap());
    assert_eq!(route_edges(&graph).await.len(), 1);
    fs::write(
        root.path().join("app/controllers/reopened.rb"),
        "class NotesController\n def show; end\nend\n",
    )
    .unwrap();
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    fs::remove_file(root.path().join("app/controllers/reopened.rb")).unwrap();
    graph.sync().await.unwrap();
    assert_eq!(route_edges(&graph).await.len(), 1);
    let before = canonical(&graph).await;
    graph.index_all().await.unwrap();
    assert_eq!(before, canonical(&graph).await);
    fs::remove_file(root.path().join("config/routes.rb")).unwrap();
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    assert_eq!(
        graph
            .db()
            .get_metadata("rails_routes_pending")
            .await
            .unwrap()
            .as_deref(),
        Some("0")
    );
}

#[tokio::test]
async fn rails_route_edits_moves_and_restart_match_full_index() {
    let root = project();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.sync().await.unwrap();
    graph
        .str_replace("config/routes.rb", "'/notes/:id'", "'/pages/:id'", None)
        .await
        .unwrap();
    let edge = route_edges(&graph).await.pop().unwrap();
    assert_eq!(
        graph.get_node(&edge.source).await.unwrap().unwrap().name,
        "GET /pages/:id"
    );
    let edited = canonical(&graph).await;
    graph.index_all().await.unwrap();
    assert_eq!(edited, canonical(&graph).await);

    fs::write(root.path().join("config/routes.rb"), "Rails.application.routes.draw do\n namespace :admin do\n  get '/notes', to: 'notes#show'\n end\nend\n").unwrap();
    fs::write(
        root.path()
            .join("app/controllers/admin_notes_controller.rb"),
        "module Admin\n class NotesController\n  def show; end\n end\nend\n",
    )
    .unwrap();
    graph.sync().await.unwrap();
    let edge = route_edges(&graph).await.pop().unwrap();
    let action = graph.get_node(&edge.target).await.unwrap().unwrap();
    assert!(action
        .qualified_name
        .ends_with("::Admin::NotesController::show"));
    let scoped = canonical(&graph).await;
    graph.index_all().await.unwrap();
    assert_eq!(scoped, canonical(&graph).await);

    fs::rename(
        root.path()
            .join("app/controllers/admin_notes_controller.rb"),
        root.path().join("app/controllers/moved.rb"),
    )
    .unwrap();
    graph.sync().await.unwrap();
    assert_eq!(route_edges(&graph).await.len(), 1);
    let moved = canonical(&graph).await;
    drop(graph);
    let graph = TokenSave::open(root.path()).await.unwrap();
    graph.sync().await.unwrap();
    assert_eq!(moved, canonical(&graph).await);
    graph.index_all().await.unwrap();
    assert_eq!(moved, canonical(&graph).await);

    fs::write(
        root.path().join("config/routes.rb"),
        "Rails.application.routes.draw do\n get '/new', to: '/notes#show'\nend\n",
    )
    .unwrap();
    graph
        .sync_if_stale(&["config/routes.rb".into()])
        .await
        .unwrap();
    let edge = route_edges(&graph).await.pop().unwrap();
    assert_eq!(
        graph.get_node(&edge.source).await.unwrap().unwrap().name,
        "GET /new"
    );
    let stale = canonical(&graph).await;
    graph.index_all().await.unwrap();
    assert_eq!(stale, canonical(&graph).await);

    fs::rename(
        root.path().join("config/routes.rb"),
        root.path().join("config/unloaded_routes.rb"),
    )
    .unwrap();
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    let removed = canonical(&graph).await;
    graph.index_all().await.unwrap();
    assert_eq!(removed, canonical(&graph).await);
    assert!(graph.db().rails_route_status().await.unwrap().is_none());
}

/// The expected table is what `ActionDispatch::Routing::RouteSet#draw` 8.1.3.1
/// builds from the same fixture (controller routes only, `(.:format)` dropped).
#[test]
fn rails_route_dsl_matches_actionpack() {
    let result = RubyExtractor.extract(
        "config/routes.rb",
        include_str!("fixtures/rails_routes_dsl.rb"),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let mut labels: Vec<&str> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Route)
        .map(|n| n.signature.as_deref().unwrap())
        .collect();
    labels.sort_unstable();
    assert_eq!(
        labels,
        vec![
            "DELETE /people/:id -> people#destroy",
            "DELETE /profile -> profiles#destroy",
            "DELETE /users/:id -> users#destroy",
            "GET / -> home#index",
            "GET /account/keys -> keys#index",
            "GET /account/settings -> accounts#settings",
            "GET /admin/stats/daily -> admin/stats#daily",
            "GET /admin/users/:id -> admin/staff/users#show",
            "GET /feed -> feed#index",
            "GET /legacy -> pages#legacy",
            "GET /login -> sessions#new",
            "GET /people -> people#index",
            "GET /people/:id -> people#show",
            "GET /people/:id/edit -> people#edit",
            "GET /people/:person_id/posts -> posts#index",
            "GET /people/new -> people#new",
            "GET /photos/:id/pre-view -> photos#pre_view",
            "GET /photos/:id/preview -> photos#preview",
            "GET /photos/:photo_id/thumbnail -> photos#thumbnail",
            "GET /photos/archive -> photos#archive",
            "GET /photos/new/draft -> photos#draft",
            "GET /photos/search -> photos#search",
            "GET /profile -> profiles#show",
            "GET /profile/edit -> profiles#edit",
            "GET /profile/new -> profiles#new",
            "GET /search -> search#index",
            "GET /staff/audit -> staff/logs#index",
            "GET /status -> health#show",
            "GET /stuff/:slug -> goods#show",
            "GET /users -> users#index",
            "GET /users/:id -> users#show",
            "GET /users/:id/edit -> users#edit",
            "GET /users/:user_id/notes -> users/notes#index",
            "GET /users/:user_id/x/a -> x/b#c",
            "GET /users/new -> users#new",
            "PATCH /people/:id -> people#update",
            "PATCH /profile -> profiles#update",
            "PATCH /users/:id -> users#update",
            "POST /admin/reports -> admin/reports#create",
            "POST /people -> people#create",
            "POST /photos/:id/rotate -> photos#rotate",
            "POST /profile -> profiles#create",
            "POST /search -> search#index",
            "POST /users -> users#create",
            "PUT /people/:id -> people#update",
            "PUT /profile -> profiles#update",
            "PUT /users/:id -> users#update",
        ]
    );
    // Default new/edit routes exist only outside API-only applications.
    let api_gated: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_name.starts_with('~'))
        .map(|r| r.reference_name.as_str())
        .collect();
    assert_eq!(
        api_gated,
        vec![
            "~UsersController#new",
            "~UsersController#edit",
            "~ProfilesController#new",
            "~ProfilesController#edit",
            "~PeopleController#new",
            "~PeopleController#edit",
        ]
    );
}

fn write(root: &TempDir, path: &str, content: &str) {
    let path = root.path().join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

async fn route_counts(graph: &TokenSave) -> serde_json::Value {
    graph.db().rails_route_status().await.unwrap().unwrap()["counts_by_status"].clone()
}

async fn linked_actions(graph: &TokenSave) -> Vec<String> {
    let mut targets = Vec::new();
    for edge in route_edges(graph).await {
        let node = graph.get_node(&edge.target).await.unwrap().unwrap();
        let constant = node
            .qualified_name
            .rsplit(".rb::")
            .next()
            .unwrap()
            .to_string();
        targets.push(constant);
    }
    targets.sort();
    targets.dedup();
    targets
}

#[tokio::test]
async fn rails_routes_follow_draw_files_engines_and_api_only_apps() {
    let root = project();
    write(&root, "config/application.rb", "module App\n class Application < Rails::Application\n  config.api_only = true\n end\nend\n");
    write(&root, "config/routes.rb", "Rails.application.routes.draw do\n resources :notes\n namespace :admin do\n  draw :admin\n end\nend\n");
    write(
        &root,
        "config/routes/admin.rb",
        "resources :reports, only: :index\nget 'ping', to: '/health#ping'\n",
    );
    write(
        &root,
        "config/routes/unused.rb",
        "get 'x', to: 'notes#show'\n",
    );
    write(&root, "app/controllers/notes_controller.rb", "class NotesController\n def index; end\n def new; end\n def edit; end\n def show; end\nend\n");
    write(
        &root,
        "app/controllers/admin/reports_controller.rb",
        "module Admin\n class ReportsController\n  def index; end\n end\nend\n",
    );
    write(
        &root,
        "app/controllers/health_controller.rb",
        "class HealthController\n def ping; end\nend\n",
    );
    // The application's block in an engine's file is not namespaced by the engine.
    write(
        &root,
        "engines/blog/config/routes.rb",
        "Blog::Engine.routes.draw do\n resources :posts, only: [:index, :new]\nend\nRails.application.routes.draw do\n get 'blog-admin', to: 'blog_admin#index'\nend\n",
    );
    write(
        &root,
        "app/controllers/blog_admin_controller.rb",
        "class BlogAdminController\n def index; end\nend\n",
    );
    let engine =
        "module Blog\n class Engine < ::Rails::Engine\n  isolate_namespace Blog\n end\nend\n";
    write(&root, "engines/blog/lib/blog/engine.rb", engine);
    write(
        &root,
        "engines/blog/app/controllers/blog/posts_controller.rb",
        "module Blog\n class PostsController\n  def index; end\n  def new; end\n end\nend\n",
    );
    write(
        &root,
        "backend/config/routes.rb",
        "Rails.application.routes.draw do\n get 'up', to: 'status#show'\nend\n",
    );
    write(
        &root,
        "backend/app/controllers/status_controller.rb",
        "class StatusController\n def show; end\nend\n",
    );
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    assert_eq!(
        linked_actions(&graph).await,
        [
            "Admin::ReportsController::index",
            "Blog::PostsController::index",
            "Blog::PostsController::new",
            "BlogAdminController::index",
            "HealthController::ping",
            "NotesController::index",
            "NotesController::show",
            "StatusController::show",
        ]
    );
    let counts = route_counts(&graph).await;
    assert_eq!(counts["resolved"], 8, "{counts}");
    assert_eq!(counts["api-only"], 2, "{counts}");
    assert_eq!(counts["not-drawn"], 1, "{counts}");
    assert_eq!(counts["missing-action"], 4, "{counts}");
    let full = canonical(&graph).await;

    // A non-isolated engine routes to top-level controllers.
    write(
        &root,
        "engines/blog/lib/blog/engine.rb",
        "module Blog\n class Engine < ::Rails::Engine\n end\nend\n",
    );
    graph.sync().await.unwrap();
    assert_eq!(route_counts(&graph).await["missing-controller"], 2);
    fs::remove_file(root.path().join("engines/blog/lib/blog/engine.rb")).unwrap();
    graph.sync().await.unwrap();
    assert_eq!(route_counts(&graph).await["missing-engine"], 2);
    write(&root, "engines/blog/lib/blog/engine.rb", engine);
    fs::remove_file(root.path().join("config/application.rb")).unwrap();
    graph.sync().await.unwrap();
    assert_eq!(route_counts(&graph).await["resolved"], 10);
    write(&root, "config/application.rb", "module App\n class Application < Rails::Application\n  config.api_only = true\n end\nend\n");
    graph.sync().await.unwrap();
    assert_eq!(full, canonical(&graph).await);

    // Undrawing a file unlinks its routes; drawing it twice in different namespaces is ambiguous.
    write(
        &root,
        "config/routes.rb",
        "Rails.application.routes.draw do\n resources :notes\nend\n",
    );
    graph.sync().await.unwrap();
    assert_eq!(route_counts(&graph).await["not-drawn"], 3);
    write(&root, "config/routes.rb", "Rails.application.routes.draw do\n draw :admin\n namespace :admin do\n  draw :admin\n end\nend\n");
    graph.sync().await.unwrap();
    assert_eq!(route_counts(&graph).await["ambiguous-draw"], 2);
    let incremental = canonical(&graph).await;
    graph.index_all().await.unwrap();
    assert_eq!(incremental, canonical(&graph).await);
}

#[tokio::test]
async fn rails_route_renames_cover_unlinked_inherited_and_class_targets() {
    let root = project();
    write(&root, "config/routes.rb", "Rails.application.routes.draw do\n get '/notes/:id', to: 'notes#show'\n resources :archives, concerns: :commentable\n namespace :admin do\n  get 'stats', to: 'stats#daily'\n end\nend\n");
    write(&root, "app/controllers/notes_controller.rb", "class BaseController\n def show; end\n def helper; end\nend\nclass NotesController < BaseController\nend\nclass ArchivesController\n def index; end\nend\nclass Formatter\n def show; end\nend\nmodule Admin\n class StatsController\n  def daily; end\n end\nend\n");
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.index_all().await.unwrap();
    assert!(route_edges(&graph).await.len() == 1);
    let blocked = |plan: &tokensave::tokensave::RenamePlan| {
        plan.blockers
            .iter()
            .any(|reason| reason.contains("route target literals"))
    };
    let node = |name: &'static str| {
        let graph = &graph;
        async move {
            graph
                .db()
                .get_all_nodes()
                .await
                .unwrap()
                .into_iter()
                .find(|n| n.qualified_name.ends_with(name))
                .unwrap()
        }
    };
    for (symbol, expected) in [
        // An inherited action is routed through the subclass.
        ("::BaseController::show", true),
        ("::BaseController::helper", false),
        ("::Formatter::show", false),
        ("::NotesController", true),
        // Only the unsupported declaration mentions these.
        ("::ArchivesController", true),
        ("::ArchivesController::index", true),
        ("::Admin", true),
        ("::Admin::StatsController::daily", true),
    ] {
        let plan = graph
            .plan_rename(node(symbol).await, Some("renamed"), None)
            .await
            .unwrap();
        assert_eq!(blocked(&plan), expected, "{symbol}: {:?}", plan.blockers);
    }
    // Without any route edge (custom inflections leave every route unlinked).
    write(
        &root,
        "config/initializers/inflections.rb",
        "ActiveSupport::Inflector.inflections(:en) do |inflect|\n inflect.acronym 'API'\nend\n",
    );
    graph.sync().await.unwrap();
    assert!(route_edges(&graph).await.is_empty());
    for symbol in [
        "::BaseController::show",
        "::NotesController",
        "::Admin::StatsController::daily",
    ] {
        let plan = graph
            .plan_rename(node(symbol).await, Some("renamed"), None)
            .await
            .unwrap();
        assert!(blocked(&plan), "{symbol}");
    }
}

/// Unreadable file permissions are how the test makes the extractor skip a file.
#[cfg(unix)]
#[tokio::test]
async fn rails_route_backfill_runs_once_even_when_a_file_is_skipped() {
    let root = project();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.sync().await.unwrap();
    graph
        .db()
        .conn()
        .execute_batch("DELETE FROM metadata WHERE key = 'rails_route_references_v3';")
        .await
        .unwrap();
    drop(graph);
    let controller = root.path().join("app/controllers/notes_controller.rb");
    let mut permissions = fs::metadata(&controller).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o000);
    fs::set_permissions(&controller, permissions.clone()).unwrap();
    let graph = TokenSave::open(root.path()).await.unwrap();
    graph.sync().await.unwrap();
    assert_eq!(
        graph
            .db()
            .get_metadata("rails_route_references_v3")
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let second = graph.sync().await.unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o644);
    fs::set_permissions(&controller, permissions).unwrap();
    assert_eq!(second.files_modified, 0);
}
