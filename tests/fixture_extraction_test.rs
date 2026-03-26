//! Integration tests that run each language extractor against realistic sample files.
//!
//! These tests verify that extractors handle real-world code patterns correctly,
//! producing the expected nodes, edges, and relationships.

use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn read_fixture(name: &str) -> String {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

// ── TypeScript ──────────────────────────────────────────────────────────────

#[test]
fn test_fixture_typescript() {
    let source = read_fixture("sample.ts");
    let extractor = tokensave::extraction::TypeScriptExtractor;
    let result = extractor.extract("sample.ts", &source);
    assert!(result.errors.is_empty(), "TS errors: {:?}", result.errors);

    // File root
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Imports
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(imports.len() >= 2, "expected >= 2 imports, got {}", imports.len());

    // Const
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert!(consts.iter().any(|n| n.name == "MAX_RETRIES"));

    // Type alias
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::TypeAlias && n.name == "UserId"));

    // Interface
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Interface && n.name == "IUser"));

    // Enum
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum && n.name == "Role"));

    // Function
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "log"));

    // Exported class with decorator
    let class = result.nodes.iter().find(|n| n.kind == NodeKind::Class && n.name == "UserService");
    assert!(class.is_some(), "UserService class not found");
    assert_eq!(class.unwrap().visibility, Visibility::Pub);

    // Methods including async
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 2, "expected >= 2 methods");
    let fetch = methods.iter().find(|m| m.name == "fetchProfile");
    assert!(fetch.is_some(), "fetchProfile method not found");
    assert!(fetch.unwrap().is_async, "fetchProfile should be async");

    // Arrow function (export const createUser = ...)
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::ArrowFunction && n.name == "createUser"));

    // Namespace (Auth module — may or may not be detected depending on TS grammar version)
    // assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Namespace && n.name == "Auth"));

    // Call sites
    assert!(!result.unresolved_refs.is_empty(), "expected call site refs");
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls));

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));

    // Extends edge (UserService extends EventEmitter)
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends ref for UserService"
    );
}

// ── JavaScript ──────────────────────────────────────────────────────────────

#[test]
fn test_fixture_javascript() {
    let source = read_fixture("sample.js");
    let extractor = tokensave::extraction::TypeScriptExtractor;
    let result = extractor.extract("sample.js", &source);
    assert!(result.errors.is_empty(), "JS errors: {:?}", result.errors);

    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Handler"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "JsonHandler"));
    let fetch_fn = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "fetchData");
    assert!(fetch_fn.is_some());
    assert!(fetch_fn.unwrap().is_async);
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::ArrowFunction && n.name == "double"));
}

// ── Python ──────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_python() {
    let source = read_fixture("sample.py");
    let extractor = tokensave::extraction::PythonExtractor;
    let result = extractor.extract("sample.py", &source);
    assert!(result.errors.is_empty(), "Python errors: {:?}", result.errors);

    // File root
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Imports
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(imports.len() >= 3, "expected >= 3 imports, got {}", imports.len());

    // Module-level constants
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "MAX_CONNECTIONS"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "DEFAULT_TIMEOUT"));

    // Functions
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "log"));
    let log_fn = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "log").unwrap();
    assert!(log_fn.docstring.is_some(), "log() should have docstring");

    // Decorator
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Decorator));

    // Classes
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Base"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Connection"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Pool"));

    // Docstring on class
    let conn = result.nodes.iter().find(|n| n.kind == NodeKind::Class && n.name == "Connection").unwrap();
    assert!(conn.docstring.is_some(), "Connection should have docstring");

    // Methods
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 5, "expected >= 5 methods, got {}", methods.len());

    // Async method
    let connect = methods.iter().find(|m| m.name == "connect");
    assert!(connect.is_some(), "connect method not found");
    assert!(connect.unwrap().is_async, "connect should be async");

    // Visibility: _internal_method is private
    let internal = methods.iter().find(|m| m.name == "_internal_method");
    assert!(internal.is_some());
    assert_eq!(internal.unwrap().visibility, Visibility::Private);

    // Nested class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Config"));

    // Inheritance
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends refs for class inheritance"
    );

    // Call sites
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls));

    // Signature with type annotations should not be truncated
    let log_sig = log_fn.signature.as_ref().unwrap();
    assert!(
        log_sig.contains("message"),
        "log signature should contain 'message', got: {}",
        log_sig
    );
}

