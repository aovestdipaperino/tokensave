//! Shared utilities.
use super::*;

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Search-result rank bonus applied per node kind, so symbol *definitions*
/// outrank mere *references* (use statements, annotation usages, modules)
/// that BM25 may otherwise score equally. Tuned so a definition with a
/// slightly worse BM25 score still surfaces above its imports.
///
/// Exhaustive match by design: when a new `NodeKind` variant is added the
/// compiler will force a re-tune here rather than silently defaulting it to
/// `0.0`, matching the project rule "crash hard if there is an unknown
/// value".
/// Coarse ranking tier used as the primary sort key in `search`. Lower
/// numbers sort first. The tiers separate "real definitions" (functions,
/// types, traits, …) from "references" (`use`, `module`, annotation usage)
/// so a re-export can never beat the thing it re-exports, no matter what
/// BM25 produces for the row.
pub(crate) fn kind_tier(kind: &NodeKind) -> u8 {
    match kind {
        // Tier 0: callable definitions and type definitions — the
        // "what is this?" answers a user usually wants when searching by
        // symbol name.
        NodeKind::Function
        | NodeKind::Method
        | NodeKind::StructMethod
        | NodeKind::Constructor
        | NodeKind::AbstractMethod
        | NodeKind::ArrowFunction
        | NodeKind::Procedure
        | NodeKind::Struct
        | NodeKind::Enum
        | NodeKind::Trait
        | NodeKind::Class
        | NodeKind::InnerClass
        | NodeKind::Interface
        | NodeKind::InterfaceType
        | NodeKind::Record
        | NodeKind::CaseClass
        | NodeKind::DataClass
        | NodeKind::SealedClass
        | NodeKind::TypeAlias
        | NodeKind::Union
        | NodeKind::Typedef
        | NodeKind::Mixin
        | NodeKind::Extension
        | NodeKind::Delegate
        | NodeKind::Template
        | NodeKind::PascalRecord
        | NodeKind::ScalaObject
        | NodeKind::KotlinObject
        | NodeKind::CompanionObject
        | NodeKind::Annotation
        | NodeKind::Event => 0,
        // Proto definitions (feature-gated)
        #[cfg(feature = "lang-protobuf")]
        NodeKind::ProtoMessage | NodeKind::ProtoService | NodeKind::ProtoRpc => 0,
        // Tier 1: impl blocks — between definitions and references.
        NodeKind::Impl => 1,
        // Tier 2: values, macros, members of types.
        NodeKind::Const
        | NodeKind::Static
        | NodeKind::Macro
        | NodeKind::PreprocessorDef
        | NodeKind::EnumVariant
        | NodeKind::Field
        | NodeKind::ValField
        | NodeKind::VarField
        | NodeKind::Property
        | NodeKind::CSharpProperty
        | NodeKind::StructTag
        | NodeKind::InitBlock
        | NodeKind::Export => 2,
        // Tier 3: containers (module, namespace, …) — usually not the
        // answer to "find symbol".
        NodeKind::Module
        | NodeKind::Package
        | NodeKind::Namespace
        | NodeKind::ScalaPackage
        | NodeKind::GoPackage
        | NodeKind::KotlinPackage
        | NodeKind::PascalUnit
        | NodeKind::Library
        | NodeKind::File
        | NodeKind::GenericParam
        | NodeKind::PascalProgram => 3,
        // Tier 4: pure references / annotations — always rank last.
        NodeKind::Use | NodeKind::Include | NodeKind::AnnotationUsage | NodeKind::Decorator => 4,
    }
}

