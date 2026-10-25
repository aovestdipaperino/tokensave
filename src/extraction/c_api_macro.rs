//! `class MYLIB_API AFoo` reparses as a `function_definition`, so class, methods and fields vanish.
//! A body or brace initializer is left alone - a `MACRO(...)` there is a call carrying edges.

use std::borrow::Cow;

/// Re-parsing a file off stored coordinates needs the extractor's bytes, not the file's.
pub(crate) fn source_for_parse<'a>(language_key: &str, source: &'a str) -> Cow<'a, str> {
    match language_key {
        "c" | "cpp" => blank_declaration_macros(source).map_or(Cow::Borrowed(source), Cow::Owned),
        _ => Cow::Borrowed(source),
    }
}

/// Blanks space-for-byte, so every offset, line and column still addresses the real file.
pub(crate) fn blank_declaration_macros(source: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let mut blanked: Option<Vec<u8>> = None;
    // One entry per open brace, `true` where a macro invocation is a call.
    let mut in_body = vec![false];
    // A record, namespace or `extern` keyword seen since the last `{`, `}` or `;`.
    let mut record_pending = false;
    let mut prev_significant: Option<u8> = None;
    let mut after_access_specifier = false;
    let mut i = 0;
    while i < bytes.len() {
        if let Some(next) = skip_non_code(bytes, i) {
            i = next;
            continue;
        }
        let byte = bytes[i];
        if byte.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        match byte {
            b'{' => {
                let record =
                    record_pending && !matches!(prev_significant, Some(b')' | b'=' | b',' | b'{'));
                in_body.push(in_body.last().copied().unwrap_or(false) || !record);
                record_pending = false;
            }
            b'}' => {
                if in_body.len() > 1 {
                    in_body.pop();
                }
                record_pending = false;
            }
            b';' => record_pending = false,
            _ => {}
        }
        if let Some(after_keyword) = record_keyword(bytes, i) {
            record_pending = true;
            if let Some(span) = api_macro_span(bytes, after_keyword) {
                blank(bytes, &mut blanked, span);
            }
            prev_significant = Some(bytes[after_keyword - 1]);
            i = after_keyword;
            continue;
        }
        if !in_body.last().copied().unwrap_or(false)
            && matches!(prev_significant, None | Some(b';' | b'{' | b'}' | b':'))
        {
            if let Some(span) = attribute_macro_span(bytes, i) {
                blank(bytes, &mut blanked, span);
                i = span.1;
                continue;
            }
        }
        let end = ident_end(bytes, i).unwrap_or(i + 1);
        let word = &bytes[i..end];
        if SCOPE_KEYWORDS.contains(&word) {
            record_pending = true;
        }
        // Only an access label ends a declaration; `: MD(MD)` and `Foo::Bar` do not, and reading
        // either as one blanks a member initializer or a qualified call.
        prev_significant = if word == b":" && !after_access_specifier {
            Some(NOT_A_BOUNDARY)
        } else {
            Some(bytes[end - 1])
        };
        after_access_specifier = ACCESS_SPECIFIERS.contains(&word);
        i = end;
    }
    blanked.map(|out| String::from_utf8(out).unwrap_or_else(|_| source.to_string()))
}

fn blank(bytes: &[u8], blanked: &mut Option<Vec<u8>>, (start, end): (usize, usize)) {
    let out = blanked.get_or_insert_with(|| bytes.to_vec());
    for byte in &mut out[start..end] {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

/// A directive spells its own macros, so `#define UPROPERTY(...)` must survive.
fn skip_non_code(bytes: &[u8], i: usize) -> Option<usize> {
    match (bytes[i], bytes.get(i + 1)) {
        (b'/', Some(b'/')) => {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            Some(j)
        }
        (b'/', Some(b'*')) => {
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            Some((j + 2).min(bytes.len()))
        }
        // `1'000` separates digits; `L'a'` quotes one.
        (b'\'', _) if is_digit_separator(bytes, i) => Some(i + 1),
        (b'"', _) if opens_raw_string(bytes, i) => Some(skip_raw_string(bytes, i)),
        (quote @ (b'"' | b'\''), _) => Some(skip_quoted(bytes, i, quote)),
        // Continues while the line ends in a backslash.
        (b'#', _) => {
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\n' {
                    let continued = bytes[..j]
                        .iter()
                        .rposition(|b| !b.is_ascii_whitespace())
                        .is_some_and(|last| bytes[last] == b'\\');
                    if !continued {
                        break;
                    }
                }
                j += 1;
            }
            Some(j)
        }
        _ => None,
    }
}

