//! Resolving the receiver of a field reference to a declared type (#458).
//!
//! `tokensave_field_sites` documents a qualified form — `GraphStats::last_sync_at`
//! narrows to one struct's field — that used to be parsed and then ignored,
//! returning every same-named field's sites under a narrow heading. Applying
//! it for real means answering, for a text site `recv.field`, what type `recv`
//! is.
//!
//! Full type inference is out of reach for a source-text scan, and guessing
//! would reintroduce the original complaint in a worse form: a narrowed answer
//! that silently drops real sites. So this resolves only what it can name a
//! reason for, and reports the rest as unattributed rather than folding them
//! into either side:
//!
//! - **`self`/`this`/`cls`, or the receiver binding named in the enclosing
//!   signature** (Go's `func (s *Server)`, Rust's `&self`) — the type is the
//!   one that owns the enclosing symbol, which the graph already knows from
//!   its qualified name.
//! - **A local binding with its type written down** in the enclosing symbol's
//!   own source range: `let x: T`, `x := T{`, `var x T`, `T x =`, `x = T(`,
//!   `x = new T(`. Only the enclosing range is searched, so a same-named
//!   variable in another function cannot leak in.
//! - **A chain through declared field types** — `self.inner.field`, where the
//!   owning type's `inner` field has a written type. Bounded to
//!   [`MAX_CHAIN_HOPS`] hops.
//! - **A static or associated access**, `T.field` / `T::field`, where the
//!   receiver text is the type name itself.
//!
//! Anything else is [`Attribution::Unknown`]. A caller is told how many of
//! those there were, so a narrowed answer never poses as a complete one.

use std::collections::HashMap;

/// How far a `a.b.c.field` chain is followed through declared field types.
/// Three covers the shapes that show up in practice without turning a text
/// scan into a resolver.
pub const MAX_CHAIN_HOPS: usize = 3;

/// What a site's receiver turned out to be, relative to the requested type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribution {
    /// The receiver resolves to the requested type. Keep the site.
    Matches,
    /// The receiver resolves to a *different* type. Drop the site — this is
    /// the narrowing doing its job.
    Excludes,
    /// The receiver could not be resolved. Reported separately, never folded
    /// into either answer.
    Unknown,
}

/// The enclosing symbol a site sits inside, as the graph knows it.
#[derive(Debug, Clone)]
pub struct Enclosing {
    /// e.g. `src/types.rs::GraphStats::last_sync_at`.
    pub qualified_name: String,
    /// Declaration text, when the graph has it — used for the receiver
    /// binding in `func (s *Server)` and `fn f(&self)`.
    pub signature: Option<String>,
    /// 0-based, inclusive, as node spans are stored.
    pub start_line: u32,
    pub end_line: u32,
}

/// Declared types of fields, keyed by `Type::field`. Values are bare type
/// names, already stripped of references and generics.
pub type FieldTypes = HashMap<String, String>;

/// Return types of functions and methods, keyed by bare name, holding the
/// declared type as written (`Result<GraphStats>`).
///
/// Keyed by bare name because a text scan cannot tell which type's method a
/// call belongs to. The builder only records a name when every function with
/// that name agrees on its return type, so a lookup here can never attribute a
/// site to the wrong one of two same-named methods.
pub type ReturnTypes = HashMap<String, String>;

/// Everything the resolver can consult beyond the source text itself.
#[derive(Debug, Default)]
pub struct TypeIndex {
    pub fields: FieldTypes,
    pub returns: ReturnTypes,
}

