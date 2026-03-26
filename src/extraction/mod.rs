mod bash_extractor;
pub mod complexity;
mod go_extractor;
mod java_extractor;
mod lua_extractor;
/// Tree-sitter based source code extraction module.
///
/// This module provides extractors that parse source files using tree-sitter
/// and produce structured graph nodes and edges.
mod rust_extractor;
mod scala_extractor;
mod typescript_extractor;
mod python_extractor;
mod c_extractor;
mod cpp_extractor;
mod kotlin_extractor;
mod dart_extractor;
mod csharp_extractor;
mod pascal_extractor;
mod php_extractor;
mod proto_extractor;
mod nix_extractor;
mod perl_extractor;
mod ruby_extractor;
mod swift_extractor;
mod batch_extractor;
mod powershell_extractor;
mod vbnet_extractor;
mod objc_extractor;
mod fortran_extractor;
mod cobol_extractor;
mod msbasic2_extractor;
mod gwbasic_extractor;
mod qbasic_extractor;
mod zig_extractor;

pub use bash_extractor::BashExtractor;
pub use go_extractor::GoExtractor;
pub use lua_extractor::LuaExtractor;
pub use java_extractor::JavaExtractor;
pub use rust_extractor::RustExtractor;
pub use scala_extractor::ScalaExtractor;
pub use typescript_extractor::TypeScriptExtractor;
pub use python_extractor::PythonExtractor;
pub use c_extractor::CExtractor;
pub use cpp_extractor::CppExtractor;
pub use kotlin_extractor::KotlinExtractor;
pub use dart_extractor::DartExtractor;
pub use csharp_extractor::CSharpExtractor;
pub use pascal_extractor::PascalExtractor;
pub use php_extractor::PhpExtractor;
pub use proto_extractor::ProtoExtractor;
pub use nix_extractor::NixExtractor;
pub use perl_extractor::PerlExtractor;
pub use ruby_extractor::RubyExtractor;
pub use swift_extractor::SwiftExtractor;
pub use batch_extractor::BatchExtractor;
pub use powershell_extractor::PowerShellExtractor;
pub use vbnet_extractor::VbNetExtractor;
pub use objc_extractor::ObjcExtractor;
pub use fortran_extractor::FortranExtractor;
pub use cobol_extractor::CobolExtractor;
pub use msbasic2_extractor::MsBasic2Extractor;
pub use gwbasic_extractor::GwBasicExtractor;
pub use qbasic_extractor::QBasicExtractor;
pub use zig_extractor::ZigExtractor;

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
                Box::new(ScalaExtractor),
                Box::new(TypeScriptExtractor),
                Box::new(PythonExtractor),
                Box::new(CExtractor),
                Box::new(CppExtractor),
                Box::new(CSharpExtractor),
                Box::new(DartExtractor),
                Box::new(KotlinExtractor),
                Box::new(PascalExtractor),
                Box::new(PhpExtractor),
                Box::new(NixExtractor),
                Box::new(PerlExtractor),
                Box::new(RubyExtractor),
                Box::new(SwiftExtractor),
                Box::new(BashExtractor),
                Box::new(LuaExtractor),
                Box::new(ZigExtractor),
                Box::new(ProtoExtractor),
                Box::new(PowerShellExtractor),
                Box::new(VbNetExtractor),
                Box::new(ObjcExtractor),
                Box::new(FortranExtractor),
                Box::new(CobolExtractor),
                Box::new(BatchExtractor),
                Box::new(MsBasic2Extractor),
                Box::new(GwBasicExtractor),
                Box::new(QBasicExtractor),
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
