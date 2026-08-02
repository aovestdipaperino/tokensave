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
    fn test_ruby_call_sites_preserve_receiver_shape() {
        let source = include_str!("fixtures/ruby_receiver_calls.rb");
        let extractor = RubyExtractor;
        let result = extractor.extract("ruby_receiver_calls.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

        let call_names: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|reference| reference.reference_kind == EdgeKind::Calls)
            .map(|reference| reference.reference_name.as_str())
            .collect();

        for expected in [
            "save",
            "self.save",
            "Account.find",
            "Services::Capture.call",
            "worker.perform",
            "worker::perform",
            "worker.call",
            "@client.call",
            "@@registry.fetch",
            "account.owner.notify",
            "account.owner",
            "user&.profile",
            "\"text\".strip",
            "Array.new",
            "self.publish",
            "Publisher.publish",
            "self.direct",
            "self.inherited",
            "self.current_class_eval",
            "self.current_instance_eval",
            "self.expression_inherited",
            "self.direct_concern",
        ] {
            assert!(
                call_names.contains(&expected),
                "expected receiver-preserving call reference {expected:?}, got {call_names:?}"
            );
        }

        for unexpected in [
            "self.foreign_instance_eval",
            "self.foreign_class_eval",
            "self.foreign_expression_eval",
            "self.anonymous_class",
            "self.nested_concern",
            "self.included_hook",
            "self.class_methods_hook",
        ] {
            assert!(
                !call_names.contains(&unexpected),
                "did not expect class/module-body call attribution for {unexpected:?}, got \
                 {call_names:?}"
            );
        }

        let block_owner = result
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Class && node.name == "BlockOwner")
            .expect("expected BlockOwner class");
        let concern_owner = result
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Module && node.name == "ConcernOwner")
            .expect("expected ConcernOwner module");
        for reference_name in [
            "self.direct",
            "self.inherited",
            "self.current_class_eval",
            "self.current_instance_eval",
            "self.expression_inherited",
        ] {
            assert!(result.unresolved_refs.iter().any(|reference| {
                reference.from_node_id == block_owner.id
                    && reference.reference_name == reference_name
            }));
        }
        assert!(result.unresolved_refs.iter().any(|reference| {
            reference.from_node_id == concern_owner.id
                && reference.reference_name == "self.direct_concern"
        }));
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
        assert_eq!(generate.kind, NodeKind::SingletonMethod);
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
    fn test_ruby_class_and_module_body_self_calls_use_the_body_owner() {
        let source = r#"
class Publisher
  def self.publish; end
  self.publish

  def instance_run
    self.publish
  end
end

module Announcer
  def self.publish; end
  self.publish
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("body_self_calls.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

        let publisher = result
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Class && node.name == "Publisher")
            .expect("expected Publisher class");
        let announcer = result
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Module && node.name == "Announcer")
            .expect("expected Announcer module");
        let instance_run = result
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Method && node.name == "instance_run")
            .expect("expected instance_run method");
        let self_calls: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|reference| {
                reference.reference_kind == EdgeKind::Calls
                    && reference.reference_name == "self.publish"
            })
            .collect();

        assert_eq!(self_calls.len(), 3);
        assert!(self_calls
            .iter()
            .any(|reference| reference.from_node_id == publisher.id));
        assert!(self_calls
            .iter()
            .any(|reference| reference.from_node_id == announcer.id));
        assert!(self_calls
            .iter()
            .any(|reference| reference.from_node_id == instance_run.id));
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
        assert_eq!(format_method.kind, NodeKind::SingletonMethod);
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
    fn test_ruby_compact_class_name_is_fully_qualified() {
        let source = r#"
class Outer::Inner
  def foo; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("compact.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let class = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class)
            .expect("expected class node");
        assert_eq!(class.name, "Outer::Inner");
        assert!(class.qualified_name.ends_with("Outer::Inner"));
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected foo method");
        assert!(foo.qualified_name.ends_with("Outer::Inner::foo"));
    }

    #[test]
    fn test_ruby_compact_module_name_is_fully_qualified() {
        let source = r#"
module A::B
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("compact.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let module = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Module)
            .expect("expected module node");
        assert_eq!(module.name, "A::B");
    }

    #[test]
    fn test_ruby_compact_class_qualified_singleton_receiver_targets_enclosing_class() {
        let source = r#"
class Outer::Inner
  class << Outer::Inner
    def foo; end
  end

  private_class_method :foo
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("compact.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected foo method");
        assert_eq!(foo.visibility, Visibility::Private);
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

    #[test]
    fn test_ruby_extend_mixin() {
        let source = r#"
class C
  extend M
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "M");
    }

    #[test]
    fn test_ruby_extend_self_not_a_mixin_ref() {
        let source = r#"
module M
  extend self
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result
                .unresolved_refs
                .iter()
                .any(|r| r.reference_kind == EdgeKind::Implements),
            "extend self should not produce an Implements ref"
        );
    }

    #[test]
    fn test_ruby_include_in_begin_rescue() {
        let source = r#"
class C
  begin
    include M
  rescue LoadError
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "M");
    }

    #[test]
    fn test_ruby_include_in_if_else_body() {
        let source = r#"
class C
  if RUBY_VERSION > "3"
    include M
  else
    include N
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let class_id = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.name == "C")
            .expect("class C node")
            .id
            .clone();
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 2, "expected two Implements refs");
        assert_eq!(implements[0].reference_name, "M");
        assert_eq!(implements[0].from_node_id, class_id);
        assert_eq!(implements[1].reference_name, "N");
        assert_eq!(implements[1].from_node_id, class_id);
    }

    // Method bodies are now traversed for definitions (and, incidentally,
    // directives) so a nested `def` attaches to the enclosing class. A
    // receiverless `include` inside a plain instance-method body rides the
    // same dispatch and is (over-permissively) treated as a mixin ref, even
    // though `self` there is an instance, not the class, so this exact code
    // would raise NoMethodError at runtime and can't occur in code that
    // actually runs. Accepted tradeoff, not a regression: see visit_method's
    // doc comment for the singleton/class-method cases where the identical
    // dispatch is required for correctness (`def self.setup; class_eval {
    // include Mixin; ... }; end`).
    #[test]
    fn test_ruby_include_in_method_body_is_treated_as_mixin_ref() {
        let source = r#"
class C
  def setup
    include M
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            result
                .unresolved_refs
                .iter()
                .any(|r| r.reference_kind == EdgeKind::Implements && r.reference_name == "M"),
            "include inside a method body is dispatched the same as any other body \
             statement, even though this particular receiverless form can't occur in \
             code that runs"
        );
    }

    #[test]
    fn test_ruby_include_in_module_body() {
        let source = r#"
module Outer
  include Other
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("outer.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let module_id = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Module && n.name == "Outer")
            .expect("module Outer node")
            .id
            .clone();
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "Other");
        assert_eq!(implements[0].from_node_id, module_id);
    }

    #[test]
    fn test_ruby_include_mixin() {
        let source = r#"
class C
  include Comparable
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let class_id = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.name == "C")
            .expect("class C node")
            .id
            .clone();
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "Comparable");
        assert_eq!(implements[0].from_node_id, class_id);
    }

    #[test]
    fn test_ruby_include_multiple_modules() {
        let source = r#"
class C
  include A, B
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 2, "expected two Implements refs");
        assert_eq!(implements[0].reference_name, "A");
        assert_eq!(implements[1].reference_name, "B");
    }

    #[test]
    fn test_ruby_include_scope_resolution() {
        let source = r#"
class C
  include ActiveSupport::Concern
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "ActiveSupport::Concern");
    }

    #[test]
    fn test_ruby_include_with_if_modifier() {
        let source = r#"
class C
  include M if enabled?
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let class_id = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.name == "C")
            .expect("class C node")
            .id
            .clone();
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "M");
        assert_eq!(implements[0].from_node_id, class_id);
    }

    #[test]
    fn test_ruby_include_with_receiver_not_a_mixin_ref() {
        let source = r#"
class C
  mod.include Bar
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result
                .unresolved_refs
                .iter()
                .any(|r| r.reference_kind == EdgeKind::Implements),
            "mod.include Bar has an explicit receiver, not a mixin"
        );
    }

    #[test]
    fn test_ruby_include_with_unless_modifier() {
        let source = r#"
class C
  include M unless skip?
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "M");
    }

    #[test]
    fn test_ruby_method_in_conditional_is_extracted() {
        let source = r#"
class C
  if X
    def foo
    end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let class_id = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.name == "C")
            .expect("class C node")
            .id
            .clone();
        let method = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Method && n.name == "foo")
            .expect("method foo node");
        assert!(
            result.edges.iter().any(|e| e.kind == EdgeKind::Contains
                && e.source == class_id
                && e.target == method.id),
            "expected Contains edge from class C to method foo"
        );
    }

    #[test]
    fn test_ruby_prepend_mixin() {
        let source = r#"
class C
  prepend M
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(implements.len(), 1, "expected one Implements ref");
        assert_eq!(implements[0].reference_name, "M");
    }

    #[test]
    fn test_ruby_top_level_include_not_a_mixin_ref() {
        let source = r#"
include Foo
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("top.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result
                .unresolved_refs
                .iter()
                .any(|r| r.reference_kind == EdgeKind::Implements),
            "top-level include has no enclosing class/module to attach to"
        );
    }

    #[test]
    fn test_ruby_visibility_directive_in_conditional_applies_after_end() {
        // Statement containers don't open a new scope, so a `private` reached only
        // through a conditional branch still switches the mode for everything that
        // follows the `end` — matching Ruby's own runtime scoping.
        let source = r#"
class C
  if legacy?
    private
  end

  def foo; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected foo method to be extracted");
        assert_eq!(foo.visibility, Visibility::Private);
    }

    // ----------------------------
    // do…end / {…} block bodies
    // ----------------------------
    //
    // Until now, `visit_node`'s "do_block" | "block" arm (and the
    // `body_statement` it recurses into) was unreachable: a do…end/{…} block
    // is always the `block` field of a `call` node, and the call arm never
    // called `visit_children` on itself. These tests pin the new
    // `visit_block_body` entry point.

    #[test]
    fn test_ruby_included_do_block_defines_instance_method() {
        let source = r#"
module Trackable
  extend ActiveSupport::Concern

  included do
    def track; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("trackable.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let track = result
            .nodes
            .iter()
            .find(|n| n.name == "track")
            .expect("expected track method inside included do block");
        assert_eq!(track.kind, NodeKind::Method);
        assert!(track.qualified_name.ends_with("Trackable::track"));
    }

    #[test]
    fn test_ruby_class_methods_do_block_matches_def_self() {
        let class_methods_source = r#"
module Trackable
  extend ActiveSupport::Concern

  class_methods do
    def find_tracked; end
  end

  private_class_method :find_tracked
end
"#;
        let def_self_source = r#"
module Trackable
  def self.find_tracked; end
end
"#;
        let extractor = RubyExtractor;
        let class_methods_result = extractor.extract("trackable.rb", class_methods_source);
        assert!(
            class_methods_result.errors.is_empty(),
            "errors: {:?}",
            class_methods_result.errors
        );
        let def_self_result = extractor.extract("trackable.rb", def_self_source);
        assert!(
            def_self_result.errors.is_empty(),
            "errors: {:?}",
            def_self_result.errors
        );
        let class_methods_find_tracked = class_methods_result
            .nodes
            .iter()
            .find(|n| n.name == "find_tracked")
            .expect("expected find_tracked method from class_methods do block");
        let def_self_find_tracked = def_self_result
            .nodes
            .iter()
            .find(|n| n.name == "find_tracked")
            .expect("expected find_tracked method from def self.find_tracked");
        assert_eq!(
            class_methods_find_tracked.qualified_name, def_self_find_tracked.qualified_name,
            "class_methods do; def foo should produce the same qualified name as def self.foo"
        );
        // Instance and singleton methods share a qualified_name, so the
        // assertion above alone would still pass if `find_tracked` had been
        // extracted as an instance method. `private_class_method` only
        // resolves against `singleton_method_ids` (see
        // `mark_method_visibility`), so this only goes Private if
        // `class_methods do` really registered it as a singleton method —
        // mirrors `test_ruby_private_class_method_targets_singleton_not_instance`.
        assert_eq!(
            class_methods_find_tracked.visibility,
            Visibility::Private,
            "class_methods do; def foo must register as a singleton method"
        );
    }

    #[test]
    fn test_ruby_receiverless_class_eval_do_block_defines_instance_method() {
        let source = r#"
class Foo
  class_eval do
    def helper; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("foo.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let helper = result
            .nodes
            .iter()
            .find(|n| n.name == "helper")
            .expect("expected helper method inside receiverless class_eval do block");
        assert_eq!(helper.kind, NodeKind::Method);
        assert!(helper.qualified_name.ends_with("Foo::helper"));
    }

    #[test]
    fn test_ruby_class_eval_with_explicit_receiver_not_extracted() {
        let source = r#"
Foo.class_eval do
  def helper; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("foo.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result.nodes.iter().any(|n| n.name == "helper"),
            "Foo.class_eval has an explicit receiver we can't resolve, so its block body \
             must not be attached to the enclosing scope"
        );
    }

    #[test]
    fn test_ruby_top_level_describe_do_block_defines_function() {
        let source = r#"
describe "widget" do
  def helper; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget_spec.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let helper = result
            .nodes
            .iter()
            .find(|n| n.name == "helper")
            .expect("expected helper defined inside a top-level describe do block");
        assert_eq!(helper.kind, NodeKind::Function);
    }

    // `included do` is `ActiveSupport::Concern`'s hook, implemented as
    // `base.class_eval(&@_included_block)` (activesupport-7.2.2.2
    // lib/active_support/concern.rb:138) — a real definee change, so it gets
    // its own fresh public visibility frame and `private` inside it does not
    // leak out. This is a documented exception to the general rule that an
    // ordinary block (`[1].each { private }`) inherits and *does* propagate
    // its enclosing visibility frame in both directions — see
    // `test_ruby_visibility_directive_flows_into_ordinary_block` and
    // `test_ruby_visibility_directive_flows_out_of_ordinary_block`.
    #[test]
    fn test_ruby_visibility_directive_in_included_do_block_is_isolated() {
        let source = r#"
module M
  extend ActiveSupport::Concern

  included do
    private
    def secret; end
  end

  def open_method; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
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
        assert_eq!(visibility_of("secret"), Visibility::Private);
        assert_eq!(
            visibility_of("open_method"),
            Visibility::Pub,
            "private inside an included do block must not leak past its end"
        );
    }

    #[test]
    fn test_ruby_brace_block_body_defines_function() {
        let source = r#"
foo { def x; end }
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("top.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let x = result
            .nodes
            .iter()
            .find(|n| n.name == "x")
            .expect("expected x defined inside a brace block body");
        assert_eq!(x.kind, NodeKind::Function);
    }

    #[test]
    fn test_ruby_include_inside_do_block_emits_implements_ref() {
        let source = r#"
module M
  included do
    include Other
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let module_id = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Module && n.name == "M")
            .expect("module M node")
            .id
            .clone();
        let implements: Vec<_> = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Implements)
            .collect();
        assert_eq!(
            implements.len(),
            1,
            "expected one Implements ref from include inside included do"
        );
        assert_eq!(implements[0].reference_name, "Other");
        assert_eq!(implements[0].from_node_id, module_id);
    }

    // Unlike `class_eval`/`module_eval`/`instance_eval` and their `*_exec`
    // forms — real `Module` methods that retarget the definee for *any*
    // receiver — `included`/`prepended`/`class_methods` have no intrinsic
    // scope-changing semantics in Ruby: they're only `ActiveSupport::Concern`
    // hooks in their receiverless (or `self.`/enclosing-constant) DSL form.
    // On an arbitrary receiver they're ordinary calls whose block inherits
    // the enclosing definee, exactly like `each`/`tap` (probed against Ruby
    // 3.4.7: `C.instance_methods(false) == [:generated]`).
    #[test]
    fn test_ruby_included_with_unresolvable_receiver_defines_instance_method() {
        let source = r#"
class C
  registry.included { def generated; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generated = result
            .nodes
            .iter()
            .find(|n| n.name == "generated")
            .expect("expected generated method inside registry.included block");
        assert_eq!(generated.kind, NodeKind::Method);
        assert!(generated.qualified_name.ends_with("C::generated"));
    }

    #[test]
    fn test_ruby_prepended_with_unresolvable_receiver_defines_instance_method() {
        let source = r#"
class C
  registry.prepended { def generated; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generated = result
            .nodes
            .iter()
            .find(|n| n.name == "generated")
            .expect("expected generated method inside registry.prepended block");
        assert_eq!(generated.kind, NodeKind::Method);
        assert!(generated.qualified_name.ends_with("C::generated"));
    }

    #[test]
    fn test_ruby_class_methods_with_unresolvable_receiver_defines_instance_method() {
        let source = r#"
class C
  registry.class_methods { def generated; end }
  private :generated
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generated = result
            .nodes
            .iter()
            .find(|n| n.name == "generated")
            .expect("expected generated method inside registry.class_methods block");
        // `private :generated` only resolves against instance method ids
        // (see `mark_method_visibility`), so this only goes Private if
        // `registry.class_methods do` registered `generated` as an instance
        // method — a stale `ReceiverSingleton` classification would leave
        // it Pub.
        assert_eq!(
            generated.visibility,
            Visibility::Private,
            "class_methods on an unresolvable receiver must not retarget the singleton class"
        );
    }

    // Fills the pre-existing gap: every other `prepended` test above used
    // `class_methods`/`included`, never `prepended` in its receiverless DSL
    // form.
    #[test]
    fn test_ruby_prepended_do_block_defines_instance_method() {
        let source = r#"
module M
  extend ActiveSupport::Concern

  prepended do
    def p1; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let p1 = result
            .nodes
            .iter()
            .find(|n| n.name == "p1")
            .expect("expected p1 method inside prepended do block");
        assert_eq!(p1.kind, NodeKind::Method);
        assert!(p1.qualified_name.ends_with("M::p1"));
    }

    // `self.class_methods` is still the DSL form (receiverless call ≡
    // `self.<call>`, per `BlockReceiver::Current`'s doc comment) — pins that
    // the gate keys on the receiver's *denotation*, not merely "has no
    // receiver at all".
    #[test]
    fn test_ruby_self_class_methods_do_block_matches_def_self() {
        let source = r#"
module M
  extend ActiveSupport::Concern

  self.class_methods { def cm; end }
  private_class_method :cm
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let cm = result
            .nodes
            .iter()
            .find(|n| n.name == "cm")
            .expect("expected cm method inside self.class_methods block");
        assert_eq!(
            cm.visibility,
            Visibility::Private,
            "self.class_methods must still register as a singleton method"
        );
    }

    // Without any `ActiveSupport::Concern` evidence, a receiverless
    // `class_methods` call is an ordinary hand-rolled hook (probed against
    // Ruby 3.4.7: `C.instance_methods(false) == [:generated]`,
    // `C.singleton_methods(false) == [:class_methods]`), so its block must
    // land on the *instance* side, not the singleton class.
    #[test]
    fn test_ruby_class_methods_without_concern_evidence_defines_instance_method() {
        let source = r#"
class C
  def self.class_methods(&block); block.call; end
  class_methods { def generated; end }
  private :generated
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generated = result
            .nodes
            .iter()
            .find(|n| n.name == "generated")
            .expect("expected generated method inside class_methods block");
        assert_eq!(
            generated.visibility,
            Visibility::Private,
            "without Concern evidence, class_methods must not retarget the singleton class"
        );
    }

    // Without any `ActiveSupport::Concern` evidence, a receiverless
    // `included` call is an ordinary block that inherits the enclosing
    // visibility frame (probed against Ruby 3.4.7:
    // `C.private_instance_methods(false) == [:gen]`), unlike the real
    // Concern hook which resets to a fresh public frame (see
    // `test_ruby_visibility_directive_in_included_do_block_is_isolated`).
    #[test]
    fn test_ruby_included_without_concern_evidence_inherits_visibility_frame() {
        let source = r#"
class C
  def self.included(&b); b.call; end
  private
  included { def gen; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let gen = result
            .nodes
            .iter()
            .find(|n| n.name == "gen")
            .expect("expected gen method inside included block");
        assert_eq!(
            gen.visibility,
            Visibility::Private,
            "without Concern evidence, included must inherit the enclosing visibility frame"
        );
    }

    // Rails' `Module#concerning` builds a module that is already `extend`ed
    // by `ActiveSupport::Concern`, so its body is genuine DSL with no
    // visible `extend` to serve as evidence (probed against activesupport
    // 8.1.3: `C.respond_to?(:find) == true`).
    #[test]
    fn test_ruby_class_methods_inside_concerning_block_matches_def_self() {
        let source = r#"
class C
  concerning :T do
    class_methods do
      def find; end
    end
  end

  private_class_method :find
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let find = result
            .nodes
            .iter()
            .find(|n| n.name == "find")
            .expect("expected find method inside concerning do block");
        assert_eq!(
            find.visibility,
            Visibility::Private,
            "class_methods inside a concerning block must register as a singleton method"
        );
    }

    // An unrelated `extend` must not be mistaken for Concern evidence.
    #[test]
    fn test_ruby_unrelated_extend_is_not_concern_evidence() {
        let source = r#"
module M
  extend Forwardable

  class_methods { def cm; end }
  private :cm
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let cm = result
            .nodes
            .iter()
            .find(|n| n.name == "cm")
            .expect("expected cm method inside class_methods block");
        assert_eq!(
            cm.visibility,
            Visibility::Private,
            "extend Forwardable is not Concern evidence, so class_methods must not retarget \
             the singleton class"
        );
    }

    // `in_concern_scope` is reset on entry to a nested module, so an outer
    // module's `extend ActiveSupport::Concern` must not leak into it.
    #[test]
    fn test_ruby_concern_scope_does_not_leak_into_nested_module() {
        let source = r#"
module Outer
  extend ActiveSupport::Concern

  module Inner
    class_methods { def cm; end }
    private :cm
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("outer.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let cm = result
            .nodes
            .iter()
            .find(|n| n.name == "cm")
            .expect("expected cm method inside nested module's class_methods block");
        assert_eq!(
            cm.visibility,
            Visibility::Private,
            "Outer's Concern evidence must not leak into the nested Inner module"
        );
    }

    // Leading `::` on the extend argument must still count as evidence.
    #[test]
    fn test_ruby_extend_leading_scope_resolution_is_concern_evidence() {
        let source = r#"
module M
  extend ::ActiveSupport::Concern

  class_methods { def cm; end }
  private_class_method :cm
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let cm = result
            .nodes
            .iter()
            .find(|n| n.name == "cm")
            .expect("expected cm method inside class_methods block");
        assert_eq!(
            cm.visibility,
            Visibility::Private,
            "extend ::ActiveSupport::Concern must still count as Concern evidence"
        );
    }

    // `self` inside a plain instance-method body is the instance, not the
    // enclosing class, so an `extend ActiveSupport::Concern` seen while
    // statically walking that body extends the instance and must not leak
    // out as Concern evidence for the enclosing class body (probe A).
    #[test]
    fn test_ruby_extend_in_method_body_is_not_concern_evidence_for_class_body() {
        let source = r#"
class C
  def setup
    extend ActiveSupport::Concern
  end

  def self.class_methods(&b)
    b.call
  end

  class_methods { def gen; end }
  private :gen
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let gen = result
            .nodes
            .iter()
            .find(|n| n.name == "gen")
            .expect("expected gen method inside class_methods block");
        assert_eq!(
            gen.visibility,
            Visibility::Private,
            "a method-body extend must not leak Concern evidence into the class body"
        );
    }

    // `in_concern_scope` is reset (not inherited) into a plain instance-method
    // body: `class_methods` there would raise `NoMethodError` in real Ruby
    // (probe C), so the enclosing module's Concern evidence must not make it
    // retarget the singleton class from in here.
    #[test]
    fn test_ruby_concern_scope_not_inherited_into_method_body() {
        let source = r#"
module M
  extend ActiveSupport::Concern

  def helper
    class_methods { def x; end }
  end

  private :x
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let x = result
            .nodes
            .iter()
            .find(|n| n.name == "x")
            .expect("expected x method inside class_methods block");
        assert_eq!(
            x.visibility,
            Visibility::Private,
            "Concern scope must not be inherited into a plain instance-method body"
        );
    }

    // `in_concern_scope` *is* inherited into a singleton-method body (`def
    // self.x`): `self` there is still the module itself, so `class_methods`
    // genuinely works from in here in real Ruby (probe D).
    #[test]
    fn test_ruby_concern_scope_inherited_into_singleton_method_body() {
        let source = r#"
module M
  extend ActiveSupport::Concern

  def self.setup
    class_methods { def x; end }
  end

  private_class_method :x
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let x = result
            .nodes
            .iter()
            .find(|n| n.name == "x")
            .expect("expected x method inside class_methods block");
        assert_eq!(
            x.visibility,
            Visibility::Private,
            "Concern scope must be inherited into a singleton-method body"
        );
    }

    // `self` inside `class << self` is the singleton class, not the enclosing
    // class, so an `extend ActiveSupport::Concern` seen while statically
    // walking that body must not leak out as Concern evidence for the
    // enclosing class body (probe E).
    #[test]
    fn test_ruby_extend_in_singleton_class_body_is_not_concern_evidence_for_class_body() {
        let source = r#"
class C
  class << self
    extend ActiveSupport::Concern
  end

  def self.class_methods(&b)
    b.call
  end

  class_methods { def gen; end }
  private :gen
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let gen = result
            .nodes
            .iter()
            .find(|n| n.name == "gen")
            .expect("expected gen method inside class_methods block");
        assert_eq!(
            gen.visibility,
            Visibility::Private,
            "a class << self extend must not leak Concern evidence into the class body"
        );
    }

    // `in_concern_scope` is reset (not inherited) into a `class << self`
    // body: `class_methods` there would raise `NoMethodError` in real Ruby
    // (probe F), so the enclosing module's Concern evidence must not make it
    // retarget the block's definee from in here.
    //
    // `def x` is lexically inside `class << self` either way, so it always
    // registers as a singleton method regardless of whether the leak
    // happened — `private`/`private_class_method` can't observe the
    // difference here (unlike the method-body case). What *is* observable
    // is `classify_block_scope`'s other effect: a `class_methods`/`included`
    // block that resolves to `ReceiverBody`/`ReceiverSingleton` unconditionally
    // resets `visibility_mode` to `Pub` for its body, where `Inherit` leaves
    // the ambient mode alone. So a `private` set just before the block, still
    // in effect on `x` afterward, pins the block as `Inherit` — i.e. that
    // `in_concern_scope` did not leak in.
    #[test]
    fn test_ruby_concern_scope_not_inherited_into_singleton_class_body() {
        let source = r#"
module M
  extend ActiveSupport::Concern

  class << self
    private

    class_methods { def x; end }
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("m.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let x = result
            .nodes
            .iter()
            .find(|n| n.name == "x")
            .expect("expected x method inside class_methods block");
        assert_eq!(
            x.visibility,
            Visibility::Private,
            "Concern scope must not be inherited into a class << self body"
        );
    }

    // An ordinary block (`each`, `tap`, …) does not change the default
    // definee, so an explicit receiver on the call it's attached to
    // (`[1].each`) is irrelevant to extraction, unlike `Foo.class_eval`
    // (see `test_ruby_class_eval_with_explicit_receiver_not_extracted`).
    #[test]
    fn test_ruby_ordinary_block_with_explicit_receiver_defines_method() {
        let source = r#"
class C
  [1].each do
    def generated; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generated = result
            .nodes
            .iter()
            .find(|n| n.name == "generated")
            .expect("expected generated method inside [1].each do block");
        assert_eq!(generated.kind, NodeKind::Method);
        assert!(generated.qualified_name.ends_with("C::generated"));
    }

    #[test]
    fn test_ruby_visibility_directive_flows_into_ordinary_block() {
        let source = r#"
class C
  private

  [1].each do
    def in_block; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let in_block = result
            .nodes
            .iter()
            .find(|n| n.name == "in_block")
            .expect("expected in_block method inside [1].each do block");
        assert_eq!(
            in_block.visibility,
            Visibility::Private,
            "an ordinary block inherits the enclosing visibility frame"
        );
    }

    #[test]
    fn test_ruby_visibility_directive_flows_out_of_ordinary_block() {
        let source = r#"
class C
  [1].each do
    private
  end

  def after; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let after = result
            .nodes
            .iter()
            .find(|n| n.name == "after")
            .expect("expected after method");
        assert_eq!(
            after.visibility,
            Visibility::Private,
            "an ordinary block is not a visibility scope boundary, so a mode \
             switch inside it is still in effect after the block ends"
        );
    }

    #[test]
    fn test_ruby_receiverless_instance_eval_defines_singleton_method() {
        let source = r#"
class Widget
  instance_eval do
    def ie; end
  end

  private_class_method :ie
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("widget.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let ie = result
            .nodes
            .iter()
            .find(|n| n.name == "ie")
            .expect("expected ie method inside instance_eval do block");
        assert_eq!(
            ie.visibility,
            Visibility::Private,
            "private_class_method only resolves against singleton_method_ids, so this \
             only goes Private if instance_eval registered ie as a singleton method"
        );
    }

    #[test]
    fn test_ruby_instance_eval_with_explicit_receiver_not_extracted() {
        let source = r#"
Foo.instance_eval do
  def x; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("foo.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result.nodes.iter().any(|n| n.name == "x"),
            "Foo.instance_eval has an explicit receiver we can't resolve, so its block \
             body must not be attached to the enclosing scope"
        );
    }

    // `CALLBACK = proc do … end` puts the block-bearing call on an
    // assignment's RHS, never in statement position — regression coverage
    // for the `visit_expression_blocks` descent from the `"assignment"` arm.
    #[test]
    fn test_ruby_block_body_reached_through_assignment_rhs() {
        let source = r#"
class C
  CALLBACK = proc do
    def generated; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generated =
            result.nodes.iter().find(|n| n.name == "generated").expect(
                "expected generated method defined inside proc do…end on an assignment RHS",
            );
        assert_eq!(generated.kind, NodeKind::Method);
        assert!(generated.qualified_name.ends_with("C::generated"));
    }

    // `foo([1].map { … })` puts the block-bearing call inside another call's
    // arguments — regression coverage for the argument-position descent.
    #[test]
    fn test_ruby_block_body_reached_through_call_argument() {
        let source = r#"
class F
  foo([1].map { def in_arg; end })
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("f.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let in_arg = result
            .nodes
            .iter()
            .find(|n| n.name == "in_arg")
            .expect("expected in_arg method defined inside a block passed as an argument");
        assert_eq!(in_arg.kind, NodeKind::Method);
        assert!(in_arg.qualified_name.ends_with("F::in_arg"));
    }

    // `[1].map { … }.first` puts the block-bearing call as another call's
    // receiver — regression coverage for the receiver-position descent.
    #[test]
    fn test_ruby_block_body_reached_through_call_receiver() {
        let source = r#"
class G
  [1].map { def in_receiver; end }.first
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("g.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let in_receiver = result
            .nodes
            .iter()
            .find(|n| n.name == "in_receiver")
            .expect("expected in_receiver method defined inside a block used as a receiver");
        assert_eq!(in_receiver.kind, NodeKind::Method);
        assert!(in_receiver.qualified_name.ends_with("G::in_receiver"));
    }

    // `L = -> { … }` is a lambda literal, not a call with a block field —
    // regression coverage for the bare `"do_block" | "block"` arm of
    // `visit_expression_blocks`.
    #[test]
    fn test_ruby_block_body_reached_through_lambda_literal() {
        let source = r#"
class H
  L = -> { def in_lambda; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("h.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let in_lambda = result
            .nodes
            .iter()
            .find(|n| n.name == "in_lambda")
            .expect("expected in_lambda method defined inside a lambda literal body");
        assert_eq!(in_lambda.kind, NodeKind::Method);
        assert!(in_lambda.qualified_name.ends_with("H::in_lambda"));
    }

    // Same explicit-receiver guard as
    // `test_ruby_class_eval_with_explicit_receiver_not_extracted`, but reached
    // through the new expression-position descent (an assignment RHS) rather
    // than statement position — the guard must survive both paths.
    #[test]
    fn test_ruby_class_eval_explicit_receiver_not_extracted_through_assignment_rhs() {
        let source = r#"
class J
  X = Other.class_eval { def foreign; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("j.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result.nodes.iter().any(|n| n.name == "foreign"),
            "Other.class_eval has an explicit receiver we can't resolve, so its block \
             body must not be attached to the enclosing class even when reached through \
             an assignment RHS"
        );
    }

    // Pins the no-double-traversal invariant: `visit_expression_blocks` skips
    // a call's own `block` field while descending its other children, then
    // hands the block to `visit_block_body` exactly once. Without that skip,
    // `once` would be extracted twice.
    #[test]
    fn test_ruby_block_body_not_double_traversed() {
        let source = r#"
class K
  foo { def once; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("k.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let count = result.nodes.iter().filter(|n| n.name == "once").count();
        assert_eq!(
            count, 1,
            "expected exactly one node named once, got {count}"
        );
    }

    // Method bodies are traversed for definitions too, one level further in
    // than the block-body work above: `def install; [1].each { def m; end };
    // end` really does define `m` as an instance method of the enclosing
    // class once `install` runs (confirmed against Ruby 3.4.7).
    #[test]
    fn test_ruby_method_body_block_nested_def_attaches_to_enclosing_class() {
        let source = r#"
class C
  def install
    [1].each { def generated; end }
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let generated = result
            .nodes
            .iter()
            .find(|n| n.name == "generated")
            .expect("expected generated method defined inside a block inside a method body");
        assert_eq!(generated.kind, NodeKind::Method);
        assert!(generated.qualified_name.ends_with("C::generated"));
    }

    // Same gap, no block involved: a bare `def` directly inside a method body
    // still attaches to the enclosing class, not to the enclosing method.
    #[test]
    fn test_ruby_method_body_bare_nested_def_attaches_to_enclosing_class() {
        let source = r#"
class D
  def install
    def bare; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("d.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let bare = result
            .nodes
            .iter()
            .find(|n| n.name == "bare")
            .expect("expected bare method defined directly inside a method body");
        assert_eq!(bare.kind, NodeKind::Method);
        assert!(bare.qualified_name.ends_with("D::bare"));
    }

    // `def self.install` puts `self` inside the body on the class, but a `def`
    // nested inside it still has no receiver of its own, so it follows the
    // ordinary cref and lands on the *instance* side, not the singleton side -
    // it must not be found by a `private_class_method` targeting it.
    #[test]
    fn test_ruby_singleton_method_body_nested_def_is_instance_side() {
        let source = r#"
class E
  def self.install
    [1].each { def from_singleton; end }
  end

  private_class_method :from_singleton
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("e.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let from_singleton = result
            .nodes
            .iter()
            .find(|n| n.name == "from_singleton")
            .expect("expected from_singleton method defined inside def self.install's body");
        assert_eq!(from_singleton.kind, NodeKind::Method);
        assert!(from_singleton.qualified_name.ends_with("E::from_singleton"));
        assert_eq!(
            from_singleton.visibility,
            Visibility::Pub,
            "from_singleton is on the instance side, so private_class_method must not match it"
        );
    }

    // Inside `class << self`, `self` inside a method's body is the singleton
    // class itself, so a nested `def` there lands on the *class* method side -
    // the mirror image of the previous test.
    #[test]
    fn test_ruby_class_shovel_self_method_body_nested_def_is_singleton_side() {
        let source = r#"
class F
  class << self
    def install
      [1].each { def m; end }
    end
  end

  private_class_method :m
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("f.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let m = result
            .nodes
            .iter()
            .find(|n| n.name == "m")
            .expect("expected m method defined inside class << self's install body");
        assert_eq!(m.kind, NodeKind::SingletonMethod);
        assert!(m.qualified_name.ends_with("F::m"));
        assert_eq!(
            m.visibility,
            Visibility::Private,
            "m is on the singleton/class-method side, so private_class_method must match it"
        );
    }

    // A method body gets a *fresh* default-visibility frame: it does not
    // inherit the enclosing class body's `private`, unlike a class-body block
    // (which does - see test_ruby_singleton_class_does_not_inherit_outer_private
    // for the mirrored case that this one deliberately differs from).
    #[test]
    fn test_ruby_method_body_gets_fresh_public_visibility_frame() {
        let source = r#"
class G
  private

  def install
    def gen; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("g.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let gen = result
            .nodes
            .iter()
            .find(|n| n.name == "gen")
            .expect("expected gen method defined inside install's body");
        assert_eq!(
            gen.visibility,
            Visibility::Pub,
            "a method body starts a fresh public visibility frame, so it must not inherit \
             the enclosing class body's `private`"
        );
    }

    // The other direction of the same fresh-frame rule: a bare `private`
    // inside a method body cannot leak back out past `end` to affect defs
    // that follow in the class body.
    #[test]
    fn test_ruby_method_body_private_directive_does_not_leak_out() {
        let source = r#"
class H
  def install
    private
  end

  def after; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("h.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let after = result
            .nodes
            .iter()
            .find(|n| n.name == "after")
            .expect("expected after method following install");
        assert_eq!(
            after.visibility,
            Visibility::Pub,
            "a bare `private` inside a method body must not leak out to affect defs \
             in the enclosing class body"
        );
    }

    // Same explicit-receiver guard as
    // test_ruby_class_eval_with_explicit_receiver_not_extracted, one level
    // deeper: it must still hold when the `class_eval` call is reached
    // through a method body rather than directly through the class body.
    #[test]
    fn test_ruby_method_body_explicit_receiver_class_eval_not_extracted() {
        let source = r#"
class I
  def install
    Other.class_eval { def foreign; end }
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("i.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result.nodes.iter().any(|n| n.name == "foreign"),
            "Other.class_eval has an explicit receiver we can't resolve, so its block body \
             must not be attached to the enclosing class even when reached through a method body"
        );
    }

    // A receiverless call *is* `self.<call>`, so `self.class_eval` inside a
    // class body must be treated identically to bare `class_eval`
    // (`test_ruby_receiverless_class_eval_do_block_defines_instance_method`)
    // rather than falling into the unresolvable-receiver bail.
    #[test]
    fn test_ruby_self_class_eval_defines_instance_method() {
        let source = r#"
class C
  self.class_eval { def x; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let x = result
            .nodes
            .iter()
            .find(|n| n.name == "x")
            .expect("expected x method inside self.class_eval block");
        assert_eq!(x.kind, NodeKind::Method);
        assert!(x.qualified_name.ends_with("C::x"));
    }

    // The receiver names the enclosing class itself, so `C.class_eval` must
    // be treated the same as a receiverless `class_eval` — the receiver is
    // resolvable, just spelled out.
    #[test]
    fn test_ruby_enclosing_constant_class_eval_defines_instance_method() {
        let source = r#"
class C
  C.class_eval { def y; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let y = result
            .nodes
            .iter()
            .find(|n| n.name == "y")
            .expect("expected y method inside C.class_eval block");
        assert_eq!(y.kind, NodeKind::Method);
        assert!(y.qualified_name.ends_with("C::y"));
    }

    #[test]
    fn test_ruby_self_instance_eval_defines_singleton_method() {
        let source = r#"
class C
  self.instance_eval { def z; end }

  private_class_method :z
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let z = result
            .nodes
            .iter()
            .find(|n| n.name == "z")
            .expect("expected z method inside self.instance_eval block");
        assert_eq!(
            z.visibility,
            Visibility::Private,
            "private_class_method only resolves against singleton_method_ids, so this \
             only goes Private if self.instance_eval registered z as a singleton method"
        );
    }

    #[test]
    fn test_ruby_enclosing_constant_instance_eval_defines_singleton_method() {
        let source = r#"
class C
  C.instance_eval { def z; end }

  private_class_method :z
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let z = result
            .nodes
            .iter()
            .find(|n| n.name == "z")
            .expect("expected z method inside C.instance_eval block");
        assert_eq!(
            z.visibility,
            Visibility::Private,
            "private_class_method only resolves against singleton_method_ids, so this \
             only goes Private if C.instance_eval registered z as a singleton method"
        );
    }

    // Inside `class << self`, the ambient ReceiverSingleton handling already
    // makes a receiverless `class_eval` register its defs as class methods
    // (see `test_ruby_class_methods_do_block_matches_def_self`); `self` must
    // resolve identically, since `self.class_eval` and `class_eval` are the
    // same call.
    #[test]
    fn test_ruby_self_class_eval_inside_class_shovel_self_defines_class_method() {
        let source = r#"
class C
  class << self
    self.class_eval { def m; end }
  end

  private_class_method :m
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let m = result
            .nodes
            .iter()
            .find(|n| n.name == "m")
            .expect("expected m method inside class << self; self.class_eval block");
        assert_eq!(
            m.visibility,
            Visibility::Private,
            "self.class_eval inside class << self must still register m as a class method"
        );
    }

    // The one row that would break a naive "treat enclosing receiver as
    // receiverless" patch: inside `class << self`, `C.class_eval` names the
    // class itself, so its body defines an *instance* method — even though
    // the ambient `singleton_scope` there is `Enclosing` for a bare `def`.
    #[test]
    fn test_ruby_enclosing_constant_class_eval_inside_class_shovel_self_defines_instance_method() {
        let source = r#"
class C
  class << self
    C.class_eval { def m; end }
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let m = result
            .nodes
            .iter()
            .find(|n| n.name == "m")
            .expect("expected m method inside class << self; C.class_eval block");
        assert_eq!(
            m.kind,
            NodeKind::Method,
            "C.class_eval inside class << self must define an instance method, not a class \
             method, even though the ambient singleton_scope there is Enclosing"
        );
        assert!(m.qualified_name.ends_with("C::m"));
    }

    // `qualified_prefix`/`parent_node_id` can only address the innermost
    // enclosing scope, so a constant naming an outer-but-not-innermost scope
    // has nothing to attach the block's defs to — must stay Unresolvable,
    // same as `Other.class_eval`.
    #[test]
    fn test_ruby_outer_but_not_innermost_constant_class_eval_not_extracted() {
        let source = r#"
module Outer
  class Inner
    Outer.class_eval { def from_inner; end }
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("outer.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result.nodes.iter().any(|n| n.name == "from_inner"),
            "Outer.class_eval, reached from inside the innermost Inner scope, names an \
             outer-but-not-innermost scope we cannot attach to, so it must not be extracted"
        );
    }

    // `Class.new do … end` puts the block's defs on the newly created
    // anonymous class, not on the enclosing scope or on the constant it gets
    // assigned to — confirmed against Ruby 3.4.7. `visit_assignment_for_const`
    // already emits a `Const` node for `K`, not a class/module node, so there
    // is nowhere to attach `x` even if we wanted to.
    #[test]
    fn test_ruby_class_new_do_block_not_extracted() {
        let source = r#"
K = Class.new do
  def x; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("k.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result.nodes.iter().any(|n| n.name == "x"),
            "Class.new's block defines methods on a brand-new anonymous class, which we \
             cannot represent, so x must not be extracted"
        );
    }

    #[test]
    fn test_ruby_class_new_do_block_inside_class_not_extracted() {
        let source = r#"
class C
  K = Class.new do
    def x; end
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            !result.nodes.iter().any(|n| n.name == "x"),
            "Class.new's block must not leak x to the enclosing class C either"
        );
    }

    // `Module.new`, `Struct.new`, and `Data.define` are the same class-factory
    // family as `Class.new`: each puts the block's defs on a brand-new
    // anonymous object, not the enclosing scope.
    #[test]
    fn test_ruby_class_factory_family_blocks_not_extracted() {
        let source = r#"
A = Module.new do
  def a; end
end

B = Struct.new(:v) do
  def b; end
end

D = Data.define(:v) do
  def d; end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("factories.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        for name in ["a", "b", "d"] {
            assert!(
                !result.nodes.iter().any(|n| n.name == name),
                "{name} is defined inside a class-factory block and must not be extracted"
            );
        }
    }

    // The receiver is load-bearing for `classify_block_scope`: an ordinary
    // `.new` (not `Class`/`Module`/`Struct.new` or `Data.define`) stays
    // `Inherit`, so its block's defs still attach to the enclosing scope.
    #[test]
    fn test_ruby_ordinary_new_do_block_still_defines_method() {
        let source = r#"
class C
  Foo.new { def x; end }
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let x = result
            .nodes
            .iter()
            .find(|n| n.name == "x")
            .expect("expected x method inside Foo.new block — an ordinary .new stays Inherit");
        assert_eq!(x.kind, NodeKind::Method);
        assert!(x.qualified_name.ends_with("C::x"));
    }

    // `self` inside a plain instance-method body is an *instance* the
    // extractor cannot name, so a receiverless `instance_eval { def gen;
    // end }` there defines `gen` on that one instance, not on the class
    // (verified against Ruby 3.4.7: `i.respond_to?(:generated)` is true,
    // `C.respond_to?(:generated)` is false). There is no node in the graph
    // that can represent "the singleton class of an instance we can't name",
    // so the block is skipped outright — the same treatment already given
    // to `obj.instance_eval` for an unresolvable `obj`. The bug this fixes
    // is that `gen` used to be wrongly attributed to `C` as a class method;
    // the fix is that it is not attributed anywhere, so `private_class_method
    // :gen` categorically cannot match it.
    #[test]
    fn test_ruby_instance_eval_in_instance_method_body_is_per_instance() {
        let source = r#"
class C
  def install
    instance_eval { def gen; end }
  end

  private_class_method :gen
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            result.nodes.iter().all(|n| n.name != "gen"),
            "gen is per-instance and cannot be attributed to any node in the graph, \
             so it must not be extracted as a method of C — and private_class_method \
             :gen must not have anything to match"
        );
    }

    // `self.instance_eval` must behave identically to the receiverless form
    // inside an instance-method body — `self.foo` and `foo` are the same
    // call in Ruby.
    #[test]
    fn test_ruby_self_instance_eval_in_instance_method_body_is_per_instance() {
        let source = r#"
class C
  def install
    self.instance_eval { def gen; end }
  end

  private_class_method :gen
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(
            result.nodes.iter().all(|n| n.name != "gen"),
            "self.instance_eval must match the receiverless form: gen is per-instance \
             and must not be extracted as a method of C"
        );
    }

    // `def self.foo` written inside an instance-method body opens the
    // singleton class of that one *instance*, not the enclosing class -
    // Ruby's own `NameError` on `private_class_method :foo` there confirms
    // it ("undefined method 'foo' for class '#<Class:B1>'").
    #[test]
    fn test_ruby_def_self_in_instance_method_body_is_per_instance() {
        let source = r#"
class C
  def install
    def self.foo; end
  end

  private_class_method :foo
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let foo = result
            .nodes
            .iter()
            .find(|n| n.name == "foo")
            .expect("expected foo singleton method inside install's body");
        assert_eq!(
            foo.visibility,
            Visibility::Pub,
            "foo is a singleton method of the instance, not the class; \
             private_class_method must not match it"
        );
    }

    // `class << self` written inside an instance-method body reopens the
    // singleton class of that one *instance* — the same conclusion as the
    // previous test via a different syntax.
    #[test]
    fn test_ruby_shovel_self_in_instance_method_body_is_per_instance() {
        let source = r#"
class C
  def install
    class << self
      def bar; end
    end
  end

  private_class_method :bar
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let bar = result
            .nodes
            .iter()
            .find(|n| n.name == "bar")
            .expect("expected bar method inside install's class << self body");
        assert_eq!(
            bar.visibility,
            Visibility::Pub,
            "bar is a singleton method of the instance, not the class; \
             private_class_method must not match it"
        );
    }

    // Contrast: inside a *singleton*-method body, `self` is the class
    // itself, so `instance_eval` there still opens the class's own
    // singleton class, exactly as in a class body. This must not regress
    // when instance-method bodies stop treating `self` as the class.
    #[test]
    fn test_ruby_instance_eval_in_singleton_method_body_still_targets_class() {
        let source = r#"
class D
  def self.install
    instance_eval { def gen; end }
  end

  private_class_method :gen
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("d.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let gen = result
            .nodes
            .iter()
            .find(|n| n.name == "gen")
            .expect("expected gen method inside self.install's instance_eval block");
        assert_eq!(
            gen.visibility,
            Visibility::Private,
            "self in a singleton-method body is still the class, so instance_eval \
             there must still target it"
        );
    }

    // `concerning`'s topic argument must be a literal naming a valid
    // constant, since activesupport routes it to `const_set` (which raises
    // `NameError: wrong constant name` for a lowercase topic, verified
    // against activesupport 8.1.3). A non-constant topic is not the Rails
    // form, so the block must fall through to ordinary `Inherit` handling
    // rather than being treated as Concern DSL.
    #[test]
    fn test_ruby_concerning_lowercase_topic_is_not_concern_dsl() {
        let source = r#"
class C
  concerning :lowercase do
    class_methods { def x; end }
  end

  private :x
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let x = result
            .nodes
            .iter()
            .find(|n| n.name == "x")
            .expect("expected x method inside the lowercase-topic concerning block");
        assert_eq!(x.kind, NodeKind::Method);
        assert!(x.qualified_name.ends_with("C::x"));
        assert_eq!(
            x.visibility,
            Visibility::Private,
            "a lowercase topic is not the Rails concerning form, so class_methods \
             falls through to Inherit and x lands on the instance side"
        );
    }

    // Regression guard: last round's headline feature — a plain `def`
    // reached through an ordinary `Inherit` block (`each`, never consulting
    // the receiver) inside an instance-method body — must still define an
    // instance method of the enclosing class.
    #[test]
    fn test_ruby_ordinary_each_block_in_instance_method_body_still_defines_instance_method() {
        let source = r#"
class C
  def install
    [1].each { def m; end }
  end
end
"#;
        let extractor = RubyExtractor;
        let result = extractor.extract("c.rb", source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let m = result
            .nodes
            .iter()
            .find(|n| n.name == "m")
            .expect("expected m method defined inside install's each block");
        assert_eq!(m.kind, NodeKind::Method);
        assert!(m.qualified_name.ends_with("C::m"));
    }
} // mod ruby_tests