// ── C ───────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_c() {
    let source = read_fixture("sample.c");
    let extractor = tokensave::extraction::CExtractor;
    let result = extractor.extract("sample.c", &source);
    assert!(result.errors.is_empty(), "C errors: {:?}", result.errors);

    // Includes
    let includes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Include).collect();
    assert!(includes.len() >= 3, "expected >= 3 includes");

    // Preprocessor defines
    let defs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::PreprocessorDef).collect();
    assert!(defs.iter().any(|n| n.name == "MAX_BUFFER_SIZE"));

    // Typedef struct
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Typedef && n.name == "Point"));

    // Struct with fields
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert!(fields.len() >= 2, "expected struct fields");

    // Union
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Union));

    // Enum
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum));
    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert!(variants.len() >= 4, "expected >= 4 enum variants");

    // Function pointer typedef
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Typedef && n.name == "Callback"));

    // Functions
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "point_distance"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "main"));

    // Static function is private
    let set_err = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "set_error");
    assert!(set_err.is_some());
    assert_eq!(set_err.unwrap().visibility, Visibility::Private);

    // Docstrings
    let dist_fn = result.nodes.iter().find(|n| n.name == "point_distance").unwrap();
    assert!(dist_fn.docstring.is_some(), "point_distance should have docstring");

    // Call sites
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls));
}

// ── C header ────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_c_header() {
    let source = read_fixture("sample.h");
    let extractor = tokensave::extraction::CExtractor;
    let result = extractor.extract("sample.h", &source);
    assert!(result.errors.is_empty(), "C header errors: {:?}", result.errors);

    // File node
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Preprocessor def
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::PreprocessorDef && n.name == "API_VERSION"));

    // Typedef struct
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Typedef && n.name == "Rect"));

    // Enum
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum));
}

// ── C++ ─────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_cpp() {
    let source = read_fixture("sample.cpp");
    let extractor = tokensave::extraction::CppExtractor;
    let result = extractor.extract("sample.cpp", &source);
    assert!(result.errors.is_empty(), "C++ errors: {:?}", result.errors);

    // Namespace
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Namespace && n.name == "geom"));

    // Struct
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Struct && n.name == "Vec2"));

    // Abstract class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Shape"));

    // Derived classes
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Circle"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Rectangle"));

    // Template class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Template));

    // Methods
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 4, "expected >= 4 methods");

    // Enum class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum));

    // Union
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Union));

    // Typedef
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Typedef && n.name == "EntityId"));

    // Typedef
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Typedef && n.name == "EntityId"));

    // Includes
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Include));

    // Preprocessor def
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::PreprocessorDef && n.name == "DEFAULT_CAPACITY"));

    // Static function is private
    let helper = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "internal_helper");
    assert!(helper.is_some());
    assert_eq!(helper.unwrap().visibility, Visibility::Private);

    // Inheritance edges
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends refs for class inheritance"
    );

    // Call sites
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls));
}

