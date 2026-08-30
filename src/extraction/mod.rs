// Lite — always available (no cfg needed)
mod astro_extractor;
pub(crate) mod c_api_macro;
mod c_extractor;
mod cpp_extractor;
mod csharp_extractor;
mod go_extractor;
mod java_extractor;
mod kotlin_extractor;
mod python_extractor;
mod rust_extractor;
mod scala_extractor;
mod svelte_extractor;
mod swift_extractor;
mod typescript_extractor;

pub mod complexity;
pub mod ts_provider;
mod ts_state;

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
#[cfg(feature = "lang-actionscript")]
mod actionscript_extractor;
#[cfg(feature = "lang-batch")]
mod batch_extractor;
#[cfg(feature = "lang-canvas")]
mod canvas_extractor;
#[cfg(feature = "lang-clojure")]
mod clojure_extractor;
#[cfg(feature = "lang-cobol")]
mod cobol_extractor;
#[cfg(feature = "lang-cuda")]
mod cuda_extractor;
#[cfg(feature = "lang-dockerfile")]
mod dockerfile_extractor;
#[cfg(feature = "lang-elixir")]
mod elixir_extractor;
#[cfg(feature = "lang-erlang")]
mod erlang_extractor;
#[cfg(feature = "lang-fortran")]
mod fortran_extractor;
#[cfg(feature = "lang-fsharp")]
mod fsharp_extractor;
#[cfg(feature = "lang-fstar")]
mod fstar_extractor;
#[cfg(feature = "lang-gdscript")]
mod gdscript_extractor;
#[cfg(feature = "lang-glsl")]
mod glsl_extractor;
#[cfg(feature = "lang-gwbasic")]
mod gwbasic_extractor;
#[cfg(feature = "lang-haskell")]
mod haskell_extractor;
#[cfg(feature = "lang-hlsl")]
mod hlsl_extractor;
#[cfg(feature = "lang-julia")]
mod julia_extractor;
#[cfg(feature = "lang-lean")]
mod lean_extractor;
#[cfg(feature = "lang-lua")]
mod lua_extractor;
#[cfg(feature = "lang-markdown")]
mod markdown_extractor;
#[cfg(feature = "lang-mcfunction")]
mod mcfunction_extractor;
#[cfg(feature = "lang-metal")]
mod metal_extractor;
#[cfg(feature = "lang-msbasic2")]
mod msbasic2_extractor;
#[cfg(feature = "lang-objc")]
mod objc_extractor;
#[cfg(feature = "lang-ocaml")]
mod ocaml_extractor;
#[cfg(feature = "lang-perl")]
mod perl_extractor;
#[cfg(feature = "lang-qbasic")]
pub(crate) mod qbasic_extractor;
#[cfg(feature = "lang-qbasic")]
mod quickbasic_extractor;
#[cfg(feature = "lang-quint")]
mod quint_extractor;
#[cfg(feature = "lang-r")]
mod r_extractor;
#[cfg(feature = "lang-sql")]
mod sql_extractor;
#[cfg(feature = "lang-systemverilog")]
mod systemverilog_extractor;
#[cfg(feature = "lang-toml")]
mod toml_extractor;
#[cfg(feature = "lang-wgsl")]
mod wgsl_extractor;
#[cfg(feature = "lang-xaml")]
mod xaml_extractor;
#[cfg(feature = "lang-zig")]
mod zig_extractor;

