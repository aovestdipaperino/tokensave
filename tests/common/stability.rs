//! Small-fixture metamorphic comparisons through the production APIs.
use std::collections::{BTreeMap, HashMap};
use tempfile::TempDir;
use tokensave::{
    graph::queries::GraphQueryManager,
    tokensave::TokenSave,
    types::{Node, NodeKind},
};

pub type Entity = (String, String, String);
pub fn entity(n: &Node) -> Entity {
    (
        n.file_path.clone(),
        n.kind.as_str().to_owned(),
        n.qualified_name.clone(),
    )
}

/// Explicit, one-to-one correspondence, never fuzzy matching or global text replacement.
#[derive(Default)]
pub struct Correspondence {
    pub entities: BTreeMap<Entity, Entity>,
    pub files: BTreeMap<String, String>,
    pub signatures: BTreeMap<Entity, Option<String>>,
}
impl Correspondence {
    fn entity(&self, n: &Node) -> Entity {
        self.entities
            .get(&entity(n))
            .cloned()
            .unwrap_or_else(|| entity(n))
    }
    fn file(&self, path: String) -> String {
        self.files.get(&path).cloned().unwrap_or(path)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    // Sorted vectors deliberately retain duplicate findings/edges.
    pub analyses: BTreeMap<&'static str, Vec<String>>,
    pub identities: BTreeMap<Entity, String>,
}
impl Snapshot {
    pub async fn capture(cg: &TokenSave, correspondence: &Correspondence) -> Self {
        let nodes = cg.get_all_nodes().await.unwrap();
        let by_id: HashMap<_, _> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let mut analyses = BTreeMap::new();
        let mut identities = BTreeMap::new();
        let mut symbols = Vec::new();
        let mut metrics = Vec::new();
        for n in nodes.iter().filter(|n| n.kind == NodeKind::Function) {
            let key = correspondence.entity(n);
            assert!(
                identities.insert(key.clone(), n.id.clone()).is_none(),
                "ambiguous fixture entity: {key:?}"
            );
            let signature = correspondence
                .signatures
                .get(&entity(n))
                .unwrap_or(&n.signature)
                .as_ref()
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "));
            symbols.push(format!(
                "{key:?} visibility={:?} async={} signature={signature:?}",
                n.visibility, n.is_async
            ));
            metrics.push(format!(
                "{key:?} {:?}",
                [
                    n.branches,
                    n.loops,
                    n.returns,
                    n.max_nesting,
                    n.unsafe_blocks,
                    n.unchecked_calls,
                    n.assertions,
                    n.cognitive_complexity,
                    n.distinct_operators,
                    n.distinct_operands,
                    n.total_operators,
                    n.total_operands
                ]
            ));
        }
        analyses.insert("symbols", symbols);
        analyses.insert("complexity", metrics);
        let dead = cg
            .find_dead_code(&[NodeKind::Function], true, false)
            .await
            .unwrap();
        analyses.insert(
            "dead_code",
            dead.iter()
                .map(|n| format!("{:?}", correspondence.entity(n)))
                .collect(),
        );
        let edges = cg.get_all_edges().await.unwrap();
        analyses.insert(
            "relationships",
            edges
                .iter()
                .map(|e| {
                    let source = by_id.get(e.source.as_str()).expect("dangling source");
                    let target = by_id.get(e.target.as_str()).expect("dangling target");
                    format!(
                        "{:?} -> {:?} kind={:?} resolved_by={:?}",
                        correspondence.entity(source),
                        correspondence.entity(target),
                        e.kind,
                        e.resolved_by
                    )
                })
                .collect(),
        );
        let qm = GraphQueryManager::new(cg.db());
        let cycles = qm.find_circular_dependencies().await.unwrap();
        analyses.insert(
            "circular_dependencies",
            cycles
                .into_iter()
                .map(|c| {
                    let mut files: Vec<_> = c.into_iter().map(|p| correspondence.file(p)).collect();
                    files.sort();
                    format!("{files:?}")
                })
                .collect(),
        );
        for (name, fan_in) in [("coupling_in", true), ("coupling_out", false)] {
            let coupling = cg.get_file_coupling(fan_in, None, 100).await.unwrap();
            assert!(coupling.len() < 100, "fixture exceeded capture limit");
            analyses.insert(
                name,
                coupling
                    .into_iter()
                    .map(|(p, count)| format!("{}: {count}", correspondence.file(p)))
                    .collect(),
            );
        }
        for values in analyses.values_mut() {
            values.sort();
        }
        Self {
            analyses,
            identities,
        }
    }

    pub fn assert_equivalent(&self, after: &Self, context: &str) {
        assert_eq!(
            self.analyses.keys().collect::<Vec<_>>(),
            after.analyses.keys().collect::<Vec<_>>()
        );
        for (analyzer, before) in &self.analyses {
            assert_eq!(before, &after.analyses[analyzer], "Analysis Stability Failure\nAnalyzer: {analyzer}\nFixture/transformation: {context}\nUnexpected semantic difference (before / after)");
        }
    }

    pub fn assert_same_identities(&self, after: &Self, context: &str) {
        assert_eq!(self.identities, after.identities, "Identity Stability Failure\n{context}\nExpected semantic entity -> raw ID correspondence (before / after)");
    }
}

/// Owns the database before the directory so connections close before cleanup.
pub struct Project {
    pub cg: TokenSave,
    pub dir: TempDir,
}
impl Project {
    pub async fn new(files: &[(&str, String)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        Self::write(dir.path(), files);
        let cg = TokenSave::init(dir.path()).await.unwrap();
        cg.index_all().await.unwrap();
        Self { cg, dir }
    }
    pub fn write(root: &std::path::Path, files: &[(&str, String)]) {
        for (path, source) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, source).unwrap();
        }
    }
    pub async fn snapshot(&self) -> Snapshot {
        Snapshot::capture(&self.cg, &Correspondence::default()).await
    }
}
