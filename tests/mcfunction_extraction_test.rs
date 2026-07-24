// Rust guideline compliant 2025-10-17
//! Tests for the Minecraft datapack `.mcfunction` extractor (#262).
#![cfg(feature = "lang-mcfunction")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use tempfile::TempDir;
use tokensave::extraction::{LanguageExtractor, LanguageRegistry, McFunctionExtractor};
use tokensave::tokensave::TokenSave;
use tokensave::types::*;

#[test]
fn test_mcfunction_registry_dispatch() {
    let registry = LanguageRegistry::new();
    let extractor = registry
        .extractor_for_file("data/example/function/a.mcfunction")
        .expect(".mcfunction must be handled");
    assert_eq!(extractor.language_name(), "MCFunction");
}

#[test]
fn test_mcfunction_file_is_the_function() {
    let result = McFunctionExtractor.extract(
        "data/example/function/a.mcfunction",
        "# The entry point.\nsay hello\n",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1, "the file is exactly one function");
    assert_eq!(fns[0].name, "example:a");
    assert_eq!(
        fns[0].docstring.as_deref(),
        Some("The entry point."),
        "leading comment block becomes the docstring"
    );

    // File `Contains` the function node.
    assert!(result
        .edges
        .iter()
        .any(|e| e.kind == EdgeKind::Contains && e.source == files[0].id && e.target == fns[0].id));
}

#[test]
fn test_mcfunction_legacy_plural_directory_and_nesting() {
    let result = McFunctionExtractor.extract(
        "packs/mypack/data/example/functions/sub/dir/c.mcfunction",
        "say hi\n",
    );
    let f = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function)
        .unwrap();
    assert_eq!(f.name, "example:sub/dir/c");
}

#[test]
fn test_mcfunction_call_forms() {
    let source = "\
# doc
function example:b
execute as @a at @s run function example:c
return run function example:d
schedule function example:b 10t append
function no_namespace
scoreboard players add @s obj 1
";
    let result = McFunctionExtractor.extract("data/example/function/a.mcfunction", source);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert_eq!(
        calls,
        vec![
            "example:b",
            "example:c",
            "example:d",
            "example:b",
            "minecraft:no_namespace",
        ],
        "one Calls ref per function command, namespace-less normalized"
    );
}

#[test]
fn test_mcfunction_macro_and_tag_targets_stay_dynamic() {
    let source = "\
$execute as @a run function example:$(which)
function example:$(dynamic)
function #example:tick_hooks
";
    let result = McFunctionExtractor.extract("data/example/function/a.mcfunction", source);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .map(|r| r.reference_name.as_str())
        .collect();
    assert_eq!(
        calls,
        vec![
            "example:$(which)",
            "example:$(dynamic)",
            "#example:tick_hooks",
        ],
        "macro targets and function tags are preserved verbatim (unresolved/dynamic)"
    );
}

#[test]
fn test_mcfunction_comments_and_non_calls_ignored() {
    let source = "\
# function example:not_a_call
say function example:also_not_a_call
tellraw @a {\"text\":\"function example:nope\"}
";
    let result = McFunctionExtractor.extract("data/example/function/a.mcfunction", source);
    assert!(
        result.unresolved_refs.is_empty(),
        "refs: {:?}",
        result.unresolved_refs
    );
}

/// End-to-end repro from #262: a synthetic datapack whose functions call
/// each other must produce `Function` nodes named by resource location and
/// resolved `Calls` edges; a macro target stays unresolved.
#[tokio::test]
async fn test_mcfunction_end_to_end_call_edges() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    let fn_dir = project.join("data/example/function");
    fs::create_dir_all(&fn_dir).unwrap();
    fs::write(
        fn_dir.join("a.mcfunction"),
        "# entry\n\
         function example:b\n\
         execute as @a run function example:c\n\
         schedule function example:b 1t\n\
         function example:$(which)\n",
    )
    .unwrap();
    fs::write(fn_dir.join("b.mcfunction"), "say b\n").unwrap();
    fs::write(fn_dir.join("c.mcfunction"), "say c\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    let result = cg.index_all().await.unwrap();
    assert_eq!(result.file_count, 3, "all three .mcfunction files indexed");

    let nodes = cg.db().get_all_nodes().await.unwrap();
    let id_of = |name: &str| {
        nodes
            .iter()
            .find(|n| n.kind == NodeKind::Function && n.name == name)
            .unwrap_or_else(|| panic!("missing function node {name}"))
            .id
            .clone()
    };
    let a = id_of("example:a");
    let b = id_of("example:b");
    let c = id_of("example:c");

    let edges = cg.db().get_all_edges().await.unwrap();
    let calls_from_a: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.source == a)
        .collect();
    assert!(
        calls_from_a.iter().any(|e| e.target == b),
        "expected call edge example:a -> example:b"
    );
    assert!(
        calls_from_a.iter().any(|e| e.target == c),
        "expected call edge example:a -> example:c"
    );
    // plain + schedule to b (two lines) + execute-run to c; the macro
    // target `example:$(which)` must NOT have produced an edge.
    assert_eq!(
        calls_from_a.len(),
        3,
        "macro/dynamic target must stay unresolved: {calls_from_a:?}"
    );
}
