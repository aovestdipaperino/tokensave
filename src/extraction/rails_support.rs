//! Rails conventions shared by the route extractor, the route rebuild and
//! the rename guard.
//!
//! Route extraction stores what it learns as `unresolved_refs` rows, so a
//! file's evidence is replaced whenever the file is re-extracted and
//! disappears with it. Route targets are `calls` references from a `route`
//! node, written as `[~][::]Const#action`: `~` marks a `new`/`edit` route
//! that exists only when the application is not API-only, and `::` marks
//! an absolute target (`to: "/notes#show"`) that ignores the namespace the
//! file is drawn into. Every other evidence row is a `uses` reference whose
//! name starts with one of the prefixes below; none of them is a graph link.

pub(crate) use super::rails_inflector::{camelize, pluralize, underscore};

/// Visibility directives that may hide an action in a reopened controller;
/// `*` denotes dynamic visibility names.
pub(crate) const RUBY_VISIBILITY_REFERENCE_PREFIX: &str = "ruby-visibility:";
pub(crate) const RUBY_CUSTOM_INFLECTIONS_REFERENCE: &str = "ruby-inflections:custom";
/// `isolate_namespace Foo` in an engine class, from the class node.
pub(crate) const RAILS_ISOLATE_NAMESPACE_PREFIX: &str = "rails-isolate-namespace:";
/// `Foo::Engine.routes.draw` as `Foo::Engine|last_row`, from the route file;
/// the evidence line is the block's first row.
pub(crate) const RAILS_ROUTES_ENGINE_PREFIX: &str = "rails-routes-engine:";
/// `draw :name` as `name|module`, from the file that draws it.
pub(crate) const RAILS_ROUTES_DRAW_PREFIX: &str = "rails-routes-draw:";
/// A declaration the extractor skipped, with the words it mentions.
pub(crate) const RAILS_ROUTES_UNSUPPORTED_PREFIX: &str = "rails-routes-unsupported:";
/// `config.api_only = true` (or a value that is not literally false).
pub(crate) const RAILS_API_ONLY_REFERENCE: &str = "rails-api-only";

/// Evidence rows are never resolved into graph edges.
pub(crate) fn is_rails_evidence(name: &str) -> bool {
    name == RUBY_CUSTOM_INFLECTIONS_REFERENCE
        || name == RAILS_API_ONLY_REFERENCE
        || [
            RUBY_VISIBILITY_REFERENCE_PREFIX,
            RAILS_ISOLATE_NAMESPACE_PREFIX,
            RAILS_ROUTES_ENGINE_PREFIX,
            RAILS_ROUTES_DRAW_PREFIX,
            RAILS_ROUTES_UNSUPPORTED_PREFIX,
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// A Rails file the route extractor reads. `root` is the application or
/// engine directory, `""` or ending in `/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteFile<'a> {
    /// `config/routes.rb`, holding a `routes.draw` block.
    Main {
        root: &'a str,
    },
    /// `config/routes/<name>.rb`, loaded by `draw :<name>`.
    Drawn {
        root: &'a str,
        name: &'a str,
    },
    Inflections,
    Application {
        root: &'a str,
    },
}

/// Splits `path` around `marker`, which must start the path or follow a `/`.
fn split<'a>(path: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    if let Some(rest) = path.strip_prefix(marker) {
        return Some(("", rest));
    }
    let index = path.find(&format!("/{marker}"))?;
    Some((&path[..=index], &path[index + 1 + marker.len()..]))
}

pub(crate) fn route_file(path: &str) -> Option<RouteFile<'_>> {
    if let Some((root, "")) = split(path, "config/routes.rb") {
        return Some(RouteFile::Main { root });
    }
    if let Some((root, rest)) = split(path, "config/routes/") {
        return rest
            .strip_suffix(".rb")
            .filter(|name| !name.is_empty())
            .map(|name| RouteFile::Drawn { root, name });
    }
    if let Some((_, "")) = split(path, "config/initializers/inflections.rb") {
        return Some(RouteFile::Inflections);
    }
    if let Some((root, "")) = split(path, "config/application.rb") {
        return Some(RouteFile::Application { root });
    }
    None
}

/// The root of the application or engine a route file belongs to.
pub(crate) fn route_root(path: &str) -> Option<&str> {
    match route_file(path)? {
        RouteFile::Main { root } | RouteFile::Drawn { root, .. } => Some(root),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RouteTarget<'a> {
    pub unless_api_only: bool,
    pub absolute: bool,
    pub constant: &'a str,
    pub action: &'a str,
}

pub(crate) fn parse_route_target(name: &str) -> Option<RouteTarget<'_>> {
    let (unless_api_only, rest) = match name.strip_prefix('~') {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    let (absolute, rest) = match rest.strip_prefix("::") {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let (constant, action) = rest.split_once('#')?;
    Some(RouteTarget {
        unless_api_only,
        absolute,
        constant,
        action,
    })
}

impl RouteTarget<'_> {
    pub(crate) fn format(&self) -> String {
        format!(
            "{}{}{}#{}",
            if self.unless_api_only { "~" } else { "" },
            if self.absolute { "::" } else { "" },
            self.constant,
            self.action
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn classifies_route_files_under_any_application_root() {
        assert_eq!(
            route_file("config/routes.rb"),
            Some(RouteFile::Main { root: "" })
        );
        assert_eq!(
            route_file("engines/blog/config/routes.rb"),
            Some(RouteFile::Main {
                root: "engines/blog/"
            })
        );
        assert_eq!(
            route_file("config/routes/admin/users.rb"),
            Some(RouteFile::Drawn {
                root: "",
                name: "admin/users"
            })
        );
        assert_eq!(
            route_file("web/config/initializers/inflections.rb"),
            Some(RouteFile::Inflections)
        );
        assert_eq!(
            route_file("web/config/application.rb"),
            Some(RouteFile::Application { root: "web/" })
        );
        for path in [
            "config/routes.rb.bak",
            "myconfig/routes.rb",
            "config/routes/.rb",
            "config/routes/notes.erb",
            "app/config.rb",
        ] {
            assert_eq!(route_file(path), None, "{path}");
        }
    }

    #[test]
    fn route_targets_round_trip() {
        for name in ["NotesController#show", "~::Admin::NotesController#edit"] {
            assert_eq!(parse_route_target(name).unwrap().format(), name);
        }
        assert!(parse_route_target("NotesController").is_none());
    }
}
