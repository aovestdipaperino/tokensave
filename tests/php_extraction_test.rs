#[cfg(feature = "lang-php")]
mod php_tests {

use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::PhpExtractor;
use tokensave::types::*;

#[test]
fn test_php_file_node() {
    let source = r#"<?php
function hello() {}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("test.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.php");
}

#[test]
fn test_php_function() {
    let source = r#"<?php
function add(int $a, int $b): int {
    return $a + $b;
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("math.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "add");
}

#[test]
fn test_php_class_with_methods() {
    let source = r#"<?php
class User {
    private string $name;

    public function __construct(string $name) {
        $this->name = $name;
    }

    public function getName(): string {
        return $this->name;
    }

    private function validate(): bool {
        return true;
    }
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("user.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "User");

    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 2, "expected >= 2 methods, got {}", methods.len());
    assert!(methods.iter().any(|m| m.name == "getName"));

    // Visibility
    assert!(
        result.nodes.iter().any(|n| n.visibility == Visibility::Private),
        "expected private members"
    );
    assert!(
        result.nodes.iter().any(|n| n.visibility == Visibility::Pub),
        "expected public members"
    );

    // Fields (properties)
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert!(!fields.is_empty(), "expected field nodes for class properties");

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

#[test]
fn test_php_interface() {
    let source = r#"<?php
interface Loggable {
    public function log(string $message): void;
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("loggable.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let traits: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Trait).collect();
    assert_eq!(traits.len(), 1, "interface should map to Trait node");
    assert_eq!(traits[0].name, "Loggable");
}

#[test]
fn test_php_trait_declaration() {
    let source = r#"<?php
trait Timestamps {
    public function createdAt(): string {
        return $this->created;
    }
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("timestamps.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let traits: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Trait).collect();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Timestamps");
}

#[test]
fn test_php_namespace() {
    let source = r#"<?php
namespace App\Models;

class Item {}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("item.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(
        result.nodes.iter().any(|n| n.kind == NodeKind::Module),
        "namespace should produce a Module node"
    );
}

#[test]
fn test_php_enum() {
    let source = r#"<?php
enum Status {
    case Active;
    case Inactive;
    case Pending;
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("status.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let enums: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Enum).collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Status");
    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert_eq!(variants.len(), 3, "expected 3 enum cases");
}

#[test]
fn test_php_class_inheritance() {
    let source = r#"<?php
class Base {
    public function id(): int { return 1; }
}
class Child extends Base {
    public function name(): string { return "x"; }
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("inherit.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends ref for class inheritance"
    );
}

#[test]
fn test_php_trait_use_inside_class() {
    let source = r#"<?php
trait Logger {
    public function log(): void {}
}
class Service {
    use Logger;
    public function run(): void {}
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("service.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(!uses.is_empty(), "expected Use node for `use Logger` inside class");
}

#[test]
fn test_php_constructor_as_method() {
    let source = r#"<?php
class Widget {
    public function __construct(private string $name) {}
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("widget.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // PHP extractor maps __construct as a regular Method
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.iter().any(|m| m.name == "__construct"), "expected __construct method");
}

#[test]
fn test_php_attributes_on_function_and_class() {
    let source = r#"<?php
#[Route('/api')]
#[Deprecated]
function hello() {}

#[Override]
class MyController {
    #[AllowDynamicProperties]
    public function handle() {}
}
"#;
    let extractor = PhpExtractor;
    let result = extractor.extract("attr.php", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Should have 4 AnnotationUsage nodes: Route, Deprecated, Override, AllowDynamicProperties
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert_eq!(annots.len(), 4, "expected 4 annotations, got: {:?}", annots.iter().map(|a| &a.name).collect::<Vec<_>>());
    assert!(annots.iter().any(|a| a.name == "Route"));
    assert!(annots.iter().any(|a| a.name == "Deprecated"));
    assert!(annots.iter().any(|a| a.name == "Override"));
    assert!(annots.iter().any(|a| a.name == "AllowDynamicProperties"));

    // Should have Annotates edges.
    let annotates_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annotates_edges.len(), 4, "expected 4 Annotates edges");

    // Should have Annotates unresolved refs.
    let annot_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annot_refs.len(), 4, "expected 4 Annotates refs");
}

#[test]
fn test_php_empty_source() {
    let extractor = PhpExtractor;
    let result = extractor.extract("empty.php", "<?php\n");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(files.len(), 1);
}

} // mod php_tests
