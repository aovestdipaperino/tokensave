//! Regression tests for #643: `tokensave_implementations` found no C#
//! implementers.
//!
//! * A C# base list cannot syntactically tell a base class from an
//!   interface, so the extractor records the first entry of a class's base
//!   list as `Extends`. When that entry resolves to an interface, the stored
//!   edge must be `Implements`, which is what `tokensave_implementations`
//!   reads.
//! * A generic base (`IProducer<TRecord>`) was recorded with its type
//!   arguments in the reference name, so it never matched the interface node
//!   named `IProducer`.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::extraction::{CSharpExtractor, LanguageExtractor};
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

const AUTH_SRC: &str = r#"
namespace App.Auth
{
    public interface IAuthService { bool Login(string user); }

    public class BaseService { }

    public class LocalAuth : IAuthService
    {
        public bool Login(string user) { return true; }
    }

    public class LdapAuth : BaseService, IAuthService
    {
        public bool Login(string user) { return false; }
    }

    public sealed class TokenAuth : IAuthService, System.IDisposable
    {
        public bool Login(string user) { return true; }
        public void Dispose() { }
    }
}
"#;

const PRODUCER_SRC: &str = r#"
namespace App.Data
{
    public interface IProducer<T> where T : struct
    {
        T Next();
    }

    public sealed class DbReaderProducer<TRecord> : IProducer<TRecord> where TRecord : struct
    {
        public TRecord Next() { return default; }

        private sealed class EmptyProducer<T> : IProducer<T> where T : struct
        {
            public T Next() { return default; }
        }
    }

    public class Pipeline : App.Data.IProducer<int>
    {
        public int Next() { return 0; }
    }
}
"#;

fn extract_text(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

#[test]
fn generic_base_types_are_recorded_without_type_arguments() {
    let result = CSharpExtractor.extract("Producer.cs", PRODUCER_SRC);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let bases: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| matches!(r.reference_kind, EdgeKind::Extends | EdgeKind::Implements))
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        bases.iter().all(|b| !b.contains('<')),
        "base refs must not carry type arguments: {bases:?}"
    );
    assert_eq!(
        bases.iter().filter(|b| **b == "IProducer").count(),
        2,
        "both generic implementers must reference `IProducer`: {bases:?}"
    );
    assert!(
        bases.contains(&"App.Data.IProducer"),
        "qualified generic base keeps its qualifier: {bases:?}"
    );
}

async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/Auth.cs"), AUTH_SRC).unwrap();
    fs::write(project.join("src/Producer.cs"), PRODUCER_SRC).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn implementer_names(cg: &TokenSave, trait_name: &str) -> Vec<String> {
    let result = handle_tool_call(
        cg,
        "tokensave_implementations",
        json!({ "trait": trait_name }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }));
    let mut names: Vec<String> = output["implementations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| e["type"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[tokio::test]
async fn implementations_finds_csharp_interface_implementers() {
    let (_dir, cg) = setup().await;
    assert_eq!(
        implementer_names(&cg, "IAuthService").await,
        vec!["LdapAuth", "LocalAuth", "TokenAuth"]
    );
}

#[tokio::test]
async fn implementations_finds_generic_csharp_interface_implementers() {
    let (_dir, cg) = setup().await;
    assert_eq!(
        implementer_names(&cg, "IProducer").await,
        vec!["DbReaderProducer", "EmptyProducer", "Pipeline"]
    );
}

#[tokio::test]
async fn csharp_base_class_stays_extends_and_hierarchy_still_works() {
    let (_dir, cg) = setup().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let base = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.name == "BaseService")
        .expect("BaseService node");
    let incoming = cg.get_incoming_edges(&base.id).await.unwrap();
    assert!(
        incoming.iter().any(|e| e.kind == EdgeKind::Extends),
        "a class base must stay `extends`: {incoming:?}"
    );
    assert!(
        !incoming.iter().any(|e| e.kind == EdgeKind::Implements),
        "a class base must not become `implements`: {incoming:?}"
    );

    for iface in ["IAuthService", "IProducer"] {
        let result = handle_tool_call(
            &cg,
            "tokensave_type_hierarchy",
            json!({ "node_id": nodes.iter().find(|n| n.name == iface && n.kind == NodeKind::Interface).expect("interface").id }),
            None,
            None,
        )
        .await
        .unwrap();
        let text = extract_text(&result.value);
        let expected: &[&str] = if iface == "IAuthService" {
            &["LocalAuth", "LdapAuth", "TokenAuth"]
        } else {
            &["DbReaderProducer", "EmptyProducer", "Pipeline"]
        };
        for name in expected {
            assert!(
                text.contains(name),
                "type_hierarchy({iface}) should list {name}: {text}"
            );
        }
    }
}
