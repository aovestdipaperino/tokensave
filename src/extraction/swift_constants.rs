//! Named constants for Swift AST extraction, type inference, and complexity metrics.

/// Swift literal keywords ignored when checking for higher-order function references.
pub(super) const SWIFT_LITERAL_IDENTIFIERS: &[&str] =
    &["true", "false", "nil", "self", "Self", "super"];

/// Swift collection and concurrency higher-order methods whose bare identifier
/// arguments commonly pass function references without an enclosing type.
pub(super) const SWIFT_HIGHER_ORDER_CALLEES: &[&str] = &[
    "allSatisfy",
    "async",
    "compactMap",
    "compactMapValues",
    "contains",
    "drop",
    "filter",
    "first",
    "flatMap",
    "forEach",
    "last",
    "map",
    "mapValues",
    "max",
    "min",
    "partition",
    "prefix",
    "reduce",
    "removeAll",
    "sorted",
    "sync",
];

/// Swift argument labels that canonically accept function/closure references.
pub(super) const SWIFT_CALLBACK_ARG_LABELS: &[&str] = &[
    "action",
    "by",
    "callback",
    "completion",
    "execute",
    "handler",
    "onAppear",
    "onChange",
    "onCommit",
    "onDisappear",
    "perform",
    "predicate",
    "transform",
    "where",
];

/// Maximum characters retained in a compact Swift declaration signature.
pub(super) const MAX_DECL_SIGNATURE_CHARS: usize = 300;

/// Selector label recorded for a trailing-closure argument (`load(id: 1) { }`
/// records `load(id:0:)`). A Swift argument label cannot start with a digit,
/// so the marker never collides with a real label; the resolver lets it match
/// whichever parameter comes next.
pub(crate) const SWIFT_TRAILING_CLOSURE_LABEL: &str = "0";
