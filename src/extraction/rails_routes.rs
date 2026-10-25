//! Static Rails route declarations as `route` nodes with `Const#action`
//! targets (see `rails_support` for the evidence format).
//!
//! The supported DSL follows `ActionDispatch::Routing::Mapper` 8.1.3.1,
//! checked against `RouteSet#draw`:
//!
//! - `resources`/`resource` map the standard actions (`only:`/`except:`
//!   filter them; the singular form pluralizes its controller and has no
//!   `index`). The block runs first, inside the resource's scope. A verb in
//!   a `resources` block is nested (`/users/:user_id/x`), one in a
//!   `resource` block is a member route; `on:` and `member`/`collection`/
//!   `new` blocks pick the level explicitly. A `resources` inside another
//!   resource, and a `namespace` inside one, is nested under it; `scope`
//!   keeps the current resource level.
//! - A verb's controller comes from `to: "c#a"`, else `controller:` or the
//!   enclosing resource/`controller` scope; its action from `to:` without
//!   `#`, else `action:`, else a plain path or symbol (`get :preview`,
//!   `get "pre-view"` maps to `pre_view`), else the `"a/b"` shorthand.
//!   `path => target` hash-rocket arguments are accepted.
//! - A target starting with `/` ignores the enclosing modules.
//!
//! Anything else is reported as unsupported, with the words it mentions,
//! so a rename can refuse to touch names a skipped declaration may use.

use std::collections::{BTreeSet, HashMap};

use tree_sitter::Node as TsNode;

use super::rails_inflector::{pluralize, singularize};
use super::rails_support::{
    camelize, route_file, RouteFile, RouteTarget, RAILS_API_ONLY_REFERENCE,
    RAILS_ROUTES_DRAW_PREFIX, RAILS_ROUTES_ENGINE_PREFIX, RAILS_ROUTES_UNSUPPORTED_PREFIX,
    RUBY_CUSTOM_INFLECTIONS_REFERENCE,
};
use crate::types::{generate_node_id, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef};

const VERBS: [&str; 6] = ["get", "post", "put", "patch", "delete", "options"];
const CANONICAL_ACTIONS: [&str; 6] = ["index", "create", "new", "show", "update", "destroy"];
const PLURAL_ACTIONS: [&str; 7] = [
    "index", "create", "new", "edit", "show", "update", "destroy",
];
/// The order `resource` adds its routes in.
const SINGULAR_ACTIONS: [&str; 6] = ["new", "edit", "show", "update", "destroy", "create"];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Level {
    #[default]
    Plain,
    Resources,
    Resource,
    Member,
    Collection,
    New,
    Nested,
}

/// A resource's paths relative to the scope path in effect where they are used.
#[derive(Clone)]
struct Resource {
    path: String,
    param: String,
    singular: String,
    singleton: bool,
}

impl Resource {
    fn member(&self) -> String {
        if self.singleton {
            self.path.clone()
        } else {
            join(&self.path, &format!(":{}", self.param))
        }
    }

    fn nested(&self) -> String {
        if self.singleton {
            self.path.clone()
        } else {
            join(&self.path, &format!(":{}_{}", self.singular, self.param))
        }
    }
}

#[derive(Clone, Default)]
struct Scope {
    path: String,
    module: String,
    controller: Option<String>,
    level: Level,
    resource: Option<Resource>,
}

impl Scope {
    fn in_resource(&self) -> bool {
        matches!(self.level, Level::Resources | Level::Resource)
    }

    /// Enters `member`/`collection`/`new`/nested scope of the enclosing resource.
    fn enter(&self, level: Level) -> Option<Self> {
        if !self.in_resource() {
            return None;
        }
        let resource = self.resource.as_ref()?;
        let relative = match level {
            Level::Member => resource.member(),
            Level::Collection => resource.path.clone(),
            Level::New => join(&resource.path, "new"),
            Level::Nested => resource.nested(),
            _ => return None,
        };
        Some(Self {
            path: join(&self.path, &relative),
            level,
            ..self.clone()
        })
    }
}