// Lite — always available (no cfg needed)
pub use astro_extractor::AstroExtractor;
pub use c_extractor::CExtractor;
pub use cpp_extractor::CppExtractor;
pub use csharp_extractor::CSharpExtractor;
pub use go_extractor::GoExtractor;
pub use java_extractor::JavaExtractor;
pub use kotlin_extractor::KotlinExtractor;
pub use python_extractor::PythonExtractor;
pub use rust_extractor::RustExtractor;
pub use scala_extractor::ScalaExtractor;
pub use svelte_extractor::SvelteExtractor;
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
#[cfg(feature = "lang-actionscript")]
pub use actionscript_extractor::ActionScriptExtractor;
#[cfg(feature = "lang-batch")]
pub use batch_extractor::BatchExtractor;
#[cfg(feature = "lang-canvas")]
pub use canvas_extractor::CanvasExtractor;
#[cfg(feature = "lang-clojure")]
pub use clojure_extractor::ClojureExtractor;
#[cfg(feature = "lang-cobol")]
pub use cobol_extractor::CobolExtractor;
#[cfg(feature = "lang-cuda")]
pub use cuda_extractor::CudaExtractor;
#[cfg(feature = "lang-dockerfile")]
pub use dockerfile_extractor::DockerfileExtractor;
#[cfg(feature = "lang-elixir")]
pub use elixir_extractor::ElixirExtractor;
#[cfg(feature = "lang-erlang")]
pub use erlang_extractor::ErlangExtractor;
#[cfg(feature = "lang-fortran")]
pub use fortran_extractor::FortranExtractor;
#[cfg(feature = "lang-fsharp")]
pub use fsharp_extractor::FSharpExtractor;
#[cfg(feature = "lang-fstar")]
pub use fstar_extractor::FStarExtractor;
#[cfg(feature = "lang-gdscript")]
pub use gdscript_extractor::GdScriptExtractor;
#[cfg(feature = "lang-glsl")]
pub use glsl_extractor::GlslExtractor;
#[cfg(feature = "lang-gwbasic")]
pub use gwbasic_extractor::GwBasicExtractor;
#[cfg(feature = "lang-haskell")]
pub use haskell_extractor::HaskellExtractor;
#[cfg(feature = "lang-hlsl")]
pub use hlsl_extractor::HlslExtractor;
#[cfg(feature = "lang-julia")]
pub use julia_extractor::JuliaExtractor;
#[cfg(feature = "lang-lean")]
pub use lean_extractor::LeanExtractor;
#[cfg(feature = "lang-lua")]
pub use lua_extractor::LuaExtractor;
#[cfg(feature = "lang-markdown")]
pub use markdown_extractor::MarkdownExtractor;
#[cfg(feature = "lang-mcfunction")]
pub use mcfunction_extractor::McFunctionExtractor;
#[cfg(feature = "lang-metal")]
pub use metal_extractor::MetalExtractor;
#[cfg(feature = "lang-msbasic2")]
pub use msbasic2_extractor::MsBasic2Extractor;
#[cfg(feature = "lang-objc")]
pub use objc_extractor::ObjcExtractor;
#[cfg(feature = "lang-ocaml")]
pub use ocaml_extractor::OcamlExtractor;
#[cfg(feature = "lang-perl")]
pub use perl_extractor::PerlExtractor;
#[cfg(feature = "lang-qbasic")]
pub use qbasic_extractor::QBasicExtractor;
#[cfg(feature = "lang-qbasic")]
pub use quickbasic_extractor::QuickBasicExtractor;
#[cfg(feature = "lang-quint")]
pub use quint_extractor::QuintExtractor;
#[cfg(feature = "lang-r")]
pub use r_extractor::RExtractor;
#[cfg(feature = "lang-sql")]
pub use sql_extractor::SqlExtractor;
#[cfg(feature = "lang-systemverilog")]
pub use systemverilog_extractor::SystemVerilogExtractor;
#[cfg(feature = "lang-toml")]
pub use toml_extractor::TomlExtractor;
#[cfg(feature = "lang-wgsl")]
pub use wgsl_extractor::WgslExtractor;
#[cfg(feature = "lang-xaml")]
pub use xaml_extractor::XamlExtractor;
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
            Box::new(SvelteExtractor),
            Box::new(AstroExtractor),
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
        #[cfg(feature = "lang-actionscript")]
        extractors.push(Box::new(ActionScriptExtractor));
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
        #[cfg(feature = "lang-quint")]
        extractors.push(Box::new(QuintExtractor));
        #[cfg(feature = "lang-dockerfile")]
        extractors.push(Box::new(DockerfileExtractor));
        #[cfg(feature = "lang-glsl")]
        extractors.push(Box::new(GlslExtractor));
        #[cfg(feature = "lang-wgsl")]
        extractors.push(Box::new(WgslExtractor));
        #[cfg(feature = "lang-xaml")]
        extractors.push(Box::new(XamlExtractor));
        #[cfg(feature = "lang-hlsl")]
        extractors.push(Box::new(HlslExtractor));
        #[cfg(feature = "lang-systemverilog")]
        extractors.push(Box::new(SystemVerilogExtractor));
        #[cfg(feature = "lang-cuda")]
        extractors.push(Box::new(CudaExtractor));
        #[cfg(feature = "lang-metal")]
        extractors.push(Box::new(MetalExtractor));
        #[cfg(feature = "lang-markdown")]
        extractors.push(Box::new(MarkdownExtractor));
        #[cfg(feature = "lang-canvas")]
        extractors.push(Box::new(CanvasExtractor));
        #[cfg(feature = "lang-r")]
        extractors.push(Box::new(RExtractor));
        #[cfg(feature = "lang-sql")]
        extractors.push(Box::new(SqlExtractor));
        #[cfg(feature = "lang-julia")]
        extractors.push(Box::new(JuliaExtractor));
        #[cfg(feature = "lang-haskell")]
        extractors.push(Box::new(HaskellExtractor));
        #[cfg(feature = "lang-ocaml")]
        extractors.push(Box::new(OcamlExtractor));
        #[cfg(feature = "lang-clojure")]
        extractors.push(Box::new(ClojureExtractor));
        #[cfg(feature = "lang-erlang")]
        extractors.push(Box::new(ErlangExtractor));
        #[cfg(feature = "lang-elixir")]
        extractors.push(Box::new(ElixirExtractor));
        #[cfg(feature = "lang-fsharp")]
        extractors.push(Box::new(FSharpExtractor));
        #[cfg(feature = "lang-fstar")]
        extractors.push(Box::new(FStarExtractor));
        #[cfg(feature = "lang-lean")]
        extractors.push(Box::new(LeanExtractor));
        #[cfg(feature = "lang-toml")]
        extractors.push(Box::new(TomlExtractor));
        #[cfg(feature = "lang-gdscript")]
        extractors.push(Box::new(GdScriptExtractor));
        #[cfg(feature = "lang-mcfunction")]
        extractors.push(Box::new(McFunctionExtractor));

        Self { extractors }
    }

    /// Returns the extractor for a file path based on its extension.
    pub fn extractor_for_file(&self, path: &str) -> Option<&dyn LanguageExtractor> {
        let ext = path.rsplit('.').next()?;
        self.extractors
            .iter()
            .find(|e| e.extensions().contains(&ext))
            .map(std::convert::AsRef::as_ref)
    }

    /// `.h` is the one extension naming no single language, so its text settles it: C++ parsed as C
    /// yields no class node at all. Everything else routes by extension.
    pub fn extractor_for_source(&self, path: &str, source: &str) -> Option<&dyn LanguageExtractor> {
        if path
            .rsplit('.')
            .next()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("h"))
        {
            if let Some(extractor) = header_dialect(source).and_then(|lang| {
                self.extractors
                    .iter()
                    .find(|e| e.language_name() == lang)
                    .map(std::convert::AsRef::as_ref)
            }) {
                return Some(extractor);
            }
        }
        self.extractor_for_file(path)
    }

    /// Returns the extractor matching a language name (case-insensitive).
    /// A registered extension (e.g. `"py"`) is accepted as an alias.
    pub fn extractor_for_language(&self, language: &str) -> Option<&dyn LanguageExtractor> {
        let lang = language.trim();
        self.extractors
            .iter()
            .find(|e| {
                e.language_name().eq_ignore_ascii_case(lang)
                    || e.extensions().iter().any(|x| x.eq_ignore_ascii_case(lang))
            })
            .map(std::convert::AsRef::as_ref)
    }

    /// Returns all registered extractors.
    pub fn extractors(&self) -> &[Box<dyn LanguageExtractor>] {
        &self.extractors
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

/// A signature lifted from source carries its line breaks, indentation and blanked-macro runs.
pub(crate) fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `None` = plain C. Comments are skipped, so prose naming a class stays C, and an `extern "C"`
/// guard is deliberately no marker - such a header IS C to a C++ reader too.
pub(crate) fn header_dialect(source: &str) -> Option<&'static str> {
    const OBJC: [&str; 4] = ["@interface", "@protocol", "@implementation", "@property"];
    const CPP_PUNCTUATED: [&str; 5] = ["::", "public:", "private:", "protected:", "extern \"C++\""];
    const CPP_KEYWORDS: [&str; 17] = [
        "class",
        "namespace",
        "template",
        "virtual",
        "operator",
        "nullptr",
        "constexpr",
        "explicit",
        "friend",
        "override",
        "noexcept",
        "decltype",
        "typename",
        "mutable",
        "using",
        "static_cast",
        "reinterpret_cast",
    ];

    let mut in_block_comment = false;
    let mut cpp_seen = false;
    let mut record_bodies = RecordBodyDepth::default();
    for line in source.lines() {
        let code = strip_comments(line, &mut in_block_comment);
        if OBJC.iter().any(|m| code.contains(m)) {
            return Some("Objective-C");
        }
        cpp_seen = cpp_seen
            || CPP_PUNCTUATED.iter().any(|m| code.contains(m))
            || CPP_KEYWORDS.iter().any(|w| contains_word(&code, w))
            || (record_bodies.inside() && is_default_member_initializer(&code));
        record_bodies.consume(&code);
    }
    cpp_seen.then_some("C++")
}

