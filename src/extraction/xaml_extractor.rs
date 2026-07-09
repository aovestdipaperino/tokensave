//! XAML / AXAML markup extractor (WPF, Avalonia, MAUI, UWP/WinUI).
//!
//! XAML files were not indexed at all (#167), so views were invisible to
//! search and impact analysis. XAML is strict, machine-generated-adjacent
//! XML, so instead of bundling another tree-sitter grammar (the binary is
//! already dominated by grammar tables) this extractor uses a minimal
//! hand-rolled tag scanner. It emits:
//!
//!   * a `Class` node from the root element's `x:Class` attribute — the same
//!     qualified name as the C# / F# / VB code-behind partial class, so the
//!     view is searchable and relates to its code-behind;
//!   * a `Field` node per `x:Name` / `Name`d element (named controls are the
//!     members the code-behind manipulates);
//!   * `Uses` refs for elements with a non-`x:` namespace prefix
//!     (`local:MyControl`, `controls:TitleBar`) — project-defined controls,
//!     giving views impact edges onto the controls they compose;
//!   * `Calls` refs for event-handler attributes (`Click="OnSave"`), which
//!     resolve to the handler methods in the code-behind.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct XamlExtractor;

/// One parsed start tag: `<prefix:name attr="v" ...>` with its 0-based line.
struct StartTag {
    prefix: Option<String>,
    name: String,
    attrs: Vec<(String, String)>,
    line: u32,
}

