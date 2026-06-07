//! Hand-written F* (`.fst` / `.fsti`) source extractor.
//!
//! F* has no maintained, complete tree-sitter grammar, and the F# grammar does
//! not parse F*-specific syntax (refinement types, effects, `Lemma`, `val`
//! declarations, typeclasses, `calc`, `eliminate`/`introduce`, the `#push-options`
//! family of pragmas, …). Rather than bolt an incomplete grammar onto the build,
//! this extractor parses F* directly with a light, declaration-level scanner.
//!
//! A top-level declaration starts in column 0. Bodies, however, are *not*
//! required to be indented — `let f x =\nif b then x else y` and even a
//! column-0 local `let y = e in …` are both legal F*. So we cannot treat
//! "column 0" alone as "new declaration". Instead the scanner tracks, across
//! lines, the bracket depth (`()`/`[]`/`{}`/`begin`…`end`), the local
//! `let`…`in` balance, and whether the previous line ended on a continuation
//! token (an operator char, an open bracket, `=`, `->`, `in`, `then`, `with`,
//! …). A column-0 declaration keyword only opens a *new* declaration when none
//! of those say "the previous declaration's term is still open" — so a
//! column-0 `let y = e in` inside a body is correctly read as a local binding,
//! not a top-level `let`. Term-level constructs (`calc`, `eliminate`,
//! `introduce`, match arms) are likewise absorbed into the enclosing body.
//!
//! The set of top-level declaration keywords and qualifiers mirrors the F* menhir
//! grammar (`FStarC.Parser.Parse.mly`) and lexer (`FStarC.Parser.LexFStar.ml`):
//!
//! * `module M.N`              → [`NodeKind::Module`] (becomes the file's scope)
//! * `module A = M.N` / `open` / `include` / `friend` → `Uses` edge
//! * `let` / `and` / `val`     → [`NodeKind::Function`]
//! * `type t = { … }`          → [`NodeKind::Struct`] + [`NodeKind::Field`]
//! * `type t = | A | B`        → [`NodeKind::Enum`] + [`NodeKind::EnumVariant`]
//! * `type t = …`              → [`NodeKind::TypeAlias`]
//! * `class c = { … }`         → [`NodeKind::Trait`] + [`NodeKind::Field`] (methods)
//! * `instance i : c = { … }`  → [`NodeKind::Impl`]
//! * `exception E`             → [`NodeKind::Struct`]
//! * `assume X : phi`          → [`NodeKind::Const`]
//! * `effect` / `new_effect` / `layered_effect` → [`NodeKind::TypeAlias`]
//! * `#set-options` / `#push-options` / `#pop-options` / … → ignored (no node)

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct FStarExtractor;

/// F* operator characters (`FStarC.Parser.LexFStar.op_char`).
fn is_op_char(c: char) -> bool {
    matches!(
        c,
        '!' | '$'
            | '%'
            | '&'
            | '*'
            | '+'
            | '-'
            | '.'
            | '<'
            | '>'
            | '='
            | '?'
            | '^'
            | '|'
            | '~'
            | ':'
            | '@'
            | '#'
            | '\\'
            | '/'
    )
}

/// Characters that may appear in an F* identifier (after the first).
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '\''
}

/// Returns the leading identifier word of `s` and the remainder.
fn next_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    let end = s.find(|c: char| !is_ident_char(c)).unwrap_or(s.len());
    (&s[..end], &s[end..])
}

/// Leading dotted module path (e.g. `FStar.List.Tot`).
fn leading_dotted(s: &str) -> String {
    let s = s.trim_start();
    let end = s
        .find(|c: char| !(is_ident_char(c) || c == '.'))
        .unwrap_or(s.len());
    s[..end].to_string()
}

/// Qualifiers that may precede a declaration keyword
/// (`FStarC.Parser.Parse.qualifier`). `assume` is handled separately because it
/// is also a standalone declaration.
fn is_qualifier(w: &str) -> bool {
    matches!(
        w,
        "inline_for_extraction"
            | "unfold"
            | "inline"
            | "irreducible"
            | "noextract"
            | "total"
            | "private"
            | "noeq"
            | "unopteq"
            | "new"
            | "logic"
            | "opaque"
            | "reifiable"
            | "reflectable"
            | "abstract"
            | "unfoldable"
    )
}

/// Top-level declaration keywords (`FStarC.Parser.Parse.rawDecl` / `decl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKw {
    Module,
    Open,
    Include,
    Friend,
    Type,
    Let,
    And,
    Val,
    Class,
    Instance,
    Effect,
    Exception,
    Assume,
}

fn decl_keyword(w: &str) -> Option<DeclKw> {
    Some(match w {
        "module" => DeclKw::Module,
        "open" => DeclKw::Open,
        "include" => DeclKw::Include,
        "friend" => DeclKw::Friend,
        "type" => DeclKw::Type,
        "let" => DeclKw::Let,
        "and" => DeclKw::And,
        "val" => DeclKw::Val,
        "class" => DeclKw::Class,
        "instance" => DeclKw::Instance,
        "effect" | "new_effect" | "layered_effect" => DeclKw::Effect,
        "exception" => DeclKw::Exception,
        "assume" => DeclKw::Assume,
        _ => return None,
    })
}

