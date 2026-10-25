//! HTML and CSS extraction — #507.
//!
//! Both file types were skipped outright ("no registered extractor"), so a
//! project's templates and stylesheets were invisible to every graph query.
//! Neither language has callable symbols, so what these extractors record is
//! the names a file *defines* and the files it pulls in.

#![cfg(all(feature = "lang-html", feature = "lang-css"))]

use tokensave::extraction::{CssExtractor, HtmlExtractor, LanguageExtractor};
use tokensave::types::{EdgeKind, NodeKind};

const PAGE: &str = r#"<!DOCTYPE html>
<html>
  <head>
    <link rel="stylesheet" href="styles/site.css">
    <link rel="icon" href="favicon.ico">
    <script src="app.js"></script>
  </head>
  <body>
    <div id="app" class="container">
      <my-widget label="hello"></my-widget>
      <div class="row">plain</div>
    </div>
    <template id="row-template"></template>
  </body>
</html>
"#;

const SHEET: &str = r#"@import "reset.css";

:root {
  --brand: #336699;
  --spacing: 8px;
}

.container {
  color: var(--brand);
  padding: var(--spacing);
}

#app {
  margin: 0;
}

.container, .row {
  display: flex;
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

@media (min-width: 40rem) {
  .row {
    gap: var(--spacing);
  }
}
"#;

fn names_of(result: &tokensave::types::ExtractionResult, kind: NodeKind) -> Vec<String> {
    let mut names: Vec<String> = result
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.name.clone())
        .collect();
    names.sort();
    names
}

#[test]
fn html_records_ids_components_and_imports() {
    let result = HtmlExtractor.extract("index.html", PAGE);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Elements carrying an `id`, whatever the tag.
    assert_eq!(names_of(&result, NodeKind::Field), ["app", "row-template"]);

    // A custom element names a component; builtin tags do not.
    assert_eq!(names_of(&result, NodeKind::Class), ["my-widget"]);

    // The page's imports: a stylesheet link and a script source. `rel="icon"`
    // is a link but not an import of anything the graph can follow.
    assert_eq!(
        names_of(&result, NodeKind::Use),
        ["app.js", "styles/site.css"]
    );

    // Every emitted node hangs off the file.
    let contains = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .count();
    assert_eq!(
        contains,
        result.nodes.len() - 1,
        "one edge per non-file node"
    );
}

#[test]
fn html_emits_no_reference_for_a_class_attribute() {
    // Class names are ordinary words. Resolving them by bare name is how a
    // stylesheet ends up owning an edge into unrelated code (#503).
    let result = HtmlExtractor.extract("index.html", PAGE);
    assert!(
        result.unresolved_refs.is_empty(),
        "markup should not guess at cross-file names: {:?}",
        result.unresolved_refs
    );
}

#[test]
fn css_records_selectors_properties_and_keyframes() {
    let result = CssExtractor.extract("site.css", SHEET);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // A selector repeated under a media query is one node, not three.
    assert_eq!(names_of(&result, NodeKind::Class), ["container", "row"]);
    assert_eq!(names_of(&result, NodeKind::Field), ["app"]);
    assert_eq!(names_of(&result, NodeKind::Const), ["--brand", "--spacing"]);
    assert_eq!(names_of(&result, NodeKind::Module), ["fade-in"]);
    assert_eq!(names_of(&result, NodeKind::Use), ["reset.css"]);
}

#[test]
fn css_references_custom_properties_and_nothing_else() {
    let result = CssExtractor.extract("site.css", SHEET);
    let mut referenced: Vec<String> = result
        .unresolved_refs
        .iter()
        .map(|r| r.reference_name.clone())
        .collect();
    referenced.sort();
    referenced.dedup();

    // `var(--brand)` and `var(--spacing)` are followed; `#336699`, `0` and
    // every builtin function are not.
    assert_eq!(referenced, ["--brand", "--spacing"]);
    assert!(
        result
            .unresolved_refs
            .iter()
            .all(|r| r.reference_name.starts_with("--")),
        "only custom properties are safe to resolve by bare name"
    );
}

#[test]
fn both_extractors_claim_their_extensions() {
    assert!(HtmlExtractor.extensions().contains(&"html"));
    assert!(HtmlExtractor.extensions().contains(&"htm"));
    assert!(CssExtractor.extensions().contains(&"css"));
}

#[test]
fn malformed_input_does_not_panic_or_error() {
    let broken_html = "<div id=\"a\"><span class=</div><my-thing>";
    let broken_css = "@import ; .a { color: var(--x) } #{}";
    assert!(HtmlExtractor
        .extract("b.html", broken_html)
        .errors
        .is_empty());
    assert!(CssExtractor.extract("b.css", broken_css).errors.is_empty());
}
