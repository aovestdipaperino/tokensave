/// CUDA / HIP extractor.
///
/// CUDA is a superset of C++, so this delegates to [`CppExtractor`] (same
/// pattern as Metal). The one exception: `<<<grid, block>>>` kernel-launch
/// syntax derails tree-sitter-cpp badly enough to drop the *enclosing
/// function* entirely, so every `<<<...>>>` span is blanked to same-length
/// spaces before parsing — the same technique the GLSL/Godot extractor uses
/// for its own dialect syntax.
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
        let parseable = Self::blank_kernel_launches(source);
        CppExtractor::extract_source(file_path, &parseable)
    }
}

impl CudaExtractor {
    /// Replace every `<<<...>>>` kernel-launch span with spaces (newlines
    /// preserved as newlines) so the C++ grammar sees a plain call
    /// expression, e.g. `kernel<<<g, b>>>(x)` becomes `kernel      (x)`.
    fn blank_kernel_launches(source: &str) -> String {
        let bytes = source.as_bytes();
        let mut out: Vec<u8> = bytes.to_vec();
        let mut i = 0;
        while i + 3 <= bytes.len() {
            if &bytes[i..i + 3] == b"<<<" {
                // Find the matching `>>>`; if the file has no closing
                // triple-angle (malformed/truncated source), leave the
                // rest untouched rather than blanking to EOF.
                if let Some(rel_end) = source[i + 3..].find(">>>") {
                    let end = i + 3 + rel_end + 3;
                    for b in &mut out[i..end] {
                        if *b != b'\n' {
                            *b = b' ';
                        }
                    }
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
        // `<<<`/`>>>` are pure ASCII, so span boundaries always land on
        // valid UTF-8 char boundaries; `from_utf8_lossy` avoids
        // `expect`/`unwrap` (denied by this crate's lint config) without
        // `unsafe`.
        String::from_utf8_lossy(&out).into_owned()
    }
}
