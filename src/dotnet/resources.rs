use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{json, Value};
use tree_sitter::Node;

use super::constants::{CONFIG_EXT, MAX_ENTRIES, MAX_TEXT};
use super::{compact, terminal, text, Query};

#[derive(Default)]
pub(super) struct Resources {
    keys: BTreeMap<(String, String), Vec<Entry>>,
    variants: BTreeMap<String, BTreeSet<String>>,
    class_names: BTreeSet<String>,
    references: BTreeMap<(String, String), Vec<Entry>>,
    dynamic: usize,
    count: usize,
    pub(super) truncated: bool,
}

struct Entry {
    path: String,
    line: usize,
    locale: String,
}

impl Entry {
    fn json(&self) -> Value {
        json!({"path": self.path, "line": self.line, "locale": self.locale})
    }
}

impl Resources {
    fn reserve(&mut self) -> bool {
        if self.count == MAX_ENTRIES {
            self.truncated = true;
            return false;
        }
        self.count += 1;
        true
    }

    pub(super) fn xml(&mut self, path: &str, source: &str) -> bool {
        let options = roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_ENTRIES as u32,
            ..roxmltree::ParsingOptions::default()
        };
        let Ok(doc) = roxmltree::Document::parse_with_options(source, options) else {
            return false;
        };
        let root = doc.root_element();
        let line_ends = source
            .match_indices('\n')
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();
        let line_of = |node: roxmltree::Node<'_, '_>| {
            line_ends.partition_point(|&offset| offset < node.range().start) + 1
        };
        let is_config = Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case(CONFIG_EXT));
        if root.tag_name().name() != if is_config { "configuration" } else { "root" } {
            return false;
        }
        if is_config {
            for section in root.children().filter(roxmltree::Node::is_element) {
                let group = section.tag_name().name();
                let attribute = match group {
                    "appSettings" => "key",
                    "connectionStrings" => "name",
                    _ => continue,
                };
                for node in section.children().filter(|node| node.has_tag_name("add")) {
                    if let Some(key) = node.attribute(attribute) {
                        self.add_key(group, key, path, line_of(node), "");
                    }
                }
            }
        } else {
            let (group, locale) = family(path);
            if let Some(class_name) = Path::new(&group).file_name().and_then(|name| name.to_str()) {
                self.class_names.insert(class_name.to_owned());
            }
            self.variants
                .entry(group.clone())
                .or_default()
                .insert(locale.to_owned());
            for node in root.children().filter(|node| node.has_tag_name("data")) {
                if let Some(key) = node.attribute("name") {
                    self.add_key(&group, key, path, line_of(node), locale);
                }
            }
        }
        true
    }

    fn add_key(&mut self, group: &str, key: &str, path: &str, line: usize, locale: &str) {
        if key.len() > MAX_TEXT {
            self.truncated = true;
        } else if self.reserve() {
            self.keys
                .entry((group.to_owned(), key.to_owned()))
                .or_default()
                .push(Entry {
                    path: path.to_owned(),
                    line,
                    locale: locale.to_owned(),
                });
        }
    }

    pub(super) fn reference(&mut self, path: &str, source: &str, node: Node<'_>) {
        let (group, key) = match node.kind() {
            "invocation_expression" => {
                let Some(function) = node.child_by_field_name("function") else {
                    return;
                };
                if !matches!(text(source, terminal(function)), "GetString" | "GetObject") {
                    return;
                }
                let receiver = function
                    .child_by_field_name("expression")
                    .map(|n| text(source, n));
                if !matches!(
                    receiver,
                    Some("ResourceManager" | "resourceMan" | "resourceManager")
                ) {
                    return;
                }
                if path
                    .to_ascii_lowercase()
                    .ends_with(super::constants::DESIGNER_SUFFIX)
                {
                    return;
                }
                ("*", argument_literal(node, source))
            }
            "element_access_expression" => {
                let Some(receiver) = node.child_by_field_name("expression") else {
                    return;
                };
                let group = text(source, terminal(receiver));
                if !matches!(group, "AppSettings" | "ConnectionStrings") {
                    return;
                }
                let group = if group == "AppSettings" {
                    "appSettings"
                } else {
                    "connectionStrings"
                };
                (group, argument_literal(node, source))
            }
            "member_access_expression" => {
                let Some(receiver) = node.child_by_field_name("expression") else {
                    return;
                };
                let group = text(source, terminal(receiver));
                if !self.class_names.contains(group) {
                    return;
                }
                (group, Some(text(source, terminal(node))))
            }
            _ => return,
        };
        let Some(key) = key else {
            self.dynamic += 1;
            return;
        };
        if key.len() <= MAX_TEXT && self.reserve() {
            let line = node.start_position().row + 1;
            self.references
                .entry((group.to_owned(), key.to_owned()))
                .or_default()
                .push(Entry {
                    path: path.to_owned(),
                    line,
                    locale: String::new(),
                });
        }
    }

    pub(super) fn finish(
        &self,
        kind: Query,
        name: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Vec<Value> {
        self.keys.iter().filter(|((_, key), _)| name.is_none_or(|name| name == key))
            .filter_map(|((group, key), entries)| {
                let family_name = group.rsplit('/').next().unwrap_or(group);
                let refs = self.references.get(&(family_name.to_owned(), key.clone()));
                let variants = self.variants.get(group);
                let indirect = variants.and_then(|_| self.references.get(&("*".to_owned(), key.clone())));
                let config = variants.is_none();
                let locales: BTreeSet<_> = entries.iter().map(|entry| entry.locale.as_str()).collect();
                let missing = variants.into_iter().flatten()
                    .filter(|locale| !locales.contains(locale.as_str())).collect::<Vec<_>>();
                let unused = !config && refs.is_none() && indirect.is_none();
                if (matches!(kind, Query::MissingTranslations) && missing.is_empty())
                    || (matches!(kind, Query::UnusedResources) && !unused)
                {
                    return None;
                }
                let mut paths = BTreeSet::new();
                let duplicate = entries.iter().any(|entry| !paths.insert(&entry.path));
                let references = refs.into_iter().flatten().chain(indirect.into_iter().flatten());
                Some(json!({
                    "group": compact(group),
                    "key": key,
                    "kind": if config { "configuration" } else { "resource" },
                    "definitions": entries.iter().take(super::constants::MAX_LOCATIONS).map(Entry::json).collect::<Vec<_>>(),
                    "definition_count": entries.len(),
                    "duplicate": duplicate,
                    "missing_locales": missing.iter().take(super::constants::MAX_LOCATIONS).collect::<Vec<_>>(),
                    "missing_locale_count": missing.len(),
                    "candidate_unused": unused,
                    "dynamic_reference_count": self.dynamic,
                    "references": references.take(super::constants::MAX_LOCATIONS).map(Entry::json).collect::<Vec<_>>(),
                }))
            }).skip(offset).take(limit + 1).collect()
    }
}

