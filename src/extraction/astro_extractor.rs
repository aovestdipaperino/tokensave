//! Tree-sitter based Astro source code extractor.
//!
//! Astro components embed TypeScript in a YAML-fenced frontmatter block at the
//! top of the file, delimited by `---` marker lines:
//!
//! ```text
//! ---
//! import Hero from './Hero.astro';
//! interface Props { title: string; }
//! const { title } = Astro.props;
//! ---
//! <html>…</html>
//! ```
//!
//! This extractor locates the frontmatter block, blanks out every line outside
//! it (preserving line numbers), then delegates to [`TypeScriptExtractor`] so
//! all existing TS/JS symbol extraction logic is reused without duplication.

use crate::extraction::typescript_extractor::TypeScriptExtractor;
use crate::extraction::LanguageExtractor;
use crate::types::ExtractionResult;

/// Extracts code graph nodes and edges from Astro component files.
#[derive(Debug)]
pub struct AstroExtractor;

impl AstroExtractor {
    /// Extract nodes and edges from an Astro source file.
    pub fn extract_astro(file_path: &str, source: &str) -> ExtractionResult {
        let masked = Self::mask_non_frontmatter(source);
        TypeScriptExtractor::extract_typescript(file_path, &masked)
    }

    /// Replace every line outside the `---` frontmatter block with an empty line.
    ///
    /// Keeping newlines in place means all line numbers in the AST produced by
    /// the TypeScript parser match positions in the original `.astro` file.
    fn mask_non_frontmatter(source: &str) -> String {
        let lines: Vec<&str> = source.lines().collect();

        // Frontmatter requires the very first line to be `---`.
        if lines.first().map(|l| l.trim()) != Some("---") {
            let blank_lines = lines.len().saturating_sub(1);
            return "\n".repeat(blank_lines);
        }

        // Find the closing `---` marker (first occurrence after line 0).
        let content_start = 1;
        let content_end = lines[content_start..]
            .iter()
            .position(|l| l.trim() == "---")
            .map_or(lines.len(), |rel| content_start + rel); // unclosed — include everything

        lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if i >= content_start && i < content_end {
                    *line
                } else {
                    ""
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl LanguageExtractor for AstroExtractor {
    fn extensions(&self) -> &[&str] {
        &["astro"]
    }

    fn language_name(&self) -> &'static str {
        "Astro"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_astro(file_path, source)
    }
}
