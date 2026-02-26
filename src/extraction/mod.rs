/// Tree-sitter based source code extraction module.
///
/// This module provides extractors that parse source files using tree-sitter
/// and produce structured graph nodes and edges.
mod rust_extractor;

pub use rust_extractor::RustExtractor;
