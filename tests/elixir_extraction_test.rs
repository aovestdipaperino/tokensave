//! ExUnit test bodies must reach the call graph — #387.
//!
//! `test "..." do ... end`, `describe`, `setup` and `setup_all` are macro
//! invocations, not language constructs: to tree-sitter they are ordinary
//! `call` nodes carrying a `do_block`. `extract_calls` is only ever invoked
//! with a `def`'s node id, so a call inside a test block was attributed to
//! nothing and never became an edge.
//!
//! The consequence was not missing edges in the abstract — `tokensave_affected`
//! returned an empty list for a directly tested source file, and
//! `tokensave_test_risk` reported `has_test: false` and `coverage_pct: 0.0`
//! for a function with a passing test sitting right next to it. Confidently
//! wrong, in the direction that makes someone write a duplicate test.
//!
//! Calls are attributed to the enclosing binding — here the test `defmodule` —
//! matching how #346 resolved the same question for TypeScript arrows passed
//! as arguments. `doctest` is deliberately not modelled; see the module-level
//! comment in the extractor for why.
#![cfg(feature = "lang-elixir")]

use tokensave::extraction::ElixirExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

/// Names of everything the extraction reported as a `Calls` reference.
fn call_targets(result: &ExtractionResult) -> Vec<String> {
    result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.clone())
        .collect()
}

/// The #387 reproduction, reduced from `mix new`'s generated project: the call
/// the test makes must be recorded.
#[test]
fn test_elixir_call_inside_a_test_block_is_recorded() {
    let source = r#"
defmodule TsElixirReproTest do
  use ExUnit.Case

  test "greets the world" do
    assert TsElixirRepro.hello() == :world
  end
end
"#;
    let extractor = ElixirExtractor;
    let result = extractor.extract("test/ts_elixir_repro_test.exs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let calls = call_targets(&result);
    assert!(
        calls
            .iter()
            .any(|c| c == "TsElixirRepro.hello" || c == "hello"),
        "the call inside `test ... do` must reach the graph, got {calls:?}"
    );
}

/// Attribution target: the enclosing `defmodule`, per the #346 precedent. A
/// test block has no named symbol of its own, and synthesising one per test
/// would invent graph nodes with no declaration behind them.
#[test]
fn test_elixir_test_block_calls_attribute_to_the_enclosing_module() {
    let source = r#"
defmodule MathTest do
  use ExUnit.Case

  test "adds" do
    assert Math.add(1, 2) == 3
  end
end
"#;
    let extractor = ElixirExtractor;
    let result = extractor.extract("test/math_test.exs", source);

    let module = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Module && n.name == "MathTest")
        .expect("the test module must be extracted");

    let add_ref = result
        .unresolved_refs
        .iter()
        .find(|r| r.reference_kind == EdgeKind::Calls && r.reference_name.contains("add"))
        .expect("the call must be recorded");

    assert_eq!(
        add_ref.from_node_id, module.id,
        "a call inside a test block must be attributed to the enclosing module"
    );
}

/// `describe` nests `test` blocks. Every call must be recorded exactly once —
/// attributing at both levels would double every edge.
#[test]
fn test_elixir_nested_describe_records_each_call_once() {
    let source = r#"
defmodule NestedTest do
  use ExUnit.Case

  describe "arithmetic" do
    test "adds" do
      assert Math.add(1, 2) == 3
    end

    test "subtracts" do
      assert Math.sub(3, 1) == 2
    end
  end
end
"#;
    let extractor = ElixirExtractor;
    let result = extractor.extract("test/nested_test.exs", source);

    for wanted in ["add", "sub"] {
        let hits = result
            .unresolved_refs
            .iter()
            .filter(|r| r.reference_kind == EdgeKind::Calls && r.reference_name.contains(wanted))
            .count();
        assert_eq!(
            hits, 1,
            "`{wanted}` must be recorded exactly once, got {hits}"
        );
    }
}

/// `setup` and `setup_all` bodies run real code and call real functions, so
/// they belong in the graph on the same footing as `test`.
#[test]
fn test_elixir_setup_block_calls_are_recorded() {
    let source = r#"
defmodule SetupTest do
  use ExUnit.Case

  setup do
    Fixtures.build_user()
    :ok
  end

  setup_all do
    Fixtures.start_server()
    :ok
  end

  test "works" do
    assert true
  end
end
"#;
    let extractor = ElixirExtractor;
    let result = extractor.extract("test/setup_test.exs", source);
    let calls = call_targets(&result);

    for wanted in ["build_user", "start_server"] {
        assert!(
            calls.iter().any(|c| c.contains(wanted)),
            "`{wanted}` in a setup block must reach the graph, got {calls:?}"
        );
    }
}