/// Result of classifying a column-0 source line.
enum Head {
    /// A declaration: keyword plus the text following it on the start line.
    Decl(DeclKw, String),
    /// Only attributes / qualifiers — a prefix to the following declaration.
    AttrOnly,
    /// Anything else (pragma, closing brace, doc comment, continuation, …).
    NotDecl,
}

/// Classifies a (column-0) line, stripping leading attribute groups (`[@@ … ]`)
/// and qualifiers to find the declaration keyword.
fn classify_head(line: &str) -> Head {
    let mut s = line.trim_start();
    let mut saw_prefix = false;
    loop {
        if s.is_empty() {
            return if saw_prefix {
                Head::AttrOnly
            } else {
                Head::NotDecl
            };
        }
        // Attribute group: `[@@ ... ]` or `[@ ... ]` (single-line span only;
        // multi-line attributes continue on indented lines, which never reach
        // this column-0 classifier).
        if s.starts_with("[@") {
            saw_prefix = true;
            match s.find(']') {
                Some(i) => {
                    s = s[i + 1..].trim_start();
                    continue;
                }
                None => return Head::AttrOnly,
            }
        }
        let (w, rest) = next_word(s);
        if w.is_empty() {
            // Starts with a non-identifier, non-attribute char (`#`, `}`, `|`,
            // `(`, …): not a declaration head.
            return Head::NotDecl;
        }
        if w == "assume" {
            // `assume val`/`assume new`/… → `assume` is a qualifier; otherwise
            // `assume Name : phi` is itself a declaration.
            let nrest = rest.trim_start();
            let (w2, _) = next_word(nrest);
            if w2 != "assume" && decl_keyword(w2).is_some() {
                saw_prefix = true;
                s = nrest;
                continue;
            }
            return Head::Decl(DeclKw::Assume, nrest.to_string());
        }
        if let Some(kw) = decl_keyword(w) {
            return Head::Decl(kw, rest.trim_start().to_string());
        }
        if is_qualifier(w) {
            saw_prefix = true;
            s = rest.trim_start();
            continue;
        }
        return Head::NotDecl;
    }
}

/// Extracts the name bound by a `let` / `val` / `and` / `instance` /
/// `assume` declaration. Handles `rec`, operator names (`( ++ )`, `( =~ )`),
/// and plain identifiers. Returns `None` for non-name patterns (tuples, …).
fn extract_value_name(rest: &str) -> Option<String> {
    let mut s = rest.trim_start();
    let (w, r) = next_word(s);
    if w == "rec" {
        s = r.trim_start();
    }
    let s = s.trim_start();
    if let Some(op) = operator_name(s) {
        return Some(op);
    }
    let (name, _) = next_word(s);
    (!name.is_empty()).then(|| name.to_string())
}

/// If `s` begins with a parenthesised operator, returns the operator symbol.
/// Handles plain symbolic operators (`( ++ )`, `( =~ )`) as well as F*
/// "let/and operators" — a `let`/`and`/`match`/`if`/`exists`/`forall` keyword
/// immediately followed by operator chars (`( let? )`, `( let:: )`,
/// `( and* )`, `( match? )`), which the F* lexer tokenises as a single
/// operator. Returns `None` for value patterns like `(x, y)`.
fn operator_name(s: &str) -> Option<String> {
    let s = s.trim_start();
    if !s.starts_with('(') {
        return None;
    }
    let close = s.find(')')?;
    let inner = s[1..close].trim();
    if inner.is_empty() {
        return None;
    }
    // Plain symbolic operator.
    if inner.chars().all(|c| is_op_char(c) || c == ' ') {
        return Some(inner.chars().filter(|c| !c.is_whitespace()).collect());
    }
    // Binding operator: keyword prefix + operator chars.
    for kw in ["let", "and", "match", "if", "exists", "forall"] {
        if let Some(rest) = inner.strip_prefix(kw) {
            let rest = rest.trim_start();
            if !rest.is_empty() && rest.chars().all(|c| is_op_char(c) || c == ' ') {
                let ops: String = rest.chars().filter(|c| !c.is_whitespace()).collect();
                return Some(format!("{kw}{ops}"));
            }
        }
    }
    None
}

/// Extracts the name introduced by `type` / `class` / `effect` / `exception`.
fn extract_type_name(rest: &str) -> Option<String> {
    let s = rest.trim_start();
    if let Some(op) = operator_name(s) {
        return Some(op);
    }
    let (name, _) = next_word(s);
    (!name.is_empty()).then(|| name.to_string())
}