fn skip_quoted(bytes: &[u8], open: usize, quote: u8) -> usize {
    let mut j = open + 1;
    while j < bytes.len() && bytes[j] != quote {
        j += if bytes[j] == b'\\' { 2 } else { 1 };
    }
    (j + 1).min(bytes.len())
}

/// Only inside a NUMBER - `u8'x'` wears the same shape and is a character literal.
fn is_digit_separator(bytes: &[u8], quote: usize) -> bool {
    if quote == 0 || !bytes[quote - 1].is_ascii_alphanumeric() {
        return false;
    }
    if !bytes
        .get(quote + 1)
        .copied()
        .is_some_and(|b| b.is_ascii_alphanumeric())
    {
        return false;
    }
    let mut start = quote - 1;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    bytes[start].is_ascii_digit()
}

/// `R"tag(...)tag"` holds unescaped quotes, so the plain scan ends the literal early and every
/// macro after it in the file goes unblanked.
fn opens_raw_string(bytes: &[u8], quote: usize) -> bool {
    if quote == 0 || bytes[quote - 1] != b'R' {
        return false;
    }
    let mut start = quote - 1;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    matches!(&bytes[start..quote], b"R" | b"LR" | b"uR" | b"UR" | b"u8R")
}

fn skip_raw_string(bytes: &[u8], quote: usize) -> usize {
    const MAX_DELIMITER: usize = 16;

    let Some(open) = bytes[quote + 1..]
        .iter()
        .position(|b| *b == b'(')
        .map(|p| quote + 1 + p)
    else {
        return skip_quoted(bytes, quote, b'"');
    };
    let delimiter = &bytes[quote + 1..open];
    if delimiter.len() > MAX_DELIMITER || delimiter.iter().any(|b| !b.is_ascii_graphic()) {
        return skip_quoted(bytes, quote, b'"');
    }
    let mut j = open + 1;
    while j < bytes.len() {
        if bytes[j] == b')'
            && bytes[j + 1..].starts_with(delimiter)
            && bytes.get(j + 1 + delimiter.len()) == Some(&b'"')
        {
            return j + delimiter.len() + 2;
        }
        j += 1;
    }
    bytes.len()
}

/// A `{` after one of these opens a scope holding declarations, not code. `class`, `struct` and
/// `union` reach `record_pending` through [`record_keyword`] instead.
const SCOPE_KEYWORDS: [&[u8]; 3] = [b"namespace", b"enum", b"extern"];

const ACCESS_SPECIFIERS: [&[u8]; 3] = [b"public", b"private", b"protected"];

/// Stands in for punctuation that ends no declaration, so the boundary test below refuses it.
const NOT_A_BOUNDARY: u8 = b'^';

fn record_keyword(bytes: &[u8], i: usize) -> Option<usize> {
    if i > 0 && is_ident_byte(bytes[i - 1]) {
        return None;
    }
    let end = ["class", "struct", "union"]
        .iter()
        .find(|kw| bytes[i..].starts_with(kw.as_bytes()))
        .map(|kw| i + kw.len())?;
    (!bytes.get(end).copied().is_some_and(is_ident_byte)).then_some(end)
}

/// Identifier after it is what tells `class FOO_API Bar` from a caps-named class.
fn api_macro_span(bytes: &[u8], from: usize) -> Option<(usize, usize)> {
    let start = skip_space(bytes, from);
    let name_end = ident_end(bytes, start)?;
    if !is_macro_name(&bytes[start..name_end]) {
        return None;
    }
    let mut end = name_end;
    if bytes.get(skip_space(bytes, end)) == Some(&b'(') {
        end = closing_paren(bytes, skip_space(bytes, end))?;
    }
    let after = skip_space(bytes, end);
    let starts_name = bytes
        .get(after)
        .copied()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_');
    starts_name.then_some((start, end))
}

