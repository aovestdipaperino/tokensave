/// Reference resolution module.
///
/// Resolves unresolved references (from tree-sitter extraction) into concrete
/// edges by matching them against known nodes in the database.
mod js_specifier;
mod resolver;
mod touched;
mod variants;

pub use js_specifier::relative_module_candidates;
pub use resolver::{
    csharp_type_name, has_typed_receiver_refs, is_csharp, is_gdscript, simple_ref_name,
    ReferenceResolver,
};
pub use touched::{index_keys_for_test, AmbiguityRefKey, TouchedNode, TouchedSet};
pub use variants::{
    emit_variant_edges, propagate_variant_edges, variant_groups_from_candidates,
    CALLABLE_KIND_NAMES,
};
