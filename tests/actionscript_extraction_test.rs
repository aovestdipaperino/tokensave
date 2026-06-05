#![cfg(feature = "lang-actionscript")]
//! ActionScript 2 (AVM1 / FFDec-decompiled) extraction tests.

use tokensave::extraction::ActionScriptExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn extract_fixture() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.as").unwrap();
    let extractor = ActionScriptExtractor;
    let result = extractor.extract("sample.as", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

fn names_of(result: &ExtractionResult, kind: NodeKind) -> Vec<String> {
    result
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.name.clone())
        .collect()
}

#[test]
fn file_root_present() {
    let r = extract_fixture();
    assert!(r.nodes.iter().any(|n| n.kind == NodeKind::File));
}

#[test]
fn class_extracted_with_short_name_and_dotted_qualified_name() {
    let r = extract_fixture();
    let classes: Vec<_> = r.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(
        classes.len(),
        1,
        "expected 1 class, got {:?}",
        names_of(&r, NodeKind::Class)
    );
    let acc = classes[0];
    assert_eq!(acc.name, "Account", "class short name");
    assert_eq!(
        acc.qualified_name, "com.example.app.Account",
        "class qualified (dotted) name"
    );
}

#[test]
fn interface_extracted() {
    let r = extract_fixture();
    let ifaces = names_of(&r, NodeKind::Interface);
    assert_eq!(ifaces, vec!["IDriveable".to_string()]);
}

#[test]
fn extends_and_implements_edges_recorded() {
    let r = extract_fixture();
    let has = |name: &str, kind: EdgeKind| {
        r.unresolved_refs
            .iter()
            .any(|u| u.reference_kind == kind && u.reference_name == name)
    };
    assert!(
        has("com.example.app.Handler", EdgeKind::Extends),
        "extends Handler"
    );
    assert!(
        has("com.example.app.IDriveable", EdgeKind::Implements),
        "implements IDriveable"
    );
}

#[test]
fn constructor_detected() {
    let r = extract_fixture();
    let ctors = names_of(&r, NodeKind::Constructor);
    assert_eq!(ctors, vec!["Account".to_string()], "constructor = same name as class");
}

#[test]
fn methods_extracted_including_accessor() {
    let r = extract_fixture();
    let methods = names_of(&r, NodeKind::Method);
    // logon, checkCredentials, drive, and the `get speed` accessor.
    assert!(methods.contains(&"logon".to_string()), "logon: {methods:?}");
    assert!(
        methods.contains(&"checkCredentials".to_string()),
        "checkCredentials: {methods:?}"
    );
    assert!(
        methods.iter().any(|m| m == "get speed"),
        "get accessor: {methods:?}"
    );
}

#[test]
fn fields_extracted() {
    let r = extract_fixture();
    // _sName, ID are Fields; DEFAULT_SPEED (static var) is also a Field.
    let fields = names_of(&r, NodeKind::Field);
    assert!(fields.contains(&"_sName".to_string()), "_sName: {fields:?}");
    assert!(fields.contains(&"ID".to_string()), "ID: {fields:?}");
}

#[test]
fn imports_become_use_nodes() {
    let r = extract_fixture();
    let uses = names_of(&r, NodeKind::Use);
    assert!(uses.contains(&"Handler".to_string()), "import Handler: {uses:?}");
    assert!(uses.contains(&"Logger".to_string()), "import Logger: {uses:?}");
}

#[test]
fn call_sites_recorded() {
    let r = extract_fixture();
    let calls: Vec<&str> = r
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls)
        .map(|u| u.reference_name.as_str())
        .collect();
    // Logger.dbg(...) in the constructor, checkCredentials(...) + connect() in logon.
    assert!(calls.contains(&"dbg"), "Logger.dbg call: {calls:?}");
    assert!(
        calls.contains(&"checkCredentials"),
        "checkCredentials call: {calls:?}"
    );
}

#[test]
fn contains_edges_link_class_to_members() {
    let r = extract_fixture();
    let class_id = &r
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class)
        .expect("class node")
        .id;
    let contained = r
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && &e.source == class_id)
        .count();
    // 3 fields + constructor + 4 methods = 8 members.
    assert!(contained >= 6, "expected class to contain members, got {contained}");
}
