/// Tree-sitter based Go source code extractor.
use crate::extraction::LanguageExtractor;
use crate::types::ExtractionResult;

/// Extracts code graph nodes and edges from Go source files.
pub struct GoExtractor;

impl LanguageExtractor for GoExtractor {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn language_name(&self) -> &str {
        "Go"
    }

    fn extract(&self, _file_path: &str, _source: &str) -> ExtractionResult {
        ExtractionResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: vec!["Go extraction not yet implemented".to_string()],
            duration_ms: 0,
        }
    }
}
