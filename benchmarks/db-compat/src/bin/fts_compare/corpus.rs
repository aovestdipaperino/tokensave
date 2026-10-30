//! Source records shared by both engines.
//!
//! A corpus is exported once from a real tokensave database (`export`) into a
//! JSONL file; every adapter then loads the same records in the same order.
//! The only synthetic parts are memory decisions (tokensave has no corpus of
//! real ones) and the incremental edit plan, both derived with a fixed seed.

use std::collections::BTreeMap;
use std::io::{BufRead, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::db::{Db, Val};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeRec {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub docstring: Option<String>,
    pub signature: Option<String>,
    pub search_terms: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BodyRec {
    pub node_id: String,
    pub file_path: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub text: String,
    pub reason: Option<String>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum Line {
    Node(NodeRec),
    Body(BodyRec),
}

pub struct Corpus {
    pub name: String,
    pub nodes: Vec<NodeRec>,
    pub bodies: Vec<BodyRec>,
    pub decisions: Vec<Decision>,
}

/// Deterministic generator (SplitMix64); no external RNG so the stream is
/// identical across platforms and dependency updates.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

pub const SEED: u64 = 0x7015_E5A7_EF75;

/// Copies a tokensave database (so the original is never opened for write)
/// and writes its FTS-relevant node fields and executable bodies as JSONL.
pub async fn export(db_path: &Path, out: &Path) -> Result<(usize, usize), String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let copy = dir.path().join("source.db");
    std::fs::copy(db_path, &copy).map_err(|e| format!("copy {}: {e}", db_path.display()))?;
    for suffix in ["-wal", "-shm"] {
        let side = db_path.with_file_name(format!(
            "{}{suffix}",
            db_path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
        ));
        if side.exists() {
            let dest = dir.path().join(format!("source.db{suffix}"));
            std::fs::copy(&side, dest).map_err(|e| e.to_string())?;
        }
    }
    let db = Db::open_libsql(&copy).await?;
    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut w = BufWriter::new(file);
    let nodes = db
        .query(
            "SELECT id, kind, name, qualified_name, file_path, docstring, signature, \
             COALESCE(search_terms, '') FROM nodes ORDER BY id",
            &[],
        )
        .await?;
    for r in &nodes {
        let opt = |v: &Val| match v {
            Val::Null => None,
            other => Some(other.text()),
        };
        let rec = NodeRec {
            id: r[0].text(),
            kind: r[1].text(),
            name: r[2].text(),
            qualified_name: r[3].text(),
            file_path: r[4].text(),
            docstring: opt(&r[5]),
            signature: opt(&r[6]),
            search_terms: r[7].text(),
        };
        write_line(&mut w, &Line::Node(rec))?;
    }
    let bodies = db
        .query(
            "SELECT node_id, file_path, body FROM executable_body_fts ORDER BY node_id, file_path, body",
            &[],
        )
        .await?;
    for r in &bodies {
        let rec = BodyRec {
            node_id: r[0].text(),
            file_path: r[1].text(),
            body: r[2].text(),
        };
        write_line(&mut w, &Line::Body(rec))?;
    }
    w.flush().map_err(|e| e.to_string())?;
    Ok((nodes.len(), bodies.len()))
}

fn write_line(w: &mut impl Write, line: &Line) -> Result<(), String> {
    serde_json::to_writer(&mut *w, line).map_err(|e| e.to_string())?;
    w.write_all(b"\n").map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Result<Corpus, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut nodes = Vec::new();
    let mut bodies = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        match serde_json::from_str::<Line>(&line).map_err(|e| e.to_string())? {
            Line::Node(n) => nodes.push(n),
            Line::Body(b) => bodies.push(b),
        }
    }
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("corpus")
        .to_string();
    let decisions = synth_decisions(&nodes, SEED);
    Ok(Corpus {
        name,
        nodes,
        bodies,
        decisions,
    })
}

/// Synthetic session-memory decisions built from the corpus vocabulary.
/// `created_at` is shuffled relative to insertion order so a query that
/// orders by rowid instead of `created_at` is detectable.
pub fn synth_decisions(nodes: &[NodeRec], seed: u64) -> Vec<Decision> {
    let mut rng = Rng::new(seed ^ 0xDEC1_5104);
    let documented: Vec<&NodeRec> = nodes
        .iter()
        .filter(|n| n.docstring.as_deref().is_some_and(|d| d.len() > 20))
        .collect();
    if documented.is_empty() {
        return Vec::new();
    }
    let count = (nodes.len() / 20).clamp(50, 2000);
    let mut stamps: Vec<i64> = (0..count as i64).map(|i| 1_700_000_000 + i * 97).collect();
    rng.shuffle(&mut stamps);
    let verbs = ["Use", "Avoid", "Prefer", "Keep", "Replace", "Document"];
    (0..count)
        .map(|i| {
            let n = documented[rng.below(documented.len())];
            let doc = n.docstring.as_deref().unwrap_or_default();
            let excerpt: String = doc
                .split_whitespace()
                .take(12)
                .collect::<Vec<_>>()
                .join(" ");
            Decision {
                text: format!(
                    "{} {} in {} for {}",
                    verbs[rng.below(verbs.len())],
                    n.name,
                    n.file_path,
                    excerpt
                ),
                reason: (rng.below(4) != 0).then(|| format!("Because {excerpt}")),
                created_at: stamps[i],
            }
        })
        .collect()
}

/// One incremental sync step: replace every node and body of `file`.
pub struct FileEdit {
    pub file_path: String,
    pub nodes: Vec<NodeRec>,
    pub bodies: Vec<BodyRec>,
}

/// Picks `files` source files with a fixed seed and rewrites them the way a
/// sync after an edit would: some symbols renamed, some docstrings changed,
/// one symbol added. Returns the edits in application order.
pub fn edit_plan(corpus: &Corpus, files: usize, seed: u64) -> Vec<FileEdit> {
    let mut by_file: BTreeMap<&str, Vec<&NodeRec>> = BTreeMap::new();
    for n in &corpus.nodes {
        by_file.entry(n.file_path.as_str()).or_default().push(n);
    }
    let mut candidates: Vec<&str> = by_file
        .iter()
        .filter(|(_, ns)| ns.len() >= 3)
        .map(|(f, _)| *f)
        .collect();
    let mut rng = Rng::new(seed ^ 0xED17);
    rng.shuffle(&mut candidates);
    candidates.truncate(files);
    candidates
        .into_iter()
        .map(|file| {
            let mut nodes: Vec<NodeRec> = by_file[file].iter().map(|n| (*n).clone()).collect();
            for n in &mut nodes {
                match rng.below(10) {
                    0 | 1 => {
                        n.name = format!("{}_revised", n.name);
                        n.qualified_name = format!("{}_revised", n.qualified_name);
                    }
                    2 | 3 => {
                        n.docstring = Some(format!(
                            "{} Revised during benchmark edit.",
                            n.docstring.clone().unwrap_or_default()
                        ));
                    }
                    _ => {}
                }
            }
            let mut added = nodes[0].clone();
            added.id = format!("{}#bench_added", added.id);
            added.name = "bench_added_symbol".into();
            added.qualified_name = format!("{file}::bench_added_symbol");
            added.docstring = Some("Symbol added by the incremental benchmark.".into());
            nodes.push(added);
            let bodies = corpus
                .bodies
                .iter()
                .filter(|b| b.file_path == file)
                .cloned()
                .collect();
            FileEdit {
                file_path: file.to_string(),
                nodes,
                bodies,
            }
        })
        .collect()
}
