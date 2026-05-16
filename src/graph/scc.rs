//! Tarjan's strongly-connected-components algorithm.
//!
//! Shared by `tokensave_circular` (file-level cycle grouping) and
//! `tokensave_port_order` (intra-cycle visibility). Both tools were
//! emitting either every walk through an SCC (`circular`'s 73-cycle
//! tail-overlap explosion) or a single flat blob of every cycle node
//! (`port_order`'s 200+ symbol mega-cycle). SCCs replace both with the
//! correct primitive: one component per mutually-recursive group.
//!
//! The implementation is iterative (no recursion) so deep graphs don't
//! blow the stack. SCCs are returned in reverse-topological order —
//! "leaves" (components with no outgoing inter-component edges) come
//! first, which is exactly the order needed for port ranking.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// Computes the strongly-connected components of the directed graph
/// described by `adj`. Every node that appears as a key OR as a value
/// becomes part of exactly one SCC in the result.
///
/// SCCs are emitted in reverse-topological order over the condensation:
/// if SCC `A` depends on SCC `B`, `B` appears in the result before `A`.
/// This matches Tarjan's natural emission order.
#[allow(clippy::implicit_hasher)]
pub fn tarjan_scc<N>(adj: &HashMap<N, HashSet<N>>) -> Vec<Vec<N>>
where
    N: Eq + Hash + Clone,
{
    // Gather every node mentioned, sources or targets, so unreachable
    // nodes still appear as singleton SCCs.
    let mut all_nodes: Vec<N> = Vec::new();
    let mut seen_nodes: HashSet<N> = HashSet::new();
    for (src, targets) in adj {
        if seen_nodes.insert(src.clone()) {
            all_nodes.push(src.clone());
        }
        for t in targets {
            if seen_nodes.insert(t.clone()) {
                all_nodes.push(t.clone());
            }
        }
    }
    drop(seen_nodes);

    let mut index_of: HashMap<N, usize> = HashMap::with_capacity(all_nodes.len());
    let mut lowlink: HashMap<N, usize> = HashMap::with_capacity(all_nodes.len());
    let mut on_stack: HashSet<N> = HashSet::new();
    let mut stack: Vec<N> = Vec::new();
    let mut sccs: Vec<Vec<N>> = Vec::new();
    let mut next_index: usize = 0;

    // Iterative DFS using an explicit call stack of (node, neighbor_iter, neighbor_index).
    // After visiting each neighbor we may need to update lowlink, so each
    // frame remembers its own progress through its neighbor list.
    for root in &all_nodes {
        if index_of.contains_key(root) {
            continue;
        }
        // Frame: (node, neighbors snapshot, current neighbor index).
        let mut work: Vec<(N, Vec<N>, usize)> = Vec::new();
        let root_neighbors: Vec<N> = adj
            .get(root)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        index_of.insert(root.clone(), next_index);
        lowlink.insert(root.clone(), next_index);
        next_index += 1;
        stack.push(root.clone());
        on_stack.insert(root.clone());
        work.push((root.clone(), root_neighbors, 0));

        // Peek the top frame each iteration instead of cloning the whole
        // tuple — earlier revisions used `work.last_mut().cloned()` which
        // deep-copied the entire neighbor `Vec<N>` once per edge visited.
        // The `while let` rewrite suggested by clippy doesn't apply: the
        // body needs to `work.push(...)` while `frame` is still borrowed,
        // which only works because NLL drops the `frame` reborrow early —
        // a `while let` binding would keep it alive for the whole body.
        #[allow(clippy::while_let_loop)]
        loop {
            let Some(frame) = work.last_mut() else { break };
            let idx = frame.2;
            if idx >= frame.1.len() {
                // Finished this node — pop frame, update parent's lowlink,
                // and emit an SCC if this node is a Tarjan root.
                let popped = work
                    .pop()
                    .unwrap_or_else(|| unreachable!("work stack non-empty"));
                let node = popped.0;
                let node_ll = *lowlink.get(&node).unwrap_or(&0);
                let node_idx = *index_of.get(&node).unwrap_or(&0);
                if node_ll == node_idx {
                    let mut component: Vec<N> = Vec::new();
                    while let Some(top) = stack.pop() {
                        on_stack.remove(&top);
                        let is_root = top == node;
                        component.push(top);
                        if is_root {
                            break;
                        }
                    }
                    sccs.push(component);
                }
                if let Some(parent_frame) = work.last() {
                    let parent_ll = *lowlink.get(&parent_frame.0).unwrap_or(&0);
                    lowlink.insert(parent_frame.0.clone(), parent_ll.min(node_ll));
                }
                continue;
            }

            // Clone only the two values we actually need before mutating
            // `work` (push invalidates `frame`).
            let next = frame.1[idx].clone();
            let node = frame.0.clone();
            frame.2 += 1;

            if let Some(&next_index_val) = index_of.get(&next) {
                if on_stack.contains(&next) {
                    let node_ll = *lowlink.get(&node).unwrap_or(&0);
                    lowlink.insert(node, node_ll.min(next_index_val));
                }
            } else {
                let child_neighbors: Vec<N> = adj
                    .get(&next)
                    .map(|s| s.iter().cloned().collect())
                    .unwrap_or_default();
                index_of.insert(next.clone(), next_index);
                lowlink.insert(next.clone(), next_index);
                next_index += 1;
                stack.push(next.clone());
                on_stack.insert(next.clone());
                work.push((next, child_neighbors, 0));
            }
        }
    }

    sccs
}