fn family(path: &str) -> (String, &str) {
    let stem = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
    if let Some((base, locale)) = stem.rsplit_once('.') {
        let mut parts = locale.split('-');
        if parts.next().is_some_and(|lang| {
            (2..=3).contains(&lang.len()) && lang.bytes().all(|c| c.is_ascii_lowercase())
        }) && locale.len() <= 24
            && parts.all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_alphanumeric()))
        {
            return (base.to_owned(), locale);
        }
    }
    (stem.to_owned(), "")
}

fn argument_literal<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    let args = node
        .child_by_field_name("arguments")
        .or_else(|| node.child_by_field_name("subscript"))?;
    let argument = args.named_child(0)?;
    let literal = argument.named_child(0)?;
    if literal.kind() != "string_literal" {
        return None;
    }
    let value = text(source, literal).strip_prefix('"')?.strip_suffix('"')?;
    (!value.contains('\\')).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dotnet::walk;

    #[test]
    fn resources_track_locales_duplicates_references_and_redact_values() {
        let mut resources = Resources::default();
        assert!(resources.xml(
            "Properties/Resources.resx",
            r#"<root><data name="Title"/><data name="Title"/><data name="Other"/></root>"#
        ));
        assert!(resources.xml(
            "Properties/Resources.fr-FR.resx",
            r#"<root><data name="Other"/></root>"#
        ));
        assert!(resources.xml("App.config", r#"<configuration><appSettings><add key="Api" value="secret-value"/></appSettings></configuration>"#));
        assert!(!resources.xml("bad.resx", "<root>"));
        assert!(!resources.xml(
            "entity.resx",
            r#"<!DOCTYPE root [<!ENTITY a SYSTEM "file:///secret">]><root>&a;</root>"#
        ));
        let source = r#"class C { void M(string key) { Show(Properties.Resources.Title); Show(resourceMan.GetString(key)); Show(ConfigurationManager.AppSettings["Api"]); } }"#;
        let mut parser = tree_sitter::Parser::new();
        assert!(parser
            .set_language(&crate::extraction::ts_provider::language("c_sharp"))
            .is_ok());
        let Some(tree) = parser.parse(source, None) else {
            panic!("C# fixture should parse");
        };
        assert!(!tree.root_node().has_error());
        walk(tree.root_node(), |node| {
            resources.reference("View.cs", source, node)
        });
        let title = resources.finish(Query::Resources, Some("Title"), 0, 1);
        assert_eq!(title[0]["duplicate"], true);
        assert_eq!(title[0]["missing_locales"], json!(["fr-FR"]));
        assert_eq!(title[0]["candidate_unused"], false);
        assert_eq!(title[0]["dynamic_reference_count"], 1);
        let api = resources.finish(Query::Resources, Some("Api"), 0, 1);
        assert_eq!(api[0]["references"][0]["path"], "View.cs");
        assert!(!api[0].to_string().contains("secret-value"));
    }
}