const TYPE_KEYWORDS: [&str; 17] = [
    "class", "struct", "union", "enum", "void", "bool", "char", "int", "short", "long", "float",
    "double", "unsigned", "signed", "auto", "template", "typename",
];
const SPECIFIERS: [&str; 10] = [
    "const",
    "volatile",
    "constexpr",
    "consteval",
    "static",
    "inline",
    "virtual",
    "mutable",
    "extern",
    "thread_local",
];

/// A `{` or a second `(` after it means the macro IS the declaration - gtest `TEST(S, C) {}` and
/// COM `STDMETHOD(Read)(args) {}` both keep theirs. Paren-less needs a type after it, since
/// `HANDLE h;` wears the same shape.
fn attribute_macro_span(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    let name_end = ident_end(bytes, i)?;
    if !is_macro_name(&bytes[i..name_end]) {
        return None;
    }
    let after_name = skip_space(bytes, name_end);
    if bytes.get(after_name) != Some(&b'(') {
        let keyword_end = ident_end(bytes, after_name)?;
        let keyword = &bytes[after_name..keyword_end];
        if TYPE_KEYWORDS.iter().any(|kw| kw.as_bytes() == keyword) {
            return Some((i, name_end));
        }
        if !SPECIFIERS.iter().any(|kw| kw.as_bytes() == keyword) {
            return None;
        }
        return names_a_type(bytes, keyword_end).then_some((i, name_end));
    }
    let end = closing_paren(bytes, after_name)?;
    if paren_group_holds_a_body(bytes, after_name, end) {
        return None;
    }
    (!matches!(bytes.get(skip_space(bytes, end)), Some(b'{' | b'('))).then_some((i, end))
}

/// A brace inside the argument list is a lambda or an initializer list - real code, and blanking it
/// deletes the calls in it.
fn paren_group_holds_a_body(bytes: &[u8], open: usize, close: usize) -> bool {
    let mut i = open;
    while i < close {
        if let Some(next) = skip_non_code(bytes, i) {
            i = next;
            continue;
        }
        if bytes[i] == b'{' {
            return true;
        }
        i += 1;
    }
    false
}

/// `MYLIB_API const int x` still names a type past the specifier; `HANDLE const h` has only the
/// declarator left, so the caps word IS the type and blanking it deletes the declaration.
fn names_a_type(bytes: &[u8], from: usize) -> bool {
    let mut identifiers = 0u32;
    let mut i = from;
    while i < bytes.len() {
        i = skip_space(bytes, i);
        if matches!(
            bytes.get(i),
            None | Some(b';' | b'=' | b'(' | b'{' | b',' | b'[')
        ) {
            break;
        }
        let Some(end) = ident_end(bytes, i) else {
            i += 1;
            continue;
        };
        let word = &bytes[i..end];
        if TYPE_KEYWORDS.iter().any(|kw| kw.as_bytes() == word) {
            return true;
        }
        if !SPECIFIERS.iter().any(|kw| kw.as_bytes() == word) {
            identifiers += 1;
        }
        i = end;
    }
    identifiers >= 2
}

/// Two bytes minimum, so `template <class T>` keeps its letter; lowercase `__declspec` parses already.
fn is_macro_name(name: &[u8]) -> bool {
    name.len() >= 2
        && name[0].is_ascii_uppercase()
        && name
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'_')
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn skip_space(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
        } else if let Some(next) = skip_comment(bytes, i) {
            i = next;
        } else {
            break;
        }
    }
    i
}

fn skip_comment(bytes: &[u8], i: usize) -> Option<usize> {
    matches!((bytes[i], bytes.get(i + 1)), (b'/', Some(b'/' | b'*')))
        .then(|| skip_non_code(bytes, i))
        .flatten()
}

fn ident_end(bytes: &[u8], start: usize) -> Option<usize> {
    if !bytes
        .get(start)
        .copied()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
    {
        return None;
    }
    let mut end = start;
    while bytes.get(end).copied().is_some_and(is_ident_byte) {
        end += 1;
    }
    Some(end)
}