/// The type a call expression yields, given its declared return type.
///
/// `Result<GraphStats>` is a `Result` — unless the initializer unwraps it,
/// which is what `?`, `.unwrap()` and `.expect(` do, and what makes
/// `let s = cg.get_stats().await?` a `GraphStats`. Only one wrapper layer is
/// peeled, and only for the wrappers whose unwrapping is unambiguous.
#[must_use]
pub fn call_result_type(declared: &str, initializer: &str) -> String {
    let outer = normalize_type(declared);
    let unwraps = initializer.contains('?')
        || initializer.contains(".unwrap()")
        || initializer.contains(".expect(");
    if !unwraps || !matches!(outer.as_str(), "Result" | "Option") {
        return outer;
    }
    // `Result<GraphStats>` -> `GraphStats`; `Result<A, B>` is left alone,
    // since which side a `?` yields is not readable from the text.
    let Some(open) = declared.find('<') else {
        return outer;
    };
    let Some(close) = declared.rfind('>') else {
        return outer;
    };
    let inner = &declared[open + 1..close];
    if inner.contains(',') {
        return outer;
    }
    normalize_type(inner)
}

/// The bare type name a qualifier refers to: `module::Type` and `Type` both
/// resolve against the same declarations, since a text scan cannot tell two
/// same-named types in different modules apart anyway.
#[must_use]
pub fn bare_type_name(qualifier: &str) -> &str {
    qualifier.rsplit("::").next().unwrap_or(qualifier)
}

/// Strips a declared type down to the name a field site can be compared
/// against: `&mut Vec<Scope>` -> `Vec`, `*Server` -> `Server`,
/// `Option<Config>` -> `Option`.
///
/// Deliberately keeps the *outer* constructor rather than unwrapping it. A
/// field declared `Option<Config>` is not a `Config`, and pretending otherwise
/// would attribute `opt.field` to `Config` on the strength of a wrapper.
#[must_use]
pub fn normalize_type(raw: &str) -> String {
    let mut t = raw.trim();
    loop {
        let trimmed = t
            .trim_start_matches(['&', '*'])
            .trim_start()
            .trim_start_matches("mut ")
            .trim_start_matches("dyn ")
            .trim_start_matches("impl ")
            .trim_start();
        if trimmed == t {
            break;
        }
        t = trimmed;
    }
    // Drop generics, array/slice brackets and anything after the head.
    let head: String = t
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':' || *c == '.')
        .collect();
    bare_type_name(head.trim_end_matches(':')).to_string()
}

/// The type named by the right-hand side of an assignment.
///
/// `GraphStats{}` and `GraphStats(...)` name the type directly, but
/// `GraphStats::new()` and `GraphStats.of()` name a *constructor on* it — the
/// last segment there is the function, not the type. Dropping a trailing
/// called segment tells the two apart, while a single segment followed by `(`
/// stays as it is, since that is Python's `GraphStats()`.
#[must_use]
pub fn type_from_constructor(rhs: &str) -> String {
    let cleaned = rhs.trim().trim_start_matches(['&', '*']).trim_start();
    let path: String = cleaned
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':' || *c == '.')
        .collect();
    let called = cleaned[path.len()..].starts_with('(');
    let flattened = path.replace("::", ".");
    let segments: Vec<&str> = flattened.split('.').filter(|s| !s.is_empty()).collect();
    let Some(last) = segments.last().copied() else {
        return String::new();
    };
    if !called {
        // `T{}`, `T`, `crate::types::T` — the tail is the type.
        return last.to_string();
    }

    // A call. Which segment is the type depends on the separator, because
    // `T::new()` and `receiver.method()` are the same shape otherwise.
    let path_before_last = &path[..path.len() - last.len()];
    if path_before_last.ends_with("::") && segments.len() > 1 {
        // Rust-style associated constructor: the type is what it hangs off.
        return segments[segments.len() - 2].to_string();
    }
    // `name()` or `module.Name()` — a construction in Python, Java and
    // TypeScript, an ordinary function or method call everywhere. Leaning on
    // the near-universal capitalization of type names is the only thing that
    // separates `GraphStats()` from `load()` and `cg.get_stats()`, and
    // guessing wrong would attribute sites to a type never involved. A
    // lowercase call is left to the return-type index instead.
    if last.starts_with(char::is_uppercase) {
        return last.to_string();
    }
    String::new()
}

