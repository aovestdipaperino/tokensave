/// Stub: C++ extractor (not yet implemented).
use crate::types::ExtractionResult;

pub struct CppExtractor;

impl CppExtractor {
    pub fn extract_cpp(_file_path: &str, _source: &str) -> ExtractionResult {
        ExtractionResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: vec!["C++ extractor not yet implemented".to_string()],
            duration_ms: 0,
        }
    }
}

impl crate::extraction::LanguageExtractor for CppExtractor {
    fn extensions(&self) -> &[&str] {
        &["cpp", "cxx", "cc", "hpp", "hxx"]
    }

    fn language_name(&self) -> &str {
        "C++"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        CppExtractor::extract_cpp(file_path, source)
    }
}
