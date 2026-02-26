/// Tree-sitter based Java source code extractor.
use crate::extraction::LanguageExtractor;
use crate::types::ExtractionResult;

/// Extracts code graph nodes and edges from Java source files.
pub struct JavaExtractor;

impl LanguageExtractor for JavaExtractor {
    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn language_name(&self) -> &str {
        "Java"
    }

    fn extract(&self, _file_path: &str, _source: &str) -> ExtractionResult {
        ExtractionResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: vec!["Java extraction not yet implemented".to_string()],
            duration_ms: 0,
        }
    }
}
