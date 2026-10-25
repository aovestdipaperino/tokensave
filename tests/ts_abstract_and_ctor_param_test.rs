//! TypeScript declarations the extractor stepped over — #413, #414.
//!
//! Both were found by @vianbas while checking what `3b294bc` still missed, and
//! both are absences rather than mistakes: nothing wrong is recorded, the
//! symbols simply never reach the graph, so every question about them answers
//! "no" with the same confidence as a real negative.
//!
//! **#413**: `abstract class` parses as `abstract_class_declaration`, a
//! different node kind from `class_declaration`, and the extractor dispatched
//! only on the latter. The class, every method on it, and every edge into
//! those methods were missing — including the `extends` target of any concrete
//! subclass. In TypeScript an abstract base is where the shared interface
//! usually lives, so this removes exactly the methods most likely to be called.
//!
//! **#414**: a constructor parameter property (`constructor(private worker:
//! Beta) {}`) declares a field, but the declaration lives in the parameter list
//! rather than the class body, so `collect_field_types` never saw it. That is
//! the standard dependency-injection form, and the one most likely to appear in
//! the code #378 was reported against.

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

/// `(qualified_name_of_source, qualified_name_of_target)` for every `calls`
/// edge, so assertions read in terms of the source.
async fn named_call_edges(cg: &TokenSave) -> Vec<(String, String)> {
    let nodes = cg.get_all_nodes().await.unwrap();
    let by_id: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.qualified_name.as_str()))
        .collect();
    let mut named: Vec<(String, String)> = cg
        .get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .filter_map(|e| {
            Some((
                (*by_id.get(e.source.as_str())?).to_string(),
                (*by_id.get(e.target.as_str())?).to_string(),
            ))
        })
        .collect();
    named.sort();
    named
}

async fn indexed(files: &[(&str, &str)]) -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    for (name, body) in files {
        std::fs::write(root.join("src").join(name), *body).unwrap();
    }
    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

/// The #413 reproduction: both abstract classes and every method on them must
/// be extracted, exported or not.
#[tokio::test]
async fn abstract_classes_and_their_methods_are_extracted() {
    let (_tmp, cg) = indexed(&[(
        "kelas.ts",
        r#"
abstract class Lokal {
  satu(): string {
    return "lokal";
  }
}

export abstract class Diekspor {
  dua(): string {
    return "diekspor";
  }
}

export class Biasa extends Diekspor {
  tiga(): string {
    return "biasa";
  }
}
"#,
    )])
    .await;

    let nodes = cg.get_all_nodes().await.unwrap();
    let names: Vec<&str> = nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Class | NodeKind::Method))
        .map(|n| n.name.as_str())
        .collect();

    for wanted in ["Lokal", "Diekspor", "Biasa", "satu", "dua", "tiga"] {
        assert!(
            names.contains(&wanted),
            "`{wanted}` must reach the graph, got {names:?}"
        );
    }
}

/// A call into an abstract base's method must resolve. This is the
/// consequence that matters: in TypeScript the shared interface usually lives
/// on the abstract base, so those are the methods most likely to be called,
/// and they were exactly the ones missing.
#[tokio::test]
async fn a_call_into_an_abstract_base_method_resolves() {
    let (_tmp, cg) = indexed(&[
        (
            "base.ts",
            r#"
export abstract class Base {
  shared(): string {
    return "shared";
  }
}
"#,
        ),
        (
            "caller.ts",
            r#"
import { Base } from "./base";

export class Caller {
  constructor(private readonly base: Base) {}
  run(): string {
    return this.base.shared();
  }
}
"#,
        ),
    ])
    .await;

    let named = named_call_edges(&cg).await;
    assert!(
        named.iter().any(|(_, target)| target.ends_with("shared")),
        "a call into an abstract base's method must resolve, got {named:?}"
    );
}

/// An abstract class must still be a usable `extends` target, so the
/// inheritance edge from a concrete subclass has somewhere to land.
#[tokio::test]
async fn a_concrete_subclass_keeps_its_extends_target() {
    let (_tmp, cg) = indexed(&[(
        "hierarchy.ts",
        r#"
export abstract class Shape {
  area(): number {
    return 0;
  }
}

export class Circle extends Shape {
  radius = 1;
}
"#,
    )])
    .await;

    let nodes = cg.get_all_nodes().await.unwrap();
    let shape = nodes
        .iter()
        .find(|n| n.name == "Shape" && n.kind == NodeKind::Class)
        .expect("the abstract base must be a node");

    let inherits = cg
        .get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .any(|e| e.kind == EdgeKind::Extends && e.target == shape.id);

    assert!(
        inherits,
        "`Circle extends Shape` must land on the abstract base"
    );
}