/// A located declaration discovered in the first scanning pass.
struct DeclStart {
    kw: DeclKw,
    /// Line of the declaration keyword.
    line: usize,
    /// First line of the leading attribute/qualifier block (== `line` when none).
    attr_start: usize,
    /// Text following the keyword on the keyword line.
    rest: String,
}

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    file_path: String,
    timestamp: u64,
    /// `(qualified_prefix, parent_id)` — top is the active scope.
    scope: Vec<(String, String)>,
}

impl ExtractionState {
    fn parent_id(&self) -> String {
        self.scope
            .last()
            .map(|(_, id)| id.clone())
            .unwrap_or_default()
    }

    fn parent_qn(&self) -> String {
        self.scope
            .last()
            .map_or_else(|| self.file_path.clone(), |(qn, _)| qn.clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_node(
        &mut self,
        kind: NodeKind,
        name: &str,
        start_line: usize,
        attr_start: usize,
        end_line: usize,
        signature: Option<String>,
        docstring: Option<String>,
        visibility: Visibility,
    ) -> String {
        let qualified_name = format!("{}::{}", self.parent_qn(), name);
        let id = generate_node_id(&self.file_path, &kind, name, start_line as u32);
        let parent_id = self.parent_id();
        self.nodes.push(Node {
            id: id.clone(),
            kind,
            name: name.to_string(),
            qualified_name,
            file_path: self.file_path.clone(),
            start_line: start_line as u32,
            attrs_start_line: attr_start as u32,
            end_line: end_line as u32,
            start_column: 0,
            end_column: 0,
            signature,
            docstring,
            visibility,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            updated_at: self.timestamp,
            parent_id: None,
        });
        if !parent_id.is_empty() {
            self.edges.push(Edge {
                source: parent_id,
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line as u32),
            });
        }
        id
    }

    /// Emits a `Uses` edge to a (possibly external) module path.
    fn emit_uses(&mut self, target_path: &str, line: usize) {
        if target_path.is_empty() {
            return;
        }
        let target_id = generate_node_id(target_path, &NodeKind::File, target_path, 0);
        let source = self.parent_id();
        if source.is_empty() {
            return;
        }
        self.edges.push(Edge {
            source,
            target: target_id,
            kind: EdgeKind::Uses,
            line: Some(line as u32),
        });
    }
}

impl FStarExtractor {
    pub fn extract_fstar(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let raw_lines: Vec<&str> = source.split('\n').collect();
        let blanked = blank_comments_and_strings(source);
        let blank_lines: Vec<&str> = blanked.split('\n').collect();
        let total = raw_lines.len();

        let file_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        let mut state = ExtractionState {
            nodes: vec![Node {
                id: file_id.clone(),
                kind: NodeKind::File,
                name: file_path.to_string(),
                qualified_name: file_path.to_string(),
                file_path: file_path.to_string(),
                start_line: 0,
                attrs_start_line: 0,
                end_line: total.saturating_sub(1) as u32,
                start_column: 0,
                end_column: 0,
                signature: None,
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
                updated_at: timestamp,
                parent_id: None,
            }],
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            file_path: file_path.to_string(),
            timestamp,
            scope: vec![(file_path.to_string(), file_id)],
        };

        // Pass 1: discover every top-level declaration.
        let decls = collect_decls(&blank_lines);

        // Pass 2: emit nodes and edges.
        let mut prev_type = false; // was the previous decl a `type`? (for `and`)
        for (i, d) in decls.iter().enumerate() {
            let body_end = decl_end(&decls, i, &blank_lines);
            let docstring = collect_docstring(&raw_lines, d.attr_start);
            let visibility = if head_is_private(&raw_lines, d.attr_start, d.line) {
                Visibility::Private
            } else {
                Visibility::Pub
            };

            let effective = if d.kw == DeclKw::And {
                if prev_type {
                    DeclKw::Type
                } else {
                    DeclKw::Let
                }
            } else {
                d.kw
            };
            prev_type = effective == DeclKw::Type;

            // Full signature: for F*, the spec (requires/ensures/decreases,
            // refinement types) is the important part, not just the name.
            let signature = build_signature(&blank_lines, d.line, body_end, effective);

            match effective {
                DeclKw::Module => {
                    handle_module(&mut state, d, total);
                }
                DeclKw::Open | DeclKw::Include | DeclKw::Friend => {
                    let target = leading_dotted(&d.rest);
                    state.emit_uses(&target, d.line);
                }
                DeclKw::Let | DeclKw::Val => {
                    if let Some(name) = extract_value_name(&d.rest) {
                        state.emit_node(
                            NodeKind::Function,
                            &name,
                            d.line,
                            d.attr_start,
                            body_end,
                            signature,
                            docstring,
                            visibility,
                        );
                    }
                }
                DeclKw::Type => {
                    handle_type(
                        &mut state,
                        d,
                        body_end,
                        &blank_lines,
                        signature,
                        docstring,
                        visibility,
                        NodeKind::Struct,
                    );
                }
                DeclKw::Class => {
                    handle_type(
                        &mut state,
                        d,
                        body_end,
                        &blank_lines,
                        signature,
                        docstring,
                        visibility,
                        NodeKind::Trait,
                    );
                }
                DeclKw::Instance => {
                    if let Some(name) = extract_value_name(&d.rest) {
                        let id = state.emit_node(
                            NodeKind::Impl,
                            &name,
                            d.line,
                            d.attr_start,
                            body_end,
                            signature,
                            docstring,
                            visibility,
                        );
                        // Record the implemented class (the head of the type
                        // after `:`) as an unresolved `Implements` reference.
                        if let Some(class) = instance_class(&d.rest) {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: id,
                                reference_name: class,
                                reference_kind: EdgeKind::Implements,
                                line: d.line as u32,
                                column: 0,
                                file_path: state.file_path.clone(),
                            });
                        }
                    }
                }
                DeclKw::Effect => {
                    if let Some(name) = extract_type_name(&d.rest) {
                        state.emit_node(
                            NodeKind::TypeAlias,
                            &name,
                            d.line,
                            d.attr_start,
                            body_end,
                            signature,
                            docstring,
                            visibility,
                        );
                    }
                }
                DeclKw::Exception => {
                    if let Some(name) = extract_type_name(&d.rest) {
                        state.emit_node(
                            NodeKind::Struct,
                            &name,
                            d.line,
                            d.attr_start,
                            body_end,
                            signature,
                            docstring,
                            visibility,
                        );
                    }
                }
                DeclKw::Assume => {
                    if let Some(name) = extract_value_name(&d.rest) {
                        state.emit_node(
                            NodeKind::Const,
                            &name,
                            d.line,
                            d.attr_start,
                            body_end,
                            signature,
                            docstring,
                            visibility,
                        );
                    }
                }
                DeclKw::And => unreachable!("And resolved to Let/Type above"),
            }
        }