/// A literal argument; symbols and strings behave differently as route paths.
enum Literal {
    Symbol(String),
    Str(String),
}

impl Literal {
    fn text(&self) -> &str {
        match self {
            Self::Symbol(s) | Self::Str(s) => s,
        }
    }
}

struct Arguments<'a> {
    positional: Vec<TsNode<'a>>,
    options: HashMap<String, TsNode<'a>>,
    /// The `"path" => target` pair of a hash-rocket route.
    hash_path: Option<(TsNode<'a>, TsNode<'a>)>,
}

struct Ctx<'r> {
    source: &'r str,
    file: Node,
    result: &'r mut ExtractionResult,
}

pub(super) fn extract(root: TsNode<'_>, source: &str, result: &mut ExtractionResult) {
    let Some(file) = result.nodes.first().cloned() else {
        return;
    };
    let path = file.file_path.clone();
    let Some(kind) = route_file(&path) else {
        return;
    };
    let mut ctx = Ctx {
        source,
        file,
        result,
    };
    match kind {
        RouteFile::Inflections => ctx.inflections(root),
        RouteFile::Application { .. } => ctx.application(root),
        _ if root.has_error() => {
            ctx.unsupported(root, "malformed source; route extraction skipped");
        }
        RouteFile::Main { .. } => {
            for node in root.named_children(&mut root.walk()) {
                ctx.draw_block(node);
            }
        }
        RouteFile::Drawn { .. } => ctx.statements(root, &Scope::default()),
    }
}

fn text<'a>(node: TsNode<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or("")
}

fn chain(node: TsNode<'_>, source: &str) -> Option<String> {
    if matches!(node.kind(), "constant" | "scope_resolution") {
        return Some(text(node, source).trim_start_matches("::").into());
    }
    if node.kind() != "call" || node.child_by_field_name("arguments").is_some() {
        return None;
    }
    let method = text(node.child_by_field_name("method")?, source);
    let receiver = chain(node.child_by_field_name("receiver")?, source)?;
    Some(format!("{receiver}.{method}"))
}

fn literal(node: TsNode<'_>, source: &str) -> Option<Literal> {
    let raw = text(node, source);
    match node.kind() {
        "simple_symbol" => Some(Literal::Symbol(raw.strip_prefix(':')?.into())),
        "string" => {
            // No Ruby evaluation or escape decoding: accept only plain quoted literals.
            let bytes = raw.as_bytes();
            if bytes.len() < 2
                || !matches!(bytes[0], b'\'' | b'"')
                || bytes[0] != bytes[bytes.len() - 1]
            {
                return None;
            }
            let inner = raw.get(1..raw.len() - 1)?;
            if inner.contains('\\')
                || node
                    .named_children(&mut node.walk())
                    .any(|n| n.kind() != "string_content")
            {
                return None;
            }
            Some(Literal::Str(inner.into()))
        }
        _ => None,
    }
}

fn literal_text(node: TsNode<'_>, source: &str) -> Option<String> {
    literal(node, source).map(|l| l.text().to_string())
}

fn arguments<'a>(node: TsNode<'a>, source: &str) -> Option<Arguments<'a>> {
    let mut parsed = Arguments {
        positional: Vec::new(),
        options: HashMap::new(),
        hash_path: None,
    };
    let Some(args) = node.child_by_field_name("arguments") else {
        return Some(parsed);
    };
    for arg in args.named_children(&mut args.walk()) {
        let pairs: Vec<TsNode<'a>> = match arg.kind() {
            "comment" => continue,
            "pair" => vec![arg],
            "hash" => arg
                .named_children(&mut arg.walk())
                .filter(|n| n.kind() != "comment")
                .collect(),
            _ => {
                parsed.positional.push(arg);
                continue;
            }
        };
        for pair in pairs {
            if pair.kind() != "pair" {
                return None;
            }
            let key = pair.child_by_field_name("key")?;
            let value = pair.child_by_field_name("value")?;
            let name = match key.kind() {
                "hash_key_symbol" => text(key, source).to_string(),
                "simple_symbol" => text(key, source).strip_prefix(':')?.to_string(),
                "string" if parsed.hash_path.is_none() => {
                    parsed.hash_path = Some((key, value));
                    continue;
                }
                _ => return None,
            };
            if parsed.options.insert(name, value).is_some() {
                return None;
            }
        }
    }
    Some(parsed)
}

