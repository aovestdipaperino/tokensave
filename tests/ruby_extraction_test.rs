#![cfg(feature = "lang-ruby")]

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
        let files: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::File)
            .collect();
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

        let classes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Class)
            .collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "Dog");

        let methods: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Method)
            .collect();
        assert!(
            methods.len() >= 2,
            "expected >= 2 methods, got {}",
            methods.len()
        );
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
        let modules: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Module)
            .collect();
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
        let classes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Class)
            .collect();
        assert_eq!(classes.len(), 2);
        assert!(
            result
                .unresolved_refs
                .iter()
                .any(|r| r.reference_kind == EdgeKind::Extends),
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
        let consts: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Const)
            .collect();
        assert_eq!(
            consts.len(),
            2,
            "expected 2 constants, got: {:?}",
            consts.iter().map(|n| &n.name).collect::<Vec<_>>()
        );
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
        let classes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Class)
            .collect();
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
            result
                .unresolved_refs
                .iter()
                .any(|r| r.reference_kind == EdgeKind::Calls),
            "expected Calls refs"
        );
    }

    #[test]
    fn test_ruby_visibility_default_public() {
        let source = r#"
class Widget
  def build; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let build = result
            .nodes
            .iter()
            .find(|n| n.name == "build")
            .expect("expected build method");
        assert_eq!(build.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_bare_private_and_public() {
        let source = r#"
class Widget
  def open; end

  private

  def hidden; end

  public

  def visible_again; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visibility_of = |name: &str| {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("expected method {name}"))
                .visibility
                .clone()
        };
        assert_eq!(visibility_of("open"), Visibility::Pub);
        assert_eq!(visibility_of("hidden"), Visibility::Private);
        assert_eq!(visibility_of("visible_again"), Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_arg_expression_does_not_switch_mode() {
        let source = r#"
class Widget
  private attr_reader :foo

  def visible; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visible = result
            .nodes
            .iter()
            .find(|n| n.name == "visible")
            .expect("expected visible method");
        assert_eq!(visible.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_protected_is_non_public() {
        let source = r#"
class Widget
  protected

  def guarded; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let guarded = result
            .nodes
            .iter()
            .find(|n| n.name == "guarded")
            .expect("expected guarded method");
        assert_eq!(guarded.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_symbol_form() {
        let source = r#"
class Widget
  def helper; end
  def other; end

  private :helper
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visibility_of = |name: &str| {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("expected method {name}"))
                .visibility
                .clone()
        };
        assert_eq!(visibility_of("helper"), Visibility::Private);
        assert_eq!(visibility_of("other"), Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_inline_form() {
        let source = r#"
class Widget
  private def secret; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let secret = result
            .nodes
            .iter()
            .find(|n| n.name == "secret")
            .expect("expected secret method to be extracted");
        assert_eq!(secret.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_private_class_method() {
        let source = r#"
class Widget
  def self.build; end

  private_class_method :build
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let build = result
            .nodes
            .iter()
            .find(|n| n.name == "build")
            .expect("expected build singleton method");
        assert_eq!(build.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_public_class_method_restores_singleton() {
        let source = r#"
class Widget
  def self.run; end
  private_class_method :run
  public_class_method :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run = result
            .nodes
            .iter()
            .find(|n| n.name == "run")
            .expect("expected run singleton method");
        assert_eq!(run.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_public_class_method_targets_singleton_not_instance() {
        let source = r#"
class Widget
  private

  def run; end
  def self.run; end

  public_class_method :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "run").collect();
        assert_eq!(run_nodes.len(), 2);
        let is_singleton = |n: &Node| n.signature.as_deref().unwrap_or("").contains("self.");
        let singleton = run_nodes
            .iter()
            .copied()
            .find(|&n| is_singleton(n))
            .expect("singleton run");
        let instance = run_nodes
            .iter()
            .copied()
            .find(|&n| !is_singleton(n))
            .expect("instance run");
        assert_eq!(singleton.visibility, Visibility::Pub);
        assert_eq!(instance.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_does_not_leak_across_classes() {
        let source = r#"
class First
  private

  def hidden; end
end

class Second
  def visible; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visibility_of = |name: &str| {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("expected method {name}"))
                .visibility
                .clone()
        };
        assert_eq!(visibility_of("hidden"), Visibility::Private);
        assert_eq!(visibility_of("visible"), Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_private_class_method_inline_singleton() {
        let source = r#"
class Widget
  private_class_method def self.build; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let build = result
            .nodes
            .iter()
            .find(|n| n.name == "build")
            .expect("expected build singleton method to be extracted, not dropped");
        assert_eq!(build.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_symbol_form_scoped_to_owning_class() {
        let source = r#"
class A
  def run; end
end

class B
  def run; end

  private :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "run").collect();
        assert_eq!(run_nodes.len(), 2);
        let a_run = run_nodes
            .iter()
            .find(|n| n.qualified_name.contains("::A::"))
            .expect("expected A#run");
        let b_run = run_nodes
            .iter()
            .find(|n| n.qualified_name.contains("::B::"))
            .expect("expected B#run");
        assert_eq!(a_run.visibility, Visibility::Pub);
        assert_eq!(b_run.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_top_level_bare_private() {
        let source = r#"
def before; end

private

def helper; end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("script.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visibility_of = |name: &str| {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("expected method {name}"))
                .visibility
                .clone()
        };
        assert_eq!(visibility_of("before"), Visibility::Pub);
        assert_eq!(visibility_of("helper"), Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_top_level_inline_private() {
        let source = r#"
private def other; end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("script.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let other = result
            .nodes
            .iter()
            .find(|n| n.name == "other")
            .expect("expected other method to be extracted");
        assert_eq!(other.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_top_level_does_not_leak_into_class() {
        let source = r#"
private

class C
  def m; end
end

def after; end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("script.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visibility_of = |name: &str| {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("expected method {name}"))
                .visibility
                .clone()
        };
        assert_eq!(visibility_of("m"), Visibility::Pub);
        assert_eq!(visibility_of("after"), Visibility::Private);
    }

    #[test]
    fn test_ruby_private_class_method_targets_singleton_not_instance() {
        let source = r#"
class Widget
  def self.run; end
  def run; end

  private_class_method :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "run").collect();
        assert_eq!(run_nodes.len(), 2);
        let is_singleton = |n: &Node| n.signature.as_deref().unwrap_or("").contains("self.");
        let singleton = run_nodes
            .iter()
            .copied()
            .find(|&n| is_singleton(n))
            .expect("singleton run");
        let instance = run_nodes
            .iter()
            .copied()
            .find(|&n| !is_singleton(n))
            .expect("instance run");
        assert_eq!(singleton.visibility, Visibility::Private);
        assert_eq!(instance.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_private_symbol_targets_instance_not_singleton() {
        let source = r#"
class Widget
  def self.run; end
  def run; end

  private :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "run").collect();
        assert_eq!(run_nodes.len(), 2);
        let is_singleton = |n: &Node| n.signature.as_deref().unwrap_or("").contains("self.");
        let singleton = run_nodes
            .iter()
            .copied()
            .find(|&n| is_singleton(n))
            .expect("singleton run");
        let instance = run_nodes
            .iter()
            .copied()
            .find(|&n| !is_singleton(n))
            .expect("instance run");
        assert_eq!(instance.visibility, Visibility::Private);
        assert_eq!(singleton.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_private_class_method_targets_singleton_regardless_of_def_order() {
        // Instance defined first this time — proves the match isn't order-dependent.
        let source = r#"
class Widget
  def run; end
  def self.run; end

  private_class_method :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "run").collect();
        assert_eq!(run_nodes.len(), 2);
        let is_singleton = |n: &Node| n.signature.as_deref().unwrap_or("").contains("self.");
        let singleton = run_nodes
            .iter()
            .copied()
            .find(|&n| is_singleton(n))
            .expect("singleton run");
        let instance = run_nodes
            .iter()
            .copied()
            .find(|&n| !is_singleton(n))
            .expect("instance run");
        assert_eq!(singleton.visibility, Visibility::Private);
        assert_eq!(instance.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_ignores_explicit_receiver_calls() {
        let source = r#"
class Widget
  policy.private

  def still_public; end

  def run; end
  config.public(:run)
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visibility_of = |name: &str| {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("expected method {name}"))
                .visibility
                .clone()
        };
        assert_eq!(visibility_of("still_public"), Visibility::Pub);
        assert_eq!(visibility_of("run"), Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_quoted_symbol_instance() {
        let source = r#"
class Widget
  def helper; end
  def other; end

  private :"helper"
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visibility_of = |name: &str| {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("expected method {name}"))
                .visibility
                .clone()
        };
        assert_eq!(visibility_of("helper"), Visibility::Private);
        assert_eq!(visibility_of("other"), Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_quoted_symbol_operator() {
        let source = r#"
class Widget
  def []=(key, value); end

  private :"[]="
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let op = result
            .nodes
            .iter()
            .find(|n| n.name == "[]=")
            .expect("expected []= method to be extracted");
        assert_eq!(op.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_visibility_quoted_class_method() {
        let source = r#"
class Widget
  def self.build; end
  def build; end

  private_class_method :"build"
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let build_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "build").collect();
        assert_eq!(build_nodes.len(), 2);
        let is_singleton = |n: &Node| n.signature.as_deref().unwrap_or("").contains("self.");
        let singleton = build_nodes
            .iter()
            .copied()
            .find(|&n| is_singleton(n))
            .expect("singleton build");
        let instance = build_nodes
            .iter()
            .copied()
            .find(|&n| !is_singleton(n))
            .expect("instance build");
        assert_eq!(singleton.visibility, Visibility::Private);
        assert_eq!(instance.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_visibility_interpolated_symbol_is_skipped() {
        let source = r##"
class Widget
  x = "helper"
  private :"#{x}"

  def visible; end
end
"##;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let visible = result
            .nodes
            .iter()
            .find(|n| n.name == "visible")
            .expect("expected visible method to be extracted");
        assert_eq!(visible.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_empty_source() {
        let extractor = RubyExtractor;
        let result = extractor.extract("empty.rb", "");
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let files: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::File)
            .collect();
        assert_eq!(files.len(), 1);
    }
} // mod ruby_tests
