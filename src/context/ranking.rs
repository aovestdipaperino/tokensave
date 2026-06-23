use crate::types::{NodeKind, SearchResult, Visibility};

/// Boost factor based on node kind.
pub fn kind_boost(kind: &NodeKind) -> f64 {
    match kind {
        NodeKind::Function
        | NodeKind::Method
        | NodeKind::StructMethod
        | NodeKind::Constructor
        | NodeKind::AbstractMethod
        | NodeKind::Procedure => 2.0,
        NodeKind::ArrowFunction => 1.8,

        NodeKind::Struct
        | NodeKind::Class
        | NodeKind::Enum
        | NodeKind::Trait
        | NodeKind::Interface
        | NodeKind::InterfaceType
        | NodeKind::DataClass
        | NodeKind::SealedClass
        | NodeKind::CaseClass
        | NodeKind::Record
        | NodeKind::Union => 1.5,

        NodeKind::Module
        | NodeKind::Impl
        | NodeKind::Namespace
        | NodeKind::ScalaObject
        | NodeKind::CompanionObject
        | NodeKind::KotlinObject => 1.2,

        NodeKind::Field | NodeKind::Property | NodeKind::ValField | NodeKind::VarField => 0.5,
        NodeKind::EnumVariant => 0.3,
        NodeKind::Use | NodeKind::Export | NodeKind::Include => 0.2,

        _ => 1.0,
    }
}

/// Boost factor based on visibility.
pub fn visibility_boost(visibility: &Visibility) -> f64 {
    match visibility {
        Visibility::Pub => 1.5,
        Visibility::PubCrate | Visibility::PubSuper => 1.2,
        Visibility::Private => 0.8,
    }
}

/// Boost factor based on file path.
pub fn path_boost(file_path: &str) -> f64 {
    if file_path.contains("tests/fixtures/")
        || file_path.contains("test/fixtures/")
        || file_path.contains("testdata/")
        || file_path.contains("__fixtures__/")
    {
        return 0.1;
    }
    if file_path.starts_with("tests/")
        || file_path.starts_with("test/")
        || file_path.contains("_test.")
        || file_path.contains(".test.")
        || file_path.contains("_spec.")
        || file_path.contains(".spec.")
    {
        return 0.4;
    }
    1.0
}

/// Path segments of generated / vendored / dependency trees that almost
/// never contain the application code a user is searching for. Matched as
/// `/segment/` (or as a leading `segment/`) against the `/`-normalized path.
const VENDOR_PATH_SEGMENTS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "target",
    "venv",
    ".venv",
    "site-packages",
    "vendor",
    "__pycache__",
    ".next",
    "out",
];

/// Path segments of non-production trees that ship alongside first-party code
/// but rarely contain the symbol a user is searching for: example programs,
/// sample snippets, benchmarks, and demos. Down-weighted softly (not filtered)
/// so an exact match still surfaces, just below equivalent production code.
const NON_PROD_PATH_SEGMENTS: &[&str] = &[
    "examples",
    "example",
    "samples",
    "sample",
    "benchmarks",
    "benchmark",
    "demos",
    "demo",
];

/// Path segments of typical application source directories. A match gives a
/// modest boost so first-party code surfaces ahead of equally-relevant code
/// living elsewhere.
const APP_PATH_SEGMENTS: &[&str] = &["src", "app", "lib"];

/// Returns `true` if `normalized` (a `/`-separated path) contains `segment`
/// as a full path component (`a/segment/b`, `segment/b`, or exactly `segment`).
fn has_path_segment(normalized: &str, segment: &str) -> bool {
    normalized.split('/').any(|c| c == segment)
}

