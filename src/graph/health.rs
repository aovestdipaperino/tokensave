//! Structural health analysis algorithms.
//!
//! Provides file-level DAG construction, Gini coefficient computation,
//! Tarjan's SCC-based acyclicity scoring, dependency depth analysis,
//! modularity estimation, and composite health scoring.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::BuildHasher;

// ---------------------------------------------------------------------------
// Task 2: Gini Coefficient
// ---------------------------------------------------------------------------

/// Computes the Gini coefficient for a slice of non-negative values.
/// Returns 0.0 for empty slices, single-element slices, or all-zero slices.
/// Result is in \[0.0, 1.0\] where 0.0 = perfect equality.
pub fn gini_coefficient(values: &[f64]) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }

    let sum: f64 = values.iter().sum();
    if sum == 0.0 {
        return 0.0;
    }

    let n = values.len() as f64;
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // G = (2 * Σ(i * x_i)) / (n * Σ(x_i)) - (n + 1) / n  (i is 1-indexed)
    let weighted_sum: f64 = sorted
        .iter()
        .enumerate()
        .map(|(idx, &x)| (idx as f64 + 1.0) * x)
        .sum();

    (2.0 * weighted_sum) / (n * sum) - (n + 1.0) / n
}

/// Returns a human-readable label for a Gini coefficient value.
/// - <0.20  → "low inequality (healthy)"
/// - <0.40  → "moderate inequality"
/// - <0.60  → "high inequality"
/// - >=0.60 → "extreme inequality (god files likely)"
pub fn gini_label(gini: f64) -> &'static str {
    if gini < 0.20 {
        "low inequality (healthy)"
    } else if gini < 0.40 {
        "moderate inequality"
    } else if gini < 0.60 {
        "high inequality"
    } else {
        "extreme inequality (god files likely)"
    }
}

// ---------------------------------------------------------------------------
// Task 3: Tarjan's SCC / Acyclicity Score
// ---------------------------------------------------------------------------

struct TarjanState<'a> {
    adj: &'a HashMap<String, HashSet<String>>,
    index_counter: usize,
    stack: Vec<String>,
    on_stack: HashSet<String>,
    index: HashMap<String, usize>,
    lowlink: HashMap<String, usize>,
    sccs: Vec<Vec<String>>,
}

impl<'a> TarjanState<'a> {
    fn new(adj: &'a HashMap<String, HashSet<String>>) -> Self {
        TarjanState {
            adj,
            index_counter: 0,
            stack: Vec::new(),
            on_stack: HashSet::new(),
            index: HashMap::new(),
            lowlink: HashMap::new(),
            sccs: Vec::new(),
        }
    }

    fn strongconnect(&mut self, v: &str) {
        let v = v.to_string();
        self.index.insert(v.clone(), self.index_counter);
        self.lowlink.insert(v.clone(), self.index_counter);
        self.index_counter += 1;
        self.stack.push(v.clone());
        self.on_stack.insert(v.clone());

        // Collect neighbors first to avoid borrow conflicts
        let neighbors: Vec<String> = self
            .adj
            .get(&v)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();

        for w in neighbors {
            if !self.index.contains_key(&w) {
                self.strongconnect(&w.clone());
                let w_low = self.lowlink[&w];
                let v_low = self.lowlink[&v];
                self.lowlink.insert(v.clone(), v_low.min(w_low));
            } else if self.on_stack.contains(&w) {
                let w_idx = self.index[&w];
                let v_low = self.lowlink[&v];
                self.lowlink.insert(v.clone(), v_low.min(w_idx));
            }
        }

        // If v is a root node, pop the stack and generate an SCC
        if self.lowlink[&v] == self.index[&v] {
            let mut scc = Vec::new();
            while let Some(w) = self.stack.pop() {
                self.on_stack.remove(&w);
                let is_v = w == v;
                scc.push(w);
                if is_v {
                    break;
                }
            }
            self.sccs.push(scc);
        }
    }
}

/// Runs Tarjan's SCC algorithm on the adjacency map.
/// Returns a list of strongly connected components (each is a list of node IDs).
fn tarjan_scc(adj: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    let mut state = TarjanState::new(adj);

    // Collect all nodes (both keys and targets) so isolated nodes are included
    let mut all_nodes: HashSet<String> = adj.keys().cloned().collect();
    for targets in adj.values() {
        all_nodes.extend(targets.iter().cloned());
    }

    for node in all_nodes {
        if !state.index.contains_key(&node) {
            state.strongconnect(&node);
        }
    }

    state.sccs
}

