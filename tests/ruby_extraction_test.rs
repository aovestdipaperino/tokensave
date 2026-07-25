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
    fn test_ruby_singleton_class_self_extracts_methods() {
        let source = r#"
class Report
  class << self
    def generate; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generate = result
            .nodes
            .iter()
            .find(|n| n.name == "generate")
            .expect("expected generate method to be extracted from class << self, not dropped");
        assert_eq!(generate.kind, NodeKind::Method);
    }

    #[test]
    fn test_ruby_singleton_class_qualified_name_matches_def_self() {
        let shovel_source = r#"
class Report
  class << self
    def generate; end
  end
end
"#;
        let def_self_source = r#"
class Report
  def self.generate; end
end
"#;
        let extractor = RubyExtractor;
        let shovel_result = extractor.extract("report.rb", shovel_source);
        assert!(
            shovel_result.errors.is_empty(),
            "errors: {:?}",
            shovel_result.errors
        );
        let def_self_result = extractor.extract("report.rb", def_self_source);
        assert!(
            def_self_result.errors.is_empty(),
            "errors: {:?}",
            def_self_result.errors
        );
        let shovel_generate = shovel_result
            .nodes
            .iter()
            .find(|n| n.name == "generate")
            .expect("expected generate method from class << self");
        let def_self_generate = def_self_result
            .nodes
            .iter()
            .find(|n| n.name == "generate")
            .expect("expected generate method from def self.generate");
        assert_eq!(
            shovel_generate.qualified_name, def_self_generate.qualified_name,
            "class << self; def foo should produce the same qualified name as def self.foo"
        );
    }

    #[test]
    fn test_ruby_singleton_class_contains_edge_from_enclosing_class() {
        let source = r#"
class Report
  class << self
    def generate; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let class_node = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.name == "Report")
            .expect("expected Report class");
        let generate = result
            .nodes
            .iter()
            .find(|n| n.name == "generate")
            .expect("expected generate method");
        assert!(
            result.edges.iter().any(|e| e.kind == EdgeKind::Contains
                && e.source == class_node.id
                && e.target == generate.id),
            "expected Contains edge from Report directly to generate"
        );
    }

    #[test]
    fn test_ruby_singleton_class_bare_private_privatizes_following_defs() {
        let source = r#"
class Report
  class << self
    def generate; end

    private

    def helper; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
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
        assert_eq!(visibility_of("generate"), Visibility::Pub);
        assert_eq!(visibility_of("helper"), Visibility::Private);
    }

    #[test]
    fn test_ruby_singleton_class_private_does_not_leak_out_to_instance_methods() {
        let source = r#"
class Report
  class << self
    private

    def helper; end
  end

  def instance_method; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
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
        assert_eq!(visibility_of("instance_method"), Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_class_does_not_inherit_outer_private() {
        let source = r#"
class Report
  private

  class << self
    def generate; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generate = result
            .nodes
            .iter()
            .find(|n| n.name == "generate")
            .expect("expected generate method");
        assert_eq!(generate.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_class_symbol_form_marks_singleton_method() {
        let source = r#"
class Report
  class << self
    def helper; end

    private :helper
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let helper = result
            .nodes
            .iter()
            .find(|n| n.name == "helper")
            .expect("expected helper method");
        assert_eq!(helper.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_private_class_method_targets_method_defined_in_singleton_class() {
        let source = r#"
class Report
  class << self
    def helper; end
  end

  private_class_method :helper
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let helper = result
            .nodes
            .iter()
            .find(|n| n.name == "helper")
            .expect("expected helper method");
        assert_eq!(helper.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_private_class_method_targets_singleton_not_instance_via_shovel() {
        let source = r#"
class Widget
  def run; end

  class << self
    def run; end
  end

  private_class_method :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "run").collect();
        assert_eq!(run_nodes.len(), 2);
        let instance = run_nodes
            .iter()
            .copied()
            .min_by_key(|n| n.start_line)
            .unwrap();
        let singleton = run_nodes
            .iter()
            .copied()
            .max_by_key(|n| n.start_line)
            .unwrap();
        assert_eq!(instance.visibility, Visibility::Pub);
        assert_eq!(singleton.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_private_symbol_targets_instance_not_singleton_via_shovel() {
        let source = r#"
class Widget
  def run; end

  class << self
    def run; end
  end

  private :run
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let run_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "run").collect();
        assert_eq!(run_nodes.len(), 2);
        let instance = run_nodes
            .iter()
            .copied()
            .min_by_key(|n| n.start_line)
            .unwrap();
        let singleton = run_nodes
            .iter()
            .copied()
            .max_by_key(|n| n.start_line)
            .unwrap();
        assert_eq!(instance.visibility, Visibility::Private);
        assert_eq!(singleton.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_class_call_sites() {
        let source = r#"
class Report
  class << self
    def generate
      prepare()
    end

    def prepare; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            result
                .unresolved_refs
                .iter()
                .any(|r| r.reference_kind == EdgeKind::Calls && r.reference_name == "prepare"),
            "expected a Calls ref for prepare from inside class << self"
        );
    }

    #[test]
    fn test_ruby_singleton_class_nested_in_module() {
        let source = r#"
module Utils
  class << self
    def format(val)
      val.to_s
    end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("utils.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let format_method = result
            .nodes
            .iter()
            .find(|n| n.name == "format")
            .expect("expected format method inside module's class << self");
        assert_eq!(format_method.kind, NodeKind::Method);
        assert!(format_method.qualified_name.ends_with("Utils::format"));
    }

    #[test]
    fn test_ruby_singleton_class_non_self_receiver_not_registered_as_singleton() {
        let source = r#"
class Report
  class << some_object
    def helper; end
  end

  private_class_method :helper
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let helper = result
            .nodes
            .iter()
            .find(|n| n.name == "helper")
            .expect("expected helper method to still be extracted from class << some_object");
        // private_class_method must not match it: it's not the enclosing class's singleton.
        assert_eq!(helper.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_method_foreign_receiver_not_targeted_by_private_class_method() {
        let source = r#"
class Report
  def obj.foo; end

  private_class_method :foo
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected def obj.foo to still be extracted");
        // `foo`'s receiver is `obj`, not `Report`, so `private_class_method` must not match it.
        assert_eq!(foo.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_method_foreign_receiver_not_targeted_by_private() {
        let source = r#"
class Report
  def obj.foo; end

  private :foo
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected def obj.foo to still be extracted");
        // `foo` isn't an instance method of Report either, so `private` must not match it -
        // it should land in neither the singleton nor the instance-method bucket.
        assert_eq!(foo.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_method_distinguishes_self_from_other_receiver() {
        let source = r#"
class Report
  def self.foo; end
  def obj.foo; end

  private_class_method :foo
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "foo").collect();
        assert_eq!(foo_nodes.len(), 2);
        let self_foo = foo_nodes
            .iter()
            .copied()
            .find(|n| n.signature.as_deref() == Some("def self.foo; end"))
            .expect("expected def self.foo");
        let obj_foo = foo_nodes
            .iter()
            .copied()
            .find(|n| n.signature.as_deref() == Some("def obj.foo; end"))
            .expect("expected def obj.foo");
        assert_eq!(self_foo.visibility, Visibility::Private);
        assert_eq!(obj_foo.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_method_enclosing_constant_receiver_is_equivalent_to_self() {
        let source = r#"
class Report
  def Report.generate; end

  private_class_method :generate
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generate = result
            .nodes
            .iter()
            .find(|n| n.name == "generate")
            .expect("expected generate method");
        assert_eq!(generate.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_singleton_scope_does_not_leak_into_nested_class() {
        let source = r#"
class Report
  class << self
    class Inner
      def foo; end
      def self.foo; end
      private :foo
    end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "foo").collect();
        assert_eq!(foo_nodes.len(), 2);
        let is_singleton = |n: &Node| n.signature.as_deref().unwrap_or("").contains("self.");
        let singleton = foo_nodes
            .iter()
            .copied()
            .find(|&n| is_singleton(n))
            .expect("singleton foo");
        let instance = foo_nodes
            .iter()
            .copied()
            .find(|&n| !is_singleton(n))
            .expect("instance foo");
        // Without the fix, the leaked singleton scope makes `private :foo` retarget
        // `def self.foo` inside Inner instead of the plain instance `def foo`.
        assert_eq!(instance.visibility, Visibility::Private);
        assert_eq!(singleton.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_scope_does_not_leak_into_nested_module() {
        let source = r#"
class Report
  class << self
    module Helpers
      def foo; end
      def self.foo; end
      private :foo
    end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo_nodes: Vec<_> = result.nodes.iter().filter(|n| n.name == "foo").collect();
        assert_eq!(foo_nodes.len(), 2);
        let is_singleton = |n: &Node| n.signature.as_deref().unwrap_or("").contains("self.");
        let singleton = foo_nodes
            .iter()
            .copied()
            .find(|&n| is_singleton(n))
            .expect("singleton foo");
        let instance = foo_nodes
            .iter()
            .copied()
            .find(|&n| !is_singleton(n))
            .expect("instance foo");
        assert_eq!(instance.visibility, Visibility::Private);
        assert_eq!(singleton.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_nested_foreign_singleton_class_does_not_inherit_outer_enclosing_scope() {
        let source = r#"
class Report
  class << self
    class << other
      def bar; end
    end
  end

  private_class_method :bar
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let bar = result
            .nodes
            .iter()
            .find(|n| n.name == "bar")
            .expect("expected bar method inside nested class << other");
        // `bar` belongs to `other`, not `Report`, even though it's nested inside
        // `class << self` - it must not inherit the outer Enclosing scope.
        assert_eq!(bar.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_foreign_singleton_class_method_not_targeted_by_private() {
        let source = r#"
class Report
  class << some_object
    def bar; end
  end

  private :bar
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let bar = result
            .nodes
            .iter()
            .find(|n| n.name == "bar")
            .expect("expected bar method inside class << some_object");
        assert_eq!(bar.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_def_self_inside_class_shovel_self_targets_outer_singleton_class() {
        let source = r#"
class Report
  class << self
    def self.meta_only; end
  end

  private_class_method :meta_only
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let meta_only = result
            .nodes
            .iter()
            .find(|n| n.name == "meta_only")
            .expect("expected meta_only method");
        // `self` inside `class << self` is the singleton class itself, so
        // `def self.meta_only` defines a method one level further out than
        // `Report` (`Report.singleton_class.meta_only`). `private_class_method`
        // at the `Report` level must not match it.
        assert_eq!(meta_only.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_def_inside_nested_class_shovel_self_targets_outer_singleton_class() {
        let source = r#"
class Report
  class << self
    class << self
      def deep; end
    end
  end

  private_class_method :deep
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let deep = result
            .nodes
            .iter()
            .find(|n| n.name == "deep")
            .expect("expected deep method");
        // The inner `class << self` is judged against the outer `Enclosing`
        // scope, so its `self` is the singleton class, not `Report` - `deep`
        // belongs one level further out and `private_class_method` here must
        // not match it.
        assert_eq!(deep.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_def_constant_inside_class_shovel_self_still_targets_enclosing_class() {
        let source = r#"
class Report
  class << self
    def Report.generate; end
  end

  private_class_method :generate
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generate = result
            .nodes
            .iter()
            .find(|n| n.name == "generate")
            .expect("expected generate method");
        // Unlike a literal `self`, the constant receiver names the enclosing
        // class regardless of singleton scope, so `def Report.generate` here
        // is still `Report.generate` and `private_class_method` must match it.
        assert_eq!(generate.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_directive_inside_foreign_singleton_class_does_not_retarget_enclosing_instance_method(
    ) {
        let source = r#"
class Report
  def process; end
  class << config
    def process; end
    private :process
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let process_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.name == "process")
            .collect();
        assert_eq!(process_nodes.len(), 2);
        let instance = process_nodes
            .iter()
            .copied()
            .min_by_key(|n| n.start_line)
            .unwrap();
        let foreign = process_nodes
            .iter()
            .copied()
            .max_by_key(|n| n.start_line)
            .unwrap();
        // `private :process` is written inside `class << config`'s body, so it
        // must mark `config`'s `process`, not fall through to `Report#process`.
        assert_eq!(instance.visibility, Visibility::Pub);
        assert_eq!(foreign.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_private_class_method_inside_class_shovel_self_targets_only_nested_def_self() {
        let source = r#"
class Report
  class << self
    def plain; end
    def self.deep; end

    private_class_method :deep
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let plain = result
            .nodes
            .iter()
            .find(|n| n.name == "plain")
            .expect("expected plain method");
        let deep = result
            .nodes
            .iter()
            .find(|n| n.name == "deep")
            .expect("expected deep method");
        // `plain` is `Report`'s own class method (registered as the enclosing
        // singleton); `def self.deep` here is one level further out, so
        // `private_class_method :deep`, written inside the same `class <<
        // self` body, must mark only `deep` and leave `plain` untouched.
        assert_eq!(plain.visibility, Visibility::Pub);
        assert_eq!(deep.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_singleton_class_qualified_receiver_targets_enclosing_class() {
        let source = r#"
module Outer
  class Inner
    class << Outer::Inner
      def foo; end
    end

    private_class_method :foo
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected foo method");
        // `Outer::Inner` names the class we're inside, so `class << Outer::Inner`
        // reopens its singleton class just like `class << self` would.
        assert_eq!(foo.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_singleton_class_partial_relative_qualified_receiver_targets_enclosing_class() {
        let source = r#"
module A
  module B
    class C
      class << B::C
        def bar; end
      end

      private_class_method :bar
    end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let bar = result
            .nodes
            .iter()
            .find(|n| n.name == "bar")
            .expect("expected bar method");
        // `B::C` is a relative path resolving up the lexical scope from `C`,
        // matching a suffix of the enclosing node stack (A, B, C).
        assert_eq!(bar.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_singleton_class_unrelated_qualified_receiver_not_targeted() {
        let source = r#"
module Outer
  class Inner
    class << Other::Thing
      def baz; end
    end

    private_class_method :baz
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let baz = result
            .nodes
            .iter()
            .find(|n| n.name == "baz")
            .expect("expected baz method");
        // `Other::Thing` names neither `Inner` nor any suffix of the enclosing
        // node stack, so it must not be treated as the enclosing class.
        assert_eq!(baz.visibility, Visibility::Pub);
    }

    #[test]
    fn test_ruby_singleton_class_absolute_qualified_receiver_targets_enclosing_class() {
        let source = r#"
module Outer
  class Inner
    class << ::Outer::Inner
      def foo; end
    end

    private_class_method :foo
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected foo method");
        // A leading `::` is an absolute path anchored at top level; it must
        // still match when it names the same object as the full node stack.
        assert_eq!(foo.visibility, Visibility::Private);
    }

    #[test]
    fn test_ruby_singleton_class_absolute_qualified_receiver_is_different_object() {
        let source = r#"
module A
  class B
    class << ::B
      def foo; end
    end

    private_class_method :foo
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("report.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected foo method");
        // `::B` is the top-level constant `B`, a different object from `A::B` -
        // an absolute path must never match via a relative suffix.
        assert_eq!(foo.visibility, Visibility::Pub);
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