        let mut result = ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: state.unresolved_refs,
            errors: Vec::new(),
            duration_ms: start.elapsed().as_millis() as u64,
        };
        result.sanitize();
        result
    }
}

/// Handles `module M.N` (declares the file scope) and `module A = M.N` (an
/// abbreviation, emitted as a `Uses` edge).
fn handle_module(state: &mut ExtractionState, d: &DeclStart, total: usize) {
    let rest = d.rest.trim();
    if rest.contains('=') {
        // `module A = M.N` or `module _ = M.N` → import-like.
        let rhs = rest.split_once('=').map_or("", |x| x.1);
        let target = leading_dotted(rhs);
        state.emit_uses(&target, d.line);
        return;
    }
    let name = leading_dotted(rest);
    if name.is_empty() {
        return;
    }
    // The top-level module becomes the enclosing scope for the rest of the
    // file (it has no `end`), mirroring how the file is one module.
    let id = state.emit_node(
        NodeKind::Module,
        &name,
        d.line,
        d.attr_start,
        total.saturating_sub(1),
        Some(format!("module {name}")),
        None,
        Visibility::Pub,
    );
    let qn = format!("{}::{}", state.file_path, name);
    state.scope.push((qn, id));
}

/// Handles a `type` or `class` declaration: distinguishes records (→ `Struct` /
/// `Trait` with `Field` children), variants (→ `Enum` with `EnumVariant`
/// children), and abbreviations (→ `TypeAlias`).
#[allow(clippy::too_many_arguments)]
fn handle_type(
    state: &mut ExtractionState,
    d: &DeclStart,
    body_end: usize,
    blank_lines: &[&str],
    signature: Option<String>,
    docstring: Option<String>,
    visibility: Visibility,
    record_kind: NodeKind,
) {
    let Some(name) = extract_type_name(&d.rest) else {
        return;
    };

    let chars = scan_span(blank_lines, d.line, body_end);
    let shape = classify_type_shape(&chars);

    let kind = match shape {
        TypeShape::Record(_) => record_kind,
        TypeShape::Variant => NodeKind::Enum,
        TypeShape::Alias => NodeKind::TypeAlias,
    };

    let parent = state.emit_node(
        kind,
        &name,
        d.line,
        d.attr_start,
        body_end,
        signature,
        docstring,
        visibility,
    );
    let parent_qn = format!("{}::{}", state.parent_qn(), name);

    match shape {
        TypeShape::Record(open_idx) => {
            for (field_name, field_line) in record_fields(&chars, open_idx) {
                emit_child(
                    state,
                    NodeKind::Field,
                    &field_name,
                    field_line,
                    &parent,
                    &parent_qn,
                );
            }
        }
        TypeShape::Variant => {
            for (ctor_name, ctor_line) in variant_constructors(&chars) {
                emit_child(
                    state,
                    NodeKind::EnumVariant,
                    &ctor_name,
                    ctor_line,
                    &parent,
                    &parent_qn,
                );
            }
        }
        TypeShape::Alias => {}
    }
}