/// Computes the acyclicity score for a directed graph.
/// Uses Tarjan's SCC algorithm. Score = 1.0 - (`edges_in_nontrivial_SCCs` / `total_edges`).
/// Returns (score, `number_of_edges_in_cycles`).
pub fn acyclicity_score<S1: BuildHasher, S2: BuildHasher>(
    adj: &HashMap<String, HashSet<String, S2>, S1>,
) -> (f64, usize) {
    let total_edges: usize = adj.values().map(HashSet::len).sum();

    if total_edges == 0 {
        return (1.0, 0);
    }

    // Build a plain HashMap for tarjan_scc (which uses default hasher)
    let plain_adj: HashMap<String, HashSet<String>> = adj
        .iter()
        .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
        .collect();

    let sccs = tarjan_scc(&plain_adj);

    // Build a set of nodes in nontrivial SCCs (size > 1)
    let mut in_cycle: HashSet<&str> = HashSet::new();
    for scc in &sccs {
        if scc.len() > 1 {
            for node in scc {
                in_cycle.insert(node.as_str());
            }
        }
    }

    // Count edges where both endpoints are in nontrivial SCCs
    let edges_in_cycles: usize = adj
        .iter()
        .filter(|(src, _)| in_cycle.contains(src.as_str()))
        .map(|(_src, targets)| {
            targets
                .iter()
                .filter(|tgt| in_cycle.contains(tgt.as_str()))
                .count()
        })
        .sum();

    let score = 1.0 - (edges_in_cycles as f64 / total_edges as f64);
    (score, edges_in_cycles)
}

// ---------------------------------------------------------------------------
// Task 4: Dependency Depth
// ---------------------------------------------------------------------------

/// A chain entry representing a file and the longest dependency chain reaching it.
pub struct DepthChain {
    pub file: String,
    pub depth: usize,
    pub chain: Vec<String>,
}

/// Result of the dependency depth analysis.
pub struct DepthResult {
    pub max_depth: usize,
    /// `ceil(log2(file_count))`
    pub ideal_depth: usize,
    pub chains: Vec<DepthChain>,
}

/// Computes longest dependency chains. Breaks cycles via Tarjan's SCC
/// (collapses each SCC to a single node), then runs topo sort + DP.
pub fn dependency_depth<S1: BuildHasher, S2: BuildHasher>(
    adj: &HashMap<String, HashSet<String, S2>, S1>,
    limit: usize,
) -> DepthResult {
    // Collect all nodes
    let mut all_nodes: HashSet<String> = adj.keys().cloned().collect();
    for targets in adj.values() {
        all_nodes.extend(targets.iter().cloned());
    }
    let file_count = all_nodes.len();

    if file_count == 0 {
        return DepthResult {
            max_depth: 0,
            ideal_depth: 0,
            chains: Vec::new(),
        };
    }

    let ideal_depth = if file_count <= 1 {
        0
    } else {
        (file_count as f64).log2().ceil() as usize
    };

    // Build a plain HashMap for tarjan_scc
    let plain_adj: HashMap<String, HashSet<String>> = adj
        .iter()
        .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
        .collect();

    // Step 1: Run Tarjan's SCC, map each node to its SCC index
    let sccs = tarjan_scc(&plain_adj);
    let mut node_to_scc: HashMap<String, usize> = HashMap::new();
    for (idx, scc) in sccs.iter().enumerate() {
        for node in scc {
            node_to_scc.insert(node.clone(), idx);
        }
    }

    // Step 2: Build DAG over SCC indices
    let scc_count = sccs.len();
    let mut scc_adj: HashMap<usize, HashSet<usize>> = HashMap::new();
    for (src, targets) in adj {
        let src_scc = node_to_scc[src];
        for tgt in targets {
            let tgt_scc = node_to_scc[tgt];
            if src_scc != tgt_scc {
                scc_adj.entry(src_scc).or_default().insert(tgt_scc);
            }
        }
    }

    // Step 3: Kahn's algorithm for topological sort
    let mut in_degree = vec![0usize; scc_count];
    for targets in scc_adj.values() {
        for &tgt in targets {
            in_degree[tgt] += 1;
        }
    }

    let mut queue: VecDeque<usize> = (0..scc_count)
        .filter(|&i| in_degree[i] == 0)
        .collect();

    let mut topo_order: Vec<usize> = Vec::new();
    while let Some(node) = queue.pop_front() {
        topo_order.push(node);
        if let Some(neighbors) = scc_adj.get(&node) {
            for &nb in neighbors {
                in_degree[nb] -= 1;
                if in_degree[nb] == 0 {
                    queue.push_back(nb);
                }
            }
        }
    }

    // Step 4: DP for longest path with predecessor tracking
    let mut dist = vec![0usize; scc_count];
    let mut pred = vec![usize::MAX; scc_count];

    for &u in &topo_order {
        if let Some(neighbors) = scc_adj.get(&u) {
            for &v in neighbors {
                if dist[u] + 1 > dist[v] {
                    dist[v] = dist[u] + 1;
                    pred[v] = u;
                }
            }
        }
    }

    // Step 5: Reconstruct chains (use first node of each SCC as representative)
    let mut max_depth = 0;
    let mut results: Vec<DepthChain> = Vec::new();

    for scc_idx in 0..scc_count {
        let depth = dist[scc_idx];
        if depth > max_depth {
            max_depth = depth;
        }

        if results.len() < limit {
            // Reconstruct the chain by walking predecessors
            let mut chain_sccs: Vec<usize> = Vec::new();
            let mut cur = scc_idx;
            loop {
                chain_sccs.push(cur);
                let p = pred[cur];
                if p == usize::MAX {
                    break;
                }
                cur = p;
            }
            chain_sccs.reverse();

            // Map SCC indices back to representative file names
            let chain: Vec<String> = chain_sccs
                .iter()
                .map(|&si| sccs[si][0].clone())
                .collect();

            let representative = sccs[scc_idx][0].clone();
            results.push(DepthChain {
                file: representative,
                depth,
                chain,
            });
        }
    }

    // Sort by depth descending for convenience
    results.sort_by_key(|ch| std::cmp::Reverse(ch.depth));

    DepthResult {
        max_depth,
        ideal_depth,
        chains: results,
    }
}

