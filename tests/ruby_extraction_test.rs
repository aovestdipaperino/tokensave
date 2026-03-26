#[cfg(feature = "lang-ruby")]
mod ruby_tests {

use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::RubyExtractor;
use tokensave::types::*;

#[test]
fn test_ruby_file_node() {
    let source = r#"
def hello
  puts "hi"
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("test.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.rb");
}

#[test]
fn test_ruby_top_level_method() {
    let source = r#"
def greet(name)
  "Hello #{name}"
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("greet.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function || n.kind == NodeKind::Method)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "greet");
}

#[test]
fn test_ruby_class_with_methods() {
    let source = r#"
class Dog
  def initialize(name)
    @name = name
  end

  def bark
    "Woof!"
  end

  def self.species
    "Canis"
  end
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("dog.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Dog");

    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert!(methods.len() >= 2, "expected >= 2 methods, got {}", methods.len());
    assert!(methods.iter().any(|m| m.name == "bark"));

    // Contains edges
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

#[test]
fn test_ruby_module() {
    let source = r#"
module Utils
  def self.format(val)
    val.to_s
  end
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("utils.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Module).collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "Utils");
}

#[test]
fn test_ruby_class_inheritance() {
    let source = r#"
class Animal
  def speak; end
end

class Cat < Animal
  def speak
    "Meow"
  end
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("animals.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 2);
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends),
        "expected Extends ref for Cat < Animal"
    );
}

#[test]
fn test_ruby_constants() {
    let source = r#"
module Config
  MAX_RETRIES = 3
  TIMEOUT = 30
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("config.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert_eq!(consts.len(), 2, "expected 2 constants, got: {:?}", consts.iter().map(|n| &n.name).collect::<Vec<_>>());
    assert!(consts.iter().any(|c| c.name == "MAX_RETRIES"));
    assert!(consts.iter().any(|c| c.name == "TIMEOUT"));
}

#[test]
fn test_ruby_nested_class() {
    let source = r#"
class Outer
  class Inner
    def work; end
  end
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("nested.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 2);
    assert!(classes.iter().any(|c| c.name == "Outer"));
    assert!(classes.iter().any(|c| c.name == "Inner"));
}

#[test]
fn test_ruby_call_sites() {
    let source = r#"
class Processor
  def run
    prepare()
    execute()
  end

  def prepare; end
  def execute; end
end
"#;
    let extractor = RubyExtractor;
    let result = extractor.extract("proc.rb", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(
        result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );
}

#[test]
fn test_ruby_empty_source() {
    let extractor = RubyExtractor;
    let result = extractor.extract("empty.rb", "");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(files.len(), 1);
}

} // mod ruby_tests