pub(crate) fn kind_rank_bonus(kind: &NodeKind) -> f64 {
    match kind {
        // Callable definitions
        NodeKind::Function
        | NodeKind::Method
        | NodeKind::StructMethod
        | NodeKind::Constructor
        | NodeKind::AbstractMethod
        | NodeKind::ArrowFunction
        | NodeKind::Procedure => 3.0,
        // Type definitions
        NodeKind::Struct
        | NodeKind::Enum
        | NodeKind::Trait
        | NodeKind::Class
        | NodeKind::InnerClass
        | NodeKind::Interface
        | NodeKind::InterfaceType
        | NodeKind::Record
        | NodeKind::CaseClass
        | NodeKind::DataClass
        | NodeKind::SealedClass
        | NodeKind::TypeAlias
        | NodeKind::Union
        | NodeKind::Typedef
        | NodeKind::Mixin
        | NodeKind::Extension
        | NodeKind::Delegate
        | NodeKind::Template
        | NodeKind::PascalRecord
        | NodeKind::ScalaObject
        | NodeKind::KotlinObject
        | NodeKind::CompanionObject
        | NodeKind::Annotation
        | NodeKind::Event => 2.5,
        // Proto definitions
        #[cfg(feature = "lang-protobuf")]
        NodeKind::ProtoMessage | NodeKind::ProtoService | NodeKind::ProtoRpc => 2.5,
        // Impl blocks (between defs and refs)
        NodeKind::Impl => 2.0,
        // Values, macros, preprocessor defs
        NodeKind::Const
        | NodeKind::Static
        | NodeKind::Macro
        | NodeKind::PreprocessorDef
        | NodeKind::EnumVariant => 1.0,
        // Members of types
        NodeKind::Field
        | NodeKind::ValField
        | NodeKind::VarField
        | NodeKind::Property
        | NodeKind::CSharpProperty
        | NodeKind::StructTag
        | NodeKind::InitBlock
        | NodeKind::Export => 0.5,
        // File / generic-parameter — neutral
        NodeKind::File | NodeKind::GenericParam | NodeKind::PascalProgram => 0.0,
        // References & containers — push below definitions
        NodeKind::Use | NodeKind::Include => -3.0,
        NodeKind::AnnotationUsage | NodeKind::Decorator => -2.0,
        NodeKind::Module
        | NodeKind::Package
        | NodeKind::Namespace
        | NodeKind::ScalaPackage
        | NodeKind::GoPackage
        | NodeKind::KotlinPackage
        | NodeKind::PascalUnit
        | NodeKind::Library => -1.5,
    }
}

/// Parses every `#[derive(A, B, C)]` attribute appearing in `content`
/// between (0-based, inclusive) `start_line` and `end_line`. Multiple
/// derive attributes stack — `#[derive(Debug)]` and `#[derive(Clone)]` on
/// the same item both contribute. The returned list is de-duplicated and
/// preserves source order (Debug before Clone if that's how they're
/// written).
pub(crate) fn parse_derives_in_attr_block(
    content: &str,
    start_line: u32,
    end_line: u32,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line as usize;
    let end = (end_line as usize).min(lines.len().saturating_sub(1));
    if start >= lines.len() {
        return out;
    }
    // Join the attribute block into a single string so multi-line
    // `#[derive(\n  Debug,\n  Clone,\n)]` (rustfmt's split form for long
    // derive lists) is handled uniformly with the single-line variant.
    let block = lines[start..=end].join("\n");
    let mut search_from = 0usize;
    while let Some(start_idx) = block[search_from..].find("#[derive(") {
        let abs_start = search_from + start_idx + "#[derive(".len();
        let Some(close_offset) = block[abs_start..].find(')') else {
            break;
        };
        let inner = &block[abs_start..abs_start + close_offset];
        for name in inner.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            // Strip the path prefix on fully-qualified derives so callers
            // see `Serialize` not `serde::Serialize`. Matches the convention
            // the static derive table uses.
            let short = name.rsplit("::").next().unwrap_or(name).to_string();
            if seen.insert(short.clone()) {
                out.push(short);
            }
        }
        search_from = abs_start + close_offset + 1;
    }
    out
}

/// Normalises an external file path (typically from a `cargo check` /
/// `cargo clippy` diagnostic span) into the project-relative,
/// forward-slash form the index stores. Handles three real-world shapes:
///
/// - Absolute paths (cargo emits them when `--manifest-path` points at a
///   project root that differs from `cwd`): strip the `project_root`
///   prefix so `/abs/path/to/project/src/lib.rs` becomes `src/lib.rs`.
/// - Backslash paths (Windows cargo): convert `\` → `/`.
/// - Already-relative forward-slash paths: pass through unchanged.
///
/// Falls back to returning the input verbatim if no transformation
/// applies — `get_nodes_by_file` will then handle "no such file" the
/// same way it always does.
pub(crate) fn normalize_lookup_path(project_root: &std::path::Path, raw: &str) -> String {
    let forward = raw.replace('\\', "/");
    let path = std::path::Path::new(&forward);
    if path.is_absolute() {
        // Try canonicalising both sides; canonicalisation handles
        // symlinks, `..` segments, and trailing slashes uniformly. If
        // either fails (file doesn't exist on disk, project root
        // moved), fall back to a raw prefix strip.
        if let (Ok(abs), Ok(root)) = (path.canonicalize(), project_root.canonicalize()) {
            if let Ok(rel) = abs.strip_prefix(&root) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }
        let root_str = project_root.to_string_lossy();
        if let Some(rel) = forward.strip_prefix(root_str.as_ref()) {
            return rel.trim_start_matches('/').to_string();
        }
    }
    forward
}