fn closing_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        if let Some(next) = skip_non_code(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blanked(source: &str) -> String {
        blank_declaration_macros(source).unwrap_or_else(|| source.to_string())
    }

    #[test]
    fn blanks_the_class_macro_and_keeps_every_offset() {
        let source = "class MYLIB_API AFoo : public ABar {};";
        let out = blanked(source);
        assert_eq!(out, "class           AFoo : public ABar {};");
        assert_eq!(out.len(), source.len());
    }

    #[test]
    fn blanks_a_class_macro_that_takes_arguments() {
        let source = "struct COMPONENT_EXPORT(BASE) Foo { int n; };";
        assert_eq!(
            blanked(source),
            "struct                        Foo { int n; };"
        );
    }

    #[test]
    fn blanks_reflection_attributes_around_members() {
        let source = concat!(
            "UCLASS()
class AFoo {
  GENERATED_BODY()
",
            "public:
  UPROPERTY(Replicated)
  int N;
};"
        );
        let out = blanked(source);
        for macro_name in ["UCLASS", "GENERATED_BODY", "UPROPERTY"] {
            assert!(!out.contains(macro_name), "{out}");
        }
        assert!(out.contains("class AFoo {"), "{out}");
        assert!(out.contains("  int N;"), "{out}");
        assert_eq!(out.len(), source.len());
        assert_eq!(out.lines().count(), source.lines().count());
    }

    #[test]
    fn blanks_an_export_macro_in_front_of_a_free_function() {
        assert_eq!(
            blanked("MYLIB_API void Reset(int n);"),
            "          void Reset(int n);"
        );
    }

    #[test]
    fn leaves_a_call_inside_a_body_alone() {
        let source = "void Go() {\n  UE_LOG(LogX, TEXT(\"%d\"), GetCount());\n}";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn leaves_a_brace_initializer_alone() {
        let source = "static const Row Table[] = {\n  ROW(1),\n  ROW(2),\n};";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn leaves_a_macro_that_owns_its_body_alone() {
        let source = "TEST(Suite, Case) {\n  int n = 0;\n}";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn leaves_a_call_in_a_body_opened_past_a_trailing_specifier_alone() {
        for source in [
            "void TearDown() override {\n  EXPECT_EQ(1, n_);\n}",
            "int Size() const {\n  return COUNT_OF(items_);\n}",
            "void Run() noexcept {\n  CHECK(ok_);\n}",
            "auto Get() -> int {\n  return VALUE_OF(x_);\n}",
            "class Fixture {\n  void SetUp() override {\n    EXPECT_EQ(1, n_);\n  }\n};",
        ] {
            assert!(
                blank_declaration_macros(source).is_none(),
                "rewrote: {source}"
            );
        }
    }

    #[test]
    fn leaves_a_member_initializer_alone() {
        for source in [
            "Trace::Trace(Dispatcher& md) : MD(md), n_(0) {}",
            "class Trace {\n  Trace(Dispatcher& md) : MD(md) {}\n};",
        ] {
            assert!(
                blank_declaration_macros(source).is_none(),
                "rewrote: {source}"
            );
        }
    }

    #[test]
    fn leaves_a_macro_that_declares_the_method_alone() {
        let source = "class Stream : public IStream {\n  STDMETHOD(Read)(void* p) override { return S_OK; }\n};";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn leaves_a_lambda_argument_alone() {
        let source = "WI_HEADER_INITIALIZATION_FUNCTION(Init, [] { Reset(); return 1; });";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn blanks_a_member_attribute_after_an_access_label() {
        let source = "class AFoo {\npublic:\n  UPROPERTY(Replicated)\n  int N;\n};";
        let out = blanked(source);
        assert!(!out.contains("UPROPERTY"), "{out}");
        assert_eq!(out.len(), source.len());
    }

    #[test]
    fn blanks_an_export_macro_inside_a_namespace_or_extern_block() {
        for (source, expected) in [
            (
                "namespace ns {\nMYLIB_API void Reset(int n);\n}",
                "namespace ns {\n          void Reset(int n);\n}",
            ),
            (
                "extern \"C\" {\nMYLIB_API void Reset(int n);\n}",
                "extern \"C\" {\n          void Reset(int n);\n}",
            ),
        ] {
            assert_eq!(blanked(source), expected);
        }
    }

    #[test]
    fn leaves_a_caps_type_declaration_alone() {
        let source = "HANDLE process;\nDWORD count = 0;";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn leaves_a_caps_type_behind_a_qualifier_alone() {
        for source in [
            "HANDLE const process = 0;",
            "HANDLE volatile process;",
            "DWORD static count;",
            "HANDLE const* const process = 0;",
        ] {
            assert!(
                blank_declaration_macros(source).is_none(),
                "rewrote: {source}"
            );
        }
    }

    #[test]
    fn blanks_an_export_macro_that_still_names_a_type() {
        assert_eq!(
            blanked("MYLIB_API const int kLimit = 4;"),
            "          const int kLimit = 4;"
        );
        assert_eq!(
            blanked("MYLIB_API static Registry gRegistry;"),
            "          static Registry gRegistry;"
        );
    }

    #[test]
    fn a_raw_string_does_not_end_the_scan_early() {
        let source = concat!(
            "const char* kPattern = R\"(a\")\";\n",
            "class MYLIB_API AFoo { int n; };"
        );
        let out = blanked(source);
        assert!(out.contains("class           AFoo"), "{out}");
        assert!(out.contains("R\"(a\")\""), "{out}");
        assert_eq!(out.len(), source.len());
    }

    #[test]
    fn a_tagged_raw_string_keeps_its_own_quotes() {
        let source = concat!(
            "const char* kJson = R\"json({\"k\": [1)\"]})json\";\n",
            "class MYLIB_API ABar { int n; };"
        );
        let out = blanked(source);
        assert!(out.contains("class           ABar"), "{out}");
        assert!(out.contains("R\"json("), "{out}");
    }

    #[test]
    fn a_digit_separator_does_not_open_a_character_literal() {
        let source = "static const int kBig = 1'000;\nclass MYLIB_API ABaz { int n; };";
        let out = blanked(source);
        assert!(out.contains("class           ABaz"), "{out}");
        assert!(out.contains("1'000"), "{out}");
    }

    #[test]
    fn a_character_literal_after_a_prefix_is_still_a_literal() {
        let source = "static const char kQuote = u8'\"';\nclass Plain { int n; };";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn leaves_a_plain_declaration_untouched() {
        for source in [
            "class Foo {};",
            "template <class T> struct Holder { T v; };",
            "class __declspec(dllexport) Foo {};",
            "enum class EState : uint8 { Idle };",
            "struct alignas(16) Vec4 { float v[4]; };",
            "class ALLCAPS {};",
            "union U { int a; float b; };",
        ] {
            assert!(
                blank_declaration_macros(source).is_none(),
                "rewrote: {source}"
            );
        }
    }

    #[test]
    fn leaves_a_directive_defining_the_macro_untouched() {
        let source = "#define GENERATED_BODY() int x;\n#define API_MACRO \\\n  MYLIB_API\n";
        assert!(blank_declaration_macros(source).is_none(), "{source}");
    }

    #[test]
    fn ignores_a_keyword_inside_a_comment_or_string() {
        let source = "// class MYLIB_API AFoo\nconst char* s = \"class MYLIB_API AFoo\";";
        assert!(blank_declaration_macros(source).is_none());
    }

    #[test]
    fn ignores_a_word_merely_ending_in_class() {
        assert!(blank_declaration_macros("int subclass FOO_API bar;").is_none());
    }

    #[test]
    fn blanks_a_forward_declaration_and_a_multiline_macro_list() {
        let source = "class MYLIB_API AFoo;\nclass EXPORT_MACRO(\n  Module\n) ABar {};";
        let out = blanked(source);
        assert!(out.contains("class           AFoo;"), "{out}");
        assert!(out.contains("ABar {};"), "{out}");
        assert!(!out.contains("EXPORT_MACRO"), "{out}");
        assert_eq!(out.lines().count(), source.lines().count());
        assert_eq!(out.len(), source.len());
    }
}