/// The name of the function called at the head of an initializer:
/// `cg.get_stats().await?` -> `get_stats`.
///
/// Only a plain dotted path followed by `(` counts. Anything else — an index,
/// a nested call as the receiver — is left unresolved rather than guessed at.
#[must_use]
pub fn called_function_name(rhs: &str) -> Option<String> {
    let path: String = rhs
        .trim()
        .trim_start_matches(['&', '*'])
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':' || *c == '.')
        .collect();
    if !rhs.trim().trim_start_matches(['&', '*']).trim_start()[path.len()..].starts_with('(') {
        return None;
    }
    let flattened = path.replace("::", ".");
    let name = flattened.rsplit('.').next()?;
    (!name.is_empty()).then(|| name.to_string())
}

/// Receiver expression immediately to the left of a `.field` site.
///
/// Returns the dotted chain as segments — `self.inner.field` at the `.field`
/// dot yields `["self", "inner"]`. Returns `None` when the receiver is not a
/// plain chain of identifiers: a call, an index, a parenthesised expression or
/// a literal are all shapes this cannot type, and must stay `Unknown` rather
/// than be half-read.
#[must_use]
pub fn receiver_chain(source: &str, dot_byte: usize) -> Option<Vec<String>> {
    let bytes = source.as_bytes();
    let mut end = dot_byte;
    let mut segments: Vec<String> = Vec::new();
    loop {
        // Skip whitespace between a segment and its dot (`foo\n  .bar`).
        while end > 0 && matches!(bytes.get(end - 1), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            end -= 1;
        }
        let mut start = end;
        while start > 0 {
            let c = bytes[start - 1];
            if c.is_ascii_alphanumeric() || c == b'_' {
                start -= 1;
            } else {
                break;
            }
        }
        if start == end {
            // Nothing identifier-shaped here: a `)`, `]`, quote or operator.
            return None;
        }
        segments.push(source[start..end].to_string());

        // A `::` before the segment means it is type-qualified (`Config::DEFAULT`);
        // keep the head only, and stop — this is a static access.
        let mut probe = start;
        while probe > 0 && matches!(bytes.get(probe - 1), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            probe -= 1;
        }
        if probe >= 2 && &source[probe - 2..probe] == "::" {
            end = probe - 2;
            continue;
        }
        if probe >= 1 && bytes[probe - 1] == b'.' {
            end = probe - 1;
            continue;
        }
        break;
    }
    segments.reverse();
    Some(segments)
}

/// The type that owns `qualified_name` — `src/types.rs::GraphStats::field`
/// owns `GraphStats`. `None` for a free function, which owns nothing.
#[must_use]
pub fn owning_type(qualified_name: &str) -> Option<&str> {
    let mut parts: Vec<&str> = qualified_name.split("::").collect();
    parts.pop()?; // the symbol itself
    let owner = parts.pop()?;
    // A path segment, not a type: `src/types.rs::free_function` leaves the
    // file path behind, which is never an owner.
    if owner.contains('/') || owner.contains('.') || owner.is_empty() {
        return None;
    }
    Some(owner)
}