/// Path-based ranking multiplier applied during both search and context
/// re-ranking. Returns a factor relative to a neutral baseline of `1.0`:
///
/// * `< 1.0` for nodes living under a known vendor / generated tree
///   (`node_modules`, `dist`, `target`, `venv`, …) so dependency or build
///   output sinks below first-party code.
/// * `< 1.0`, but milder, for non-production trees (`examples/`, `samples/`,
///   `benchmarks/`, `demos/`, …) so they rank below equivalent production code
///   without being filtered out.
/// * `> 1.0` for nodes under a typical app source dir (`src/`, `app/`,
///   `lib/`) so application code surfaces first by default.
/// * exactly `1.0` for everything else (neutral).
///
/// The effect is proportional, not a filter: a strong exact-name match in
/// `node_modules` or `examples/` can still appear, just ranked lower. Vendor
/// classification wins over both non-production and app classification when a
/// path matches more than one (e.g. a `src` dir nested inside `node_modules`,
/// or an `examples` dir under `target`).
pub fn path_rank_multiplier(file_path: &str) -> f64 {
    let normalized = file_path.replace('\\', "/");
    if VENDOR_PATH_SEGMENTS
        .iter()
        .any(|seg| has_path_segment(&normalized, seg))
    {
        return 0.4;
    }
    if NON_PROD_PATH_SEGMENTS
        .iter()
        .any(|seg| has_path_segment(&normalized, seg))
    {
        // Between the vendor (0.4) and neutral (1.0) factors: example/demo
        // code is more relevant than vendored deps but less than production.
        return 0.6;
    }
    if APP_PATH_SEGMENTS
        .iter()
        .any(|seg| has_path_segment(&normalized, seg))
    {
        return 1.25;
    }
    1.0
}