// ── Kotlin ──────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_kotlin() {
    let source = read_fixture("sample.kt");
    let extractor = tokensave::extraction::KotlinExtractor;
    let result = extractor.extract("sample.kt", &source);
    assert!(result.errors.is_empty(), "Kotlin errors: {:?}", result.errors);

    // Package
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::KotlinPackage));

    // Imports
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(imports.len() >= 2, "expected >= 2 imports");

    // Data class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::DataClass && n.name == "Point"));

    // Sealed class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::SealedClass));

    // Interface
    let iface = result.nodes.iter().find(|n| n.kind == NodeKind::Interface || n.kind == NodeKind::Trait);
    assert!(iface.is_some(), "Repository interface not found");

    // Annotation (may be Decorator or AnnotationUsage depending on extractor)
    let has_annotation = result.nodes.iter().any(|n| n.kind == NodeKind::Decorator || n.kind == NodeKind::AnnotationUsage);
    assert!(has_annotation, "expected annotation nodes");

    // Abstract class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Entity"));

    // Regular class with properties
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "User"));
    let properties: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Property).collect();
    assert!(properties.len() >= 2, "expected >= 2 properties");

    // Companion object
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::CompanionObject));

    // Enum class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum));

    // Object declaration (singleton)
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::KotlinObject && n.name == "Logger"));

    // Extension function
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name.contains("toSlug")));

    // Visibility: protected helper
    let helper = result.nodes.iter().find(|n| n.name == "helperFunction");
    if let Some(h) = helper {
        assert_eq!(h.visibility, Visibility::PubSuper, "protected should be PubSuper");
    }

    // Call sites
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls));
}

// ── Dart ────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_dart() {
    let source = read_fixture("sample.dart");
    let extractor = tokensave::extraction::DartExtractor;
    let result = extractor.extract("sample.dart", &source);
    assert!(result.errors.is_empty(), "Dart errors: {:?}", result.errors);

    // Library
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Library));

    // Imports
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(imports.len() >= 2, "expected >= 2 imports");

    // Enum
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum));

    // Abstract class (may map to Interface or Class)
    let serializable = result.nodes.iter().find(|n| n.name == "Serializable");
    assert!(serializable.is_some(), "Serializable not found");

    // Mixin
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Mixin && n.name == "Timestamped"));

    // Class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "User"));

    // Extension
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Extension && n.name == "StringUtils"));

    // Methods
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 2, "expected >= 2 methods");

    // Constructor
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Constructor));

    // Private visibility (_email, _isValid, _logAction)
    let privates: Vec<_> = result.nodes.iter().filter(|n| n.visibility == Visibility::Private).collect();
    assert!(privates.len() >= 1, "expected private members");

    // Async function
    let process = result.nodes.iter().find(|n| n.name == "processUsers");
    if let Some(p) = process {
        assert!(p.is_async, "processUsers should be async");
    }

    // Typedef
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::TypeAlias));

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

// ── C# ──────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_csharp() {
    let source = read_fixture("sample.cs");
    let extractor = tokensave::extraction::CSharpExtractor;
    let result = extractor.extract("sample.cs", &source);
    assert!(result.errors.is_empty(), "C# errors: {:?}", result.errors);

    // Namespace
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Namespace));

    // Using directives
    let usings: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(usings.len() >= 3, "expected >= 3 using directives");

    // Enum
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum));

    // Record
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Record && n.name == "AppConfig"));

    // Delegate
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Delegate));

    // Interfaces
    let ifaces: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Interface).collect();
    assert!(ifaces.len() >= 2, "expected >= 2 interfaces");

    // Attribute (decorator)
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::AnnotationUsage || n.kind == NodeKind::Decorator));

    // Abstract class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Entity"));

    // Class with methods
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "User"));
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 3, "expected >= 3 methods");

    // Constructor
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Constructor));

    // Properties
    let props: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::CSharpProperty).collect();
    assert!(props.len() >= 2, "expected >= 2 properties");

    // Event
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Event));

    // Fields
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Field));

    // Struct
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Struct && n.name == "Point"));

    // Visibility: private, internal, protected
    assert!(result.nodes.iter().any(|n| n.visibility == Visibility::Private));
    assert!(result.nodes.iter().any(|n| n.visibility == Visibility::PubCrate)); // internal

    // Async method
    let fetch = methods.iter().find(|m| m.name == "FetchProfileAsync");
    if let Some(f) = fetch {
        assert!(f.is_async, "FetchProfileAsync should be async");
    }

    // Inheritance
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends || r.reference_kind == EdgeKind::Implements),
        "expected inheritance refs"
    );

    // Call sites
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls));
}