/// The receiver binding a method signature names for itself: `s` in
/// `func (s *Server) Handle()`, or `self` in `fn f(&self)`.
#[must_use]
pub fn receiver_binding(signature: &str) -> Option<String> {
    let sig = signature.trim();
    // Go: `func (s *Server) Name(...)`.
    if let Some(rest) = sig.strip_prefix("func (") {
        let inner = rest.split(')').next()?;
        let name = inner.split_whitespace().next()?;
        if is_identifier(name) {
            return Some(name.to_string());
        }
    }
    // Rust and friends spell it `self` in the parameter list.
    if sig.contains("self") {
        return Some("self".to_string());
    }
    None
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Is `name` one of the implicit self-receivers?
fn is_self_word(name: &str) -> bool {
    matches!(name, "self" | "this" | "cls" | "Self" | "me")
}

/// Finds a written-down type for `binding` within `lines`, the enclosing
/// symbol's own source. Returns `None` unless exactly one declaration shape
/// matches — an ambiguous read must stay `Unknown`.
///
/// Only the enclosing range is searched, so a same-named local in a different
/// function cannot leak in.
#[must_use]
pub fn declared_type_in(lines: &[&str], binding: &str, returns: &ReturnTypes) -> Option<String> {
    let mut written: Vec<String> = Vec::new();
    let mut inferred: Vec<String> = Vec::new();
    for line in lines {
        let line = line.trim();
        let found = declaration_candidates(line, binding, returns);
        for candidate in found.written {
            let ty = normalize_type(&candidate);
            if is_identifier(&ty) {
                written.push(ty);
            }
        }
        for candidate in found.inferred {
            let ty = normalize_type(&candidate);
            if is_identifier(&ty) {
                inferred.push(ty);
            }
        }
    }
    // A type the author wrote down outranks one read off a right-hand side:
    // `GraphStats s = load()` states the type outright, and the call tells us
    // only what the initializer was spelled as.
    unanimous(&written).or_else(|| unanimous(&inferred))
}

/// The single type in `candidates`, or `None` if they disagree or there are
/// none. Disagreement means a rebind or a misread — either way, not something
/// to guess at.
fn unanimous(candidates: &[String]) -> Option<String> {
    let first = candidates.first()?;
    candidates.iter().all(|c| c == first).then(|| first.clone())
}

/// Types a line claims for a binding, split by how strong the claim is.
#[derive(Default)]
struct Candidates {
    /// The type is written down: `let x: T`, `var x T`, `T x`.
    written: Vec<String>,
    /// The type is read off an initializer: `x = T::new()`.
    inferred: Vec<String>,
}

/// Every type a single line claims for `binding`, across the declaration
/// shapes the supported languages use.
fn declaration_candidates(line: &str, binding: &str, returns: &ReturnTypes) -> Candidates {
    let mut out = Candidates::default();

    // `let x: T`, `const x: T`, `var x: T`, `x: T =` (Python annotation).
    for prefix in ["let ", "const ", "var ", "final ", ""] {
        let head = format!("{prefix}{binding}");
        if let Some(rest) = strip_binding(line, &head) {
            if let Some(after) = rest.strip_prefix(':') {
                // Stop at `=` so `let x: T = y` reads `T`, not `T = y`.
                let ty = after.split('=').next().unwrap_or(after);
                out.written.push(ty.to_string());
            }
        }
    }

    // `x := T{...}`, `x := &T{...}`, `x = T::new(`, `x = T(`, `x = new T(`.
    //
    // The word boundary is checked on the binding itself and the operator is
    // matched separately: folding the operator into the needle would demand a
    // boundary after the `=`, which the right-hand side never gives.
    if let Some(idx) = find_binding_at(line, binding) {
        let after = line[idx + binding.len()..].trim_start();
        let rhs = after
            .strip_prefix(":=")
            .or_else(|| {
                // `==` is a comparison, not a binding.
                after
                    .strip_prefix('=')
                    .filter(|rest| !rest.starts_with('='))
            })
            .map(str::trim_start);
        if let Some(rhs) = rhs {
            let rhs = rhs.strip_prefix("new ").unwrap_or(rhs);
            let ty = type_from_constructor(rhs);
            if ty.is_empty() {
                // Not a construction. It may still be a call whose return
                // type the graph knows: `let s = cg.get_stats().await?`.
                if let Some(ty) = called_function_name(rhs)
                    .and_then(|name| returns.get(&name))
                    .map(|declared| call_result_type(declared, rhs))
                {
                    if !ty.is_empty() {
                        out.inferred.push(ty);
                    }
                }
            } else {
                out.inferred.push(ty);
            }
        }
    }

    // `var x T` (Go) and `T x = ...` (Java, C#, C++).
    if let Some(rest) = strip_binding(line, &format!("var {binding}")) {
        let rest = rest.trim();
        if !rest.starts_with('=') && !rest.is_empty() {
            out.written.push(rest.to_string());
        }
    }
    if let Some(idx) = find_binding_at(line, binding) {
        let before = line[..idx].trim();
        // Exactly one word before the binding, and the line assigns or
        // declares — `Server s = ...`, `final Config c;`.
        let words: Vec<&str> = before.split_whitespace().collect();
        if let [ty] = words[..] {
            if is_identifier(&normalize_type(ty)) && !is_keyword(ty) {
                out.written.push((*ty).to_string());
            }
        }
    }
    out
}

/// Words that can precede a binding without being its type.
fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "let"
            | "const"
            | "var"
            | "final"
            | "return"
            | "if"
            | "while"
            | "for"
            | "match"
            | "case"
            | "else"
            | "new"
            | "await"
            | "yield"
            | "in"
            | "and"
            | "or"
            | "not"
    )
}

