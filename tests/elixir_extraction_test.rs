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