/// A `class` body settles the dialect itself; in these two only a member init gives C++ away.
#[derive(Default)]
struct RecordBodyDepth {
    open: Vec<bool>,
    keyword_pending: bool,
    last_significant: Option<char>,
}

impl RecordBodyDepth {
    fn inside(&self) -> bool {
        self.open.last().copied().unwrap_or(false)
    }

    fn consume(&mut self, code: &str) {
        let mut word = String::new();
        for c in code.chars() {
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
                continue;
            }
            self.take_word(&word);
            word.clear();
            match c {
                // `int f(struct Row* r) {` names a struct and opens a
                // function body, so the `)` decides, not the keyword.
                '{' => {
                    let record = self.keyword_pending && self.last_significant != Some(')');
                    self.open.push(record);
                    self.keyword_pending = false;
                }
                '}' | ';' => {
                    if c == '}' {
                        self.open.pop();
                    }
                    self.keyword_pending = false;
                }
                _ => {}
            }
            if !c.is_whitespace() {
                self.last_significant = Some(c);
            }
        }
        self.take_word(&word);
    }

    fn take_word(&mut self, word: &str) {
        if word.is_empty() {
            return;
        }
        if word == "struct" || word == "union" {
            self.keyword_pending = true;
        }
        self.last_significant = word.chars().next_back();
    }
}

