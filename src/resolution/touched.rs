//! Which reference names a sync could have changed the answer for (#484).
//!
//! A reference's resolution depends on exactly three things: the reference
//! itself, the candidate set for its name, and the scoring inputs. The scoring
//! inputs are all properties either of the reference's own file or of the
//! candidate nodes, so they are covered by the first two. Therefore the only
//! references whose outcome a sync can change are:
//!
//! ```text
//! refs from files re-extracted this sync
//!   ∪  refs whose name (or trailing simple name) is a key of the resolver's
//!      name index for some node inserted or deleted this sync
//! ```
//!
//! Everything else provably resolves exactly as it did last sync, and its edge
//! is already in the table. On this repository a one-line comment edit used to
//! re-attempt all 189,757 references; this narrows that to the handful the
//! edit could reach.
//!
//! ## Why deletions count, not just insertions
//!
//! Re-extracting a file deletes its nodes and reinserts them, and
//! `delete_nodes_by_file` deletes every edge touching those ids — including
//! edges pointing *into* the file from elsewhere. Those edges come from
//! references in other files whose name matches a node in the re-extracted
//! file, which is exactly what the touched-name set catches. Dropping the
//! deleted half is the way this change silently loses edges, so both halves
//! are recorded and `tests/incremental_resolution_test.rs` pins it.

use std::collections::HashSet;

use crate::types::{Node, NodeKind};

/// The identity of one reference, for scoping the ambiguity replacement to the
/// references a pass actually re-resolved (#484 phase 3).
///
/// The same four columns the `ambiguous_calls` insert keys on, so a delete by
/// this key removes exactly the row a re-resolution of that reference would
/// overwrite — and removes it even when the reference no longer resolves
/// ambiguously, which is the case a file-scoped delete gets right and a
/// missing delete gets wrong.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AmbiguityRefKey {
    pub from_node_id: String,
    pub reference_name: String,
    pub file_path: String,
    pub line: u32,
}

impl From<&crate::types::UnresolvedRef> for AmbiguityRefKey {
    fn from(uref: &crate::types::UnresolvedRef) -> Self {
        Self {
            from_node_id: uref.from_node_id.clone(),
            reference_name: uref.reference_name.clone(),
            file_path: uref.file_path.clone(),
            line: uref.line,
        }
    }
}

/// The identifying fields of a node, as much of them as the touched-name set
/// needs. Lets a caller record a node it is about to delete without loading
/// the whole row.
#[derive(Debug, Clone)]
pub struct TouchedNode {
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
}

impl From<&Node> for TouchedNode {
    fn from(node: &Node) -> Self {
        Self {
            kind: node.kind.clone(),
            name: node.name.clone(),
            qualified_name: node.qualified_name.clone(),
        }
    }
}

/// Every key under which a node is reachable in the resolver's name index.
///
/// This must mirror `ReferenceResolver::from_nodes`: `name_cache` keyed by
/// `name`, `qualified_name_cache` keyed by `qualified_name`, and `suffix_cache`
/// keyed by each `::` suffix of the qualified name — the three sets whose union
/// is `known_names`, which is what the resolver pre-filters on. `Use` nodes are
/// skipped there and so are skipped here.
///
/// `tests/incremental_resolution_test.rs` asserts that the union of these keys
/// over a node set equals that set's `known_names`, so the two cannot drift
/// apart silently.
pub fn index_keys(node: &TouchedNode, out: &mut HashSet<String>) {
    if node.kind == NodeKind::Use {
        return;
    }
    out.insert(node.name.clone());
    let qn = node.qualified_name.as_str();
    out.insert(qn.to_string());
    let mut pos = 0;
    while let Some(idx) = qn[pos..].find("::") {
        let suffix = &qn[pos + idx + 2..];
        if !suffix.is_empty() {
            out.insert(suffix.to_string());
        }
        pos += idx + 2;
    }
}

/// The invalidation set for one sync: which files were re-extracted, and which
/// name-index keys gained or lost a node.
#[derive(Debug, Default)]
pub struct TouchedSet {
    files: HashSet<String>,
    names: HashSet<String>,
}

impl TouchedSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a file as re-extracted. Every reference originating in it is
    /// re-attempted, since the references themselves may have moved or changed.
    pub fn touch_file(&mut self, path: &str) {
        self.files.insert(path.to_string());
    }

    /// Records nodes that were inserted or deleted this sync.
    pub fn touch_nodes<'n>(&mut self, nodes: impl IntoIterator<Item = &'n TouchedNode>) {
        for node in nodes {
            index_keys(node, &mut self.names);
        }
    }

    /// Whether this reference must be re-attempted.
    ///
    /// The name test mirrors the resolver's own pre-filter — literal name, then
    /// trailing simple name — because a qualified ref such as `Self::method`
    /// reaches its candidates through the simple name, not verbatim (#141).
    pub fn needs_resolve(&self, file_path: &str, reference_name: &str) -> bool {
        self.files.contains(file_path)
            || self.names.contains(reference_name)
            || self.names.contains(super::simple_ref_name(reference_name))
    }

    /// The files whose references were re-extracted this sync.
    pub fn files(&self) -> &HashSet<String> {
        &self.files
    }

    pub fn name_count(&self) -> usize {
        self.names.len()
    }
}

/// Every name-index key the touched set derives for a node slice.
///
/// Exists for `tests/incremental_resolution_test.rs`, which asserts this equals
/// the resolver's real `known_names` over the same nodes. That equality is the
/// whole safety argument for #484: if the resolver admits a name the touched set
/// never produces, a sync touching that node skips references that could now
/// resolve, and the edge silently disappears.
pub fn index_keys_for_test(nodes: &[Node]) -> HashSet<String> {
    let mut out = HashSet::new();
    for node in nodes {
        index_keys(&TouchedNode::from(node), &mut out);
    }
    out
}
