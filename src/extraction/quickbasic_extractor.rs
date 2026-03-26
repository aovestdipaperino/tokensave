/// Tree-sitter based QuickBasic 4.5 source code extractor.
///
/// QuickBasic 4.5 is a superset of QBasic with separate compilation,
/// `$INCLUDE` metacommands, `REDIM`, and compiled `.EXE` output.
/// The grammar is identical to QBasic (parsed by `tree-sitter-qbasic`),
/// so this extractor delegates to `QBasicExtractor` for all extraction
/// and registers the QuickBasic-specific file extensions (`.bi`, `.bm`).
use crate::extraction::qbasic_extractor::QBasicExtractor;
use crate::types::ExtractionResult;

/// Extracts code graph nodes and edges from QuickBasic 4.5 source files.
///
/// Uses the same tree-sitter grammar and extraction logic as
/// [`QBasicExtractor`] — the languages are syntactically identical.
pub struct QuickBasicExtractor;

impl crate::extraction::LanguageExtractor for QuickBasicExtractor {
    fn extensions(&self) -> &[&str] {
        &["bi", "bm"]
    }

    fn language_name(&self) -> &str {
        "QuickBASIC"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        QBasicExtractor::extract_qbasic(file_path, source)
    }
}
