//! Incremental reference invalidation must resolve identically — #484.
//!
//! A one-line comment edit used to re-attempt all 189,757 references on this
//! repository, 63.5% of which (`assert`, `unwrap`, `to_string`, …) can never
//! resolve to a node in the project and were re-attempted forever. #484 narrows
//! each sync to the references it could actually have changed the answer for:
//! those in re-extracted files, plus those whose name (or trailing simple name)
//! gained or lost a candidate node.
//!
//! The failure mode of that change is a **silently missing edge** — nobody
//! files a bug for a call graph that is quietly a little smaller than it should
//! be. So the contract is not "it should be the same", it is asserted: the same
//! edit sequence is driven twice over two copies of a project, once
//! incrementally and once with `TOKENSAVE_FULL_RESOLVE=1` forcing the old
//! whole-table pass, and the resulting edge and ambiguity sets must match
//! exactly after *every* edit.
//!
//! The edits are chosen to cover each way the invalidation can be wrong:
//! adding a definition, removing one, renaming one, moving one between files
//! (the case that needs the *deleted* names, not just the inserted ones), and
//! changing only a comment (the case that must invalidate almost nothing).

use std::collections::HashSet;
use std::path::Path;

use tempfile::tempdir;
use tokensave::resolution::{index_keys_for_test, ReferenceResolver};
use tokensave::tokensave::TokenSave;

