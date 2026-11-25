//! Regressions for the name-based resolver fallbacks that SCIP comparison
//! found binding references wrongly (#1109): `same-file-blocklist`,
//! `exact-match-scored` and the `::` path tail.
use crate::common::stability::Project;
use std::collections::HashMap;
use std::path::Path;
use tokensave::types::EdgeKind;

/// Every edge as `(source qualified name, kind, target qualified name)`.
async fn named_edges(p: &Project) -> Vec<(String, EdgeKind, String)> {
    let nodes = p.cg.get_all_nodes().await.unwrap();
    let names: HashMap<_, _> = nodes
        .iter()
        .map(|n| (n.id.clone(), n.qualified_name.clone()))
        .collect();
    p.cg.get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .filter_map(|e| {
            Some((
                names.get(&e.source)?.clone(),
                e.kind,
                names.get(&e.target)?.clone(),
            ))
        })
        .collect()
}

fn has_edge(edges: &[(String, EdgeKind, String)], from: &str, kind: EdgeKind, to: &str) -> bool {
    edges
        .iter()
        .any(|(s, k, t)| s == from && *k == kind && t == to)
}

const MAP_RS: &str = r"use std::collections::BTreeMap;

pub struct Map {
    map: BTreeMap<String, u32>,
}

impl Map {
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn retain(&mut self, keep: u32) {
        self.map.retain(|_, v| *v == keep)
    }

    pub fn clear(&mut self) {
        self.map.clear()
    }

    pub fn reset(&mut self) {
        self.clear()
    }
}

impl Clone for Map {
    fn clone(&self) -> Self {
        Map {
            map: self.map.clone(),
        }
    }
}
";

/// A second owner of the wrapper names, so the scored path runs too.
const SET_RS: &str = r"pub struct Set;

impl Set {
    pub fn contains_key(&self, _key: &str) -> bool {
        false
    }

    pub fn retain(&mut self, _keep: u32) {}
}
";

#[tokio::test]
async fn delegating_wrappers_do_not_call_themselves() {
    let p = Project::new(&[
        ("src/map.rs", MAP_RS.to_string()),
        ("src/set.rs", SET_RS.to_string()),
    ])
    .await;
    let edges = named_edges(&p).await;

    for (source, kind, target) in &edges {
        assert!(
            !(*kind == EdgeKind::Calls && source == target),
            "fake recursion: {source} calls itself\n{edges:#?}"
        );
    }
    // A call on `self` still reaches the type's own method.
    assert!(
        has_edge(
            &edges,
            "src/map.rs::Map::reset",
            EdgeKind::Calls,
            "src/map.rs::Map::clear"
        ),
        "{edges:#?}"
    );
}

const NUMBER_RS: &str = r"pub enum ParserNumber {
    F64(f64),
    String(String),
}

impl From<ParserNumber> for String {
    fn from(_n: ParserNumber) -> String {
        String::new()
    }
}

pub fn parse_into(buf: &mut String) -> ParserNumber {
    buf.push('0');
    ParserNumber::F64(0.0)
}

pub fn render(_n: &ParserNumber) -> String {
    String::new()
}
";

#[tokio::test]
async fn type_positions_skip_enum_variants_and_impl_blocks() {
    let p = Project::new(&[("src/number.rs", NUMBER_RS.to_string())]).await;
    let edges = named_edges(&p).await;

    for (source, kind, target) in &edges {
        if matches!(kind, EdgeKind::TypeOf | EdgeKind::Returns) {
            assert!(
                !target.ends_with("::String"),
                "{source} -{kind:?}-> {target}: std String bound to a project node\n{edges:#?}"
            );
        }
    }
    assert!(
        has_edge(
            &edges,
            "src/number.rs::parse_into",
            EdgeKind::Returns,
            "src/number.rs::ParserNumber"
        ),
        "{edges:#?}"
    );
}

const VISIT_RS: &str = r"pub struct Value;
pub struct Error;

pub trait Visitor {
    type Value;
    type Error;
    fn visit(self) -> Result<Self::Value, Self::Error>;
}

pub struct Counter;

impl Visitor for Counter {
    type Value = u32;
    type Error = std::fmt::Error;
    fn visit(self) -> Result<Self::Value, Self::Error> {
        Ok(1)
    }
}

pub fn drive<V: Visitor>(v: V) -> Option<V::Value> {
    v.visit().ok()
}

pub fn failure<V: Visitor>(_v: &V) -> Option<V::Error> {
    None
}

pub fn io_failure() -> std::io::Error {
    std::io::Error::other(String::new())
}

pub fn make() -> Value {
    Value
}
";

