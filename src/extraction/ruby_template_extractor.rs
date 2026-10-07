//! Ruby projections of ERB and Slim templates. Only copied source contributes graph locations.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use tree_sitter::Node as TsNode;

use super::ruby_extractor::RubyExtractor;
use super::ts_state::ExtractionState;
use super::LanguageExtractor;
use crate::types::{generate_node_id, EdgeKind, ExtractionResult, NodeKind, UnresolvedRef};

pub struct RubyTemplateExtractor;

impl LanguageExtractor for RubyTemplateExtractor {
    fn extensions(&self) -> &[&str] {
        &["erb", "slim"]
    }

    fn language_name(&self) -> &'static str {
        "Ruby"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        let mut projection = Projection::new(source);
        if file_path
            .rsplit('.')
            .next()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("slim"))
        {
            project_slim(&mut projection);
        } else {
            project_erb(&mut projection);
        }
        let mut result = RubyExtractor::extract_template(file_path, &projection.ruby);
        projection.remap(&mut result);
        result
    }
}

struct Span {
    generated: Range<usize>,
    original: usize,
}

struct Projection<'a> {
    source: &'a str,
    ruby: String,
    spans: Vec<Span>,
}

impl<'a> Projection<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            ruby: String::new(),
            spans: Vec::new(),
        }
    }

    fn copy(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let start = self.ruby.len();
        self.ruby.push_str(&self.source[range.clone()]);
        self.spans.push(Span {
            generated: start..self.ruby.len(),
            original: range.start,
        });
    }

    fn expression(&mut self, range: Range<usize>) {
        self.copy(range);
        self.ruby.push('\n');
    }

    fn remap(&self, result: &mut ExtractionResult) {
        let generated_lines = line_offsets(&self.ruby);
        let original_lines = line_offsets(self.source);
        let position = |line: u32, column: u32| {
            let offset = generated_lines
                .get(line as usize)
                .copied()
                .unwrap_or(self.ruby.len())
                + column as usize;
            let index = self
                .spans
                .partition_point(|span| span.generated.start <= offset)
                .saturating_sub(1);
            let original = self.spans.get(index).map_or(0, |span| {
                span.original
                    + offset
                        .saturating_sub(span.generated.start)
                        .min(span.generated.len())
            });
            let row = original_lines
                .partition_point(|&start| start <= original)
                .saturating_sub(1);
            (row as u32, (original - original_lines[row]) as u32)
        };
        let mut ids = HashMap::new();
        for node in &mut result.nodes {
            if node.kind == NodeKind::File {
                node.end_line = self.source.lines().count().saturating_sub(1) as u32;
                // end_column stays 0, as on every RubyExtractor file node.
                continue;
            }
            (node.attrs_start_line, _) = position(node.attrs_start_line, 0);
            (node.start_line, node.start_column) = position(node.start_line, node.start_column);
            (node.end_line, node.end_column) = position(node.end_line, node.end_column);
            // Same key RubyExtractor hashes (short name, not qualified name),
            // so a template node's id follows the convention of every other
            // Ruby node.
            let id = generate_node_id(&node.file_path, &node.kind, &node.name, node.start_line);
            ids.insert(node.id.clone(), id.clone());
            node.id = id;
        }
        for node in &mut result.nodes {
            if let Some(parent) = node.parent_id.as_mut() {
                if let Some(id) = ids.get(parent) {
                    parent.clone_from(id);
                }
            }
        }
        for reference in &mut result.unresolved_refs {
            (reference.line, reference.column) = position(reference.line, reference.column);
            if let Some(id) = ids.get(&reference.from_node_id) {
                reference.from_node_id.clone_from(id);
            }
        }
        for edge in &mut result.edges {
            edge.line = edge.line.map(|line| position(line, 0).0);
            if let Some(id) = ids.get(&edge.source) {
                edge.source.clone_from(id);
            }
            if let Some(id) = ids.get(&edge.target) {
                edge.target.clone_from(id);
            }
        }
    }
}

fn line_offsets(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(i, byte)| (byte == b'\n').then_some(i + 1)),
        )
        .collect()
}

