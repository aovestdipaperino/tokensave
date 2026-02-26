mod go_extractor;
mod java_extractor;
/// Tree-sitter based source code extraction module.
///
/// This module provides extractors that parse source files using tree-sitter
/// and produce structured graph nodes and edges.
mod rust_extractor;

pub use go_extractor::GoExtractor;
pub use java_extractor::JavaExtractor;
pub use rust_extractor::RustExtractor;

use crate::types::ExtractionResult;

/// Trait for language-specific source code extractors.
///
/// Each implementation handles a single programming language,
/// using tree-sitter to parse source and emit graph nodes and edges.
pub trait LanguageExtractor: Send + Sync {
    /// File extensions this extractor handles (without leading dot).
    fn extensions(&self) -> &[&str];

    /// Human-readable language name.
    fn language_name(&self) -> &str;

    /// Extract nodes, edges, and unresolved refs from source code.
    ///
    /// `file_path` is the relative path used for qualified names and node IDs.
    /// `source` is the source code to parse.
    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult;
}

/// Registry of all available language extractors.
///
/// Dispatches to the correct extractor based on file extension.
pub struct LanguageRegistry {
    extractors: Vec<Box<dyn LanguageExtractor>>,
}

impl LanguageRegistry {
    /// Creates a new registry with all built-in language extractors.
    pub fn new() -> Self {
        Self {
            extractors: vec![
                Box::new(RustExtractor),
                Box::new(GoExtractor),
                Box::new(JavaExtractor),
            ],
        }
    }

    /// Returns the extractor for a file path based on its extension.
    pub fn extractor_for_file(&self, path: &str) -> Option<&dyn LanguageExtractor> {
        let ext = path.rsplit('.').next()?;
        self.extractors
            .iter()
            .find(|e| e.extensions().contains(&ext))
            .map(|e| e.as_ref())
    }

    /// Returns all supported file extensions across all extractors.
    pub fn supported_extensions(&self) -> Vec<&str> {
        self.extractors
            .iter()
            .flat_map(|e| e.extensions().iter().copied())
            .collect()
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
