use crate::types::{NodeKind, SearchResult, Visibility};

/// Boost factor based on node kind.
pub fn kind_boost(kind: &NodeKind) -> f64 {
    match kind {
        NodeKind::Function | NodeKind::Method | NodeKind::StructMethod => 2.0,
        NodeKind::Constructor | NodeKind::AbstractMethod | NodeKind::Procedure => 2.0,
        NodeKind::ArrowFunction => 1.8,

        NodeKind::Struct | NodeKind::Class | NodeKind::Enum | NodeKind::Trait => 1.5,
        NodeKind::Interface | NodeKind::InterfaceType => 1.5,
        NodeKind::DataClass | NodeKind::SealedClass | NodeKind::CaseClass => 1.5,
        NodeKind::Record | NodeKind::Union => 1.5,

        NodeKind::Module | NodeKind::Impl | NodeKind::Namespace => 1.2,
        NodeKind::ScalaObject | NodeKind::CompanionObject | NodeKind::KotlinObject => 1.2,

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

/// Applies a log-scale connectivity boost based on incoming call counts.
/// `call_counts` maps node_id → incoming "calls" edge count.
pub fn apply_connectivity_boost(
    candidates: &mut [SearchResult],
    call_counts: &std::collections::HashMap<String, u64>,
) {
    for candidate in candidates.iter_mut() {
        let count = call_counts.get(&candidate.node.id).copied().unwrap_or(0);
        // log2(count + 1) scaled to 1.0–2.0 range, capped at 4.0 bits
        let boost = 1.0 + (count as f64 + 1.0).log2().min(4.0) / 4.0;
        candidate.score *= boost;
    }
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}

/// Re-ranks search result candidates using structural signals.
pub fn rerank_candidates(candidates: &mut [SearchResult]) {
    for candidate in candidates.iter_mut() {
        let boost = kind_boost(&candidate.node.kind)
            * visibility_boost(&candidate.node.visibility)
            * path_boost(&candidate.node.file_path);
        candidate.score *= boost;
    }
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}

#[cfg(test)]
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
                updated_at: 0,
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