/// The control: production code must be unaffected. A call inside `def` was
/// always attributed to that function and must still be, rather than being
/// re-attributed to the module.
#[test]
fn test_elixir_calls_inside_def_still_attribute_to_the_function() {
    let source = r#"
defmodule Caller do
  def run do
    Helper.work()
  end
end
"#;
    let extractor = ElixirExtractor;
    let result = extractor.extract("lib/caller.ex", source);

    let run = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "run")
        .expect("`run` must be extracted");

    let work = result
        .unresolved_refs
        .iter()
        .find(|r| r.reference_kind == EdgeKind::Calls && r.reference_name.contains("work"))
        .expect("the call must be recorded");

    assert_eq!(
        work.from_node_id, run.id,
        "a call inside `def` must stay attributed to the function, not the module"
    );
}

/// `doctest Foo` generates tests from `@doc` examples at compile time. There is
/// no call expression in the source to attribute, so nothing is claimed for it.
/// Pinned so the decision is visible rather than incidental: if someone later
/// makes `doctest` imply module-wide coverage, this fails and they have to say
/// so deliberately.
#[test]
fn test_elixir_doctest_is_not_modelled_as_a_call() {
    let source = r#"
defmodule DoctestOnlyTest do
  use ExUnit.Case
  doctest MyLib
end
"#;
    let extractor = ElixirExtractor;
    let result = extractor.extract("test/doctest_only_test.exs", source);

    let calls = call_targets(&result);
    assert!(
        !calls.iter().any(|c| c.contains("MyLib")),
        "doctest must not fabricate a call edge to the module it names, got {calls:?}"
    );
}

#[test]
fn test_elixir_eex_template_extracts_embedded_code() {
    let source = "é<main>static_only()</main>\né<%= top_level_helper() %>\n\
<%== raw_output_helper() %>\n<%- trim_helper() -%>\n\
<%# commented_helper() %>\n<%% escaped_helper() %>\n\
<% defmodule EexFixture do %>\n<% def render do %>\n\
  <%= Nested.helper() %>\n<% end %>\n<% end %>\n";
    let registry = tokensave::extraction::LanguageRegistry::new();
    assert!(
        registry.extractor_for_file("views/show.eex").is_some(),
        "EEx files must dispatch to the Elixir extractor"
    );
    let extractor = registry
        .extractor_for_file("views/show.eex")
        .expect("EEx extractor exists");
    assert_eq!(extractor.language_name(), "Elixir");
    assert!(
        registry.extractor_for_file("views/show.heex").is_none(),
        "HEEx support is outside the Elixir EEx extension"
    );

    let result = extractor.extract("views/show.eex", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls = call_targets(&result);

    let top_level = result
        .unresolved_refs
        .iter()
        .find(|reference| reference.reference_name == "top_level_helper")
        .expect("top-level EEx expressions must be attributed to the file");
    let file = result
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File)
        .expect("the EEx file node must exist");
    assert_eq!(top_level.from_node_id, file.id);
    assert_eq!((top_level.line, top_level.column), (1, 6));
    assert!(source
        .lines()
        .nth(top_level.line as usize)
        .expect("line exists")[top_level.column as usize..]
        .starts_with("top_level_helper"));
    for wanted in ["raw_output_helper", "trim_helper"] {
        assert_eq!(
            calls.iter().filter(|call| call.contains(wanted)).count(),
            1,
            "expected one top-level reference to `{wanted}`"
        );
    }

    let render = result
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.name == "render")
        .expect("Elixir definitions in EEx tags must be extracted");
    let nested = result
        .unresolved_refs
        .iter()
        .find(|reference| reference.reference_name == "Nested.helper")
        .expect("calls inside EEx definitions must be extracted");
    assert_eq!(nested.from_node_id, render.id);
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.as_str() == "Nested.helper")
            .count(),
        1,
        "nested calls must not be recorded twice"
    );

    for ignored in ["static_only", "commented_helper", "escaped_helper"] {
        assert!(
            !calls.iter().any(|call| call.contains(ignored)),
            "non-code EEx content must not contribute `{ignored}`"
        );
    }
}

#[test]
fn test_elixir_eex_escaped_open_does_not_skip_active_tag() {
    let source = "<%% literal <%= active_after_escaped() %>";
    let result = ElixirExtractor.extract("views/escaped.eex", source);

    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|reference| reference.reference_name == "active_after_escaped"),
        "an escaped opening delimiter must not consume a later active tag"
    );
}

#[test]
fn test_elixir_eex_bang_comment_is_ignored() {
    let source = "<%!-- hidden_in_comment() --%><%= visible_after_comment() %>";
    let result = ElixirExtractor.extract("views/bang-comment.eex", source);

    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|reference| reference.reference_name == "visible_after_comment"),
        "active tags after a bang comment must still be extracted"
    );
    assert!(
        !result
            .unresolved_refs
            .iter()
            .any(|reference| reference.reference_name == "hidden_in_comment"),
        "code inside a bang comment must not be extracted"
    );
}

#[test]
fn test_elixir_eex_comment_percent_escape_does_not_skip_active_tag() {
    let source = "<%# comment %%> <%= active_after_comment() %>";
    let result = ElixirExtractor.extract("views/comment-percent.eex", source);

    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|reference| reference.reference_name == "active_after_comment"),
        "the first EEx closing delimiter must end the comment tag"
    );
}
