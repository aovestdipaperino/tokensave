/// Reference resolution module.
///
/// Resolves unresolved references (from tree-sitter extraction) into concrete
/// edges by matching them against known nodes in the database.
mod resolver;
mod variants;

pub use resolver::ReferenceResolver;
pub use variants::propagate_variant_edges;