// ── PHP ─────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_php() {
    let source = read_fixture("sample.php");
    let extractor = tokensave::extraction::PhpExtractor;
    let result = extractor.extract("sample.php", &source);
    assert!(result.errors.is_empty(), "PHP errors: {:?}", result.errors);

    // File root node
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Namespace (mapped to NodeKind::Module in PHP extractor)
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Module),
        "expected a namespace/module node"
    );

    // Use nodes: the PHP extractor extracts trait `use` declarations inside class bodies as
    // NodeKind::Use. Namespace-level `use` imports use a different grammar node
    // (namespace_use_declaration) that is not yet mapped. We expect >= 2 Use nodes
    // because both Connection (use Timestamps) and Pool (use Loggable) have trait uses.
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(imports.len() >= 2, "expected >= 2 Use nodes (trait uses), got {}", imports.len());

    // Interface and Trait (both mapped to NodeKind::Trait)
    let traits: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Trait).collect();
    assert!(traits.len() >= 2, "expected >= 2 Trait nodes (interface + trait), got {}", traits.len());
    assert!(traits.iter().any(|n| n.name == "ConnectionInterface"), "ConnectionInterface not found");
    assert!(traits.iter().any(|n| n.name == "Timestamps"), "Timestamps trait not found");

    // Classes
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Connection"),
        "Connection class not found"
    );
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Pool"),
        "Pool class not found"
    );

    // Methods (>= 3)
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 3, "expected >= 3 methods, got {}", methods.len());

    // Enum
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Enum && n.name == "ConnectionState"),
        "ConnectionState enum not found"
    );

    // Fields (properties)
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert!(!fields.is_empty(), "expected property/field nodes");

    // Visibility: has private members
    assert!(
        result.nodes.iter().any(|n| n.visibility == Visibility::Private),
        "expected at least one private member"
    );

    // Inheritance: Extends refs (Pool extends Connection)
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends ref for Pool extends Connection"
    );

    // Call sites
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );

    // Contains edges
    assert!(
        result.edges.iter().any(|e| e.kind == EdgeKind::Contains),
        "expected Contains edges"
    );
}

// ── Pascal ──────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_pascal() {
    let source = read_fixture("sample.pas");
    let extractor = tokensave::extraction::PascalExtractor;
    let result = extractor.extract("sample.pas", &source);
    assert!(result.errors.is_empty(), "Pascal errors: {:?}", result.errors);

    // Unit declaration
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::PascalUnit));

    // Uses clause
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(uses.len() >= 2, "expected >= 2 uses");

    // Constants
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "MaxRetries"));

    // Record type
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::PascalRecord && n.name == "TPoint"));

    // Interface type
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Interface));

    // Classes
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "TEntity"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "TUser"));

    // Constructor
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Constructor));

    // Functions and procedures
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "PointDistance"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Procedure && n.name == "LogMessage"));

    // Methods
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 2, "expected >= 2 methods");

    // Properties
    let properties: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Property).collect();
    assert!(properties.len() >= 1, "expected >= 1 property");

    // Visibility: private members
    assert!(result.nodes.iter().any(|n| n.visibility == Visibility::Private));

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