impl XamlExtractor {
    /// Extract nodes, edges, and unresolved refs from a XAML source file.
    pub fn extract_xaml(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut unresolved_refs = Vec::new();

        let file_node_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        nodes.push(Self::make_node(
            file_node_id.clone(),
            NodeKind::File,
            file_path.to_string(),
            file_path.to_string(),
            file_path,
            0,
            source.lines().count().saturating_sub(1) as u32,
            None,
            timestamp,
        ));

        let tags = Self::scan_tags(source);

        // The root element's x:Class is the view's identity; everything in
        // the file hangs off it. Without x:Class (resource dictionaries,
        // styles) children parent to the file node.
        let class_id = tags.first().and_then(|root| {
            let class_attr = root
                .attrs
                .iter()
                .find(|(k, _)| k == "x:Class")
                .map(|(_, v)| v.clone())?;
            let short_name = class_attr
                .rsplit('.')
                .next()
                .unwrap_or(&class_attr)
                .to_string();
            let id = generate_node_id(file_path, &NodeKind::Class, &short_name, root.line);
            nodes.push(Self::make_node(
                id.clone(),
                NodeKind::Class,
                short_name,
                class_attr,
                file_path,
                root.line,
                source.lines().count().saturating_sub(1) as u32,
                Some(format!("<{}>", root.name)),
                timestamp,
            ));
            edges.push(Edge {
                source: file_node_id.clone(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(root.line),
            });
            Some(id)
        });
        let parent_id = class_id.as_deref().unwrap_or(&file_node_id);

        let mut seen_uses: std::collections::HashSet<String> = std::collections::HashSet::new();
        for tag in &tags {
            // Project-defined control: any namespace prefix except the XAML
            // language namespaces (x:) and designer/markup-compat noise
            // (d:, mc:). Prefixed elements name CLR types brought in via
            // xmlns:foo="using:..." / "clr-namespace:...".
            if let Some(prefix) = &tag.prefix {
                if prefix != "x"
                    && prefix != "d"
                    && prefix != "mc"
                    && seen_uses.insert(tag.name.clone())
                {
                    unresolved_refs.push(UnresolvedRef {
                        from_node_id: parent_id.to_string(),
                        reference_name: tag.name.clone(),
                        reference_kind: EdgeKind::Uses,
                        line: tag.line,
                        column: 0,
                        file_path: file_path.to_string(),
                    });
                }
            }

            for (attr, value) in &tag.attrs {
                if attr == "x:Name" || attr == "Name" {
                    let id = generate_node_id(file_path, &NodeKind::Field, value, tag.line);
                    nodes.push(Self::make_node(
                        id.clone(),
                        NodeKind::Field,
                        value.clone(),
                        format!(
                            "{}::{}",
                            nodes
                                .get(1)
                                .map_or(file_path, |c| c.qualified_name.as_str()),
                            value
                        ),
                        file_path,
                        tag.line,
                        tag.line,
                        Some(format!("<{} x:Name=\"{}\">", tag.name, value)),
                        timestamp,
                    ));
                    edges.push(Edge {
                        source: parent_id.to_string(),
                        target: id,
                        kind: EdgeKind::Contains,
                        line: Some(tag.line),
                    });
                } else if Self::is_event_attr(attr) && Self::is_identifier(value) {
                    unresolved_refs.push(UnresolvedRef {
                        from_node_id: parent_id.to_string(),
                        reference_name: value.clone(),
                        reference_kind: EdgeKind::Calls,
                        line: tag.line,
                        column: 0,
                        file_path: file_path.to_string(),
                    });
                }
            }
        }

        let mut result = ExtractionResult {
            nodes,
            edges,
            unresolved_refs,
            errors: Vec::new(),
            duration_ms: start.elapsed().as_millis() as u64,
        };
        result.sanitize();
        result
    }

    /// XAML event attributes take a code-behind method name as their value.
    /// There is no schema available at index time, so match the framework
    /// naming convention for routed/input events; false positives are cheap
    /// because a Calls ref only becomes an edge if a matching method exists.
    fn is_event_attr(attr: &str) -> bool {
        const EVENT_SUFFIXES: [&str; 12] = [
            "Click",
            "Tapped",
            "Pressed",
            "Released",
            "Changed",
            "Opened",
            "Closed",
            "Closing",
            "Loaded",
            "Unloaded",
            "Focus",
            "Completed",
        ];
        const EVENTS: [&str; 8] = [
            "KeyDown",
            "KeyUp",
            "Drop",
            "DragOver",
            "DragEnter",
            "DragLeave",
            "Activated",
            "Deactivated",
        ];
        // Attached properties (Grid.Row) and namespaced attrs are never events.
        if attr.contains('.') || attr.contains(':') {
            return false;
        }
        EVENT_SUFFIXES.iter().any(|s| attr.ends_with(s)) || EVENTS.contains(&attr)
    }

    /// A bare method name: `OnSaveClicked`. Bindings (`{Binding Save}`),
    /// paths, and prose all contain non-identifier characters.
    fn is_identifier(value: &str) -> bool {
        !value.is_empty()
            && value
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// Minimal XML start-tag scanner. Handles comments, processing
    /// instructions, CDATA, and single/double-quoted attribute values;
    /// ignores end tags and text content. Line numbers are 0-based.
    fn scan_tags(source: &str) -> Vec<StartTag> {
        let bytes = source.as_bytes();
        let mut tags = Vec::new();
        let mut i = 0;
        let mut line: u32 = 0;

        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    line += 1;
                    i += 1;
                }
                b'<' => {
                    if bytes[i..].starts_with(b"<!--") {
                        i = Self::skip_until(bytes, i + 4, b"-->", &mut line);
                    } else if bytes[i..].starts_with(b"<![CDATA[") {
                        i = Self::skip_until(bytes, i + 9, b"]]>", &mut line);
                    } else if bytes[i..].starts_with(b"<?")
                        || bytes[i..].starts_with(b"<!")
                        || bytes[i..].starts_with(b"</")
                    {
                        // Processing instruction, DOCTYPE, or end tag.
                        i = Self::skip_until(bytes, i + 2, b">", &mut line);
                    } else if let Some((tag, next)) = Self::parse_start_tag(source, i, &mut line) {
                        tags.push(tag);
                        i = next;
                    } else {
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
        tags
    }

    /// Advance past the next occurrence of `needle`, counting newlines.
    fn skip_until(bytes: &[u8], mut i: usize, needle: &[u8], line: &mut u32) -> usize {
        while i < bytes.len() && !bytes[i..].starts_with(needle) {
            if bytes[i] == b'\n' {
                *line += 1;
            }
            i += 1;
        }
        (i + needle.len()).min(bytes.len())
    }

    /// Parse `<prefix:Name attr="v" ...>` starting at the `<` at byte `pos`.
    /// Returns the tag and the byte index just past the closing `>`.
    fn parse_start_tag(source: &str, pos: usize, line: &mut u32) -> Option<(StartTag, usize)> {
        let bytes = source.as_bytes();
        let tag_line = *line;
        let mut i = pos + 1;

        // Tag name: [prefix:]Name
        let name_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b':')
        {
            i += 1;
        }
        if i == name_start {
            return None;
        }
        let full_name = &source[name_start..i];
        let (prefix, name) = match full_name.split_once(':') {
            Some((p, n)) => (Some(p.to_string()), n.to_string()),
            None => (None, full_name.to_string()),
        };

        // Attributes until `>` or `/>`.
        let mut attrs = Vec::new();
        while i < bytes.len() {
            match bytes[i] {
                b'>' => {
                    i += 1;
                    break;
                }
                b'\n' => {
                    *line += 1;
                    i += 1;
                }
                b'/' | b' ' | b'\t' | b'\r' => i += 1,
                _ => {
                    // attr name
                    let attr_start = i;
                    while i < bytes.len()
                        && (bytes[i].is_ascii_alphanumeric()
                            || bytes[i] == b'_'
                            || bytes[i] == b':'
                            || bytes[i] == b'.')
                    {
                        i += 1;
                    }
                    if i == attr_start {
                        i += 1;
                        continue;
                    }
                    let attr_name = source[attr_start..i].to_string();
                    // skip whitespace and `=`
                    while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
                        if bytes[i] == b'\n' {
                            *line += 1;
                        }
                        i += 1;
                    }
                    if i >= bytes.len() || bytes[i] != b'=' {
                        continue; // valueless attribute — not valid XML, skip
                    }
                    i += 1;
                    while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
                        if bytes[i] == b'\n' {
                            *line += 1;
                        }
                        i += 1;
                    }
                    let quote = *bytes.get(i)?;
                    if quote != b'"' && quote != b'\'' {
                        continue;
                    }
                    i += 1;
                    let val_start = i;
                    while i < bytes.len() && bytes[i] != quote {
                        if bytes[i] == b'\n' {
                            *line += 1;
                        }
                        i += 1;
                    }
                    let value = source.get(val_start..i)?.to_string();
                    i += 1; // closing quote
                    attrs.push((attr_name, value));
                }
            }
        }

        Some((
            StartTag {
                prefix,
                name,
                attrs,
                line: tag_line,
            },
            i,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn make_node(
        id: String,
        kind: NodeKind,
        name: String,
        qualified_name: String,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        signature: Option<String>,
        timestamp: u64,
    ) -> Node {
        Node {
            id,
            kind,
            name,
            qualified_name,
            file_path: file_path.to_string(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column: 0,
            end_column: 0,
            signature,
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: timestamp,
            parent_id: None,
        }
    }
}

impl crate::extraction::LanguageExtractor for XamlExtractor {
    fn extensions(&self) -> &[&str] {
        &["xaml", "axaml"]
    }

    fn language_name(&self) -> &'static str {
        "XAML"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_xaml(file_path, source)
    }
}
