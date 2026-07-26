use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::{Node, Parser};
use walkdir::WalkDir;

const EXTERNAL_ENV_VARS: &[&str] = &[
    // Operating-system configuration.
    "HOME",
    "USERPROFILE",
    "PATH",
    "APPDATA",
    "LOCALAPPDATA",
    "XDG_CONFIG_HOME",
    // Agent-owned configuration.
    "CLAUDE_CONFIG_DIR",
    "KIRO_HOME",
    "PI_CODING_AGENT_DIR",
    "VIBE_HOME",
    "TOOL_INPUT",
    // Git subprocess isolation.
    "GIT_CONFIG_GLOBAL",
];
const EXTERNAL_PREFIXES: &[&str] = &["CARGO_"];
const LEGACY_TOKENSAVE_ENV_VARS: &[&str] = &["DISABLE_TOKENSAVE"];

fn env_literals(source: &str) -> Vec<String> {
    let language = tokensave_large_treesitters::all_languages()
        .into_iter()
        .find_map(|(name, language)| (name == "rust").then(|| language.into()))
        .expect("bundled Rust grammar");
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("load bundled Rust grammar");
    let tree = parser.parse(source, None).expect("parse Rust source");
    let mut names = Vec::new();
    collect_env_literals(tree.root_node(), source.as_bytes(), &mut names);
    names.sort_by_key(|(position, _)| *position);
    names.into_iter().map(|(_, name)| name).collect()
}

fn collect_env_literals(node: Node<'_>, source: &[u8], names: &mut Vec<(usize, String)>) {
    let literal = match node.kind() {
        "call_expression" => call_env_literal(node, source),
        "macro_invocation" => macro_env_literal(node, source),
        _ => None,
    };
    if let Some(name) = literal {
        names.push((node.start_byte(), name));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_env_literals(child, source, names);
    }
}

fn call_env_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    let function = node.child_by_field_name("function")?;
    let function = function.utf8_text(source).ok()?;
    let is_env_api = ["var", "var_os", "set_var", "remove_var"]
        .iter()
        .any(|name| {
            function == format!("env::{name}") || function.ends_with(&format!("::env::{name}"))
        })
        || function.ends_with(".env")
        || function.ends_with(".env_remove");
    if !is_env_api {
        return None;
    }

    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let first = arguments.named_children(&mut cursor).next()?;
    string_literal(first, source)
}

fn macro_env_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    let name = node
        .child_by_field_name("macro")
        .or_else(|| node.named_child(0))?
        .utf8_text(source)
        .ok()?;
    if name != "env" && name != "option_env" {
        return None;
    }
    first_string_literal(node, source)
}

fn first_string_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    if let Some(value) = string_literal(node, source) {
        return Some(value);
    }
    let mut cursor = node.walk();
    let result = node
        .named_children(&mut cursor)
        .find_map(|child| first_string_literal(child, source));
    result
}

fn string_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() != "string_literal" && node.kind() != "raw_string_literal" {
        return None;
    }
    let literal = node.utf8_text(source).ok()?;
    let start = literal.find('"')? + 1;
    let end = literal.rfind('"')?;
    (end >= start).then(|| literal[start..end].to_string())
}

fn is_allowed_env_var(name: &str) -> bool {
    name.starts_with("TOKENSAVE_")
        || EXTERNAL_ENV_VARS.contains(&name)
        || EXTERNAL_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        || LEGACY_TOKENSAVE_ENV_VARS.contains(&name)
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    for directory in ["src", "benches", "examples"] {
        let directory = root.join(directory);
        if !directory.exists() {
            continue;
        }
        sources.extend(
            WalkDir::new(directory)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .map(|entry| entry.into_path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "rs")),
        );
    }
    let build_script = root.join("build.rs");
    if build_script.exists() {
        sources.push(build_script);
    }
    sources
}

#[test]
fn extractor_finds_literal_environment_names_in_source_order() {
    let source = r#"
        std::env::var("UNSCOPED_ONE");
        std::env::var_os ( "TOKENSAVE_OK" );
        command.env("EXTERNAL_NAME", "value");
        command.env_remove("REMOVED_NAME");
        env!("CARGO_PKG_VERSION");
        option_env!("OPTIONAL_EXTERNAL");
    "#;

    assert_eq!(
        env_literals(source),
        vec![
            "UNSCOPED_ONE",
            "TOKENSAVE_OK",
            "EXTERNAL_NAME",
            "REMOVED_NAME",
            "CARGO_PKG_VERSION",
            "OPTIONAL_EXTERNAL",
        ]
    );
}

#[test]
fn extractor_ignores_comments_and_nonliteral_names() {
    let source = r#"
        // std::env::var("COMMENTED_LINE");
        /* command.env("COMMENTED_BLOCK", "value"); */
        std::env::var(SHARED_NAME);
        command.env(TOKEN_ENV_VAR, "value");
        std::env::var("TOKENSAVE_REAL");
    "#;

    assert_eq!(env_literals(source), vec!["TOKENSAVE_REAL"]);
}

#[test]
fn policy_rejects_new_unnamespaced_variables() {
    assert!(!is_allowed_env_var("UNSCOPED_ONE"));
    assert!(is_allowed_env_var("TOKENSAVE_EXAMPLE"));
    assert!(is_allowed_env_var("DISABLE_TOKENSAVE"));
    assert!(is_allowed_env_var("HOME"));
}

#[test]
fn first_party_environment_variables_are_namespaced() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();

    for path in rust_sources(root) {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        for name in env_literals(&source) {
            if !is_allowed_env_var(&name) {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                violations.push(format!("{}:{name}", relative.display()));
            }
        }
    }

    violations.sort();
    assert!(
        violations.is_empty(),
        "tokensave-owned environment variables must start with TOKENSAVE_: {}",
        violations.join(", ")
    );
}
