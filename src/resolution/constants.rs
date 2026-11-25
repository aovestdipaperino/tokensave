//! Named values shared by the reference resolver.

/// A resolution below this confidence is not turned into an edge.
pub(super) const CONFIDENCE_FLOOR: f64 = 0.6;

/// Confidence of a winner picked by scoring among several same-named
/// candidates (`exact-match-scored` and the other `-scored` tags).
///
/// Kept below the exact strategies (0.85 and above) because scoring has no
/// type information for a receiver: on `serde_json` and `toml_edit` its winners
/// were right in 33% and 51% of the cases SCIP could judge (#1109).
pub(super) const SCORED_MATCH_CONFIDENCE: f64 = 0.65;

/// Confidence of a blocklisted common name bound to the one same-file
/// candidate (`same-file-blocklist`).
///
/// Measured at 5% and 7% precision before #1109 restricted it. It stays at the
/// scored level, above [`CONFIDENCE_FLOOR`], so the edges it keeps are not
/// dropped but rank below the exact strategies.
pub(super) const SAME_FILE_BLOCKLIST_CONFIDENCE: f64 = 0.65;

/// Receiver names that denote the object whose method is running.
///
/// A dotted call on one of these may be real recursion (`self.clear()` inside
/// `clear`); a call on any other receiver is a call on some other object.
pub(super) const SELF_RECEIVERS: &[&str] = &["self", "this", "Self", "cls", "base", "super"];

/// Leading `::` path segments that name a crate or module position rather
/// than a module, so they carry no evidence about the target's location.
pub(super) const RELATIVE_PATH_SEGMENTS: &[&str] = &["crate", "self", "super", "Self"];

/// File stems that stand for their directory (`foo/mod.rs` is module `foo`).
pub(super) const DIRECTORY_MODULE_STEMS: &[&str] = &["mod", "lib", "main", "index", "__init__"];

/// Rust path roots that name the standard library, whose items have no node
/// in a project graph.
pub(super) const RUST_EXTERNAL_ROOTS: &[&str] = &["std", "core", "alloc"];

/// How many enclosing nodes a scope check walks up from a candidate. Bounds
/// the walk on a malformed or cyclic parent chain; real nesting is shallow.
pub(super) const MAX_SCOPE_OWNER_DEPTH: usize = 16;

/// The receiver segment `Self` in a `Self::Name` path.
pub(super) const SELF_TYPE: &str = "Self";
