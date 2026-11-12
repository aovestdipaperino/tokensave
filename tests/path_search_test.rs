//! The bounded path search behind `tokensave_call_chain` with
//! `follow: "any"` (`TokenSave::get_connection_path`).
//!
//! The search runs from both ends. These tests pin that it still returns a
//! shortest path, and that a well-connected node near one end does not use
//! the node budget.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::fs;
use tempfile::TempDir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{Edge, EdgeKind, ResolvedBy};

/// A project with `count` functions named `f0`, `f1`, ... and no edges.
/// Returns the node id of each function, by index.
async fn functions(count: usize) -> (TempDir, TokenSave, Vec<String>) {
    let dir = TempDir::new().unwrap();
    let mut source = String::new();
    for i in 0..count {
        let _ = writeln!(source, "pub fn f{i}() {{}}");
    }
    fs::write(dir.path().join("lib.rs"), source).unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let nodes = cg.get_nodes_by_name(&format!("f{i}")).await.unwrap();
        ids.push(nodes[0].id.clone());
    }
    (dir, cg, ids)
}

fn edge(ids: &[String], source: usize, target: usize, kind: EdgeKind) -> Edge {
    Edge {
        source: ids[source].clone(),
        target: ids[target].clone(),
        kind,
        line: Some(1),
        resolved_by: Some(ResolvedBy::ExactMatch),
    }
}

/// The shape that used the whole budget before the search ran from both
/// ends: the start is next to a node with 300 callers, and the path to the
/// end leaves through the last of them.
///
/// `f0 -> f1 <- f303 -> f2 -> f3`, with `f4..=f303` all calling `f1`.
#[tokio::test]
async fn a_hub_next_to_the_start_does_not_use_the_budget() {
    let (_dir, cg, ids) = functions(304).await;
    let mut edges = vec![
        edge(&ids, 0, 1, EdgeKind::Calls),
        edge(&ids, 303, 2, EdgeKind::Calls),
        edge(&ids, 2, 3, EdgeKind::Calls),
    ];
    for caller in 4..304 {
        edges.push(edge(&ids, caller, 1, EdgeKind::Calls));
    }
    cg.db().insert_edges(&edges).await.unwrap();

    let search = cg
        .get_connection_path(&ids[0], &ids[3], 8, 50)
        .await
        .unwrap();
    assert!(
        !search.budget_exhausted,
        "the search used its 50-node budget"
    );
    let names: Vec<String> = search
        .path
        .expect("a path")
        .iter()
        .map(|(node, _)| node.name.clone())
        .collect();
    assert_eq!(names, ["f0", "f1", "f303", "f2", "f3"]);
}

/// The search reads the edges of no more nodes than its budget, and says so
/// when it stops there.
#[tokio::test]
async fn the_budget_bounds_the_nodes_read() {
    let (_dir, cg, ids) = functions(6).await;
    let chain: Vec<Edge> = (0..5)
        .map(|i| edge(&ids, i, i + 1, EdgeKind::Calls))
        .collect();
    cg.db().insert_edges(&chain).await.unwrap();

    let spent = cg
        .get_connection_path(&ids[0], &ids[5], 8, 2)
        .await
        .unwrap();
    assert!(spent.path.is_none());
    assert!(spent.budget_exhausted);
    assert_eq!(spent.expanded, 2);

    let enough = cg
        .get_connection_path(&ids[0], &ids[5], 8, 100)
        .await
        .unwrap();
    assert_eq!(enough.path.expect("a path").len(), 6);
    assert!(!enough.budget_exhausted);
    assert!(enough.expanded <= 100);
}

/// Shortest distance in edges between every pair, ignoring edge direction:
/// the slow reference the search must agree with.
fn reference_distances(count: usize, edges: &[(usize, usize)]) -> Vec<Vec<Option<usize>>> {
    let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); count];
    for &(a, b) in edges {
        adjacent[a].push(b);
        adjacent[b].push(a);
    }
    (0..count)
        .map(|start| {
            let mut distance = vec![None; count];
            distance[start] = Some(0);
            let mut queue = VecDeque::from([start]);
            while let Some(current) = queue.pop_front() {
                let next = distance[current].map(|d| d + 1);
                for &neighbor in &adjacent[current] {
                    if distance[neighbor].is_none() {
                        distance[neighbor] = next;
                        queue.push_back(neighbor);
                    }
                }
            }
            distance
        })
        .collect()
}

/// On random graphs, the search finds a path exactly when the reference says
/// one exists within `max_depth`, the path has the reference length, and
/// each step's edge joins that step to the one before.
#[tokio::test]
async fn the_search_agrees_with_a_reference_search_on_random_graphs() {
    const COUNT: usize = 40;
    for seed in [1u64, 2, 3] {
        let (_dir, cg, ids) = functions(COUNT).await;
        // A fixed generator, so a failure names a graph that can be rebuilt.
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut next = |bound: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(state >> 33).unwrap() % bound
        };
        // Sparse on purpose: some pairs are far apart and some unconnected.
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        while pairs.len() < 45 {
            let (a, b) = (next(COUNT), next(COUNT));
            if a == b {
                continue;
            }
            let kind = if next(2) == 0 {
                EdgeKind::Calls
            } else {
                EdgeKind::Uses
            };
            pairs.push((a, b));
            edges.push(edge(&ids, a, b, kind));
        }
        cg.db().insert_edges(&edges).await.unwrap();
        let reference = reference_distances(COUNT, &pairs);
        let index_of: HashMap<&str, usize> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let joined: HashSet<(usize, usize)> =
            pairs.iter().flat_map(|&(a, b)| [(a, b), (b, a)]).collect();

        for _ in 0..120 {
            let (from, to) = (next(COUNT), next(COUNT));
            for max_depth in [2usize, 3, 8] {
                let search = cg
                    .get_connection_path(&ids[from], &ids[to], max_depth, usize::MAX)
                    .await
                    .unwrap();
                let context = format!("seed {seed}, f{from} -> f{to}, max_depth {max_depth}");
                let expected = reference[from][to].filter(|d| *d <= max_depth);
                let Some(expected) = expected else {
                    assert!(
                        search.path.is_none(),
                        "{context}: found a path, expected none"
                    );
                    continue;
                };
                let path = search
                    .path
                    .unwrap_or_else(|| panic!("{context}: no path, expected {expected} edges"));
                assert_eq!(path.len(), expected + 1, "{context}: not a shortest path");
                assert_eq!(path[0].0.id, ids[from], "{context}");
                assert_eq!(path[path.len() - 1].0.id, ids[to], "{context}");
                assert!(path[0].1.is_none(), "{context}");
                for step in 1..path.len() {
                    let before = index_of[path[step - 1].0.id.as_str()];
                    let here = index_of[path[step].0.id.as_str()];
                    assert!(joined.contains(&(before, here)), "{context}: step {step}");
                    let via = path[step].1.as_ref().expect("an edge");
                    let ends = [index_of[via.source.as_str()], index_of[via.target.as_str()]];
                    assert!(
                        ends == [before, here] || ends == [here, before],
                        "{context}: step {step} carries an edge of other nodes"
                    );
                }
            }
        }
    }
}
