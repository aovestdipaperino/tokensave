/// Metal Shading Language extractor.
///
/// Metal is a strict superset of C++14, so the C++ grammar covers its syntax
/// correctly. This extractor delegates to [`CppExtractor`] and adds the `.metal`
/// extension mapping.
use crate::extraction::CppExtractor;
use crate::types::ExtractionResult;

pub struct MetalExtractor;

impl crate::extraction::LanguageExtractor for MetalExtractor {
    fn extensions(&self) -> &[&str] {
        &["metal"]
    }

    fn language_name(&self) -> &'static str {
        "Metal"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        CppExtractor::extract_source(file_path, source)
    }
}