/// A project with real cross-file structure: a trait, two impls that make
/// `area` ambiguous, callers in a third file, and a re-export.
fn write_project(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub mod shapes;\npub mod extra;\npub mod render;\npub use shapes::Circle;\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/shapes.rs"),
        r#"
pub trait Shape {
    fn area(&self) -> f64;
}

pub struct Circle {
    pub radius: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

pub fn unit_circle() -> Circle {
    Circle { radius: 1.0 }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/extra.rs"),
        r#"
use crate::shapes::Shape;

pub struct Square {
    pub side: f64,
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.side * self.side
    }
}

pub fn unit_square() -> Square {
    Square { side: 1.0 }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/render.rs"),
        r#"
use crate::extra::{unit_square, Square};
use crate::shapes::{unit_circle, Circle, Shape};

pub fn describe(c: &Circle) -> String {
    let a = c.area();
    format!("area {a}")
}

pub fn describe_square(s: &Square) -> String {
    let a = s.area();
    format!("area {a}")
}

pub fn both() -> f64 {
    let c = unit_circle();
    let s = unit_square();
    c.area() + s.area()
}
"#,
    )
    .unwrap();
}

/// One edit, applied to whichever copy of the project it is handed.
struct Edit {
    name: &'static str,
    apply: fn(&Path),
}

fn edits() -> Vec<Edit> {
    vec![
        Edit {
            // Must invalidate essentially nothing: no name gains or loses a
            // node, and only one file is re-extracted.
            name: "comment-only change",
            apply: |root| {
                append(root, "src/shapes.rs", "\n// a comment, and nothing else\n");
            },
        },
        Edit {
            // A new definition whose name is called from an existing file: the
            // *inserted* half of the touched set has to reach `render.rs`.
            name: "add a definition called from elsewhere",
            apply: |root| {
                append(
                    root,
                    "src/shapes.rs",
                    "\npub fn perimeter(c: &Circle) -> f64 {\n    6.28318 * c.radius\n}\n",
                );
                append(
                    root,
                    "src/render.rs",
                    "\npub fn outline(c: &Circle) -> f64 {\n    crate::shapes::perimeter(c)\n}\n",
                );
            },
        },
        Edit {
            // Removing a definition deletes the edges pointing *into* it. The
            // deleted half of the touched set is what re-resolves the callers.
            name: "remove a definition",
            apply: |root| {
                let path = root.join("src/shapes.rs");
                let text = std::fs::read_to_string(&path).unwrap();
                std::fs::write(
                    &path,
                    text.replace(
                        "pub fn perimeter(c: &Circle) -> f64 {\n    6.28318 * c.radius\n}",
                        "",
                    ),
                )
                .unwrap();
            },
        },
        Edit {
            name: "rename a definition",
            apply: |root| {
                replace_in(root, "src/shapes.rs", "unit_circle", "make_unit_circle");
                replace_in(root, "src/render.rs", "unit_circle", "make_unit_circle");
            },
        },
        Edit {
            // The sharpest case: `unit_square` keeps its name but changes file.
            // Its old node is deleted (taking the edge from `render.rs` with
            // it) and a new one appears elsewhere, so both halves of the
            // touched set are needed to put the edge back.
            name: "move a definition between files",
            apply: |root| {
                replace_in(
                    root,
                    "src/extra.rs",
                    "pub fn unit_square() -> Square {\n    Square { side: 1.0 }\n}",
                    "",
                );
                append(
                    root,
                    "src/shapes.rs",
                    "\npub fn unit_square() -> crate::extra::Square {\n    crate::extra::Square { side: 1.0 }\n}\n",
                );
            },
        },
        Edit {
            // Removing one of two same-named candidates, *without touching the
            // caller's file*. This is the case that needs the deleted half of
            // the touched set and nothing else: `Square::area` disappears, so
            // `render.rs`'s `s.area()` — a reference in a file this sync never
            // re-extracts — can now bind where it could not before, or stops
            // being ambiguous. The inserted half cannot catch it, because the
            // name that changed is one that no longer exists anywhere in the
            // re-extracted file. Delete the deleted-half calls in
            // `sync_single_files`/`sync_with_progress_verbose` and this edit is
            // what fails.
            name: "remove one of two same-named candidates, caller untouched",
            apply: |root| {
                replace_in(
                    root,
                    "src/extra.rs",
                    "impl Shape for Square {\n    fn area(&self) -> f64 {\n        self.side * self.side\n    }\n}",
                    "",
                );
            },
        },
        Edit {
            // Deleting a whole file, again leaving `render.rs` alone: every
            // name the file defined is touched only via the deleted half.
            name: "delete a file, caller untouched",
            apply: |root| {
                std::fs::remove_file(root.join("src/extra.rs")).unwrap();
                replace_in(root, "src/lib.rs", "pub mod extra;\n", "");
            },
        },
    ]
}

fn append(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    let mut current = std::fs::read_to_string(&path).unwrap();
    current.push_str(text);
    std::fs::write(&path, current).unwrap();
}

fn replace_in(root: &Path, rel: &str, from: &str, to: &str) {
    let path = root.join(rel);
    let current = std::fs::read_to_string(&path).unwrap();
    assert!(
        current.contains(from),
        "{rel}: fixture edit expected to find {from:?}"
    );
    std::fs::write(&path, current.replace(from, to)).unwrap();
}

/// Edges and ambiguity records, in a form that compares by value rather than
/// by row order or by node-id ordering.
type Snapshot = (Vec<String>, Vec<String>);

async fn snapshot(cg: &TokenSave) -> Snapshot {
    let db = cg.db();
    let mut edges: Vec<String> = db
        .get_all_edges()
        .await
        .unwrap()
        .iter()
        .map(|e| format!("{}|{}|{:?}|{:?}", e.source, e.target, e.kind, e.line))
        .collect();
    edges.sort();
    let mut ambiguous: Vec<String> = db
        .get_ambiguous_calls(None, 1_000_000)
        .await
        .unwrap()
        .iter()
        .map(|a| {
            let mut candidates = a.candidate_node_ids.clone();
            candidates.sort();
            format!(
                "{}|{}|{}|{}|{}",
                a.from_node_id,
                a.reference_name,
                a.file_path,
                a.line,
                candidates.join(",")
            )
        })
        .collect();
    ambiguous.sort();
    (edges, ambiguous)
}

/// The contract. Two identical trees, the same edits in the same order, one
/// resolved incrementally and one resolved in full, compared after every step.
#[tokio::test]
async fn incremental_resolution_matches_a_full_pass_across_a_series_of_edits() {
    let inc_dir = tempdir().unwrap();
    let full_dir = tempdir().unwrap();
    write_project(inc_dir.path());
    write_project(full_dir.path());

    // `init` is a full index in both trees, so they start identical by
    // construction — anything that diverges later is the invalidation set.
    let inc = TokenSave::init(inc_dir.path()).await.unwrap();
    let full = TokenSave::init(full_dir.path()).await.unwrap();
    inc.sync().await.unwrap();
    std::env::set_var("TOKENSAVE_FULL_RESOLVE", "1");
    full.sync().await.unwrap();
    std::env::remove_var("TOKENSAVE_FULL_RESOLVE");

    let (inc_edges, _) = snapshot(&inc).await;
    assert!(
        !inc_edges.is_empty(),
        "precondition: the fixture must resolve some cross-file edges"
    );

    for edit in edits() {
        (edit.apply)(inc_dir.path());
        (edit.apply)(full_dir.path());

        // Same second-resolution mtime granularity concerns as the rest of the
        // suite: content hashing is what decides staleness, so no sleep needed.
        std::env::remove_var("TOKENSAVE_FULL_RESOLVE");
        inc.sync().await.unwrap();
        std::env::set_var("TOKENSAVE_FULL_RESOLVE", "1");
        full.sync().await.unwrap();
        std::env::remove_var("TOKENSAVE_FULL_RESOLVE");

        let (inc_edges, inc_ambiguous) = snapshot(&inc).await;
        let (full_edges, full_ambiguous) = snapshot(&full).await;

        let missing: Vec<_> = full_edges
            .iter()
            .filter(|e| !inc_edges.contains(e))
            .collect();
        assert!(
            missing.is_empty(),
            "after {}: incremental resolution dropped {} edge(s) a full pass found: {missing:?}",
            edit.name,
            missing.len()
        );
        assert_eq!(
            inc_edges, full_edges,
            "after {}: edge sets must be identical",
            edit.name
        );
        assert_eq!(
            inc_ambiguous, full_ambiguous,
            "after {}: ambiguity records must be identical",
            edit.name
        );
    }
}

/// The touched-name set is only correct if it enumerates the *same* keys the
/// resolver's pre-filter admits. `index_keys` duplicates the shape of
/// `from_nodes`'s three caches by necessity — it runs over nodes being deleted,
/// which no resolver has loaded — so this pins the two together: every key of
/// the real `known_names` index must be produced by `index_keys` over the same
/// nodes, and vice versa. Add a fourth cache to `from_nodes` and this fails.
#[tokio::test]
async fn touched_name_keys_match_the_resolvers_own_name_index() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path());
    let cg = TokenSave::init(tmp.path()).await.unwrap();
    cg.sync().await.unwrap();
    let db = cg.db();

    let nodes = db.get_all_nodes_for_resolution().await.unwrap();
    assert!(
        !nodes.is_empty(),
        "precondition: the fixture must have nodes"
    );

    let resolver = ReferenceResolver::from_nodes(db, &nodes);
    let known: HashSet<String> = resolver
        .known_names()
        .iter()
        .map(|n| (*n).to_string())
        .collect();
    let derived = index_keys_for_test(&nodes);

    let mut only_known: Vec<_> = known.difference(&derived).cloned().collect();
    let mut only_derived: Vec<_> = derived.difference(&known).cloned().collect();
    only_known.sort();
    only_derived.sort();

    assert!(
        only_known.is_empty(),
        "the resolver admits names the touched set never produces, so a sync \
         touching those nodes would skip references that could now resolve: {only_known:?}"
    );
    assert!(
        only_derived.is_empty(),
        "the touched set produces names the resolver's index does not have, \
         which over-invalidates rather than under-invalidates, but means the \
         two have drifted: {only_derived:?}"
    );
}