/// Emits a child node (field / variant) parented to `parent`.
fn emit_child(
    state: &mut ExtractionState,
    kind: NodeKind,
    name: &str,
    line: usize,
    parent_id: &str,
    parent_qn: &str,
) {
    let id = generate_node_id(&state.file_path, &kind, name, line as u32);
    state.nodes.push(Node {
        id: id.clone(),
        kind,
        name: name.to_string(),
        qualified_name: format!("{parent_qn}::{name}"),
        file_path: state.file_path.clone(),
        start_line: line as u32,
        attrs_start_line: line as u32,
        end_line: line as u32,
        start_column: 0,
        end_column: 0,
        signature: None,
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
        updated_at: state.timestamp,
        parent_id: None,
    });
    state.edges.push(Edge {
        source: parent_id.to_string(),
        target: id,
        kind: EdgeKind::Contains,
        line: Some(line as u32),
    });
}

#[derive(Clone, Copy)]
enum TypeShape {
    /// Record body; carries the index (into the scanned char vec) of the `{`.
    Record(usize),
    Variant,
    Alias,
}

/// Determines whether a `type` / `class` definition is a record, a variant, or
/// a plain abbreviation, by inspecting the text after the definitional `=`.
fn classify_type_shape(chars: &[(usize, char)]) -> TypeShape {
    // `class c = { … }` always has `=`; an `=`-less `type t` is abstract.
    let Some(eq) = find_def_eq(chars) else {
        return TypeShape::Alias;
    };
    // First meaningful char after `=`, skipping whitespace and an attribute
    // group (`[@@ … ]`) that can precede a record body.
    let mut i = eq + 1;
    loop {
        i = skip_ws(chars, i);
        if i < chars.len() && chars[i].1 == '[' {
            // Skip a `[ … ]` attribute group.
            i = skip_bracketed(chars, i);
            continue;
        }
        break;
    }
    if i < chars.len() && chars[i].1 == '{' {
        return TypeShape::Record(i);
    }
    // Variant if there is a top-level `|` anywhere in the definition.
    if has_top_level_bar(&chars[eq + 1..]) {
        return TypeShape::Variant;
    }
    TypeShape::Alias
}

fn skip_ws(chars: &[(usize, char)], mut i: usize) -> usize {
    while i < chars.len() && chars[i].1.is_whitespace() {
        i += 1;
    }
    i
}

/// Skips a balanced `[ … ]` group starting at `i` (which is the `[`).
fn skip_bracketed(chars: &[(usize, char)], i: usize) -> usize {
    let mut depth = 0i32;
    let mut j = i;
    while j < chars.len() {
        match chars[j].1 {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    j
}

/// Finds the definitional `=` (a standalone `=`, not part of `==`, `:=`, `=~`,
/// `<=`, …) at bracket depth 0.
fn find_def_eq(chars: &[(usize, char)]) -> Option<usize> {
    let mut depth = 0i32;
    let mut prev = ' ';
    for (idx, &(_, c)) in chars.iter().enumerate() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 && c == '=' {
            let next = chars.get(idx + 1).map_or(' ', |&(_, c)| c);
            if !is_op_char(prev) && !is_op_char(next) {
                return Some(idx);
            }
        }
        prev = c;
    }
    None
}

fn has_top_level_bar(chars: &[(usize, char)]) -> bool {
    let mut depth = 0i32;
    for &(_, c) in chars {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '|' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Extracts `(name, line)` for each field of a record body. `open_idx` is the
/// position of the opening `{` in `chars`.
fn record_fields(chars: &[(usize, char)], open_idx: usize) -> Vec<(String, usize)> {
    let mut fields = Vec::new();
    let mut depth = 0i32;
    let mut seg: Vec<(usize, char)> = Vec::new();
    let mut i = open_idx;
    while i < chars.len() {
        let (_, c) = chars[i];
        match c {
            '{' | '(' | '[' => {
                depth += 1;
                if depth > 1 {
                    seg.push(chars[i]);
                }
            }
            '}' | ')' | ']' => {
                depth -= 1;
                if depth == 0 {
                    // End of the record body.
                    if let Some(f) = parse_field(&seg) {
                        fields.push(f);
                    }
                    break;
                }
                seg.push(chars[i]);
            }
            ';' if depth == 1 => {
                if let Some(f) = parse_field(&seg) {
                    fields.push(f);
                }
                seg.clear();
            }
            _ if depth >= 1 => seg.push(chars[i]),
            _ => {}
        }
        i += 1;
    }
    fields
}

/// Parses a single record-field segment (`name : type`). Returns `None` for
/// segments that are not field declarations (no `:`).
fn parse_field(seg: &[(usize, char)]) -> Option<(String, usize)> {
    let s: String = seg.iter().map(|&(_, c)| c).collect();
    if !s.contains(':') {
        return None;
    }
    // Find first non-whitespace, skipping a leading attribute group.
    let mut i = skip_ws(seg, 0);
    if i < seg.len() && seg[i].1 == '[' {
        i = skip_bracketed(seg, i);
        i = skip_ws(seg, i);
    }
    if i >= seg.len() {
        return None;
    }
    let field_line = seg[i].0;
    let rest: String = seg[i..].iter().map(|&(_, c)| c).collect();
    let name = if let Some(op) = operator_name(rest.trim_start()) {
        op
    } else {
        next_word(&rest).0.to_string()
    };
    (!name.is_empty()).then_some((name, field_line))
}

/// Extracts `(name, line)` for each constructor of a variant type.
fn variant_constructors(chars: &[(usize, char)]) -> Vec<(String, usize)> {
    let Some(eq) = find_def_eq(chars) else {
        return Vec::new();
    };
    let region = &chars[eq + 1..];
    let mut ctors = Vec::new();
    let mut depth = 0i32;
    let mut seg: Vec<(usize, char)> = Vec::new();
    for &(ln, c) in region {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                seg.push((ln, c));
            }
            ')' | ']' | '}' => {
                depth -= 1;
                seg.push((ln, c));
            }
            '|' if depth == 0 => {
                push_ctor(&mut ctors, &seg);
                seg.clear();
            }
            _ => seg.push((ln, c)),
        }
    }
    push_ctor(&mut ctors, &seg);
    ctors
}

fn push_ctor(ctors: &mut Vec<(String, usize)>, seg: &[(usize, char)]) {
    let mut i = skip_ws(seg, 0);
    if i < seg.len() && seg[i].1 == '[' {
        i = skip_bracketed(seg, i);
        i = skip_ws(seg, i);
    }
    if i >= seg.len() {
        return;
    }
    let ctor_line = seg[i].0;
    let rest: String = seg[i..].iter().map(|&(_, c)| c).collect();
    let name = next_word(&rest).0.to_string();
    // Constructors start with an upper-case letter; skip anything else
    // (e.g. an attribute leftover or a doc fragment).
    if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        ctors.push((name, ctor_line));
    }
}