fn join(prefix: &str, suffix: &str) -> String {
    [prefix.trim_matches('/'), suffix.trim_matches('/')]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// `defaults:` may not name the controller or action, which would change dispatch.
fn static_defaults(node: TsNode<'_>, source: &str) -> bool {
    node.kind() == "hash"
        && node
            .named_children(&mut node.walk())
            .filter(|n| n.kind() != "comment")
            .all(|pair| {
                pair.kind() == "pair"
                    && pair.child_by_field_name("key").is_some_and(|key| {
                        !dispatch_key(text(key, source).trim_matches([':', '\'', '"']))
                    })
            })
}

fn dispatch_key(key: &str) -> bool {
    matches!(key, "controller" | "action" | "module")
}

/// Options that only filter or name routes, never change their target.
fn neutral_option(key: &str, value: TsNode<'_>, source: &str) -> bool {
    match key {
        "as" | "format" | "constraints" | "anchor" | "internal" => true,
        "defaults" => static_defaults(value, source),
        _ => false,
    }
}

fn collect_words(node: TsNode<'_>, source: &str, words: &mut BTreeSet<String>) {
    match node.kind() {
        "simple_symbol" | "hash_key_symbol" | "string_content" | "bare_symbol" => {
            for word in text(node, source)
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .filter(|w| !w.is_empty())
            {
                words.insert(word.to_string());
            }
        }
        _ => {
            for child in node.named_children(&mut node.walk()) {
                collect_words(child, source, words);
            }
        }
    }
}

fn has_inflections(node: TsNode<'_>, source: &str) -> bool {
    if node.kind() == "call"
        && node
            .child_by_field_name("method")
            .is_some_and(|n| text(n, source) == "inflections")
    {
        return true;
    }
    node.named_children(&mut node.walk())
        .any(|n| has_inflections(n, source))
}

/// Whether `config.api_only` is assigned anything but a literal `false`/`nil`.
fn assigns_api_only(node: TsNode<'_>, source: &str) -> bool {
    if node.kind() == "assignment"
        && node.child_by_field_name("left").is_some_and(|left| {
            left.kind() == "call"
                && left
                    .child_by_field_name("method")
                    .is_some_and(|m| text(m, source) == "api_only")
        })
    {
        return !node
            .child_by_field_name("right")
            .is_some_and(|right| matches!(right.kind(), "false" | "nil"));
    }
    node.named_children(&mut node.walk())
        .any(|n| assigns_api_only(n, source))
}

impl Ctx<'_> {
    fn evidence(&mut self, name: String, line: usize) {
        self.result.unresolved_refs.push(UnresolvedRef {
            from_node_id: self.file.id.clone(),
            reference_name: name,
            reference_kind: EdgeKind::Uses,
            line: line as u32,
            column: 0,
            file_path: self.file.file_path.clone(),
        });
    }

    fn inflections(&mut self, root: TsNode<'_>) {
        if root.has_error() || has_inflections(root, self.source) {
            self.evidence(RUBY_CUSTOM_INFLECTIONS_REFERENCE.into(), 1);
            self.result
                .errors
                .push("Rails routes: custom inflections prevent static controller naming".into());
        }
    }

    fn application(&mut self, root: TsNode<'_>) {
        if root.has_error() || assigns_api_only(root, self.source) {
            self.evidence(RAILS_API_ONLY_REFERENCE.into(), 1);
        }
    }

    fn unsupported(&mut self, node: TsNode<'_>, reason: &str) {
        let line = node.start_position().row;
        let mut words = BTreeSet::new();
        collect_words(node, self.source, &mut words);
        let words: Vec<String> = words.into_iter().collect();
        self.evidence(
            format!("{RAILS_ROUTES_UNSUPPORTED_PREFIX}{}", words.join(" ")),
            line,
        );
        self.result
            .errors
            .push(format!("Rails routes:{}: {reason}", line + 1));
    }

    fn draw_block(&mut self, node: TsNode<'_>) {
        let Some(chain) = chain(node, self.source) else {
            return;
        };
        let Some(receiver) = chain.strip_suffix(".routes.draw") else {
            return;
        };
        let engine = receiver == "Engine" || receiver.ends_with("::Engine");
        if !(engine || receiver == "Rails.application" || receiver.ends_with("::Application")) {
            return;
        }
        let Some(block) = node.child_by_field_name("block") else {
            return;
        };
        if engine {
            self.evidence(
                format!(
                    "{RAILS_ROUTES_ENGINE_PREFIX}{receiver}|{}",
                    node.end_position().row
                ),
                node.start_position().row,
            );
        }
        if let Some(body) = block.child_by_field_name("body") {
            self.statements(body, &Scope::default());
        }
    }

    fn block_statements(&mut self, node: TsNode<'_>, scope: &Scope) -> bool {
        let Some(block) = node.child_by_field_name("block") else {
            return false;
        };
        if let Some(body) = block.child_by_field_name("body") {
            self.statements(body, scope);
        }
        true
    }

    fn statements(&mut self, body: TsNode<'_>, scope: &Scope) {
        for node in body.named_children(&mut body.walk()) {
            if node.kind() == "comment" {
                continue;
            }
            let node_count = self.result.nodes.len();
            let ref_count = self.result.unresolved_refs.len();
            let error_count = self.result.errors.len();
            if !self.call(node, scope) {
                self.result.nodes.truncate(node_count);
                self.result.unresolved_refs.truncate(ref_count);
                self.result.errors.truncate(error_count);
                self.unsupported(node, "unsupported or dynamic declaration");
            }
        }
    }

    fn call(&mut self, node: TsNode<'_>, scope: &Scope) -> bool {
        if node.kind() != "call" || node.child_by_field_name("receiver").is_some() {
            return false;
        }
        let Some(method) = node.child_by_field_name("method") else {
            return false;
        };
        let name = text(method, self.source);
        let Some(args) = arguments(node, self.source) else {
            return false;
        };
        let has_block = node.child_by_field_name("block").is_some();
        match name {
            "namespace" | "scope" => self.scope_call(node, name, &args, scope),
            "constraints"
                if has_block && args.positional.len() <= 1 && args.hash_path.is_none() =>
            {
                self.block_statements(node, scope)
            }
            "defaults"
                if has_block
                    && args.positional.is_empty()
                    && args.hash_path.is_none()
                    && !args.options.keys().any(|key| dispatch_key(key)) =>
            {
                self.block_statements(node, scope)
            }
            "controller" if has_block && args.positional.len() == 1 && args.options.is_empty() => {
                let Some(controller) = literal_text(args.positional[0], self.source) else {
                    return false;
                };
                let inner = Scope {
                    controller: Some(controller),
                    ..scope.clone()
                };
                self.block_statements(node, &inner)
            }
            "member" | "collection" | "new" if has_block && args.positional.is_empty() => {
                let level = match name {
                    "member" => Level::Member,
                    "collection" => Level::Collection,
                    _ => Level::New,
                };
                match scope.enter(level) {
                    Some(inner) if args.options.is_empty() => self.block_statements(node, &inner),
                    _ => false,
                }
            }
            "resources" | "resource" => self.resources(node, name == "resource", &args, scope),
            "draw" if !has_block => self.draw(node, &args, scope),
            // Mounted applications and URL helpers dispatch to no controller.
            "mount" | "direct" | "resolve" => true,
            "root" | "match" if !has_block => self.verb(node, name, &args, scope),
            _ if VERBS.contains(&name) && !has_block => self.verb(node, name, &args, scope),
            _ => false,
        }
    }

    fn scope_call(
        &mut self,
        node: TsNode<'_>,
        name: &str,
        args: &Arguments<'_>,
        scope: &Scope,
    ) -> bool {
        if args.positional.len() > 1 || args.hash_path.is_some() {
            return false;
        }
        for (key, value) in &args.options {
            let allowed = matches!(key.as_str(), "path" | "module")
                || (name == "scope" && key == "controller")
                || neutral_option(key, *value, self.source);
            if !allowed {
                return false;
            }
        }
        let base = match args.positional.first() {
            Some(n) => match literal_text(*n, self.source) {
                Some(base) => Some(base),
                None => return false,
            },
            None if name == "namespace" => return false,
            None => None,
        };
        let static_option = |key: &str| -> Result<Option<String>, ()> {
            match args.options.get(key) {
                // Rails 8.1.3.1 keeps the enclosing path for `scope path: nil`.
                Some(n) if key == "path" && n.kind() == "nil" => Ok(None),
                Some(n) => literal_text(*n, self.source).map(Some).ok_or(()),
                None => Ok(None),
            }
        };
        let (Ok(path), Ok(module), Ok(controller)) = (
            static_option("path"),
            static_option("module"),
            static_option("controller"),
        ) else {
            return false;
        };
        let path = path.or_else(|| base.clone());
        let module = module.or_else(|| (name == "namespace").then(|| base.clone()).flatten());
        // Rails 8.1 rejects slash-prefixed module options; only route targets may be absolute.
        if module
            .as_ref()
            .is_some_and(|module| module.starts_with('/') || module.is_empty())
        {
            return false;
        }
        // A namespace in a resource scope is nested under the resource.
        let outer = if name == "namespace" && scope.in_resource() {
            match scope.enter(Level::Nested) {
                Some(outer) => outer,
                None => return false,
            }
        } else {
            scope.clone()
        };
        let mut inner = outer.clone();
        if let Some(path) = path {
            inner.path = join(&outer.path, &path);
        }
        if let Some(module) = module {
            inner.module = join(&outer.module, &module);
        }
        if controller.is_some() {
            inner.controller = controller;
        }
        self.block_statements(node, &inner)
    }

    fn draw(&mut self, node: TsNode<'_>, args: &Arguments<'_>, scope: &Scope) -> bool {
        if scope.level != Level::Plain
            || scope.controller.is_some()
            || args.positional.len() != 1
            || !args.options.is_empty()
            || args.hash_path.is_some()
        {
            return false;
        }
        let Some(name) = literal_text(args.positional[0], self.source) else {
            return false;
        };
        if !name.split('/').all(is_name) {
            return false;
        }
        self.evidence(
            format!("{RAILS_ROUTES_DRAW_PREFIX}{name}|{}", scope.module),
            node.start_position().row,
        );
        true
    }

    fn resources(
        &mut self,
        node: TsNode<'_>,
        singleton: bool,
        args: &Arguments<'_>,
        scope: &Scope,
    ) -> bool {
        if args.positional.is_empty() || args.hash_path.is_some() {
            return false;
        }
        for (key, value) in &args.options {
            let allowed = matches!(
                key.as_str(),
                "only" | "except" | "controller" | "path" | "param" | "module"
            ) || (key != "as" && neutral_option(key, *value, self.source));
            if !allowed && key != "as" {
                return false;
            }
        }
        let static_option = |key: &str| -> Result<Option<String>, ()> {
            match args.options.get(key) {
                Some(n) => literal_text(*n, self.source).map(Some).ok_or(()),
                None => Ok(None),
            }
        };
        let (Ok(controller), Ok(path), Ok(param), Ok(module), Ok(alias)) = (
            static_option("controller"),
            static_option("path"),
            static_option("param"),
            static_option("module"),
            static_option("as"),
        ) else {
            return false;
        };
        if module
            .as_ref()
            .is_some_and(|module| module.starts_with('/') || module.is_empty())
            || param.as_ref().is_some_and(|param| param.contains(':'))
        {
            return false;
        }
        let action_list = |key: &str| -> Result<Option<Vec<String>>, ()> {
            let Some(node) = args.options.get(key) else {
                return Ok(None);
            };
            if let Some(name) = literal_text(*node, self.source) {
                return Ok(Some(vec![name]));
            }
            if node.kind() != "array" {
                return Err(());
            }
            node.named_children(&mut node.walk())
                .filter(|n| n.kind() != "comment")
                .map(|n| literal_text(n, self.source).ok_or(()))
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        };
        let (Ok(only), Ok(except)) = (action_list("only"), action_list("except")) else {
            return false;
        };
        let valid: &[&str] = if singleton {
            &SINGULAR_ACTIONS
        } else {
            &PLURAL_ACTIONS
        };
        if only
            .iter()
            .chain(except.iter())
            .flatten()
            .any(|action| !valid.contains(&action.as_str()))
        {
            return false;
        }
        let Some(names) = args
            .positional
            .iter()
            .map(|n| literal_text(*n, self.source).filter(|name| is_name(name)))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        // Options outside the resource's own set wrap it in a scope.
        let mut outer = if scope.in_resource() {
            match scope.enter(Level::Nested) {
                Some(outer) => outer,
                None => return false,
            }
        } else {
            scope.clone()
        };
        if let Some(module) = module {
            outer.module = join(&outer.module, &module);
        }
        for name in names {
            let controller = controller.clone().unwrap_or_else(|| {
                if singleton {
                    pluralize(&name)
                } else {
                    name.clone()
                }
            });
            let resource = Resource {
                path: path.clone().unwrap_or_else(|| name.clone()),
                param: param.clone().unwrap_or_else(|| "id".into()),
                singular: singularize(alias.as_deref().unwrap_or(&name)),
                singleton,
            };
            let inner = Scope {
                controller: Some(controller.clone()),
                level: if singleton {
                    Level::Resource
                } else {
                    Level::Resources
                },
                resource: Some(resource.clone()),
                ..outer.clone()
            };
            if node.child_by_field_name("block").is_some() {
                self.block_statements(node, &inner);
            }
            for action in valid {
                if only
                    .as_ref()
                    .is_some_and(|only| !only.iter().any(|a| a == action))
                    || except
                        .as_ref()
                        .is_some_and(|except| except.iter().any(|a| a == action))
                {
                    continue;
                }
                let (verbs, relative): (&[&str], String) = match *action {
                    "index" => (&["GET"], resource.path.clone()),
                    "create" => (&["POST"], resource.path.clone()),
                    "new" => (&["GET"], join(&resource.path, "new")),
                    "edit" => (&["GET"], join(&resource.member(), "edit")),
                    "show" => (&["GET"], resource.member()),
                    "update" => (&["PATCH", "PUT"], resource.member()),
                    _ => (&["DELETE"], resource.member()),
                };
                // API-only applications drop new/edit unless `only:` names them.
                let unless_api_only = only.is_none() && matches!(*action, "new" | "edit");
                for verb in verbs {
                    if !self.emit(
                        node,
                        &outer.module,
                        verb,
                        &join(&outer.path, &relative),
                        &controller,
                        action,
                        unless_api_only,
                    ) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn verb(&mut self, node: TsNode<'_>, name: &str, args: &Arguments<'_>, scope: &Scope) -> bool {
        for (key, value) in &args.options {
            let allowed = matches!(key.as_str(), "to" | "controller" | "action" | "on" | "path")
                || (name == "match" && key == "via")
                || neutral_option(key, *value, self.source);
            if !allowed {
                return false;
            }
        }
        let static_option = |key: &str| -> Result<Option<String>, ()> {
            match args.options.get(key) {
                Some(n) => literal_text(*n, self.source).map(Some).ok_or(()),
                None => Ok(None),
            }
        };
        // `to: redirect(...)` serves a redirect, not a controller action.
        if args.options.get("to").is_some_and(|to| {
            to.kind() == "call"
                && to
                    .child_by_field_name("method")
                    .is_some_and(|m| text(m, self.source) == "redirect")
        }) {
            return true;
        }
        let (Ok(mut to), Ok(mut controller), Ok(mut action), Ok(on), Ok(path_option)) = (
            static_option("to"),
            static_option("controller"),
            static_option("action"),
            static_option("on"),
            static_option("path"),
        ) else {
            return false;
        };
        let verbs: Vec<String> = match name {
            "match" => {
                let Some(via) = args.options.get("via") else {
                    return false;
                };
                let nodes: Vec<TsNode<'_>> = if via.kind() == "array" {
                    via.named_children(&mut via.walk())
                        .filter(|n| n.kind() != "comment")
                        .collect()
                } else {
                    vec![*via]
                };
                let Some(verbs) = nodes
                    .iter()
                    .map(|n| {
                        literal_text(*n, self.source).and_then(|v| match v.as_str() {
                            "all" => Some("ANY".to_string()),
                            v if VERBS.contains(&v) => Some(v.to_ascii_uppercase()),
                            _ => None,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                if verbs.is_empty() {
                    return false;
                }
                verbs
            }
            "root" => vec!["GET".into()],
            _ => vec![name.to_ascii_uppercase()],
        };
        // The route path, or a symbol naming the action.
        let target = if name == "root" {
            if args.hash_path.is_some() || args.positional.len() > 1 || scope.in_resource() {
                return false;
            }
            if let Some(n) = args.positional.first() {
                if to.is_some() {
                    return false;
                }
                let Some(Literal::Str(t)) = literal(*n, self.source) else {
                    return false;
                };
                to = Some(t);
            }
            Literal::Str("/".into())
        } else if let Some(n) = args.positional.first() {
            if args.positional.len() > 1 || args.hash_path.is_some() {
                return false;
            }
            let Some(target) = literal(*n, self.source) else {
                return false;
            };
            target
        } else {
            let Some((key, value)) = args.hash_path else {
                return false;
            };
            let Some(Literal::Str(path)) = literal(key, self.source) else {
                return false;
            };
            match literal(value, self.source) {
                Some(Literal::Symbol(a)) => action = action.or(Some(a)),
                Some(Literal::Str(t)) if t.contains('#') => to = to.or(Some(t)),
                Some(Literal::Str(c)) => controller = controller.or(Some(c)),
                None => return false,
            }
            Literal::Str(path)
        };
        if matches!(target, Literal::Str(_)) && path_option.is_some() {
            return false;
        }
        if let Literal::Str(path) = &target {
            if to.is_none() && action.is_none() {
                to = shorthand(path);
            }
        }
        let level = match on.as_deref() {
            Some("member") => Level::Member,
            Some("collection") => Level::Collection,
            Some("new") => Level::New,
            Some(_) => return false,
            None => match scope.level {
                Level::Resources => Level::Nested,
                Level::Resource => Level::Member,
                level => level,
            },
        };
        let current = if level == scope.level {
            scope.clone()
        } else {
            match scope.enter(level) {
                Some(current) => current,
                None => return false,
            }
        };
        let route_path = match (&target, &path_option) {
            (Literal::Str(path), _) | (Literal::Symbol(_), Some(path)) => join(&current.path, path),
            (Literal::Symbol(a), None)
                if matches!(level, Level::Member | Level::Collection | Level::New)
                    && CANONICAL_ACTIONS.contains(&a.as_str()) =>
            {
                current.path.clone()
            }
            (Literal::Symbol(a), None) => join(&current.path, a),
        };
        let raw = target.text();
        if action.is_none()
            && !raw.is_empty()
            && !raw.contains('/')
            && raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            action = Some(raw.replace('-', "_"));
        }
        let controller = controller.or_else(|| current.controller.clone());
        let (controller, action) = match to {
            Some(to) => match to.split_once('#') {
                Some((c, a)) if !a.contains('#') => (Some(c.to_string()), Some(a.to_string())),
                Some(_) => return false,
                None => (controller, Some(to)),
            },
            None => (controller, action),
        };
        let (Some(controller), Some(action)) = (controller, action) else {
            return false;
        };
        verbs.iter().all(|verb| {
            self.emit(
                node,
                &current.module,
                verb,
                &route_path,
                &controller,
                &action,
                false,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        node: TsNode<'_>,
        module: &str,
        verb: &str,
        path: &str,
        controller: &str,
        action: &str,
        unless_api_only: bool,
    ) -> bool {
        if action.is_empty()
            || !action
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return false;
        }
        let (absolute, controller) = match controller.strip_prefix('/') {
            Some(controller) => (true, controller.to_string()),
            None => (false, join(module, controller)),
        };
        // Rails 8.1.3.1 validates module prefixes too, except when an absolute target bypasses them.
        if !controller.split('/').all(is_name) {
            return false;
        }
        let path = format!("/{}", path.trim_matches('/'));
        // Rails 8.1.3.1 lets named and glob controller/action captures override the literal target.
        // Inspect the complete scoped path, including optional groups and punctuation boundaries.
        if path.split([':', '*']).skip(1).any(|segment| {
            matches!(
                segment
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next(),
                Some("controller" | "action")
            )
        }) {
            return false;
        }
        let label = format!("{verb} {path} -> {controller}#{action}");
        let start = node.start_position();
        let route = Node {
            id: generate_node_id(
                &self.file.file_path,
                &NodeKind::Route,
                &format!("{}:{}:{label}", node.start_byte(), self.result.nodes.len()),
                start.row as u32,
            ),
            kind: NodeKind::Route,
            name: format!("{verb} {path}"),
            qualified_name: format!("{}::{label}", self.file.file_path),
            file_path: self.file.file_path.clone(),
            signature: Some(label),
            docstring: None,
            start_line: start.row as u32,
            attrs_start_line: start.row as u32,
            end_line: node.end_position().row as u32,
            start_column: start.column as u32,
            end_column: node.end_position().column as u32,
            parent_id: Some(self.file.id.clone()),
            visibility: crate::types::Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: self.file.updated_at,
        };
        let constant = format!("{}Controller", camelize(&controller));
        self.result.unresolved_refs.push(UnresolvedRef {
            from_node_id: route.id.clone(),
            reference_name: RouteTarget {
                unless_api_only,
                absolute,
                constant: &constant,
                action,
            }
            .format(),
            reference_kind: EdgeKind::Calls,
            line: route.start_line,
            column: route.start_column,
            file_path: self.file.file_path.clone(),
        });
        self.result.nodes.push(route);
        true
    }
}

/// `get "photos/search"` with no target means `photos#search`.
fn shorthand(path: &str) -> Option<String> {
    let path = path.strip_suffix("(.:format)").unwrap_or(path);
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    let word = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    };
    let (controller, action) = trimmed.rsplit_once('/')?;
    let first = controller.split('/').next()?;
    if !word(first) || !controller.split('/').all(|s| s.is_empty() || word(s)) {
        return None;
    }
    if !action
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    Some(format!("{controller}#{action}").replace('-', "_"))
}
