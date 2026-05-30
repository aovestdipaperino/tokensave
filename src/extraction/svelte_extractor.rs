//! Tree-sitter based Svelte source code extractor.
//!
//! Svelte single-file components mix HTML template markup with TypeScript/JavaScript
//! inside one or two `<script>` blocks. This extractor locates those blocks,
//! blanks out every line outside them (preserving line numbers), then delegates
//! to [`TypeScriptExtractor`] so all existing TS/JS symbol extraction logic is
//! reused without duplication.
//!
//! Supported block forms (Svelte 4 and 5):
//!
//! * `<script lang="ts">` — component instance script
//! * `<script>` — component instance script (plain JS)
//! * `<script module>` — module-level script (Svelte 5)
//! * `<script context="module">` — module-level script (Svelte 4)

use crate::extraction::typescript_extractor::TypeScriptExtractor;
use crate::extraction::LanguageExtractor;
use crate::types::ExtractionResult;

/// Extracts code graph nodes and edges from Svelte single-file components.
#[derive(Debug)]
pub struct SvelteExtractor;

impl SvelteExtractor {
    /// Extract nodes and edges from a Svelte source file.
    pub fn extract_svelte(file_path: &str, source: &str) -> ExtractionResult {
        let masked = Self::mask_non_script(source);
        TypeScriptExtractor::extract_typescript(file_path, &masked)
    }

    /// Replace every line outside `<script>` blocks with an empty line.
    ///
    /// Keeping newlines in place means all line numbers in the AST produced by
    /// the TypeScript parser match positions in the original `.svelte` file.
    fn mask_non_script(source: &str) -> String {
        let ranges = Self::script_content_line_ranges(source);
        if ranges.is_empty() {
            let blank_lines = source.lines().count().saturating_sub(1);
            return "\n".repeat(blank_lines);
        }
        source
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if ranges.iter().any(|&(s, e)| i >= s && i < e) {
                    line
                } else {
                    ""
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Return `(content_start, content_end_exclusive)` line-index pairs for
    /// every `<script>` block found in `source`. Tag lines themselves are
    /// excluded so they do not confuse the TypeScript parser.
    fn script_content_line_ranges(source: &str) -> Vec<(usize, usize)> {
        let lines: Vec<&str> = source.lines().collect();
        let mut ranges = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if Self::is_script_open(lines[i]) {
                let content_start = i + 1;
                let mut j = content_start;
                while j < lines.len() {
                    if Self::is_script_close(lines[j]) {
                        if j > content_start {
                            ranges.push((content_start, j));
                        }
                        i = j + 1;
                        break;
                    }
                    j += 1;
                }
                if j == lines.len() {
                    // Unclosed tag — treat remainder as content.
                    if content_start < lines.len() {
                        ranges.push((content_start, lines.len()));
                    }
                    break;
                }
            } else {
                i += 1;
            }
        }
        ranges
    }

    fn is_script_open(line: &str) -> bool {
        let t = line.trim_start();
        // Must start with `<script` and close its tag on the same line.
        t.starts_with("<script") && t.contains('>')
    }

    fn is_script_close(line: &str) -> bool {
        line.trim_start().starts_with("</script")
    }
}

impl LanguageExtractor for SvelteExtractor {
    fn extensions(&self) -> &[&str] {
        &["svelte"]
    }

    fn language_name(&self) -> &'static str {
        "Svelte"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_svelte(file_path, source)
    }
}
