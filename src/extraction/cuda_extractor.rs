/// CUDA / HIP extractor.
///
/// CUDA is a superset of C++, so this delegates to [`CppExtractor`] (same
/// pattern as Metal) but parses with the dedicated `cuda` grammar, which
/// handles `<<<grid, block>>>` kernel launches and `__global__`/`__device__`/
/// `__host__` specifiers natively — no launch blanking needed.
use crate::extraction::CppExtractor;
use crate::types::ExtractionResult;

pub struct CudaExtractor;

impl crate::extraction::LanguageExtractor for CudaExtractor {
    fn extensions(&self) -> &[&str] {
        &["cu", "cuh"]
    }

    fn language_name(&self) -> &'static str {
        "CUDA"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        CppExtractor::extract_source_with_key(file_path, source, "cuda")
    }
}