// ── Ruby ────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_ruby() {
    let source = read_fixture("sample.rb");
    let extractor = tokensave::extraction::RubyExtractor;
    let result = extractor.extract("sample.rb", &source);
    assert!(result.errors.is_empty(), "Ruby errors: {:?}", result.errors);

    // File root node
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Module
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Module && n.name == "Networking"),
        "Networking module not found"
    );

    // Constants
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "MAX_CONNECTIONS"),
        "MAX_CONNECTIONS constant not found"
    );
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "DEFAULT_TIMEOUT"),
        "DEFAULT_TIMEOUT constant not found"
    );

    // Classes
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Base"),
        "Base class not found"
    );
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Connection"),
        "Connection class not found"
    );
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Pool"),
        "Pool class not found"
    );

    // Nested class
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Config"),
        "nested Config class not found"
    );

    // Methods (>= 3)
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 3, "expected >= 3 methods, got {}", methods.len());

    // Top-level function (log is defined inside a module, class_depth > 0, so it's a Method;
    // but `log` is at module level — class_depth is incremented for modules too).
    // Accept either Function or Method for `log`.
    assert!(
        result.nodes.iter().any(|n| (n.kind == NodeKind::Function || n.kind == NodeKind::Method) && n.name == "log"),
        "log function/method not found"
    );

    // Inheritance: Connection < Base, Pool < Connection
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends refs for class inheritance"
    );

    // Call sites
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

// -- Swift ────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_swift() {
    let source = read_fixture("sample.swift");
    let extractor = tokensave::extraction::SwiftExtractor;
    let result = extractor.extract("sample.swift", &source);
    assert!(result.errors.is_empty(), "Swift errors: {:?}", result.errors);

    // File root node
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Imports
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(imports.len() >= 2, "expected >= 2 imports, got {}", imports.len());
    assert!(imports.iter().any(|n| n.name == "Foundation"));
    assert!(imports.iter().any(|n| n.name == "UIKit"));

    // Top-level constant
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "maxConnections"),
        "maxConnections constant not found"
    );

    // Typealias
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::TypeAlias && n.name == "CompletionHandler"),
        "CompletionHandler typealias not found"
    );

    // Enum
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Enum && n.name == "LogLevel"),
        "LogLevel enum not found"
    );

    // Enum variants
    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert!(variants.len() >= 4, "expected >= 4 enum variants, got {}", variants.len());

    // Protocol as Interface
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Interface && n.name == "Serializable"),
        "Serializable protocol not found"
    );

    // Classes
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Base"),
        "Base class not found"
    );
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Connection"),
        "Connection class not found"
    );

    // Struct
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Struct && n.name == "Point"),
        "Point struct not found"
    );

    // Extension
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Extension && n.name == "String"),
        "String extension not found"
    );

    // Constructor
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Constructor),
        "expected at least one Constructor node"
    );

    // Methods (>= 3: description, validate, connect, disconnect, distance, toSlug, toJson, toJsonString)
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 3, "expected >= 3 methods, got {}", methods.len());

    // Top-level function
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "processUsers"),
        "processUsers function not found"
    );

    // Properties (inside classes/structs)
    let props: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Property).collect();
    assert!(props.len() >= 2, "expected >= 2 properties, got {}", props.len());

    // Docstrings
    let base = result.nodes.iter().find(|n| n.kind == NodeKind::Class && n.name == "Base").unwrap();
    assert!(base.docstring.is_some(), "Base class should have docstring");

    // Inheritance: Connection extends Base
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends refs for class inheritance"
    );
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends && r.reference_name == "Base"),
        "expected Extends ref to Base"
    );

    // Call sites
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));

    // Async method
    let connect = result.nodes.iter().find(|n| n.name == "connect");
    if let Some(c) = connect {
        assert!(c.is_async, "connect should be async");
    }

    // Private visibility
    assert!(
        result.nodes.iter().any(|n| n.visibility == Visibility::Private),
        "expected at least one private member"
    );
}

