#![cfg(feature = "lang-ruby")]

use tokensave::extraction::{
    LanguageExtractor, LanguageRegistry, RubyExtractor, RubyTemplateExtractor,
};
use tokensave::types::{EdgeKind, ExtractionResult, NodeKind};

fn extract(path: &str, source: &str) -> ExtractionResult {
    let result = RubyTemplateExtractor.extract(path, source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    result
}

fn calls(result: &ExtractionResult) -> Vec<&str> {
    result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect()
}

fn assert_location(result: &ExtractionResult, source: &str, name: &str, token: &str) {
    let reference = result
        .unresolved_refs
        .iter()
        .find(|r| r.reference_name == name)
        .unwrap_or_else(|| panic!("missing {name}: {:?}", calls(result)));
    assert!(
        source.lines().nth(reference.line as usize).unwrap()[reference.column as usize..]
            .starts_with(token),
        "wrong location for {name}: {reference:?}"
    );
}

#[test]
fn template_registry_dispatch() {
    let registry = LanguageRegistry::new();
    for path in [
        "app/views/items/show.html.erb",
        "mail.text.erb",
        "view.html.slim",
    ] {
        let extractor = registry.extractor_for_file(path).unwrap();
        assert_eq!(extractor.language_name(), "Ruby");
    }
}

#[test]
fn erb_fixture_calls_and_locations() {
    let source = include_str!("fixtures/sample.html.erb");
    let result = extract("sample.html.erb", source);
    for name in [
        "page_title",
        "load_rows",
        "visible?",
        "rows.each",
        "format_row",
        "empty_message",
    ] {
        assert_location(&result, source, name, name.split('.').next().unwrap());
    }
    for name in [
        "rows",
        "row",
        "ignored_helper",
        "escaped_helper",
        "literal_helper",
    ] {
        assert!(!calls(&result).contains(&name), "unexpected {name}");
    }
    let file = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .unwrap();
    assert!(result
        .unresolved_refs
        .iter()
        .all(|r| r.from_node_id == file.id));
    assert_eq!(file.end_line as usize, source.lines().count() - 1);
}

#[test]
fn slim_fixture_calls_and_locations() {
    let source = include_str!("fixtures/sample.html.slim");
    let result = extract("sample.html.slim", source);
    for name in [
        "page_title",
        "load_rows",
        "visible?",
        "rows.each",
        "format_row",
        "empty_message",
        "tooltip",
        "greeting",
    ] {
        assert_location(&result, source, name, name.split('.').next().unwrap());
    }
    for name in [
        "rows",
        "row",
        "ignored_helper",
        "literal_helper",
        "p",
        "h1",
        "title",
    ] {
        assert!(!calls(&result).contains(&name), "unexpected {name}");
    }
}

#[test]
fn erb_trim_comments_multiline_and_unicode() {
    let source = "é<%- if enabled? -%><%= first() %><%= second() %><% end %>\r\n<%= format_value(\n  value()\n) %>\n<%# ignored(\n  hidden()\n) %>\n<%# eof";
    let result = extract("view.erb", source);
    for name in ["enabled?", "first", "second", "format_value", "value"] {
        assert_location(&result, source, name, name);
    }
    assert!(!calls(&result).contains(&"ignored"));
    assert!(!calls(&result).contains(&"hidden"));
}

#[test]
fn erb_definitions_keep_ownership_and_original_locations() {
    let source = "<p>hello</p>\n<% def nested_helper\n  inner_call()\nend %>\n<%= outer_call() %>";
    let result = extract("view.erb", source);
    let helper = result
        .nodes
        .iter()
        .find(|n| n.name == "nested_helper")
        .unwrap();
    assert_eq!(
        (helper.start_line, helper.start_column, helper.end_line),
        (1, 3, 3)
    );
    let inner = result
        .unresolved_refs
        .iter()
        .find(|r| r.reference_name == "inner_call")
        .unwrap();
    assert_eq!(inner.from_node_id, helper.id);
    assert_eq!(
        calls(&result)
            .iter()
            .filter(|&&name| name == "inner_call")
            .count(),
        1
    );
    assert_location(&result, source, "outer_call", "outer_call");
}

#[test]
fn slim_attributes_interpolation_and_text_blocks() {
    let source = "p(title=tooltip() class=(classes(flag()))) Hello #{label({key: value()})}\np data-info=payload()\np title=\"Hello #{caption()}\"\np *attributes()\n| Text\n  = literal() #{text_helper()}\n/ comment\n  #{hidden()}\n";
    let result = extract("view.slim", source);
    for name in [
        "tooltip",
        "classes",
        "flag",
        "label",
        "value",
        "payload",
        "caption",
        "attributes",
        "text_helper",
    ] {
        assert_location(&result, source, name, name);
    }
    for name in ["literal", "hidden", "key"] {
        assert!(!calls(&result).contains(&name), "unexpected {name}");
    }
}

#[test]
fn slim_blocks_continuations_and_embedded_ruby() {
    let source = "= form_for(record()) do |form|\n  = form.field :name\n- unless allowed?\n  = denied()\n- else\n  = accepted()\n= combine(first(),\n    second())\nruby:\n  label = calculate()\np = label\n= final_call()\n";
    let result = extract("view.slim", source);
    for name in [
        "form_for",
        "record",
        "form.field",
        "allowed?",
        "denied",
        "accepted",
        "combine",
        "first",
        "second",
        "calculate",
        "final_call",
    ] {
        assert_location(&result, source, name, name.split('.').next().unwrap());
    }
    for name in ["form", "label"] {
        assert!(!calls(&result).contains(&name), "unexpected {name}");
    }
}

#[tokio::test]
async fn templates_resolve_calls_to_ruby_helpers() {
    use tokensave::resolution::ReferenceResolver;
    let dir = tempfile::tempdir().unwrap();
    let (db, _) = tokensave::db::Database::initialize(&dir.path().join("test.db"))
        .await
        .unwrap();
    let ruby = RubyExtractor.extract("helpers.rb", "def template_helper\nend\n");
    for (path, source) in [
        ("view.erb", "<%= template_helper() %>"),
        ("view.slim", "= template_helper()\n"),
    ] {
        let template = extract(path, source);
        let mut nodes = ruby.nodes.clone();
        nodes.extend(template.nodes);
        let resolver = ReferenceResolver::from_nodes(&db, &nodes);
        let resolved = resolver.resolve_all(&template.unresolved_refs);
        assert!(resolved
            .resolved
            .iter()
            .any(|reference| reference.target_node_id
                == ruby
                    .nodes
                    .iter()
                    .find(|n| n.name == "template_helper")
                    .unwrap()
                    .id));
    }
}

#[test]
fn templates_with_only_markup_have_no_ruby_calls() {
    for (path, source) in [
        ("empty.erb", "<div>hello()</div>"),
        ("empty.slim", "p hello()\n/ comment\n  = hidden()\n"),
        ("empty.slim", ""),
    ] {
        let result = extract(path, source);
        assert!(calls(&result).is_empty(), "{:?}", calls(&result));
        assert_eq!(result.nodes.len(), 1);
    }
}

#[test]
fn slim_multiline_attributes_and_tag_expansion() {
    let source = "div(\n  title=tooltip()\n  class=(\n    classes(flag())\n  )\n)\n  p: span = nested_label()\n= after_attributes()\n";
    let result = extract("view.slim", source);
    for name in [
        "tooltip",
        "classes",
        "flag",
        "nested_label",
        "after_attributes",
    ] {
        assert_location(&result, source, name, name);
    }
    assert_eq!(calls(&result).len(), 5, "{:?}", calls(&result));
}

#[test]
fn template_local_bindings_are_not_helpers() {
    let erb = "<% left, right = pair() %><% rows().each do |item; scratch| %><%= show(item, left, right, scratch) %><% end %><% for entry in entries() %><%= show(entry) %><% end %>";
    let result = extract("view.erb", erb);
    for name in ["left", "right", "item", "scratch", "entry"] {
        assert!(
            !calls(&result).contains(&name),
            "unexpected {name}: {:?}",
            calls(&result)
        );
    }
}

#[test]
fn erb_escaped_closing_delimiter_keeps_following_ruby() {
    let source = "<% value = \"%%>\"; log_value(value) %><%= label() %>";
    let result = extract("view.erb", source);
    for name in ["log_value", "label"] {
        assert_location(&result, source, name, name);
    }
    assert!(!calls(&result).contains(&"value"));
}

#[test]
fn slim_unescaped_interpolation_and_output() {
    let source = "p #{{raw_label()}}\np == raw_body()\n== raw_footer()\n";
    let result = extract("view.slim", source);
    for name in ["raw_label", "raw_body", "raw_footer"] {
        assert_location(&result, source, name, name);
    }
    assert_eq!(calls(&result).len(), 3);
}

#[test]
fn template_alias_names_are_not_calls() {
    let result = extract(
        "view.erb",
        "<% alias new_name old_name; undef retired_name %>",
    );
    assert!(calls(&result).is_empty(), "{:?}", calls(&result));
}

#[test]
fn template_dynamic_method_bare_calls_keep_their_owner() {
    let result = extract(
        "view.erb",
        "<% define_method(:caption) { nested_helper } %><%= page_helper %>",
    );
    let method = result.nodes.iter().find(|n| n.name == "caption").unwrap();
    let reference = result
        .unresolved_refs
        .iter()
        .find(|r| r.reference_name == "nested_helper")
        .unwrap();
    assert_eq!(reference.from_node_id, method.id);
    let file = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .unwrap();
    let outer = result
        .unresolved_refs
        .iter()
        .find(|r| r.reference_name == "page_helper")
        .unwrap();
    assert_eq!(outer.from_node_id, file.id);
}

#[test]
fn slim_inline_html_keeps_nested_slim_code() {
    let source = "<html data-title=\"#{html_title()}\">\n  - if ready?\n    = heading()\n  - items.each do |item|\n    = label(item)\n</html>\n= footer()\n";
    let result = extract("layout.slim", source);
    for name in [
        "html_title",
        "ready?",
        "heading",
        "items.each",
        "label",
        "footer",
    ] {
        assert_location(&result, source, name, name.split('.').next().unwrap());
    }
    assert!(!calls(&result).contains(&"item"));
}

#[test]
fn slim_layout_fixture_preserves_calls_and_bindings() {
    let source = include_str!("fixtures/sample_layout.html.slim");
    let result = extract("layout.slim", source);
    for name in [
        "items.each",
        "title_for",
        "link_to",
        "item_path",
        "price",
        "qty",
        "payload",
        "show",
        "footer",
    ] {
        assert_location(&result, source, name, name.split('.').next().unwrap());
    }
    for name in ["doc", "total", "title", "value"] {
        assert!(!calls(&result).contains(&name), "unexpected {name}");
    }
}

#[test]
fn slim_broken_line_call_arguments_keep_source_locations() {
    let source = "= link_to \\\n  \"x\", path\n- total = price \\\n  * qty\n= total\n= footer()\n";
    let result = extract("view.slim", source);
    for name in ["link_to", "path", "price", "qty", "footer"] {
        assert_location(&result, source, name, name);
    }
    assert!(!calls(&result).contains(&"total"));
}

#[test]
fn template_pattern_bindings_are_not_helper_calls() {
    for (code, bindings) in [
        ("case payload(); in [a, b]; show(a, b); end", vec!["a", "b"]),
        ("case payload(); in Integer => n; show(n); end", vec!["n"]),
        ("payload() => x; show(x)", vec!["x"]),
        ("payload() in [a, *rest]; show(a, rest)", vec!["a", "rest"]),
        (
            "case payload(); in {title:, key: value, **rest}; show(title, value, rest); end",
            vec!["title", "value", "rest"],
        ),
        (
            "case payload(); in [*, target, *]; show(target); end",
            vec!["target"],
        ),
        (
            "case payload(); in [_choice] | {key: _choice}; show(_choice); end",
            vec!["_choice"],
        ),
        (
            "case payload(); in {\"title\":}; show(title); end",
            vec!["title"],
        ),
    ] {
        for (path, source) in [
            ("view.erb", format!("<% {code} %>")),
            ("view.slim", format!("ruby:\n  {code}\n")),
        ] {
            let result = extract(path, &source);
            for name in &bindings {
                assert!(
                    !calls(&result).contains(name),
                    "unexpected binding {name} in {code}: {:?}",
                    calls(&result)
                );
            }
            for name in ["payload", "show"] {
                assert_location(&result, &source, name, name);
            }
        }
    }
}

#[test]
fn template_pattern_guards_pins_and_interpolation_still_call_helpers() {
    let source = "<% existing = 1; case payload(); in [^existing, ^(pin_helper), bound] if guard_helper(bound); show(bound); end; case payload(); in \"#{pattern_label}\"; show(); end %>";
    let result = extract("view.erb", source);
    for name in [
        "payload",
        "pin_helper",
        "guard_helper",
        "show",
        "pattern_label",
    ] {
        assert_location(&result, source, name, name);
    }
    for name in ["existing", "bound"] {
        assert!(!calls(&result).contains(&name), "unexpected {name}");
    }
    let slim = "- case payload()\n- in [a, b] if guard_helper(a)\n  = show(a, b)\n= footer()\n";
    let result = extract("view.slim", slim);
    for name in ["payload", "guard_helper", "show", "footer"] {
        assert_location(&result, slim, name, name);
    }
    for name in ["a", "b"] {
        assert!(!calls(&result).contains(&name), "unexpected {name}");
    }
}

#[test]
fn template_receivers_are_recorded_for_the_helper_filter() {
    // Receivers stay in the extraction (a helper like `current_user` is often
    // one); the resolver's helper-only filter is what keeps partial locals off
    // unrelated methods.
    let erb = "<%= item.title %><%= item&.name %><%= item[0] %><%= item.price.round(2) %>";
    let slim = "p = item.title\np = item&.name\n= item[0]\n";
    for (path, source) in [("_row.html.erb", erb), ("_row.html.slim", slim)] {
        let result = extract(path, source);
        assert!(calls(&result).contains(&"item"), "{path}");
        assert!(calls(&result).contains(&"item.title"), "{path}");
    }
}

/// Resolve a template's references against some Ruby files plus itself,
/// returning every node and each resolved (reference name, target id).
async fn resolve_template(
    ruby: &[(&str, &str)],
    template: (&str, &str),
) -> (Vec<tokensave::types::Node>, Vec<(String, String)>) {
    use tokensave::resolution::ReferenceResolver;
    let dir = tempfile::tempdir().unwrap();
    let (db, _) = tokensave::db::Database::initialize(&dir.path().join("test.db"))
        .await
        .unwrap();
    let mut nodes = Vec::new();
    for (path, source) in ruby {
        nodes.extend(RubyExtractor.extract(path, source).nodes);
    }
    let extracted = extract(template.0, template.1);
    nodes.extend(extracted.nodes.clone());
    let resolver = ReferenceResolver::from_nodes(&db, &nodes);
    let resolution = resolver.resolve_all(&extracted.unresolved_refs);
    let edges = resolution
        .resolved
        .iter()
        .map(|r| (r.original.reference_name.clone(), r.target_node_id.clone()))
        .collect();
    (nodes, edges)
}

#[tokio::test]
async fn template_partial_locals_do_not_bind_to_unrelated_methods() {
    let ruby = [
        (
            "app/models/order.rb",
            "class Order\n  def item\n  end\nend\n",
        ),
        (
            "app/helpers/application_helper.rb",
            "module ApplicationHelper\n  def format_price(value)\n  end\nend\n",
        ),
    ];
    for template in [
        (
            "app/views/orders/_row.html.erb",
            "<%= item.title %>\n<%= item&.name %>\n<%= item[0] %>\n<%= item %>\n<%= format_price(item.price) %>\n",
        ),
        (
            "app/views/orders/_row.html.slim",
            "p = item.title\np = item&.name\np = item[0]\np = item\np = format_price(item.price)\n",
        ),
    ] {
        let (nodes, edges) = resolve_template(&ruby, template).await;
        let id_of = |name: &str| {
            nodes
                .iter()
                .find(|n| n.name == name && n.kind != NodeKind::File)
                .unwrap()
                .id
                .clone()
        };
        let order_item = id_of("item");
        assert!(
            edges.iter().all(|(_, target)| *target != order_item),
            "{}: partial local bound to Order#item: {edges:?}",
            template.0
        );
        let helper = id_of("format_price");
        assert!(
            edges
                .iter()
                .any(|(name, target)| name == "format_price" && *target == helper),
            "{}: helper call unresolved: {edges:?}",
            template.0
        );
    }
}

#[tokio::test]
async fn template_bare_calls_reach_helper_shaped_targets_only() {
    let ruby = [
        (
            "lib/formatting_helper.rb",
            "module FormattingHelper\n  def money(value)\n  end\nend\n",
        ),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController\n  helper_method :current_user\n  def current_user\n  end\nend\n",
        ),
        (
            "app/models/report.rb",
            "class Report\n  def summary\n  end\nend\n",
        ),
    ];
    let (nodes, edges) = resolve_template(
        &ruby,
        (
            "app/views/reports/show.html.erb",
            "<%= money(1) %><%= current_user %><%= summary %>",
        ),
    )
    .await;
    let resolved: Vec<&str> = edges.iter().map(|(name, _)| name.as_str()).collect();
    assert!(resolved.contains(&"money"), "{resolved:?}");
    assert!(resolved.contains(&"current_user"), "{resolved:?}");
    let summary = nodes.iter().find(|n| n.name == "summary").unwrap();
    assert!(
        edges.iter().all(|(_, target)| *target != summary.id),
        "{resolved:?}"
    );
}

#[tokio::test]
async fn template_helper_receivers_resolve_to_the_helper() {
    let ruby = [(
        "app/controllers/application_controller.rb",
        "class ApplicationController\n  helper_method :current_user\n  def current_user\n  end\nend\n",
    )];
    for template in [
        (
            "app/views/layouts/application.html.erb",
            "<%= current_user.name %>",
        ),
        (
            "app/views/layouts/application.html.slim",
            "p = current_user.name\n",
        ),
    ] {
        let (nodes, edges) = resolve_template(&ruby, template).await;
        let current_user = nodes
            .iter()
            .find(|n| n.name == "current_user" && n.kind == NodeKind::Method)
            .unwrap();
        assert!(
            edges
                .iter()
                .any(|(name, target)| name == "current_user" && *target == current_user.id),
            "{}: {edges:?}",
            template.0
        );
    }
}