/// A member with a default value, which no C struct may carry; a `(` first means a default arg.
fn is_default_member_initializer(code: &str) -> bool {
    let Some((declaration, _)) = code.split_once('=') else {
        return false;
    };
    if declaration.contains('(') || declaration.contains('[') {
        return false;
    }
    let words = declaration
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .count();
    words >= 2
        && declaration
            .trim_end()
            .ends_with(|c: char| c.is_alphanumeric() || c == '_')
}

/// String literals are untracked - a `//` inside one costs a marker at worst, never invents one.
fn strip_comments(line: &str, in_block_comment: &mut bool) -> String {
    let mut code = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if *in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                *in_block_comment = false;
            }
            continue;
        }
        if c == '/' {
            match chars.peek() {
                Some('/') => break,
                Some('*') => {
                    chars.next();
                    *in_block_comment = true;
                    continue;
                }
                _ => {}
            }
        }
        code.push(c);
    }
    code
}

/// Whole identifier, never a substring of one.
fn contains_word(haystack: &str, word: &str) -> bool {
    let mut rest = haystack;
    while let Some(idx) = rest.find(word) {
        let tail = &rest[idx + word.len()..];
        let is_edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        if is_edge(rest[..idx].chars().next_back()) && is_edge(tail.chars().next()) {
            return true;
        }
        rest = tail;
    }
    false
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::types::NodeKind;

    fn language_of(path: &str, source: &str) -> Option<String> {
        let registry = LanguageRegistry::new();
        registry
            .extractor_for_source(path, source)
            .map(|e| e.language_name().to_string())
    }

    const CPP_HEADER: &str = r"
#pragma once

class Widget
{
public:
    void Start();
private:
    int Total;
};
";

    #[test]
    fn cpp_header_routes_to_cpp_not_c() {
        assert_eq!(language_of("widget.h", CPP_HEADER).as_deref(), Some("C++"));
        assert_eq!(language_of("widget.H", CPP_HEADER).as_deref(), Some("C++"));
    }

    #[test]
    fn cpp_header_yields_class_members_once_routed() {
        let registry = LanguageRegistry::new();
        let extractor = registry
            .extractor_for_source("widget.h", CPP_HEADER)
            .expect("an extractor for a .h");
        let result = extractor.extract("widget.h", CPP_HEADER);
        let kinds: Vec<_> = result
            .nodes
            .iter()
            .map(|n| (n.kind.clone(), n.name.as_str()))
            .collect();
        assert!(
            kinds.contains(&(NodeKind::Class, "Widget")),
            "nodes: {kinds:?}"
        );
        assert!(
            kinds.contains(&(NodeKind::Method, "Start")),
            "nodes: {kinds:?}"
        );
        assert!(
            kinds.contains(&(NodeKind::Field, "Total")),
            "nodes: {kinds:?}"
        );
    }

    #[test]
    fn plain_c_header_stays_c() {
        let source = r"
#ifndef LIST_H
#define LIST_H
struct List { int len; };
int list_len(struct List* l);
#endif
";
        assert_eq!(language_of("list.h", source).as_deref(), Some("C"));
    }

    #[test]
    fn c_header_naming_a_class_in_prose_stays_c() {
        let source = r"
/* Allocates one node per class of error; see the template in docs. */
// A namespace is not a thing here.
int alloc_node(int kind);
";
        assert_eq!(language_of("alloc.h", source).as_deref(), Some("C"));
    }

    #[test]
    fn extern_c_guard_alone_stays_c() {
        let source = r#"
#ifdef __cplusplus
extern "C" {
#endif
int api_call(int x);
#ifdef __cplusplus
}
#endif
"#;
        assert_eq!(language_of("api.h", source).as_deref(), Some("C"));
    }

    #[test]
    fn objc_header_routes_to_objc() {
        let source = r"
@interface Widget : NSObject
- (void)start;
@end
";
        assert_eq!(
            language_of("Widget.h", source).as_deref(),
            Some("Objective-C")
        );
    }

    #[test]
    fn other_extensions_ignore_the_source() {
        assert_eq!(language_of("widget.c", CPP_HEADER).as_deref(), Some("C"));
        assert_eq!(
            language_of("widget.hpp", CPP_HEADER).as_deref(),
            Some("C++")
        );
        assert_eq!(
            language_of("widget.rs", CPP_HEADER).as_deref(),
            Some("Rust")
        );
    }

    #[test]
    fn a_struct_with_default_member_values_is_cpp() {
        let source = r"
struct FCameraTunables
{
  float HalflifePos = 0.4f;
  int32 Steps = 3;
};
";
        assert_eq!(language_of("tunables.h", source).as_deref(), Some("C++"));
    }

    #[test]
    fn a_scope_operator_is_cpp() {
        let source = r"
struct FPose { FVector2D Offset; };
void reset(struct FPose* p);
static const int Limit = Limits::Max;
";
        assert_eq!(language_of("pose.h", source).as_deref(), Some("C++"));
    }

    #[test]
    fn a_c_initializer_outside_a_record_stays_c() {
        let source = r"
static const int kLimit = 4;
struct Row { int a; };
static struct Row rows[2] = { { 1 }, { 2 } };
int row_count(void);
";
        assert_eq!(language_of("rows.h", source).as_deref(), Some("C"));
    }

    #[test]
    fn a_c_function_body_initializer_stays_c() {
        let source = r"
struct Row { int a; };
static int row_first(struct Row* r) {
  int value = r->a;
  return value;
}
";
        assert_eq!(language_of("rows.h", source).as_deref(), Some("C"));
    }

    #[test]
    fn a_word_marker_needs_whole_identifier_boundaries() {
        let source = r"
int classify(int x);
struct namespaced { int templates; };
";
        assert_eq!(language_of("words.h", source).as_deref(), Some("C"));
    }
}