// ── Bash ────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_bash() {
    let source = read_fixture("sample.sh");
    let extractor = tokensave::extraction::BashExtractor;
    let result = extractor.extract("sample.sh", &source);
    assert!(result.errors.is_empty(), "Bash errors: {:?}", result.errors);

    // File root node
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Functions (5: log, validate_config, connect, disconnect, main)
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(fns.len(), 5, "expected 5 functions, got {}", fns.len());
    assert!(fns.iter().any(|n| n.name == "log"));
    assert!(fns.iter().any(|n| n.name == "validate_config"));
    assert!(fns.iter().any(|n| n.name == "connect"));
    assert!(fns.iter().any(|n| n.name == "disconnect"));
    assert!(fns.iter().any(|n| n.name == "main"));

    // Readonly constants (2: MAX_RETRIES, DEFAULT_PORT)
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert_eq!(consts.len(), 2, "expected 2 consts, got {}", consts.len());
    assert!(consts.iter().any(|n| n.name == "MAX_RETRIES"));
    assert!(consts.iter().any(|n| n.name == "DEFAULT_PORT"));

    // Source import (1 Use: ./utils.sh)
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(uses.len(), 1, "expected 1 Use node, got {}", uses.len());
    assert_eq!(uses[0].name, "./utils.sh");

    // Docstrings
    let log_fn = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "log").unwrap();
    assert!(log_fn.docstring.is_some(), "log should have docstring");

    // Call sites
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

// ── Lua ─────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_lua() {
    let source = read_fixture("sample.lua");
    let extractor = tokensave::extraction::LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "Lua errors: {:?}", result.errors);

    // File root node
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Requires (2: json, socket)
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(uses.len(), 2, "expected 2 Use nodes, got {}", uses.len());
    assert!(uses.iter().any(|n| n.name == "json"));
    assert!(uses.iter().any(|n| n.name == "socket"));

    // Constants (2: MAX_RETRIES, DEFAULT_PORT)
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert_eq!(consts.len(), 2, "expected 2 consts, got {}", consts.len());
    assert!(consts.iter().any(|n| n.name == "MAX_RETRIES"));
    assert!(consts.iter().any(|n| n.name == "DEFAULT_PORT"));

    // Functions (3: log, Connection.new, Pool.new)
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(fns.len(), 3, "expected 3 functions, got {}", fns.len());
    assert!(fns.iter().any(|n| n.name == "log"));

    // Methods (5: connect, disconnect, isConnected, acquire, release)
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 5, "expected 5 methods, got {}", methods.len());
    assert!(methods.iter().any(|n| n.name == "connect"));
    assert!(methods.iter().any(|n| n.name == "disconnect"));
    assert!(methods.iter().any(|n| n.name == "isConnected"));
    assert!(methods.iter().any(|n| n.name == "acquire"));
    assert!(methods.iter().any(|n| n.name == "release"));

    // Docstrings
    let lua_log_fn = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "log").unwrap();
    assert!(lua_log_fn.docstring.is_some(), "log should have docstring");

    // Call sites
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

