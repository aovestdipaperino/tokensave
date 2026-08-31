/// Reference resolution module.
///
/// Resolves unresolved references (from tree-sitter extraction) into concrete
/// edges by matching them against known nodes in the database.
mod resolver;
mod touched;
mod variants;

pub use resolver::{simple_ref_name, ReferenceResolver};
pub use touched::{index_keys_for_test, AmbiguityRefKey, TouchedNode, TouchedSet};
pub use variants::{
    emit_variant_edges, propagate_variant_edges, variant_groups_from_candidates,
    CALLABLE_KIND_NAMES,
};