/// True when the user-supplied query matches either the node's short `name`
/// or its `qualified_name`. Matching is exact on the short name and substring
/// on the qualified name, so callers can pass either form for the impl/trait
/// filter on `tokensave_impls`.
pub(crate) fn node_name_matches(node: &Node, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    node.name == query || node.qualified_name == query || node.qualified_name.contains(query)
}

/// Returns `true` if the file path looks like a test file.
pub fn is_test_file(path: &str) -> bool {
    let test_segments = [
        "test/",
        "tests/",
        "__tests__/",
        "spec/",
        "e2e/",
        ".test.",
        ".spec.",
        "_test.",
        "_spec.",
    ];
    let lower = path.to_ascii_lowercase();
    test_segments.iter().any(|s| lower.contains(s))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod derive_parse_tests {
    use super::parse_derives_in_attr_block;

    #[test]
    pub(crate) fn parses_single_derive_block() {
        let src = "\
#[derive(Debug, Clone, PartialEq)]
pub struct Foo;
";
        let derives = parse_derives_in_attr_block(src, 0, 1);
        assert_eq!(derives, vec!["Debug", "Clone", "PartialEq"]);
    }

    #[test]
    pub(crate) fn stacks_multiple_derive_attributes() {
        let src = "\
#[derive(Debug)]
#[derive(Clone, Hash)]
pub enum K {}
";
        let derives = parse_derives_in_attr_block(src, 0, 2);
        assert_eq!(derives, vec!["Debug", "Clone", "Hash"]);
    }

    #[test]
    pub(crate) fn strips_path_prefix_on_qualified_derive() {
        let src = "#[derive(serde::Serialize, Debug)]\npub struct S;\n";
        let derives = parse_derives_in_attr_block(src, 0, 1);
        assert_eq!(derives, vec!["Serialize", "Debug"]);
    }

    #[test]
    pub(crate) fn ignores_non_derive_attributes() {
        let src = "\
#[cfg(feature = \"foo\")]
#[serde(rename = \"x\")]
#[derive(Debug)]
pub struct S;
";
        let derives = parse_derives_in_attr_block(src, 0, 3);
        assert_eq!(derives, vec!["Debug"]);
    }

    #[test]
    pub(crate) fn deduplicates_repeated_derives() {
        let src = "#[derive(Debug, Debug, Clone)]\npub struct S;\n";
        let derives = parse_derives_in_attr_block(src, 0, 1);
        assert_eq!(derives, vec!["Debug", "Clone"]);
    }

    /// Regression: rustfmt splits long derive lists across lines:
    ///   `#[derive(\n    Debug,\n    Clone,\n    PartialEq,\n)]`
    /// The previous line-bounded parser dropped all of these because it
    /// only matched `#[derive(...)]` when the closing `)` was on the
    /// same line. Production codebases with realistic-sized derive
    /// lists were getting empty `derives` output.
    #[test]
    pub(crate) fn parses_multiline_derive_attribute() {
        let src = "\
#[derive(
    Debug,
    Clone,
    PartialEq,
)]
pub struct Wide;
";
        let derives = parse_derives_in_attr_block(src, 0, 5);
        assert_eq!(derives, vec!["Debug", "Clone", "PartialEq"]);
    }

    #[test]
    pub(crate) fn parses_multiline_derive_mixed_with_single_line() {
        let src = "\
#[derive(Debug)]
#[derive(
    Clone,
    Hash,
)]
pub struct M;
";
        let derives = parse_derives_in_attr_block(src, 0, 5);
        assert_eq!(derives, vec!["Debug", "Clone", "Hash"]);
    }
}