#[tokio::test]
async fn associated_and_std_types_do_not_bind_to_project_types() {
    let p = Project::new(&[("src/visit.rs", VISIT_RS.to_string())]).await;
    let edges = named_edges(&p).await;

    let project_types = ["src/visit.rs::Value", "src/visit.rs::Error"];
    for (source, kind, target) in &edges {
        if matches!(kind, EdgeKind::TypeOf | EdgeKind::Returns) && source != "src/visit.rs::make" {
            assert!(
                !project_types.contains(&target.as_str()),
                "{source} -{kind:?}-> {target}: associated or std type bound to a project type\n{edges:#?}"
            );
        }
    }
    // A plain project type is still found.
    assert!(
        has_edge(
            &edges,
            "src/visit.rs::make",
            EdgeKind::Returns,
            "src/visit.rs::Value"
        ),
        "{edges:#?}"
    );
}

const ARRAY_RS: &str = r"pub enum Item {
    Empty,
}

pub struct Array {
    values: Vec<u32>,
}

impl Array {
    pub fn new() -> Self {
        Array { values: Vec::new() }
    }

    pub fn boxed(&self) -> Box<u32> {
        Box::new(1)
    }

    pub fn fresh() -> Self {
        Self::new()
    }
}

impl Iterator for Array {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.values.pop()
    }
}

pub fn first(_a: &Array) -> Item {
    Item::Empty
}
";

#[tokio::test]
async fn std_paths_and_bare_types_stay_out_of_the_file() {
    let p = Project::new(&[("src/array.rs", ARRAY_RS.to_string())]).await;
    let edges = named_edges(&p).await;

    // `Vec::new()` and `Box::new(1)` are std calls, not `Array::new`.
    for caller in ["src/array.rs::Array::new", "src/array.rs::Array::boxed"] {
        assert!(
            !has_edge(&edges, caller, EdgeKind::Calls, "src/array.rs::Array::new"),
            "{caller} bound a std constructor to Array::new\n{edges:#?}"
        );
    }
    assert!(
        has_edge(
            &edges,
            "src/array.rs::Array::fresh",
            EdgeKind::Calls,
            "src/array.rs::Array::new"
        ),
        "{edges:#?}"
    );
    // A bare `Item` is the enum, never the impl's associated type.
    assert!(
        has_edge(
            &edges,
            "src/array.rs::first",
            EdgeKind::Returns,
            "src/array.rs::Item"
        ),
        "{edges:#?}"
    );
    assert!(
        !has_edge(
            &edges,
            "src/array.rs::first",
            EdgeKind::Returns,
            "src/array.rs::Array::Item"
        ),
        "{edges:#?}"
    );
}

const BAG_PY: &str = r"class Bag:
    def __init__(self):
        self.items = []

    def append(self, item):
        self.items.append(item)

    def add(self, item):
        self.append(item)
";

#[tokio::test]
async fn calls_on_another_value_skip_the_callers_own_type() {
    // Python qualified names repeat the module path.
    const APPEND: &str = "bag.py::bag.py::Bag::append";
    const ADD: &str = "bag.py::bag.py::Bag::add";
    let p = Project::new(&[("bag.py", BAG_PY.to_string())]).await;
    let edges = named_edges(&p).await;

    assert!(
        !has_edge(&edges, APPEND, EdgeKind::Calls, APPEND),
        "{edges:#?}"
    );
    assert!(has_edge(&edges, ADD, EdgeKind::Calls, APPEND), "{edges:#?}");
}

fn scip_fixture_source(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("scip")
        .join("ambiguous")
        .join("src")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn module_paths_and_trait_declarations_in_the_scip_fixture() {
    let p = Project::new(&[
        ("src/lib.rs", scip_fixture_source("lib.rs")),
        ("src/alpha.rs", scip_fixture_source("alpha.rs")),
        ("src/beta.rs", scip_fixture_source("beta.rs")),
    ])
    .await;
    let edges = named_edges(&p).await;

    // `crate::alpha::helper()` in beta.rs names alpha's helper.
    let beta_check = "src/beta.rs::Config::check";
    assert!(
        has_edge(&edges, beta_check, EdgeKind::Calls, "src/alpha.rs::helper"),
        "{edges:#?}"
    );
    assert!(
        !has_edge(&edges, beta_check, EdgeKind::Calls, "src/beta.rs::helper"),
        "{edges:#?}"
    );
    // `a.check()` has no evidence for the trait declaration over the impls.
    assert!(
        !has_edge(
            &edges,
            "src/lib.rs::run",
            EdgeKind::Calls,
            "src/lib.rs::Validate::check"
        ),
        "{edges:#?}"
    );
}