fn project_erb(projection: &mut Projection<'_>) {
    let source = projection.source;
    let mut cursor = 0;
    // ERB 6.0.7 scans delimiters even inside Ruby strings. `<%%` is literal text, `<%#` is a comment, and trim markers are not part of the Ruby expression.
    while let Some(relative) = source[cursor..].find("<%") {
        let open = cursor + relative;
        let mut start = open + 2;
        if source.as_bytes().get(start) == Some(&b'%') {
            cursor = start + 1;
            continue;
        }
        let mut scan = start;
        let mut escapes = Vec::new();
        let close = loop {
            let Some(close) = source[scan..].find("%>").map(|i| scan + i) else {
                break None;
            };
            if close > scan && source.as_bytes()[close - 1] == b'%' {
                escapes.push(close - 1);
                scan = close + 2;
            } else {
                break Some(close);
            }
        };
        let Some(close) = close else {
            break;
        };
        if source.as_bytes().get(start) != Some(&b'#') {
            if matches!(source.as_bytes().get(start), Some(b'=' | b'-')) {
                start += 1;
                if source.as_bytes().get(start) == Some(&b'=') {
                    start += 1;
                }
            }
            let end = if close > start && source.as_bytes()[close - 1] == b'-' {
                close - 1
            } else {
                close
            };
            for escape in escapes {
                projection.copy(start..escape);
                start = escape + 1;
            }
            projection.expression(start..end);
        }
        cursor = close + 2;
    }
}

#[derive(Clone, Copy)]
enum TextBody {
    Ignore,
    Interpolate,
    Ruby,
}

fn project_slim(projection: &mut Projection<'_>) {
    let source = projection.source;
    let mut blocks: Vec<usize> = Vec::new();
    let mut text_body: Option<(usize, TextBody)> = None;
    let mut continuation = false;
    let mut continuation_indent = 0;
    let mut consumed_until = 0;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        let trimmed = content.trim_start();
        let start = offset + content.len() - trimmed.len();
        let end = offset + content.len();
        offset += line.len();
        if trimmed.is_empty() || start < consumed_until {
            continue;
        }
        let indent = content[..content.len() - trimmed.len()]
            .chars()
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum::<usize>();
        if continuation {
            continuation = copy_slim_code(projection, start..end);
            if !continuation && opens_block(trimmed) && blocks.last() != Some(&continuation_indent)
            {
                blocks.push(continuation_indent);
            }
            continue;
        }
        if let Some((parent_indent, body)) = text_body {
            if indent > parent_indent {
                match body {
                    TextBody::Ignore => {}
                    TextBody::Interpolate => interpolation(projection, start..end),
                    TextBody::Ruby => projection.expression(start..end),
                }
                continue;
            }
            text_body = None;
        }
        let control = trimmed.strip_prefix('-').map(str::trim_start);
        let branch = control.is_some_and(|code| {
            matches!(
                first_word(code),
                "else" | "elsif" | "when" | "in" | "rescue" | "ensure"
            )
        });
        if trimmed.starts_with('/') && !trimmed.starts_with("/!") && !trimmed.starts_with("/[") {
            text_body = Some((indent, TextBody::Ignore));
            continue;
        }
        while blocks
            .last()
            .is_some_and(|&depth| depth > indent || (depth == indent && !branch))
        {
            blocks.pop();
            projection.ruby.push_str("end\n");
        }
        if trimmed == "ruby:" {
            text_body = Some((indent, TextBody::Ruby));
        } else if trimmed.ends_with(':')
            && trimmed[..trimmed.len() - 1]
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            text_body = Some((indent, TextBody::Interpolate));
        } else if trimmed.starts_with('<') {
            // Slim 5.2.2 gives inline HTML an ordinary Slim child block, unlike verbatim text lines.
            interpolation(projection, start..end);
        } else if trimmed.starts_with(['|', '\'']) || trimmed.starts_with("/!") {
            interpolation(projection, start..end);
            text_body = Some((indent, TextBody::Interpolate));
        } else if let Some(code) = control {
            let code_start = end - code.len();
            continuation = copy_slim_code(projection, code_start..end);
            if !branch && opens_block(code) {
                blocks.push(indent);
            }
        } else if trimmed.starts_with('=') {
            let code = trimmed.trim_start_matches(['=', '<', '>']);
            continuation = copy_slim_code(projection, end - code.len()..end);
            if opens_block(code.trim()) {
                blocks.push(indent);
            }
        } else if !trimmed.starts_with("doctype ") {
            let (code, text, consumed) = slim_tag(projection, start..end);
            consumed_until = consumed;
            if let Some(range) = code {
                continuation = copy_slim_code(projection, range.clone());
                if opens_block(source[range].trim()) {
                    blocks.push(indent);
                }
            } else if text {
                text_body = Some((indent, TextBody::Interpolate));
            }
        }
        if continuation {
            continuation_indent = indent;
        }
    }
    for _ in blocks {
        projection.ruby.push_str("end\n");
    }
}