/// True when an SCC represents a genuine cycle. A single-node component
/// is only cyclic if it has an explicit self-edge in `adj`; otherwise
/// it's just an isolated vertex. Components of size >= 2 are always
/// cyclic by Tarjan's definition.
#[allow(clippy::implicit_hasher)]
pub fn is_cyclic_scc<N>(scc: &[N], adj: &HashMap<N, HashSet<N>>) -> bool
where
    N: Eq + Hash,
{
    if scc.len() >= 2 {
        return true;
    }
    if let Some(only) = scc.first() {
        if let Some(neighbors) = adj.get(only) {
            return neighbors.contains(only);
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn edge<N: Eq + Hash + Clone>(adj: &mut HashMap<N, HashSet<N>>, from: N, to: N) {
        adj.entry(from).or_default().insert(to);
    }

    #[test]
    fn dag_has_only_trivial_sccs() {
        let mut adj: HashMap<&str, HashSet<&str>> = HashMap::new();
        edge(&mut adj, "a", "b");
        edge(&mut adj, "b", "c");
        let sccs = tarjan_scc(&adj);
        assert_eq!(sccs.len(), 3);
        for s in &sccs {
            assert_eq!(s.len(), 1);
            assert!(!is_cyclic_scc(s, &adj));
        }
    }

    #[test]
    fn detects_two_node_cycle() {
        let mut adj: HashMap<&str, HashSet<&str>> = HashMap::new();
        edge(&mut adj, "a", "b");
        edge(&mut adj, "b", "a");
        let sccs = tarjan_scc(&adj);
        let cyclic: Vec<_> = sccs.iter().filter(|s| is_cyclic_scc(s, &adj)).collect();
        assert_eq!(cyclic.len(), 1);
        assert_eq!(cyclic[0].len(), 2);
    }

    #[test]
    fn detects_three_node_cycle_plus_tail() {
        let mut adj: HashMap<&str, HashSet<&str>> = HashMap::new();
        edge(&mut adj, "a", "b");
        edge(&mut adj, "b", "c");
        edge(&mut adj, "c", "a");
        edge(&mut adj, "c", "d");
        edge(&mut adj, "d", "e");
        let sccs = tarjan_scc(&adj);
        assert_eq!(sccs.len(), 3, "[abc] + [d] + [e]");
        let cyclic: Vec<_> = sccs.iter().filter(|s| is_cyclic_scc(s, &adj)).collect();
        assert_eq!(cyclic.len(), 1);
        let mut sorted = cyclic[0].clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec!["a", "b", "c"]);
    }

    #[test]
    fn self_loop_classified_as_cyclic() {
        let mut adj: HashMap<&str, HashSet<&str>> = HashMap::new();
        edge(&mut adj, "a", "a");
        edge(&mut adj, "a", "b");
        let sccs = tarjan_scc(&adj);
        let a_scc = sccs.iter().find(|s| s.contains(&"a")).unwrap();
        assert!(is_cyclic_scc(a_scc, &adj));
    }

    #[test]
    fn reverse_topological_order() {
        // a -> b -> c. Tarjan emits in reverse-topo: leaves first.
        let mut adj: HashMap<&str, HashSet<&str>> = HashMap::new();
        edge(&mut adj, "a", "b");
        edge(&mut adj, "b", "c");
        let sccs = tarjan_scc(&adj);
        let order: Vec<&str> = sccs.iter().map(|s| s[0]).collect();
        let pos_a = order.iter().position(|n| *n == "a").unwrap();
        let pos_c = order.iter().position(|n| *n == "c").unwrap();
        assert!(
            pos_c < pos_a,
            "c (leaf) should come before a (root); got {order:?}"
        );
    }
}