/// The implemented class of an `instance name : Class args = …` declaration:
/// the head identifier of the type after the first top-level `:`.
fn instance_class(rest: &str) -> Option<String> {
    let chars: Vec<(usize, char)> = rest.char_indices().map(|(_, c)| (0, c)).collect();
    let mut depth = 0i32;
    for (idx, &(_, c)) in chars.iter().enumerate() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ':' if depth == 0 => {
                let after: String = chars[idx + 1..].iter().map(|&(_, c)| c).collect();
                let name = leading_dotted(after.trim_start());
                // Take the last component of a dotted class path.
                let short = name.rsplit('.').next().unwrap_or(&name).to_string();
                return (!short.is_empty()).then_some(short);
            }
            _ => {}
        }
    }
    None
}

/// Builds a declaration's signature — the part that matters for F*: binders,
/// the return type, and (crucially) the `requires`/`ensures`/`decreases`
/// specification and any refinement types. Comments are stripped (we read the
/// blanked text) and whitespace is normalised to single spaces.
///
/// For `val`/`assume`/`exception` the whole declaration is signature (there is
/// no implementation). For everything else the signature is the text up to the
/// definitional `=`, so a `let`-defined lemma keeps its `: Lemma (requires …)
/// (ensures …)` but drops the proof term.
fn build_signature(blank_lines: &[&str], start: usize, end: usize, kw: DeclKw) -> Option<String> {
    // Generous cap: real Lemma specs are well under this; guards against a
    // pathological run-on.
    const MAX: usize = 800;
    let cut = match kw {
        DeclKw::Val | DeclKw::Assume | DeclKw::Exception => None,
        _ => find_def_eq_pos(blank_lines, start, end),
    };
    let end = end.min(blank_lines.len().saturating_sub(1));
    let mut buf = String::new();
    for line_idx in start..=end {
        let line = blank_lines.get(line_idx).copied().unwrap_or("");
        match cut {
            Some((cl, col)) if line_idx == cl => {
                buf.push(' ');
                buf.push_str(&line[..col.min(line.len())]);
                break;
            }
            _ => {
                buf.push(' ');
                buf.push_str(line);
            }
        }
    }
    let sig = buf.split_whitespace().collect::<Vec<_>>().join(" ");
    if sig.is_empty() {
        return None;
    }
    if sig.chars().count() > MAX {
        let mut s: String = sig.chars().take(MAX).collect();
        s.push('…');
        Some(s)
    } else {
        Some(sig)
    }
}