/// The #414 reproduction: a constructor parameter property declares a field,
/// so a call through it must resolve exactly as an annotated body field does.
#[tokio::test]
async fn a_constructor_parameter_property_resolves_calls_through_it() {
    let (_tmp, cg) = indexed(&[
        (
            "beta.ts",
            "export class Beta {\n  handle(tag: string): string {\n    return tag;\n  }\n}\n",
        ),
        (
            "alpha.ts",
            "export class Alpha {\n  handle(tag: string): string {\n    return tag;\n  }\n}\n",
        ),
        (
            "callers.ts",
            r#"
import { Beta } from "./beta";

export class ViaField {
  private worker: Beta = new Beta();
  run(): string {
    return this.worker.handle("field");
  }
}

export class ViaCtorParam {
  constructor(private readonly worker: Beta) {}
  run(): string {
    return this.worker.handle("ctor");
  }
}
"#,
        ),
    ])
    .await;

    let named = named_call_edges(&cg).await;
    let from_ctor: Vec<&(String, String)> = named
        .iter()
        .filter(|(source, _)| source.contains("ViaCtorParam"))
        .collect();

    assert_eq!(
        from_ctor.len(),
        1,
        "the call through a constructor parameter property must resolve, got {named:?}"
    );
    assert!(
        from_ctor[0].1.contains("Beta"),
        "it must bind to Beta, not the other class that also defines `handle`, got {from_ctor:?}"
    );
}

/// The control from the reporter's own fixture: the annotated body field that
/// `3b294bc` already handled must keep working.
#[tokio::test]
async fn an_annotated_body_field_still_resolves() {
    let (_tmp, cg) = indexed(&[
        (
            "beta.ts",
            "export class Beta {\n  handle(tag: string): string {\n    return tag;\n  }\n}\n",
        ),
        (
            "callers.ts",
            "import { Beta } from \"./beta\";\n\
             export class ViaField {\n  private worker: Beta = new Beta();\n\
             \n  run(): string {\n    return this.worker.handle(\"field\");\n  }\n}\n",
        ),
    ])
    .await;

    let named = named_call_edges(&cg).await;
    assert!(
        named
            .iter()
            .any(|(source, target)| source.contains("ViaField") && target.contains("Beta")),
        "the annotated field form must keep resolving, got {named:?}"
    );
}

/// #424: a method declared without a body inside an `abstract class` parses as
/// `abstract_method_signature`, a kind `visit_class_body` did not match, so the
/// declaration never became a node at all. With a single subclass the call
/// still landed on that one implementation by name, which is why the shape hid
/// behind the fixtures above; with two, the resolver could not separate them
/// and the call produced no edge at all.
///
/// The assertion is on the edge rather than on the node. Kind filtering runs
/// ahead of scoring, so a node of a newly emitted kind reaching the graph does
/// not by itself mean a call can resolve to it.
#[tokio::test]
async fn a_call_into_an_abstract_declaration_resolves_past_two_implementations() {
    let (_tmp, cg) = indexed(&[(
        "hierarchy.ts",
        r#"
export abstract class Base {
  abstract shared(): string;
}

export class First extends Base {
  shared(): string {
    return "first";
  }
}

export class Second extends Base {
  shared(): string {
    return "second";
  }
}

export class Caller {
  constructor(private readonly base: Base) {}
  run(): string {
    return this.base.shared();
  }
}
"#,
    )])
    .await;

    let nodes = cg.get_all_nodes().await.unwrap();
    let declaration = nodes
        .iter()
        .find(|n| n.kind == NodeKind::AbstractMethod && n.name == "shared")
        .expect("the abstract declaration must reach the graph as an AbstractMethod node");

    let named = named_call_edges(&cg).await;
    assert!(
        named.iter().any(
            |(source, target)| source.ends_with("run") && *target == declaration.qualified_name
        ),
        "the call must resolve to the declaration on the base, got {named:?}"
    );
}
