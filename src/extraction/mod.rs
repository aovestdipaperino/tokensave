// Lite — always available (no cfg needed)
mod c_extractor;
mod cpp_extractor;
mod csharp_extractor;
mod go_extractor;
mod java_extractor;
mod kotlin_extractor;
mod python_extractor;
mod rust_extractor;
mod scala_extractor;
mod swift_extractor;
mod typescript_extractor;

pub mod complexity;
pub mod ts_provider;

#[cfg(feature = "lang-bash")]
mod bash_extractor;

// Medium
#[cfg(feature = "lang-dart")]
mod dart_extractor;
#[cfg(feature = "lang-nix")]
mod nix_extractor;
#[cfg(feature = "lang-pascal")]
mod pascal_extractor;
#[cfg(feature = "lang-php")]
mod php_extractor;
#[cfg(feature = "lang-powershell")]
mod powershell_extractor;
#[cfg(feature = "lang-protobuf")]
mod proto_extractor;
#[cfg(feature = "lang-ruby")]
mod ruby_extractor;
#[cfg(feature = "lang-vbnet")]
mod vbnet_extractor;

// Full
#[cfg(feature = "lang-batch")]
mod batch_extractor;
#[cfg(feature = "lang-cobol")]
mod cobol_extractor;
#[cfg(feature = "lang-dockerfile")]
mod dockerfile_extractor;
#[cfg(feature = "lang-fortran")]
mod fortran_extractor;
#[cfg(feature = "lang-gwbasic")]
mod gwbasic_extractor;
#[cfg(feature = "lang-lua")]
mod lua_extractor;
#[cfg(feature = "lang-msbasic2")]
mod msbasic2_extractor;
#[cfg(feature = "lang-objc")]
mod objc_extractor;
#[cfg(feature = "lang-perl")]
mod perl_extractor;
#[cfg(feature = "lang-qbasic")]
pub(crate) mod qbasic_extractor;
#[cfg(feature = "lang-qbasic")]
mod quickbasic_extractor;
#[cfg(feature = "lang-zig")]
mod zig_extractor;

// Lite — always available (no cfg needed)
pub use c_extractor::CExtractor;
pub use cpp_extractor::CppExtractor;
pub use csharp_extractor::CSharpExtractor;
pub use go_extractor::GoExtractor;
pub use java_extractor::JavaExtractor;
pub use kotlin_extractor::KotlinExtractor;
pub use python_extractor::PythonExtractor;
pub use rust_extractor::RustExtractor;
pub use scala_extractor::ScalaExtractor;
pub use swift_extractor::SwiftExtractor;
pub use typescript_extractor::TypeScriptExtractor;

// Medium
#[cfg(feature = "lang-bash")]
pub use bash_extractor::BashExtractor;
#[cfg(feature = "lang-dart")]
pub use dart_extractor::DartExtractor;
#[cfg(feature = "lang-nix")]
pub use nix_extractor::NixExtractor;
#[cfg(feature = "lang-pascal")]
pub use pascal_extractor::PascalExtractor;
#[cfg(feature = "lang-php")]
pub use php_extractor::PhpExtractor;
#[cfg(feature = "lang-powershell")]
pub use powershell_extractor::PowerShellExtractor;
#[cfg(feature = "lang-protobuf")]
pub use proto_extractor::ProtoExtractor;
#[cfg(feature = "lang-ruby")]
pub use ruby_extractor::RubyExtractor;
#[cfg(feature = "lang-vbnet")]
pub use vbnet_extractor::VbNetExtractor;

// Full
#[cfg(feature = "lang-batch")]
pub use batch_extractor::BatchExtractor;
#[cfg(feature = "lang-cobol")]
pub use cobol_extractor::CobolExtractor;
#[cfg(feature = "lang-dockerfile")]
pub use dockerfile_extractor::DockerfileExtractor;
#[cfg(feature = "lang-fortran")]
pub use fortran_extractor::FortranExtractor;
#[cfg(feature = "lang-gwbasic")]
pub use gwbasic_extractor::GwBasicExtractor;
#[cfg(feature = "lang-lua")]
pub use lua_extractor::LuaExtractor;
#[cfg(feature = "lang-msbasic2")]
pub use msbasic2_extractor::MsBasic2Extractor;
#[cfg(feature = "lang-objc")]
pub use objc_extractor::ObjcExtractor;
#[cfg(feature = "lang-perl")]
pub use perl_extractor::PerlExtractor;
#[cfg(feature = "lang-qbasic")]
pub use qbasic_extractor::QBasicExtractor;
#[cfg(feature = "lang-qbasic")]
pub use quickbasic_extractor::QuickBasicExtractor;
#[cfg(feature = "lang-zig")]
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
        #[allow(unused_mut)]
        let mut extractors: Vec<Box<dyn LanguageExtractor>> = vec![
            // Lite — always available
            Box::new(RustExtractor),
            Box::new(GoExtractor),
            Box::new(JavaExtractor),
            Box::new(ScalaExtractor),
            Box::new(TypeScriptExtractor),
            Box::new(PythonExtractor),
            Box::new(CExtractor),
            Box::new(CppExtractor),
            Box::new(CSharpExtractor),
            Box::new(KotlinExtractor),
            Box::new(SwiftExtractor),
        ];

        // Medium
        #[cfg(feature = "lang-dart")]
        extractors.push(Box::new(DartExtractor));
        #[cfg(feature = "lang-pascal")]
        extractors.push(Box::new(PascalExtractor));
        #[cfg(feature = "lang-php")]
        extractors.push(Box::new(PhpExtractor));
        #[cfg(feature = "lang-ruby")]
        extractors.push(Box::new(RubyExtractor));
        #[cfg(feature = "lang-bash")]
        extractors.push(Box::new(BashExtractor));
        #[cfg(feature = "lang-protobuf")]
        extractors.push(Box::new(ProtoExtractor));
        #[cfg(feature = "lang-powershell")]
        extractors.push(Box::new(PowerShellExtractor));
        #[cfg(feature = "lang-nix")]
        extractors.push(Box::new(NixExtractor));
        #[cfg(feature = "lang-vbnet")]
        extractors.push(Box::new(VbNetExtractor));

        // Full
        #[cfg(feature = "lang-lua")]
        extractors.push(Box::new(LuaExtractor));
        #[cfg(feature = "lang-zig")]
        extractors.push(Box::new(ZigExtractor));
        #[cfg(feature = "lang-objc")]
        extractors.push(Box::new(ObjcExtractor));
        #[cfg(feature = "lang-perl")]
        extractors.push(Box::new(PerlExtractor));
        #[cfg(feature = "lang-batch")]
        extractors.push(Box::new(BatchExtractor));
        #[cfg(feature = "lang-fortran")]
        extractors.push(Box::new(FortranExtractor));
        #[cfg(feature = "lang-cobol")]
        extractors.push(Box::new(CobolExtractor));
        #[cfg(feature = "lang-msbasic2")]
        extractors.push(Box::new(MsBasic2Extractor));
        #[cfg(feature = "lang-gwbasic")]
        extractors.push(Box::new(GwBasicExtractor));
        #[cfg(feature = "lang-qbasic")]
        extractors.push(Box::new(QBasicExtractor));
        #[cfg(feature = "lang-qbasic")]
        extractors.push(Box::new(QuickBasicExtractor));
        #[cfg(feature = "lang-dockerfile")]
        extractors.push(Box::new(DockerfileExtractor));

        Self { extractors }
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
