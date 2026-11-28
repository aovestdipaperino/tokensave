use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use tree_sitter::Node;

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

pub(crate) mod constants;
mod literals;
mod resources;
pub use constants::DEFAULT_LIMIT;

#[derive(Clone, Copy, Debug, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Query {
    Assignment,
    Call,
    Attribute,
    Comparison,
    Strings,
    Resources,
    MissingTranslations,
    UnusedResources,
}

fn config_error(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

/// Query currently indexed paths, parsing each selected file within a fixed budget.
pub async fn query(
    project: &TokenSave,
    kind: Query,
    path: Option<&str>,
    name: Option<&str>,
    offset: usize,
    limit: usize,
) -> Result<Value> {
    use constants::{
        MAX_BYTES, MAX_FILES, MAX_LIMIT, MAX_TEXT, MAX_TOTAL_BYTES, PARSE_TIMEOUT, SOURCE_EXT,
    };

    if project.serving_branch().is_some_and(|branch| {
        crate::branch::current_branch(project.project_root()).as_deref() != Some(branch)
    }) {
        return Err(config_error(
            "C# and .NET discovery requires the indexed checkout",
        ));
    }
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(config_error("invalid .NET query limit"));
    }
    if name.is_some_and(|name| name.len() > MAX_TEXT) {
        return Err(config_error(".NET query name exceeds the size limit"));
    }
    let is_resources = matches!(
        kind,
        Query::Resources | Query::MissingTranslations | Query::UnusedResources
    );
    let path = path
        .map(|path| path.trim_end_matches('/'))
        .filter(|path| !path.is_empty());
    let files = project.db().dotnet_paths(path, is_resources).await?;
    if files.is_empty() && path.is_some() {
        return Err(config_error(
            "path is unsupported, excluded, or absent from the index",
        ));
    }
    let root = project.project_root().canonicalize()?;
    let language = crate::extraction::ts_provider::language("c_sharp");
    let mut parser = tree_sitter::Parser::new();
    let supports_source = parser.set_language(&language).is_ok();
    if !supports_source && !is_resources {
        return Err(config_error("C# parsing is unavailable in this build"));
    }

    let mut resources = resources::Resources::default();
    let mut items = Vec::new();
    let mut matched = 0;
    let mut skipped = 0;
    let mut scanned = 0;
    let mut bytes = 0;
    let mut truncated = false;
    for file in files {
        let is_xml = !Path::new(&file)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case(SOURCE_EXT));
        if scanned == MAX_FILES || bytes >= MAX_TOTAL_BYTES {
            truncated = true;
            break;
        }
        scanned += 1;
        if file.len() > constants::MAX_PATH_BYTES {
            skipped += 1;
            continue;
        }
        let allowance = MAX_BYTES.min(MAX_TOTAL_BYTES - bytes);
        let Ok((source, read_bytes)) = read(&root, &file, allowance) else {
            skipped += 1;
            continue;
        };
        bytes += read_bytes;
        let Some(source) = source else {
            skipped += 1;
            continue;
        };
        if is_xml {
            if !resources.xml(&file, &source) {
                skipped += 1;
            }
            continue;
        }
        let start = std::time::Instant::now();
        let mut progress = |_: &tree_sitter::ParseState| {
            if start.elapsed() >= PARSE_TIMEOUT {
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        };
        let options = tree_sitter::ParseOptions::new().progress_callback(&mut progress);
        let mut input =
            |byte: usize, _: tree_sitter::Point| source.as_bytes().get(byte..).unwrap_or_default();
        let tree = supports_source
            .then(|| parser.parse_with_options(&mut input, None, Some(options)))
            .flatten();
        let Some(tree) = tree else {
            parser.reset();
            skipped += 1;
            continue;
        };
        if tree.root_node().has_error() {
            skipped += 1;
            continue;
        }
        walk(tree.root_node(), |node| {
            if is_resources {
                resources.reference(&file, &source, node);
            } else if matches!(kind, Query::Strings) {
                if let Some(category) = literals::category(source.as_str(), node)
                    .filter(|category| name.is_none_or(|name| *category == name))
                {
                    if matched >= offset && items.len() <= limit {
                        let mut item = location(&file, &source, node, text(&source, node));
                        item["category"] = json!(category);
                        item["language"] = json!("C#");
                        item["value_truncated"] =
                            json!(text(&source, node).chars().count() > constants::MAX_TEXT);
                        items.push(item);
                    }
                    matched += 1;
                }
            } else if let Some(target) =
                target(node, kind, &source).filter(|target| name.is_none_or(|name| *target == name))
            {
                if matched >= offset && items.len() <= limit {
                    items.push(location(&file, &source, node, target));
                }
                matched += 1;
            }
        });
    }
    if is_resources {
        truncated |= resources.truncated;
        items = resources.finish(kind, name, offset, limit);
    }
    let mut page_bytes = 0;
    let page_len = items
        .iter()
        .take(limit)
        .take_while(|item| {
            page_bytes += item.to_string().len() + 1;
            page_bytes <= constants::MAX_PAGE_BYTES
        })
        .count();
    let next_offset = (items.len() > page_len).then(|| offset + page_len);
    items.truncate(page_len);
    Ok(json!({
        "items": items,
        "next_offset": next_offset,
        "scanned_files": scanned,
        "skipped_files": skipped,
        "scan_truncated": truncated,
        "coverage": if is_resources {
            "Indexed RESX, appSettings, connectionStrings and common C# references. Dynamic and out-of-scope references may exist."
        } else {
            "Indexed C# syntax. Malformed, unreadable and oversized files are skipped."
        }
    }))
}