/// Score = min(1.0, ideal\_depth / max\_depth). Shallower is better.
/// Returns 1.0 when `max_depth == 0`.
pub fn depth_score(max_depth: usize, ideal_depth: usize) -> f64 {
    if max_depth == 0 {
        return 1.0;
    }
    (ideal_depth as f64 / max_depth as f64).min(1.0)
}

// ---------------------------------------------------------------------------
// Task 5: Modularity Score
// ---------------------------------------------------------------------------

/// Estimates modularity by removing hub nodes and counting connected components.
/// Hub nodes = files with (fan\_in + fan\_out) > mean + 2\*stddev.
/// Score = 1.0 - (1.0 / component\_count), clamped to \[0, 1\].
/// Returns (score, component\_count\_after\_hub\_removal).
pub fn modularity_score<S1: BuildHasher, S2: BuildHasher>(
    adj: &HashMap<String, HashSet<String, S2>, S1>,
) -> (f64, usize) {
    if adj.is_empty() {
        return (1.0, 0);
    }

    // Collect all nodes
    let mut all_nodes: HashSet<String> = adj.keys().cloned().collect();
    for targets in adj.values() {
        all_nodes.extend(targets.iter().cloned());
    }

    if all_nodes.is_empty() {
        return (1.0, 0);
    }

    // Build undirected connectivity count per node (fan_in + fan_out)
    let mut connectivity: HashMap<&str, usize> = HashMap::new();
    for node in &all_nodes {
        connectivity.insert(node.as_str(), 0);
    }
    for (src, targets) in adj {
        *connectivity.entry(src.as_str()).or_insert(0) += targets.len();
        for tgt in targets {
            *connectivity.entry(tgt.as_str()).or_insert(0) += 1;
        }
    }

    // Compute mean and stddev
    let n = connectivity.len() as f64;
    let values: Vec<f64> = connectivity.values().map(|&v| v as f64).collect();
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
    let stddev = variance.sqrt();
    let threshold = mean + 2.0 * stddev;

    // Identify hub nodes
    let hubs: HashSet<&str> = connectivity
        .iter()
        .filter(|(_, &v)| v as f64 > threshold)
        .map(|(&k, _)| k)
        .collect();

    // Build undirected graph without hubs
    let non_hub_nodes: Vec<&str> = all_nodes
        .iter()
        .map(String::as_str)
        .filter(|n| !hubs.contains(n))
        .collect();

    if non_hub_nodes.is_empty() {
        return (1.0, 0);
    }

    let mut undirected: HashMap<&str, HashSet<&str>> = HashMap::new();
    for &node in &non_hub_nodes {
        undirected.entry(node).or_default();
    }
    for (src, targets) in adj {
        if hubs.contains(src.as_str()) {
            continue;
        }
        for tgt in targets {
            if hubs.contains(tgt.as_str()) {
                continue;
            }
            undirected
                .entry(src.as_str())
                .or_default()
                .insert(tgt.as_str());
            undirected
                .entry(tgt.as_str())
                .or_default()
                .insert(src.as_str());
        }
    }

    // Count connected components via BFS
    let mut visited: HashSet<&str> = HashSet::new();
    let mut components = 0;

    for &start in &non_hub_nodes {
        if visited.contains(start) {
            continue;
        }
        components += 1;
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited.insert(start);
        while let Some(curr) = queue.pop_front() {
            if let Some(neighbors) = undirected.get(curr) {
                for &nb in neighbors {
                    if !visited.contains(nb) {
                        visited.insert(nb);
                        queue.push_back(nb);
                    }
                }
            }
        }
    }

    let score = (1.0 - 1.0 / components as f64).clamp(0.0, 1.0);
    (score, components)
}

// ---------------------------------------------------------------------------
// Task 6: Composite Health Score
// ---------------------------------------------------------------------------

/// All five health dimensions, each in \[0.0, 1.0\].
#[derive(Debug, Clone)]
pub struct HealthDimensions {
    pub acyclicity: f64,
    pub depth: f64,
    pub equality: f64,
    pub redundancy: f64,
    pub modularity: f64,
}

/// Computes quality signal (0–10000) from geometric mean of all five dimensions.
/// Formula: `(product of all 5).powf(1.0/5.0) * 10000.0`, rounded.
/// Zero in any dimension → 0.
pub fn compute_composite_health(dims: &HealthDimensions) -> u32 {
    let product =
        dims.acyclicity * dims.depth * dims.equality * dims.redundancy * dims.modularity;

    if product <= 0.0 {
        return 0;
    }

    (product.powf(1.0 / 5.0) * 10_000.0).round() as u32
}
