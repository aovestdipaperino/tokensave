//! Tiny XML element extractor used by `pom.xml` and `*.csproj` parsers.
//!
//! This is deliberately not a real XML parser — we avoid adding `quick-xml`
//! as a dep for what amounts to "find the next `<tag>` and capture its text
//! up to the matching `</tag>`". Comments and CDATA are ignored. Mixed
//! content and namespaces are tolerated but not interpreted.

/// Iterate child elements matching `tag` inside `parent`, returning each
/// child's inner content (the slice between `<tag>` / `<tag attrs>` and
/// `</tag>`). Empty self-closing tags (`<tag />`) yield an empty string.
pub fn find_elements<'a>(parent: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = parent;
    let open_prefix = format!("<{tag}");
    let close = format!("</{tag}>");

    while let Some(idx) = rest.find(&open_prefix) {
        let after_prefix = &rest[idx + open_prefix.len()..];

        // Distinguish `<tag>`, `<tag attrs>`, `<tag/>`, `<tag attrs/>` from
        // unrelated tags like `<tagName>` (prefix collision).
        let first_char = after_prefix.chars().next();
        let next_is_boundary = matches!(first_char, Some('>' | ' ' | '\t' | '\r' | '\n' | '/'));
        if !next_is_boundary {
            // False positive — skip past this open-prefix and keep scanning.
            rest = &rest[idx + open_prefix.len()..];
            continue;
        }

        // Find the matching `>` to close the open tag.
        let Some(open_close) = after_prefix.find('>') else {
            break;
        };
        // Self-closing: `<tag ... />`
        let self_closing = after_prefix[..open_close].trim_end().ends_with('/');
        let content_start = idx + open_prefix.len() + open_close + 1;

        if self_closing {
            out.push("");
            rest = &rest[content_start..];
            continue;
        }

        let after_open = &rest[content_start..];
        let Some(end_idx) = after_open.find(&close) else {
            break;
        };
        out.push(&after_open[..end_idx]);
        rest = &after_open[end_idx + close.len()..];
    }
    out
}

/// Return the first element value or `None`. Handy when a parent has at most
/// one of a given child (e.g. `<version>` inside a `<dependency>`).
pub fn first_text<'a>(parent: &'a str, tag: &str) -> Option<&'a str> {
    find_elements(parent, tag).into_iter().next()
}

/// Extract a named attribute value from a self-contained open tag.
/// Given `<PackageReference Include="Foo" Version="1.0" />`, calling with
/// attr=`"Include"` returns `Some("Foo")`.
pub fn attr_value<'a>(element: &'a str, attr: &str) -> Option<&'a str> {
    let needle = format!("{attr}=\"");
    let start = element.find(&needle)? + needle.len();
    let rest = &element[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Find self-closing or open elements with the given tag, returning the raw
/// element text (including attributes) so callers can pull attributes out.
pub fn find_element_tags<'a>(parent: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = parent;
    let open_prefix = format!("<{tag}");

    while let Some(idx) = rest.find(&open_prefix) {
        let after_prefix = &rest[idx + open_prefix.len()..];
        let first_char = after_prefix.chars().next();
        if !matches!(first_char, Some('>' | ' ' | '\t' | '\r' | '\n' | '/')) {
            rest = &rest[idx + open_prefix.len()..];
            continue;
        }
        let Some(open_close) = after_prefix.find('>') else {
            break;
        };
        let end_of_open = idx + open_prefix.len() + open_close + 1;
        out.push(&rest[idx..end_of_open]);
        rest = &rest[end_of_open..];
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_elements() {
        let xml = r#"
<project>
  <dependencies>
    <dependency>
      <groupId>com.foo</groupId>
      <artifactId>bar</artifactId>
      <version>1.2.3</version>
    </dependency>
    <dependency>
      <groupId>com.baz</groupId>
      <artifactId>qux</artifactId>
      <version>4.5</version>
      <scope>test</scope>
    </dependency>
  </dependencies>
</project>"#;
        let deps = find_elements(xml, "dependency");
        assert_eq!(deps.len(), 2);
        assert_eq!(first_text(deps[0], "groupId"), Some("com.foo"));
        assert_eq!(first_text(deps[1], "scope"), Some("test"));
    }

    #[test]
    fn handles_self_closing_tag() {
        let xml = r#"<root><PackageReference Include="Foo" Version="1.0" /></root>"#;
        let refs = find_element_tags(xml, "PackageReference");
        assert_eq!(refs.len(), 1);
        assert_eq!(attr_value(refs[0], "Include"), Some("Foo"));
        assert_eq!(attr_value(refs[0], "Version"), Some("1.0"));
    }

    #[test]
    fn ignores_prefix_collisions() {
        // `<groupIdRef>` must not be matched when looking for `<groupId>`.
        let xml = r#"<root><groupIdRef>x</groupIdRef><groupId>actual</groupId></root>"#;
        let found = find_elements(xml, "groupId");
        assert_eq!(found, vec!["actual"]);
    }
}