fn read(root: &Path, path: &str, allowance: usize) -> std::io::Result<(Option<String>, usize)> {
    let target = root.join(path).canonicalize()?;
    if !target.starts_with(root) || !target.is_file() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut buffer = Vec::new();
    File::open(target)?
        .take(allowance as u64 + 1)
        .read_to_end(&mut buffer)?;
    let len = buffer.len();
    let source = if len <= allowance {
        String::from_utf8(buffer).ok()
    } else {
        None
    };
    Ok((source, len))
}

fn walk(root: Node<'_>, mut visit: impl FnMut(Node<'_>)) {
    let mut cursor = root.walk();
    loop {
        visit(cursor.node());
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

fn text<'a>(source: &'a str, node: Node<'_>) -> &'a str {
    source.get(node.byte_range()).unwrap_or_default()
}

fn terminal(node: Node<'_>) -> Node<'_> {
    let node = node.child_by_field_name("name").unwrap_or(node);
    if node.kind() == "generic_name" {
        node.named_child(0).unwrap_or(node)
    } else {
        node
    }
}

fn target<'a>(node: Node<'_>, kind: Query, source: &'a str) -> Option<&'a str> {
    let field = match kind {
        Query::Assignment if node.kind() == "assignment_expression" => {
            let rhs = node.child_by_field_name("right")?;
            if !rhs.kind().ends_with("_literal")
                || text(source, node.child_by_field_name("operator")?) != "="
            {
                return None;
            }
            "left"
        }
        Query::Call if node.kind() == "invocation_expression" => "function",
        Query::Attribute if node.kind() == "attribute" => "name",
        Query::Comparison if node.kind() == "binary_expression" => {
            let operator = text(source, node.child_by_field_name("operator")?);
            return matches!(operator, "==" | "!=" | "<" | "<=" | ">" | ">=").then_some(operator);
        }
        _ => return None,
    };
    Some(text(source, terminal(node.child_by_field_name(field)?)))
}

fn compact(value: &str) -> String {
    value.chars().take(constants::MAX_TEXT).collect()
}

fn location(file: &str, source: &str, node: Node<'_>, target: &str) -> Value {
    let mut parent = node.parent();
    let mut symbol = None;
    while let Some(enclosing) = parent {
        if enclosing.kind().ends_with("_declaration") {
            symbol = enclosing
                .child_by_field_name("name")
                .map(|node| compact(text(source, node)));
            if symbol.is_some() {
                break;
            }
        }
        parent = enclosing.parent();
    }
    let start = node.start_position();
    let end = node.end_position();
    json!({
        "path": file,
        "start": [start.row + 1, start.column + 1],
        "end": [end.row + 1, end.column + 1],
        "kind": node.kind(),
        "target": compact(target),
        "enclosing_symbol": symbol,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(source: &str, query: Query) -> Vec<Value> {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser
            .set_language(&crate::extraction::ts_provider::language("c_sharp"))
            .is_ok());
        let Some(tree) = parser.parse(source, None) else {
            panic!("C# fixture should parse");
        };
        assert!(!tree.root_node().has_error());
        let mut matches = Vec::new();
        walk(tree.root_node(), |node| {
            if let Some(name) = target(node, query, source) {
                matches.push(location("View.cs", source, node, name));
            }
        });
        matches
    }

    #[tokio::test]
    async fn indexed_query_respects_exclusions_and_paginates() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let mut config = crate::config::TokenSaveConfig::default();
        config.exclude.push("Ignored.cs".to_owned());
        crate::config::save_config(root, &config)?;
        let project = TokenSave::init(root).await?;
        std::fs::write(
            root.join("View.cs"),
            "class View { void Init() { Text = \"first\"; Text = \"second\"; } }",
        )?;
        std::fs::write(
            root.join("Ignored.cs"),
            "class Ignored { void Init() { Text = \"excluded\"; } }",
        )?;
        project.index_all().await?;
        let first = query(&project, Query::Assignment, None, Some("Text"), 0, 1).await?;
        assert_eq!(first["items"][0]["path"], "View.cs");
        assert_eq!(first["next_offset"], 1);
        let last = query(&project, Query::Assignment, None, Some("Text"), 1, 1).await?;
        assert_eq!(last["items"][0]["path"], "View.cs");
        assert!(last["next_offset"].is_null());
        assert_ne!(first["items"][0]["start"], last["items"][0]["start"]);
        Ok(())
    }

    #[test]
    fn compare_assignment_search_with_regex() -> std::result::Result<(), regex::Error> {
        let source = r#"
            class View {
                void Init() {
                    // label.Text = "fake";
                    label.Text = "a\"b";
                    label
                        .Text =
                        "second";
                }
            }
        "#;
        let pattern = regex::Regex::new(r#"\bText[ \t]*=[ \t]*""#)?;
        let iterations = 100;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(pattern.find_iter(source).count());
        }
        let regex_elapsed = start.elapsed();
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(matches(source, Query::Assignment));
        }
        let syntax_elapsed = start.elapsed();
        let regex_context = source
            .lines()
            .filter(|line| pattern.is_match(line))
            .collect::<Vec<_>>();
        let syntax_matches = matches(source, Query::Assignment);
        assert_eq!(regex_context.len(), 2);
        assert!(regex_context[0].contains("//"));
        assert_eq!(syntax_matches.len(), 2);
        assert_eq!(syntax_matches[0]["start"][0], 5);
        assert_eq!(syntax_matches[1]["start"][0], 6);
        let regex_bytes = json!(regex_context
            .iter()
            .map(|line| line.trim())
            .collect::<Vec<_>>())
        .to_string()
        .len();
        let syntax_bytes = json!(syntax_matches).to_string().len();
        eprintln!("Context bytes: regex {regex_bytes}, AST {syntax_bytes}");
        eprintln!("{iterations} passes, regex {regex_elapsed:?}, AST {syntax_elapsed:?}");
        Ok(())
    }

    #[test]
    fn finds_literal_assignments_and_calls_while_ignoring_comments() {
        let source = r#"
            class View {
                void Init() {
                    // Label.Text = "fake"; Fake();
                    label.Text = "a\"b";
                    label.HeaderText = GetTitle();
                    label
                        .Text =
                        "second";
                }
            }
        "#;
        let assignments = matches(source, Query::Assignment);
        assert_eq!(assignments.len(), 2);
        assert!(assignments.iter().all(|entry| entry["target"] == "Text"));
        let calls = matches(source, Query::Call);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["target"], "GetTitle");
    }
}