/// `(line, byte-column)` of the definitional `=` (a standalone `=`, not part of
/// `==`/`:=`/`<=`/`=~`/…) at bracket depth 0 within `[start, end]`.
fn find_def_eq_pos(blank_lines: &[&str], start: usize, end: usize) -> Option<(usize, usize)> {
    let end = end.min(blank_lines.len().saturating_sub(1));
    let mut depth = 0i32;
    let mut prev = ' ';
    for line_idx in start..=end {
        let line = blank_lines.get(line_idx).copied().unwrap_or("");
        for (byte_off, c) in line.char_indices() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = (depth - 1).max(0),
                _ => {}
            }
            if depth == 0 && c == '=' {
                let next = line[byte_off + c.len_utf8()..]
                    .chars()
                    .next()
                    .unwrap_or(' ');
                if !is_op_char(prev) && !is_op_char(next) {
                    return Some((line_idx, byte_off));
                }
            }
            prev = c;
        }
        prev = ' '; // a line break separates operator tokens
    }
    None
}

/// Flattens span lines `[start, end]` into `(abs_line, char)` pairs, inserting a
/// newline marker between lines (tagged with the line it terminates).
fn scan_span(blank_lines: &[&str], start: usize, end: usize) -> Vec<(usize, char)> {
    let mut v = Vec::new();
    let end = end.min(blank_lines.len().saturating_sub(1));
    for (offset, line) in blank_lines[start..=end].iter().enumerate() {
        let abs = start + offset;
        for c in line.chars() {
            v.push((abs, c));
        }
        v.push((abs, '\n'));
    }
    v
}

/// Pass 1: walk every line, grouping leading attribute/qualifier blocks with the
/// declaration they precede.
///
/// A column-0 declaration keyword only opens a new declaration when the previous
/// declaration's term has closed — tracked via bracket depth, the local
/// `let`…`in` balance, and whether the last line ended on a continuation token.
/// This distinguishes a top-level `let` from a column-0 local `let … in` (F*
/// does not require bodies to be indented).
fn collect_decls(blank_lines: &[&str]) -> Vec<DeclStart> {
    let mut decls = Vec::new();
    let mut pending_attr: Option<usize> = None;
    let mut depth: i32 = 0; // (), [], {}, begin/end
    let mut let_in: i32 = 0; // unclosed local `let`s awaiting `in`
    let mut cont = false; // previous non-blank line ended on a continuation token

    for (i, line) in blank_lines.iter().enumerate() {
        let trimmed_end = line.trim_end();
        if trimmed_end.trim_start().is_empty() {
            // Blank line: attributes attach contiguously, so a gap clears them.
            // It does not close an open term (a blank line inside a body is fine).
            pending_attr = None;
            continue;
        }
        let is_col0 = !trimmed_end.starts_with(char::is_whitespace);
        // The previous declaration's term is still open if brackets are unbalanced,
        // a local `let` awaits its `in`, or the last line ended mid-term.
        let body_open = depth > 0 || let_in > 0 || cont;

        // Text whose tokens feed the running counters: for a brand-new declaration
        // we skip the leading keyword (so a top-level `let` is not counted as a
        // pending local `let`); otherwise the whole (blanked) line.
        let mut scan_text: String = trimmed_end.trim_start().to_string();

        if is_col0 && !body_open {
            match classify_head(trimmed_end) {
                Head::Decl(kw, rest) => {
                    let attr_start = pending_attr.take().unwrap_or(i);
                    let_in = 0;
                    // `rest` is the line with the keyword (and qualifiers) removed.
                    scan_text.clone_from(&rest);
                    decls.push(DeclStart {
                        kw,
                        line: i,
                        attr_start,
                        rest,
                    });
                }
                Head::AttrOnly => {
                    if pending_attr.is_none() {
                        pending_attr = Some(i);
                    }
                }
                Head::NotDecl => {
                    pending_attr = None;
                }
            }
        }

        update_counters(&scan_text, &mut depth, &mut let_in);
        cont = ends_with_continuation(&scan_text);
    }
    decls
}

/// Updates bracket depth and the local `let`…`in` balance from one line's tokens
/// (operating on the comment/string-blanked text).
fn update_counters(text: &str, depth: &mut i32, let_in: &mut i32) {
    let mut word = String::new();
    for c in text.chars() {
        if is_ident_char(c) {
            word.push(c);
            continue;
        }
        apply_word(&word, c, depth, let_in);
        word.clear();
        match c {
            '(' | '[' | '{' => *depth += 1,
            ')' | ']' | '}' => *depth = (*depth - 1).max(0),
            _ => {}
        }
    }
    apply_word(&word, ' ', depth, let_in);
}

/// Applies a single identifier word to the bracket / `let`…`in` counters.
/// `delim` is the character that terminated the word; a keyword immediately
/// followed by an operator char (e.g. `let?`, `and*`) is a binding-operator
/// token, not the keyword, so it is not counted.
fn apply_word(w: &str, delim: char, depth: &mut i32, let_in: &mut i32) {
    if is_op_char(delim) {
        return;
    }
    match w {
        "begin" => *depth += 1,
        "end" => *depth = (*depth - 1).max(0),
        "let" => *let_in += 1,
        "in" => *let_in = (*let_in - 1).max(0),
        _ => {}
    }
}