/// Applies a log-scale connectivity boost based on incoming call counts.
/// `call_counts` maps `node_id` → incoming "calls" edge count.
pub fn apply_connectivity_boost<S: std::hash::BuildHasher>(
    candidates: &mut [SearchResult],
    call_counts: &std::collections::HashMap<String, u64, S>,
) {
    for candidate in candidates.iter_mut() {
        let count = call_counts.get(&candidate.node.id).copied().unwrap_or(0);
        // log2(count + 1) scaled to 1.0–2.0 range, capped at 4.0 bits
        let boost = 1.0 + (count as f64 + 1.0).log2().min(4.0) / 4.0;
        candidate.score *= boost;
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Re-ranks search result candidates using structural signals.
pub fn rerank_candidates(candidates: &mut [SearchResult]) {
    for candidate in candidates.iter_mut() {
        let boost = kind_boost(&candidate.node.kind)
            * visibility_boost(&candidate.node.visibility)
            * path_boost(&candidate.node.file_path)
            * path_rank_multiplier(&candidate.node.file_path);
        candidate.score *= boost;
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::uninlined_format_args)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::types::Node;

    fn make_result(kind: NodeKind, vis: Visibility, path: &str, score: f64) -> SearchResult {
        SearchResult {
            node: Node {
                id: format!("test:{}", path),
                kind,
                name: "test_sym".to_string(),
                qualified_name: format!("{}::test_sym", path),
                file_path: path.to_string(),
                start_line: 1,
                attrs_start_line: 1,
                end_line: 5,
                start_column: 0,
                end_column: 1,
                signature: None,
                docstring: None,
                visibility: vis,
                is_async: false,
                branches: 0,
                loops: 0,
                returns: 0,
                max_nesting: 0,
                unsafe_blocks: 0,
                unchecked_calls: 0,
                assertions: 0,
                cognitive_complexity: 0,
                distinct_operators: 0,
                distinct_operands: 0,
                total_operators: 0,
                total_operands: 0,
                updated_at: 0,
                parent_id: None,
            },
            score,
        }
    }

    #[test]
    fn test_function_outranks_field_same_fts_score() {
        let mut candidates = vec![
            make_result(NodeKind::Field, Visibility::Pub, "src/lib.rs", 10.0),
            make_result(NodeKind::Function, Visibility::Pub, "src/lib.rs", 10.0),
        ];
        rerank_candidates(&mut candidates);
        assert_eq!(candidates[0].node.kind, NodeKind::Function);
    }

    #[test]
    fn test_public_outranks_private() {
        let mut candidates = vec![
            make_result(NodeKind::Function, Visibility::Private, "src/lib.rs", 10.0),
            make_result(NodeKind::Function, Visibility::Pub, "src/lib.rs", 10.0),
        ];
        rerank_candidates(&mut candidates);
        assert_eq!(candidates[0].node.visibility, Visibility::Pub);
    }

    #[test]
    fn test_fixtures_ranked_below_source() {
        let mut candidates = vec![
            make_result(
                NodeKind::EnumVariant,
                Visibility::Pub,
                "tests/fixtures/sample.m",
                10.0,
            ),
            make_result(NodeKind::Function, Visibility::Pub, "src/logging.rs", 5.0),
        ];
        rerank_candidates(&mut candidates);
        assert_eq!(candidates[0].node.file_path, "src/logging.rs");
    }

    #[test]
    fn test_test_files_penalized_vs_source() {
        let mut candidates = vec![
            make_result(
                NodeKind::Function,
                Visibility::Pub,
                "tests/sync_test.rs",
                10.0,
            ),
            make_result(NodeKind::Function, Visibility::Pub, "src/sync.rs", 10.0),
        ];
        rerank_candidates(&mut candidates);
        assert_eq!(candidates[0].node.file_path, "src/sync.rs");
    }

    #[test]
    fn test_rerank_preserves_order_when_boosts_equal() {
        let mut candidates = vec![
            make_result(NodeKind::Function, Visibility::Pub, "src/a.rs", 10.0),
            make_result(NodeKind::Function, Visibility::Pub, "src/b.rs", 5.0),
        ];
        rerank_candidates(&mut candidates);
        assert_eq!(candidates[0].node.file_path, "src/a.rs");
        assert_eq!(candidates[1].node.file_path, "src/b.rs");
    }

    #[test]
    fn test_enum_variant_low_boost() {
        assert!(kind_boost(&NodeKind::EnumVariant) < 1.0);
        assert!(kind_boost(&NodeKind::Function) > 1.0);
    }

    #[test]
    fn test_kind_boost_values() {
        assert_eq!(kind_boost(&NodeKind::Function), 2.0);
        assert_eq!(kind_boost(&NodeKind::Method), 2.0);
        assert_eq!(kind_boost(&NodeKind::Struct), 1.5);
        assert_eq!(kind_boost(&NodeKind::EnumVariant), 0.3);
        assert_eq!(kind_boost(&NodeKind::Use), 0.2);
    }

    #[test]
    fn test_visibility_boost_values() {
        assert_eq!(visibility_boost(&Visibility::Pub), 1.5);
        assert_eq!(visibility_boost(&Visibility::Private), 0.8);
        assert_eq!(visibility_boost(&Visibility::PubCrate), 1.2);
    }

    #[test]
    fn test_path_boost_values() {
        assert_eq!(path_boost("src/lib.rs"), 1.0);
        assert_eq!(path_boost("tests/fixtures/sample.m"), 0.1);
        assert_eq!(path_boost("tests/sync_test.rs"), 0.4);
        assert_eq!(path_boost("test/fixtures/foo.js"), 0.1);
        assert_eq!(path_boost("src/components/Button.test.tsx"), 0.4);
    }

    #[test]
    fn test_path_rank_multiplier_vendor_below_one() {
        assert!(path_rank_multiplier("node_modules/foo/index.js") < 1.0);
        assert!(path_rank_multiplier("frontend/node_modules/foo/index.js") < 1.0);
        assert!(path_rank_multiplier("dist/bundle.js") < 1.0);
        assert!(path_rank_multiplier("target/debug/build/x.rs") < 1.0);
        assert!(path_rank_multiplier(".venv/lib/python3.11/site-packages/x.py") < 1.0);
        assert!(path_rank_multiplier("vendor/github.com/foo/bar.go") < 1.0);
        assert!(path_rank_multiplier("project/__pycache__/mod.pyc") < 1.0);
        assert!(path_rank_multiplier("web/.next/server/page.js") < 1.0);
        assert!(path_rank_multiplier("out/main.js") < 1.0);
    }

    #[test]
    fn test_path_rank_multiplier_app_above_neutral() {
        assert!(path_rank_multiplier("src/lib.rs") > 1.0);
        assert!(path_rank_multiplier("app/models/user.rb") > 1.0);
        assert!(path_rank_multiplier("lib/widget.dart") > 1.0);
        assert!(path_rank_multiplier("crate/src/foo.rs") > 1.0);
    }

    #[test]
    fn test_path_rank_multiplier_neutral_is_baseline() {
        assert_eq!(path_rank_multiplier("foo/bar.rs"), 1.0);
        assert_eq!(path_rank_multiplier("README.md"), 1.0);
        assert_eq!(path_rank_multiplier("docs/guide.md"), 1.0);
    }

    #[test]
    fn test_path_rank_multiplier_vendor_wins_over_app() {
        // A `src` dir nested inside node_modules is still vendored code.
        assert!(path_rank_multiplier("node_modules/pkg/src/index.js") < 1.0);
    }

    #[test]
    fn test_path_rank_multiplier_nonprod_below_one() {
        assert!(path_rank_multiplier("examples/foo.rs") < 1.0);
        assert!(path_rank_multiplier("example/foo.rs") < 1.0);
        assert!(path_rank_multiplier("samples/foo.rs") < 1.0);
        assert!(path_rank_multiplier("sample/foo.rs") < 1.0);
        assert!(path_rank_multiplier("benchmarks/bench.rs") < 1.0);
        assert!(path_rank_multiplier("benchmark/bench.rs") < 1.0);
        assert!(path_rank_multiplier("demos/app.rs") < 1.0);
        assert!(path_rank_multiplier("demo/app.rs") < 1.0);
        assert!(path_rank_multiplier("crate/examples/usage.rs") < 1.0);
    }

    #[test]
    fn test_path_rank_multiplier_nonprod_above_vendor() {
        // Non-production code is more relevant than vendored deps.
        assert!(
            path_rank_multiplier("examples/foo.rs") > path_rank_multiplier("node_modules/foo.js")
        );
    }

    #[test]
    fn test_path_rank_multiplier_vendor_wins_over_nonprod() {
        // An `examples` dir nested inside a build/vendor tree is still vendored.
        assert_eq!(
            path_rank_multiplier("target/examples/foo.rs"),
            path_rank_multiplier("target/debug/foo.rs")
        );
        assert!(
            path_rank_multiplier("target/examples/foo.rs")
                < path_rank_multiplier("examples/foo.rs")
        );
    }

    #[test]
    fn test_path_rank_multiplier_nonprod_substring_not_matched() {
        // "demolition" contains "demo" but is not the `demo` dir.
        assert_eq!(path_rank_multiplier("demolition/handler.rs"), 1.0);
        // "exampled" contains "example" but is not the `example` dir.
        assert_eq!(path_rank_multiplier("exampled/foo.rs"), 1.0);
    }

    #[test]
    fn test_src_outranks_examples_in_rerank() {
        let mut candidates = vec![
            make_result(
                NodeKind::Function,
                Visibility::Pub,
                "examples/handler.rs",
                10.0,
            ),
            make_result(NodeKind::Function, Visibility::Pub, "src/handler.rs", 10.0),
        ];
        rerank_candidates(&mut candidates);
        assert_eq!(candidates[0].node.file_path, "src/handler.rs");
    }

    #[test]
    fn test_path_rank_multiplier_normalizes_backslashes() {
        assert!(path_rank_multiplier("web\\node_modules\\foo.js") < 1.0);
        assert!(path_rank_multiplier("crate\\src\\foo.rs") > 1.0);
    }

    #[test]
    fn test_path_rank_multiplier_substring_not_matched() {
        // "outbound" contains "out" but is not the `out` build dir.
        assert_eq!(path_rank_multiplier("outbound/handler.rs"), 1.0);
        // "library" contains "lib" but is not the `lib` app dir.
        assert_eq!(path_rank_multiplier("library/foo.rs"), 1.0);
    }

    #[test]
    fn test_app_source_outranks_vendor_in_rerank() {
        let mut candidates = vec![
            make_result(
                NodeKind::Function,
                Visibility::Pub,
                "node_modules/pkg/index.js",
                10.0,
            ),
            make_result(NodeKind::Function, Visibility::Pub, "src/handler.rs", 10.0),
        ];
        rerank_candidates(&mut candidates);
        assert_eq!(candidates[0].node.file_path, "src/handler.rs");
    }

    #[test]
    fn test_connectivity_boost_prefers_high_fanin() {
        let mut candidates = vec![
            make_result(NodeKind::Function, Visibility::Pub, "src/a.rs", 10.0),
            make_result(NodeKind::Function, Visibility::Pub, "src/b.rs", 10.0),
        ];
        rerank_candidates(&mut candidates);
        let base_score = candidates[0].score;
        assert_eq!(candidates[1].score, base_score, "same base score");

        let mut counts = std::collections::HashMap::new();
        counts.insert("test:src/a.rs".to_string(), 15u64);

        apply_connectivity_boost(&mut candidates, &counts);
        assert_eq!(
            candidates[0].node.file_path, "src/a.rs",
            "high fan-in should rank first"
        );
        assert!(candidates[0].score > candidates[1].score);
    }
}