/// Byte index of `needle` in `line` when it appears as a whole word.
fn find_binding_at(line: &str, needle: &str) -> Option<usize> {
    let mut from = 0usize;
    while let Some(rel) = line[from..].find(needle) {
        let idx = from + rel;
        let before_ok = idx == 0
            || !line.as_bytes()[idx - 1].is_ascii_alphanumeric()
                && line.as_bytes()[idx - 1] != b'_';
        let after = idx + needle.len();
        let after_ok = line
            .as_bytes()
            .get(after)
            .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_');
        if before_ok && after_ok {
            return Some(idx);
        }
        from = idx + needle.len().max(1);
    }
    None
}

/// The remainder of `line` after a whole-word `head`, trimmed.
fn strip_binding<'a>(line: &'a str, head: &str) -> Option<&'a str> {
    let idx = find_binding_at(line, head)?;
    Some(line[idx + head.len()..].trim_start())
}

/// Resolves the receiver of one field site and says how it relates to
/// `qualifier`.
///
/// `index` carries the declared field types a chain like `self.inner.field` is
/// followed through, and the return types that let `let s = f()` be typed.
#[must_use]
pub fn attribute_site(
    source: &str,
    dot_byte: usize,
    qualifier: &str,
    enclosing: Option<&Enclosing>,
    index: &TypeIndex,
) -> Attribution {
    let want = bare_type_name(qualifier);
    let Some(chain) = receiver_chain(source, dot_byte) else {
        return Attribution::Unknown;
    };
    let Some((base, rest)) = chain.split_first() else {
        return Attribution::Unknown;
    };

    // `Type.field` / `Type::field` — the receiver is the type itself.
    if rest.is_empty() && base == want {
        return Attribution::Matches;
    }

    let Some(mut current) = resolve_base(source, base, enclosing, index) else {
        return Attribution::Unknown;
    };

    // Follow the rest of the chain through declared field types.
    for segment in rest.iter().take(MAX_CHAIN_HOPS) {
        match index.fields.get(&format!("{current}::{segment}")) {
            Some(next) => current.clone_from(next),
            None => return Attribution::Unknown,
        }
    }
    if rest.len() > MAX_CHAIN_HOPS {
        return Attribution::Unknown;
    }

    if current == want {
        Attribution::Matches
    } else {
        Attribution::Excludes
    }
}