/// Whether a (blanked) line ends on a token that requires the term to continue
/// on the following line — an operator char, an open bracket / separator, or a
/// continuation keyword. Used so a column-0 keyword that follows such a line is
/// treated as a continuation rather than a new declaration.
fn ends_with_continuation(text: &str) -> bool {
    let t = text.trim_end();
    let Some(last) = t.chars().last() else {
        return false;
    };
    if is_op_char(last) || matches!(last, '(' | '[' | '{' | ',' | ';') {
        return true;
    }
    // Trailing run of identifier characters (empty if `last` is e.g. `)`).
    let mut rev: Vec<char> = t.chars().rev().take_while(|c| is_ident_char(*c)).collect();
    rev.reverse();
    let last_word: String = rev.into_iter().collect();
    matches!(
        last_word.as_str(),
        "then"
            | "else"
            | "with"
            | "begin"
            | "fun"
            | "match"
            | "if"
            | "in"
            | "of"
            | "requires"
            | "ensures"
            | "decreases"
            | "let"
            | "and"
            | "when"
            | "returns"
            | "by"
            | "forall"
            | "exists"
    )
}

/// Last line of declaration `i`'s body: the line before the next declaration's
/// attribute block, with trailing blank lines trimmed.
fn decl_end(decls: &[DeclStart], i: usize, blank_lines: &[&str]) -> usize {
    let raw_end = if i + 1 < decls.len() {
        decls[i + 1].attr_start.saturating_sub(1)
    } else {
        blank_lines.len().saturating_sub(1)
    };
    let mut end = raw_end.max(decls[i].line);
    while end > decls[i].line {
        let l = blank_lines.get(end).map_or("", |s| s.trim());
        if l.is_empty() {
            end -= 1;
        } else {
            break;
        }
    }
    end
}

/// Collects a contiguous block of `///` doc-comment lines directly above
/// `attr_start`, returning them joined (markers stripped).
fn collect_docstring(raw_lines: &[&str], attr_start: usize) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut idx = attr_start;
    while idx > 0 {
        idx -= 1;
        let t = raw_lines.get(idx).map_or("", |s| s.trim());
        if let Some(rest) = t.strip_prefix("///") {
            lines.push(rest.trim().to_string());
        } else {
            break;
        }
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(lines.join("\n"))
}

/// Whether the declaration carries a `private` qualifier (scanning the attribute
/// block through the keyword line).
fn head_is_private(raw_lines: &[&str], attr_start: usize, kw_line: usize) -> bool {
    (attr_start..=kw_line).any(|i| {
        raw_lines
            .get(i)
            .is_some_and(|l| l.split(|c: char| !is_ident_char(c)).any(|w| w == "private"))
    })
}

/// Replaces the contents of comments (`(* … *)`, nested; and `// …`) and string
/// literals with spaces, preserving newlines and overall line structure so the
/// structural scanner never trips over braces, `=`, `|` or `;` inside them.
fn blank_comments_and_strings(source: &str) -> String {
    #[derive(PartialEq)]
    enum St {
        Normal,
        Line,
        Block(u32),
        Str,
    }
    let mut out = String::with_capacity(source.len());
    let mut st = St::Normal;
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied().unwrap_or('\0');
        match st {
            St::Normal => {
                if c == '(' && next == '*' {
                    st = St::Block(1);
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                }
                if c == '/' && next == '/' {
                    st = St::Line;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                }
                if c == '"' {
                    // Preserve the quote characters (blank only the interior) so
                    // a string-valued RHS like `let x = "y"` doesn't collapse to
                    // a trailing `=` and look like an unterminated declaration
                    // that swallows the following declaration.
                    st = St::Str;
                    out.push('"');
                    i += 1;
                    continue;
                }
                out.push(c);
                i += 1;
            }
            St::Line => {
                if c == '\n' {
                    st = St::Normal;
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
            St::Block(depth) => {
                if c == '(' && next == '*' {
                    st = St::Block(depth + 1);
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                }
                if c == '*' && next == ')' {
                    st = if depth == 1 {
                        St::Normal
                    } else {
                        St::Block(depth - 1)
                    };
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                }
                out.push(if c == '\n' { '\n' } else { ' ' });
                i += 1;
            }
            St::Str => {
                if c == '\\' {
                    // Escaped char: blank both, but preserve a newline if the
                    // escape is at end of line (shouldn't normally happen).
                    out.push(' ');
                    if next == '\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 2;
                    continue;
                }
                if c == '"' {
                    st = St::Normal;
                    out.push('"');
                    i += 1;
                    continue;
                }
                out.push(if c == '\n' { '\n' } else { ' ' });
                i += 1;
            }
        }
    }
    out
}

impl crate::extraction::LanguageExtractor for FStarExtractor {
    fn extensions(&self) -> &[&str] {
        &["fst", "fsti"]
    }

    fn language_name(&self) -> &'static str {
        "F*"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_fstar(file_path, source)
    }
}