fn first_word(code: &str) -> &str {
    code.split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
}

fn opens_block(code: &str) -> bool {
    // Match Slim 5.2.2's `\bdo\s*(\|[^|]*\|)?\s*$` suffix without searching inside parameter names.
    let tail = code.trim_end();
    let before_parameters = tail
        .strip_suffix('|')
        .and_then(|prefix| prefix.rsplit_once('|'))
        .map_or(tail, |(prefix, _)| prefix.trim_end());
    let ends_with_do = before_parameters.strip_suffix("do").is_some_and(|prefix| {
        prefix
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
    });
    matches!(
        first_word(code),
        "if" | "unless" | "case" | "begin" | "while" | "until" | "for"
    ) || ends_with_do
}

fn copy_slim_code(projection: &mut Projection<'_>, range: Range<usize>) -> bool {
    let end = range.start + projection.source[range.clone()].trim_end().len();
    let last = projection.source.as_bytes().get(end.saturating_sub(1));
    let continued = matches!(last, Some(b',' | b'\\'));
    // Slim's parse_broken_line retains the backslash; Ruby needs it to join the next line into this expression.
    projection.expression(range.start..end);
    continued
}

/// Copy interpolation bodies, balancing Ruby braces and quoted strings instead of treating the first `}` in a hash or string as the end of the interpolation.
fn interpolation(projection: &mut Projection<'_>, range: Range<usize>) {
    let bytes = projection.source.as_bytes();
    let mut cursor = range.start;
    while cursor + 1 < range.end {
        if bytes[cursor] == b'\\' {
            cursor += 2;
            continue;
        }
        if &bytes[cursor..cursor + 2] == b"#{" {
            let end = balanced_end(bytes, cursor + 1, range.end);
            if end <= range.end && bytes.get(end.saturating_sub(1)) == Some(&b'}') {
                let mut body = cursor + 2..end - 1;
                // Slim 5.2.2 uses an extra brace pair for unescaped output: `#{{helper()}}` evaluates `helper()`, not a Ruby hash/block.
                if projection.source[body.clone()].starts_with('{')
                    && projection.source[body.clone()].ends_with('}')
                {
                    body = body.start + 1..body.end - 1;
                }
                projection.expression(body);
            }
            cursor = end;
        } else {
            cursor += 1;
        }
    }
}

fn balanced_end(bytes: &[u8], start: usize, limit: usize) -> usize {
    find_balanced_end(bytes, start, limit).unwrap_or(limit)
}