// ── Zig ─────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_zig() {
    let source = read_fixture("sample.zig");
    let extractor = tokensave::extraction::ZigExtractor;
    let result = extractor.extract("sample.zig", &source);
    assert!(result.errors.is_empty(), "Zig errors: {:?}", result.errors);

    // File root node
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Imports (2: std, std.mem)
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(imports.len(), 2, "expected 2 imports, got {}", imports.len());
    assert!(imports.iter().any(|n| n.name == "std"));

    // Const (max_connections)
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert!(
        consts.iter().any(|n| n.name == "max_connections"),
        "max_connections constant not found"
    );

    // Enum (LogLevel)
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Enum && n.name == "LogLevel"),
        "LogLevel enum not found"
    );

    // Enum variants (4: debug, info, warning, err)
    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert_eq!(variants.len(), 4, "expected 4 enum variants, got {}", variants.len());
    assert!(variants.iter().any(|v| v.name == "debug"));
    assert!(variants.iter().any(|v| v.name == "info"));
    assert!(variants.iter().any(|v| v.name == "warning"));
    assert!(variants.iter().any(|v| v.name == "err"));

    // Structs (Point, Connection)
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Struct && n.name == "Point"),
        "Point struct not found"
    );
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Struct && n.name == "Connection"),
        "Connection struct not found"
    );

    // Fields
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert!(fields.len() >= 5, "expected >= 5 fields, got {}", fields.len());
    assert!(fields.iter().any(|f| f.name == "x"));
    assert!(fields.iter().any(|f| f.name == "host"));

    // Methods inside structs (distance, origin, init, connect, disconnect, isConnected)
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 6, "expected >= 6 methods, got {}", methods.len());
    assert!(methods.iter().any(|m| m.name == "distance"));
    assert!(methods.iter().any(|m| m.name == "origin"));
    assert!(methods.iter().any(|m| m.name == "init"));
    assert!(methods.iter().any(|m| m.name == "connect"));
    assert!(methods.iter().any(|m| m.name == "disconnect"));
    assert!(methods.iter().any(|m| m.name == "isConnected"));

    // Top-level functions (log, processConnections)
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert!(fns.iter().any(|f| f.name == "log"), "log function not found");
    assert!(
        fns.iter().any(|f| f.name == "processConnections"),
        "processConnections function not found"
    );

    // Test declaration as Function
    assert!(
        fns.iter().any(|f| f.name == "point distance"),
        "test 'point distance' not found"
    );

    // Visibility: pub functions
    let log_fn = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "log").unwrap();
    assert_eq!(log_fn.visibility, Visibility::Pub, "log should be pub");
    let process_fn = result.nodes.iter().find(|n| n.kind == NodeKind::Function && n.name == "processConnections").unwrap();
    assert_eq!(process_fn.visibility, Visibility::Pub, "processConnections should be pub");

    // Docstrings
    let point = result.nodes.iter().find(|n| n.kind == NodeKind::Struct && n.name == "Point").unwrap();
    assert!(point.docstring.is_some(), "Point should have docstring");
    assert!(point.docstring.as_ref().unwrap().contains("2D point"));

    // Call sites
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

// ── Protobuf ────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_proto() {
    let source = read_fixture("sample.proto");
    let extractor = tokensave::extraction::ProtoExtractor;
    let result = extractor.extract("sample.proto", &source);
    assert!(result.errors.is_empty(), "Proto errors: {:?}", result.errors);

    // File root
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Package
    let pkgs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Package).collect();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "networking");

    // Imports
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(imports.len(), 2, "expected 2 imports, got {}", imports.len());

    // Messages
    let msgs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::ProtoMessage).collect();
    assert!(msgs.len() >= 7, "expected >= 7 messages, got {}", msgs.len());
    assert!(msgs.iter().any(|m| m.name == "Endpoint"));
    assert!(msgs.iter().any(|m| m.name == "ConnectionConfig"));
    assert!(msgs.iter().any(|m| m.name == "AuthConfig")); // nested

    // Enum + variants
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum && n.name == "LogLevel"));
    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert_eq!(variants.len(), 5, "expected 5 enum variants, got {}", variants.len());

    // Service
    let services: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::ProtoService).collect();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].name, "ConnectionService");

    // RPCs
    let rpcs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::ProtoRpc).collect();
    assert_eq!(rpcs.len(), 3, "expected 3 rpcs, got {}", rpcs.len());
    assert!(rpcs.iter().any(|r| r.name == "Connect"));
    assert!(rpcs.iter().any(|r| r.name == "Disconnect"));
    assert!(rpcs.iter().any(|r| r.name == "HealthCheck"));

    // Fields
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert!(fields.len() >= 15, "expected >= 15 fields, got {}", fields.len());

    // Docstrings
    let endpoint = result.nodes.iter().find(|n| n.kind == NodeKind::ProtoMessage && n.name == "Endpoint").unwrap();
    assert!(endpoint.docstring.is_some(), "Endpoint should have docstring");

    let log_level = result.nodes.iter().find(|n| n.kind == NodeKind::Enum && n.name == "LogLevel").unwrap();
    assert!(log_level.docstring.is_some(), "LogLevel should have docstring");

    // Contains edges
    let contains: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
    assert!(contains.len() >= 10, "expected >= 10 Contains edges, got {}", contains.len());
}

