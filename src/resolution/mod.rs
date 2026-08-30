/// Reference resolution module.
///
/// Resolves unresolved references (from tree-sitter extraction) into concrete
/// edges by matching them against known nodes in the database.
mod resolver;
mod variants;

pub use resolver::ReferenceResolver;
pub use variants::{
    emit_variant_edges, propagate_variant_edges, variant_groups_from_candidates,
    CALLABLE_KIND_NAMES,
};