/// The offset just past the delimiter that closes the one at `start`, or
/// `None` when nothing before `limit` closes it.
fn find_balanced_end(bytes: &[u8], start: usize, limit: usize) -> Option<usize> {
    let mut stack = vec![match bytes[start] {
        b'(' => b')',
        b'[' => b']',
        _ => b'}',
    }];
    let mut quote = None;
    let mut i = start + 1;
    while i < limit {
        let byte = bytes[i];
        if byte == b'\\' {
            i = (i + 2).min(limit);
            continue;
        }
        if let Some(q) = quote {
            if byte == q {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' | b'`' => quote = Some(byte),
                b'(' => stack.push(b')'),
                b'[' => stack.push(b']'),
                b'{' => stack.push(b'}'),
                _ if stack.last() == Some(&byte) => {
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i + 1);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

fn skip_space(bytes: &[u8], mut i: usize, end: usize) -> usize {
    while i < end && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn slim_tag(
    projection: &mut Projection<'_>,
    range: Range<usize>,
) -> (Option<Range<usize>>, bool, usize) {
    let bytes = projection.source.as_bytes();
    let mut end = range.end;
    let mut i = range.start;
    // Tag names and .class/#id shortcuts contain no evaluated Ruby.
    while i < end
        && !bytes[i].is_ascii_whitespace()
        && !matches!(bytes[i], b'=' | b'(' | b'[' | b'{' | b'<' | b'>' | b'\'')
    {
        if bytes[i] == b':' && (i + 1 == end || bytes[i + 1].is_ascii_whitespace()) {
            break;
        }
        i += 1;
    }
    while i < end && matches!(bytes[i], b'<' | b'>' | b'\'') {
        i += 1;
    }
    i = skip_space(bytes, i, end);
    let outer = if i < end {
        match bytes[i] {
            b'(' => Some(b')'),
            b'[' => Some(b']'),
            b'{' => Some(b'}'),
            _ => None,
        }
    } else {
        None
    };
    if outer.is_some() {
        // A wrapper may span lines, but an unclosed one must not swallow the
        // rest of the template: it stays on its own line.
        let close = find_balanced_end(bytes, i, bytes.len()).unwrap_or(end);
        if close > end {
            end = projection.source[close..]
                .find('\n')
                .map_or(bytes.len(), |next| close + next);
        }
        i += 1;
    }
    loop {
        i = skip_space(bytes, i, end);
        if i >= end {
            return (None, false, end);
        }
        if Some(bytes[i]) == outer {
            i += 1;
            break;
        }
        if bytes[i] == b'=' {
            break;
        }
        if bytes[i] == b':' {
            return slim_tag(projection, skip_space(bytes, i + 1, end)..end);
        }
        let attribute_start = i;
        let splat = bytes[i] == b'*';
        if splat {
            i += 1;
        } else {
            while i < end
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'='
                && Some(bytes[i]) != outer
            {
                i += 1;
            }
            i = skip_space(bytes, i, end);
            if i >= end || bytes[i] != b'=' {
                if outer.is_some() && i > attribute_start {
                    continue;
                }
                interpolation(projection, attribute_start..end);
                return (None, true, end);
            }
            i += 1;
            if i < end && bytes[i] == b'=' {
                i += 1;
            }
            i = skip_space(bytes, i, end);
        }
        let value_start = i;
        if i < end && matches!(bytes[i], b'\'' | b'"') {
            let quote = bytes[i];
            i += 1;
            while i < end && bytes[i] != quote {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(end);
                } else if i + 1 < end && &bytes[i..i + 2] == b"#{" {
                    i = balanced_end(bytes, i + 1, end);
                } else {
                    i += 1;
                }
            }
            interpolation(projection, value_start + 1..i);
            i = (i + 1).min(end);
        } else {
            while i < end && !bytes[i].is_ascii_whitespace() && Some(bytes[i]) != outer {
                if matches!(bytes[i], b'(' | b'[' | b'{') {
                    i = balanced_end(bytes, i, end);
                } else {
                    i += 1;
                }
            }
            projection.expression(value_start..i);
        }
    }
    i = skip_space(bytes, i, end);
    if i < end && bytes[i] == b'=' {
        while i < end && matches!(bytes[i], b'=' | b'<' | b'>' | b'\'') {
            i += 1;
        }
        (Some(i..end), false, end)
    } else {
        interpolation(projection, i..end);
        (None, i < end, end)
    }
}

pub(super) fn extract_bare_calls(state: &mut ExtractionState, root: TsNode<'_>, owner: &str) {
    // Bindings are suppressed across the projection: incomplete template scope information should omit an uncertain helper call rather than invent one.
    let mut locals = HashSet::new();
    let mut identifiers = Vec::new();
    collect_identifiers(state, root, false, &mut locals, &mut identifiers);
    for identifier in identifiers {
        let name = state.node_text(identifier);
        if !locals.contains(&name) {
            let position = (
                identifier.start_position().row as u32,
                identifier.start_position().column as u32,
            );
            let call_owner = state
                .nodes
                .iter()
                .rev()
                .find(|node| {
                    matches!(
                        node.kind,
                        NodeKind::Function | NodeKind::Method | NodeKind::SingletonMethod
                    ) && (node.start_line, node.start_column) <= position
                        && position < (node.end_line, node.end_column)
                })
                .map_or(owner, |node| node.id.as_str());
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: call_owner.to_string(),
                reference_name: name,
                reference_kind: EdgeKind::Calls,
                line: identifier.start_position().row as u32,
                column: identifier.start_position().column as u32,
                file_path: state.file_path.clone(),
            });
        }
    }
}

fn collect_identifiers<'a>(
    state: &ExtractionState,
    node: TsNode<'a>,
    binding: bool,
    locals: &mut HashSet<String>,
    identifiers: &mut Vec<TsNode<'a>>,
) {
    // Ruby 3.4.7 evaluates pins and string interpolation within patterns; tree-sitter-ruby exposes these separately from binding identifiers.
    let binding = binding
        && !matches!(
            node.kind(),
            "expression_reference_pattern" | "interpolation"
        );
    if matches!(
        node.kind(),
        "method" | "singleton_method" | "class" | "module" | "singleton_class" | "alias" | "undef"
    ) {
        return;
    }
    if node.kind() == "identifier" {
        if binding {
            locals.insert(state.node_text(node));
        } else {
            identifiers.push(node);
        }
        return;
    }
    if binding && node.kind() == "keyword_pattern" && node.child_by_field_name("value").is_none() {
        if let Some(key) = node.child_by_field_name("key") {
            if key.kind() == "hash_key_symbol" {
                locals.insert(state.node_text(key));
            } else if key.kind() == "string" && key.named_child_count() == 1 {
                if let Some(content) = key
                    .named_child(0)
                    .filter(|child| child.kind() == "string_content")
                {
                    locals.insert(state.node_text(content));
                }
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if node.kind() == "call" && node.child_by_field_name("method") == Some(child) {
            continue;
        }
        let is_binding = binding
            || node.kind().ends_with("parameters")
            || (matches!(node.kind(), "assignment" | "operator_assignment")
                && node.child_by_field_name("left") == Some(child))
            || (node.kind() == "for" && node.child_by_field_name("pattern") == Some(child))
            || (matches!(node.kind(), "in_clause" | "match_pattern" | "test_pattern")
                && node.child_by_field_name("pattern") == Some(child))
            || (node.kind() == "rescue" && node.child_by_field_name("variable") == Some(child));
        // Receivers (`current_user.name`) are emitted too: the resolver binds
        // a template's bare names to helper-shaped targets only, which keeps
        // an unbound partial local like `item` off unrelated `def item`s.
        collect_identifiers(state, child, is_binding, locals, identifiers);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn assert_slim_parses(source: &str) {
        let mut projection = Projection::new(source);
        project_slim(&mut projection);
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::super::ts_provider::language("ruby"))
            .unwrap();
        let tree = parser.parse(&projection.ruby, None).unwrap();
        assert!(
            !tree.root_node().has_error(),
            "{}\n{}",
            projection.ruby,
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn slim_do_block_parameter_names_do_not_hide_the_keyword() {
        for parameters in ["doc", "todo", "download", "doc, todo", "document; todo"] {
            for prefix in ["-", "=", "p ="] {
                assert_slim_parses(&format!(
                    "{prefix} items.each do |{parameters}|\n  = label()\n= footer()\n"
                ));
            }
        }
        assert_slim_parses("- items.each do|doc|\n  = label(doc)\n= footer()\n");
        for identifier in ["todo", "undo", "download", "shadow"] {
            assert!(!opens_block(identifier));
        }
    }

    #[test]
    fn slim_broken_lines_preserve_ruby_continuations() {
        for source in [
            "= link_to \\\n  \"x\", path\n= footer()\n",
            "p = link_to \\\n  \"x\", path\n= footer()\n",
            "- total = price \\\n  * qty\n= total\n",
            "= combine \\\n  first(), \\\n  second()\n",
        ] {
            assert_slim_parses(source);
        }
    }

    #[test]
    fn slim_projections_parse_without_recovery() {
        let samples = [
            include_str!("../../tests/fixtures/sample.html.slim"),
            include_str!("../../tests/fixtures/sample_layout.html.slim"),
            "p #{{raw_label()}}\np == raw_body()\n== raw_footer()\n",
            "- if ready?\n  = first()\n/ comment\n  ignored\n- else\n  = second()\n= after()\n",
            "- case kind()\n- when :first\n  = first()\n- else\n  = second()\n",
            "- begin\n  = risky()\n- rescue StandardError => error\n  = message(error)\n- ensure\n  = cleanup()\n",
            "- while ready?\n  = tick()\n= after()\n",
            "= form_for(record(),\n    options()) do |form|\n  = form.field(:name)\n= after()\n",
            "div(\n  title=tooltip()\n  class=(classes(flag()))\n)\n  p: span = nested_label()\n",
        ];
        for source in samples {
            assert_slim_parses(source);
        }
    }
}