/// The type of the first segment of a receiver chain.
fn resolve_base(
    source: &str,
    base: &str,
    enclosing: Option<&Enclosing>,
    index: &TypeIndex,
) -> Option<String> {
    let enclosing = enclosing?;
    let self_binding = enclosing.signature.as_deref().and_then(receiver_binding);
    if is_self_word(base) || self_binding.as_deref() == Some(base) {
        return owning_type(&enclosing.qualified_name).map(str::to_string);
    }

    // A local binding, looked for only inside the enclosing symbol's body.
    let all: Vec<&str> = source.lines().collect();
    let start = enclosing.start_line as usize;
    let end = (enclosing.end_line as usize + 1).min(all.len());
    let body = all.get(start..end)?;
    declared_type_in(body, base, &index.returns)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn enclosing(qname: &str, sig: &str, start: u32, end: u32) -> Enclosing {
        Enclosing {
            qualified_name: qname.to_string(),
            signature: Some(sig.to_string()),
            start_line: start,
            end_line: end,
        }
    }

    /// Site byte for the `.` that begins `.field` — what the caller passes.
    fn dot_of(source: &str, needle: &str) -> usize {
        source.find(needle).expect("needle in source")
    }

    #[test]
    fn a_chain_is_read_right_to_left_and_stops_at_a_non_identifier() {
        let src = "let x = self.inner.last_sync_at;";
        assert_eq!(
            receiver_chain(src, dot_of(src, ".last_sync_at")),
            Some(vec!["self".into(), "inner".into()])
        );
        // A call, an index and a literal are not typeable by a text scan.
        for src in [
            "let x = build().last_sync_at;",
            "let x = items[0].last_sync_at;",
            "let x = \"s\".last_sync_at;",
        ] {
            assert_eq!(
                receiver_chain(src, dot_of(src, ".last_sync_at")),
                None,
                "must refuse to read a receiver it cannot type: {src}"
            );
        }
    }

    #[test]
    fn a_receiver_split_across_lines_is_still_one_chain() {
        let src = "let x = self\n    .inner\n    .last_sync_at;";
        assert_eq!(
            receiver_chain(src, dot_of(src, ".last_sync_at")),
            Some(vec!["self".into(), "inner".into()])
        );
    }

    #[test]
    fn owning_type_ignores_the_file_path_segment() {
        assert_eq!(
            owning_type("src/types.rs::GraphStats::last_sync_at"),
            Some("GraphStats")
        );
        // A free function owns nothing — the segment before it is the path.
        assert_eq!(owning_type("src/types.rs::free_function"), None);
    }

    #[test]
    fn a_receiver_binding_is_read_from_go_and_rust_signatures() {
        assert_eq!(
            receiver_binding("func (s *Server) Handle(w http.ResponseWriter)"),
            Some("s".into())
        );
        assert_eq!(
            receiver_binding("fn tick(&mut self, n: u32)"),
            Some("self".into())
        );
        assert_eq!(receiver_binding("fn free(n: u32)"), None);
    }

    #[test]
    fn normalize_keeps_the_outer_constructor() {
        assert_eq!(normalize_type("&mut Vec<Scope>"), "Vec");
        assert_eq!(normalize_type("*Server"), "Server");
        assert_eq!(normalize_type("pub last_sync_at: u64"), "pub");
        // A wrapper is not the thing it wraps: `Option<Config>` must never
        // attribute a site to `Config`.
        assert_eq!(normalize_type("Option<Config>"), "Option");
    }

    #[test]
    fn a_self_receiver_takes_the_type_that_owns_the_enclosing_symbol() {
        let src = "fn tick(&mut self) {\n    self.last_sync_at = 1;\n}\n";
        let enc = enclosing("src/types.rs::GraphStats::tick", "fn tick(&mut self)", 0, 2);
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                Some(&enc),
                &TypeIndex::default()
            ),
            Attribution::Matches
        );
        // The same text inside another type's method is the narrowing working.
        let other = enclosing("src/other.rs::Cache::tick", "fn tick(&mut self)", 0, 2);
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                Some(&other),
                &TypeIndex::default()
            ),
            Attribution::Excludes
        );
    }

    #[test]
    fn a_go_receiver_binding_resolves_like_self() {
        let src = "func (s *Server) Handle() {\n    s.last_sync_at = 1\n}\n";
        let enc = enclosing(
            "pkg/server.go::Server::Handle",
            "func (s *Server) Handle()",
            0,
            2,
        );
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "Server",
                Some(&enc),
                &TypeIndex::default()
            ),
            Attribution::Matches
        );
    }

    #[test]
    fn a_local_binding_is_typed_from_its_declaration() {
        let cases = [
            (
                "fn f() {\n    let s: GraphStats = load();\n    s.last_sync_at = 1;\n}\n",
                "GraphStats",
            ),
            (
                "fn f() {\n    let s = GraphStats::new();\n    s.last_sync_at = 1;\n}\n",
                "GraphStats",
            ),
            (
                "func f() {\n    s := GraphStats{}\n    s.last_sync_at = 1\n}\n",
                "GraphStats",
            ),
            (
                "func f() {\n    var s GraphStats\n    s.last_sync_at = 1\n}\n",
                "GraphStats",
            ),
            (
                "void f() {\n    GraphStats s = load();\n    s.last_sync_at = 1;\n}\n",
                "GraphStats",
            ),
            (
                "def f():\n    s: GraphStats = load()\n    s.last_sync_at = 1\n",
                "GraphStats",
            ),
            (
                "def f():\n    s = GraphStats()\n    s.last_sync_at = 1\n",
                "GraphStats",
            ),
        ];
        for (src, ty) in cases {
            let enc = enclosing("src/a.rs::f", "fn f()", 0, 3);
            assert_eq!(
                attribute_site(
                    src,
                    dot_of(src, ".last_sync_at"),
                    ty,
                    Some(&enc),
                    &TypeIndex::default()
                ),
                Attribution::Matches,
                "must type the local in: {src}"
            );
            assert_eq!(
                attribute_site(
                    src,
                    dot_of(src, ".last_sync_at"),
                    "Other",
                    Some(&enc),
                    &TypeIndex::default()
                ),
                Attribution::Excludes,
                "and must exclude a different type for: {src}"
            );
        }
    }

    /// A local whose type is written down twice, differently, is a rebind or a
    /// misread. Either way it must not be guessed at.
    #[test]
    fn a_binding_with_two_conflicting_types_is_unknown() {
        let src = "fn f() {\n    let s: GraphStats = a();\n    let s: Cache = b();\n    s.last_sync_at = 1;\n}\n";
        let enc = enclosing("src/a.rs::f", "fn f()", 0, 4);
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                Some(&enc),
                &TypeIndex::default()
            ),
            Attribution::Unknown
        );
    }

    /// The declaration search is bounded to the enclosing symbol, so a
    /// same-named local in a neighbouring function cannot type this one.
    #[test]
    fn a_binding_declared_in_another_function_does_not_leak_in() {
        let src = "fn a() {\n    let s: GraphStats = load();\n}\n\nfn b() {\n    s.last_sync_at = 1;\n}\n";
        let enc = enclosing("src/a.rs::b", "fn b()", 4, 6);
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                Some(&enc),
                &TypeIndex::default()
            ),
            Attribution::Unknown
        );
    }

    #[test]
    fn a_chain_is_followed_through_declared_field_types() {
        let src = "fn tick(&self) {\n    self.stats.last_sync_at;\n}\n";
        let enc = enclosing("src/a.rs::Server::tick", "fn tick(&self)", 0, 2);
        let mut types = TypeIndex::default();
        types
            .fields
            .insert("Server::stats".into(), "GraphStats".into());
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                Some(&enc),
                &types
            ),
            Attribution::Matches
        );
        // Without the field's declared type there is nothing to follow.
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                Some(&enc),
                &TypeIndex::default()
            ),
            Attribution::Unknown
        );
    }

    /// `Type.field` — the receiver is the type itself, so no enclosing symbol
    /// is needed. (`Type::field` never reaches here: the site scanner looks
    /// for `.field`, so a `::` access is not a site in the first place.)
    #[test]
    fn a_call_is_typed_from_its_return_type_when_the_initializer_unwraps() {
        let src = "fn f() {\n    let s = cg.get_stats().await?;\n    s.last_sync_at = 1;\n}\n";
        let enc = enclosing("src/a.rs::f", "fn f()", 0, 3);
        let mut index = TypeIndex::default();
        index
            .returns
            .insert("get_stats".into(), "Result<GraphStats>".into());
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                Some(&enc),
                &index
            ),
            Attribution::Matches
        );
    }

    /// Without an unwrap the binding really is the wrapper, and calling it a
    /// `GraphStats` would be the wrong-answer-that-looks-right this whole
    /// module exists to avoid.
    #[test]
    fn a_wrapped_return_is_not_unwrapped_on_its_own() {
        assert_eq!(
            call_result_type("Result<GraphStats>", "cg.get_stats()"),
            "Result"
        );
        assert_eq!(
            call_result_type("Result<GraphStats>", "cg.get_stats()?"),
            "GraphStats"
        );
        assert_eq!(
            call_result_type("Result<GraphStats>", "cg.get_stats().unwrap()"),
            "GraphStats"
        );
        // Two type parameters: which side a `?` yields is not readable here.
        assert_eq!(call_result_type("Result<A, B>", "f()?"), "Result");
        assert_eq!(call_result_type("GraphStats", "make()"), "GraphStats");
    }

    #[test]
    fn a_called_function_name_is_read_off_the_head_of_an_initializer() {
        assert_eq!(
            called_function_name("cg.get_stats().await?"),
            Some("get_stats".into())
        );
        assert_eq!(
            called_function_name("crate::db::open()"),
            Some("open".into())
        );
        assert_eq!(called_function_name("GraphStats{}"), None);
        assert_eq!(called_function_name("items[0].get()"), None);
    }

    #[test]
    fn a_static_access_names_its_own_type() {
        let src = "let x = GraphStats.last_sync_at;";
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                None,
                &TypeIndex::default()
            ),
            Attribution::Matches
        );
    }

    #[test]
    fn a_constructor_call_names_the_type_not_the_constructor() {
        assert_eq!(type_from_constructor("GraphStats::new()"), "GraphStats");
        assert_eq!(type_from_constructor("GraphStats::default()"), "GraphStats");
        assert_eq!(
            type_from_constructor("crate::types::GraphStats::new()"),
            "GraphStats"
        );
        assert_eq!(type_from_constructor("GraphStats{}"), "GraphStats");
        assert_eq!(type_from_constructor("&GraphStats{}"), "GraphStats");
        // Python and Java construct with a bare call — nothing to drop.
        assert_eq!(type_from_constructor("GraphStats()"), "GraphStats");
        assert_eq!(
            type_from_constructor("crate::types::GraphStats"),
            "GraphStats"
        );
    }

    #[test]
    fn a_qualifier_matches_on_its_bare_name() {
        let src = "fn tick(&self) {\n    self.last_sync_at = 1;\n}\n";
        let enc = enclosing("src/types.rs::GraphStats::tick", "fn tick(&self)", 0, 2);
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "crate::types::GraphStats",
                Some(&enc),
                &TypeIndex::default()
            ),
            Attribution::Matches
        );
    }

    /// Outside any symbol the graph knows, there is nothing to resolve
    /// against — and that must read as unknown, never as a match.
    #[test]
    fn a_site_with_no_enclosing_symbol_is_unknown() {
        let src = "s.last_sync_at = 1;";
        assert_eq!(
            attribute_site(
                src,
                dot_of(src, ".last_sync_at"),
                "GraphStats",
                None,
                &TypeIndex::default()
            ),
            Attribution::Unknown
        );
    }
}