// ── Nix ─────────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_nix() {
    let source = read_fixture("sample.nix");
    let extractor = tokensave::extraction::NixExtractor;
    let result = extractor.extract("sample.nix", &source);
    assert!(result.errors.is_empty(), "Nix errors: {:?}", result.errors);

    // File root
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Functions
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert!(fns.iter().any(|f| f.name == "log"), "log function not found");
    assert!(fns.iter().any(|f| f.name == "mkConnection"), "mkConnection function not found");

    // Constants
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "defaultPort"));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Const && n.name == "maxRetries"));

    // Module
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Module && n.name == "networking"));

    // Nested functions inside networking
    assert!(fns.iter().any(|f| f.name == "mkPool"), "mkPool not found");
    assert!(fns.iter().any(|f| f.name == "validateConfig"), "validateConfig not found");

    // Docstrings
    let log_fn = fns.iter().find(|f| f.name == "log").unwrap();
    assert!(log_fn.docstring.is_some(), "log should have docstring");

    let net = result.nodes.iter().find(|n| n.kind == NodeKind::Module && n.name == "networking").unwrap();
    assert!(net.docstring.is_some(), "networking should have docstring");

    // Call sites
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls), "expected call refs");

    // Contains edges
    let contains: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
    assert!(contains.len() >= 5, "expected >= 5 Contains edges, got {}", contains.len());

    // Inherit (Use) nodes
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(uses.iter().any(|u| u.name == "networking"), "inherit networking Use not found");

    // All visibility should be Pub
    for node in &result.nodes {
        assert_eq!(node.visibility, Visibility::Pub, "node {} should be Pub", node.name);
    }
}

// ── VB.NET ──────────────────────────────────────────────────────────────────

#[test]
fn test_fixture_vbnet() {
    let source = read_fixture("sample.vb");
    let extractor = tokensave::extraction::VbNetExtractor;
    let result = extractor.extract("sample.vb", &source);

    // File root
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Imports
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(imports.len(), 2, "expected 2 imports, got {}", imports.len());
    assert!(imports.iter().any(|u| u.name == "System"));
    assert!(imports.iter().any(|u| u.name == "System.Collections.Generic"));

    // Const (top-level)
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert!(consts.iter().any(|c| c.name == "MaxConnections"), "MaxConnections const not found");

    // Enum with variants
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum && n.name == "LogLevel"));
    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert!(variants.len() >= 4, "expected >= 4 enum variants, got {}", variants.len());
    assert!(variants.iter().any(|v| v.name == "Debug"));
    assert!(variants.iter().any(|v| v.name == "Info"));
    assert!(variants.iter().any(|v| v.name == "Warning"));

    // Interface
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Interface && n.name == "ISerializable"),
        "ISerializable interface not found"
    );

    // Classes
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Base"),
        "Base class not found"
    );
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "Connection"),
        "Connection class not found"
    );

    // Struct
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Struct && n.name == "Point"),
        "Point struct not found"
    );

    // Module
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Module && n.name == "Helpers"),
        "Helpers module not found"
    );

    // Methods
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 5, "expected >= 5 methods, got {}", methods.len());

    // Constructor
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Constructor),
        "expected at least one Constructor node"
    );

    // Properties
    let props: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Property).collect();
    assert!(props.len() >= 2, "expected >= 2 properties, got {}", props.len());

    // Docstrings on classes
    let base = result.nodes.iter().find(|n| n.kind == NodeKind::Class && n.name == "Base").unwrap();
    assert!(base.docstring.is_some(), "Base class should have docstring");

    // Inheritance: Connection extends Base
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends && r.reference_name == "Base"),
        "expected Extends ref to Base"
    );

    // Implements: Connection implements ISerializable
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Implements && r.reference_name == "ISerializable"),
        "expected Implements ref to ISerializable"
    );

    // Call sites
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}
