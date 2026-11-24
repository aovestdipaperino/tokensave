//! Parser for SCIP symbol strings (#1100).
//!
//! Grammar (from `scip.proto`): `local <id>` for document-local symbols, or
//! `<scheme> ' ' <manager> ' ' <package> ' ' <version> ' ' <descriptors>`
//! where a double space escapes a space inside a header field and `.` marks
//! an empty field. Descriptors are `name/` (namespace), `name#` (type),
//! `name.` (term), `name:` (meta), `name!` (macro), `name(disambiguator).`
//! (method), `[name]` (type parameter) and `(name)` (parameter). A name is an
//! identifier or a backtick-quoted string in which a doubled backtick escapes
//! one.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suffix {
    Namespace,
    Type,
    Term,
    Method,
    TypeParameter,
    Parameter,
    Meta,
    Macro,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub name: String,
    pub suffix: Suffix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub scheme: String,
    pub manager: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbol {
    Local,
    Global {
        package: Package,
        descriptors: Vec<Descriptor>,
    },
}

impl Symbol {
    /// Whether the last descriptor is a type parameter (`[T]`), whose range
    /// spells the parameter rather than the item [`Symbol::primary`] names.
    #[must_use]
    pub fn last_is_type_parameter(&self) -> bool {
        matches!(
            self,
            Symbol::Global { descriptors, .. }
                if descriptors.last().is_some_and(|d| d.suffix == Suffix::TypeParameter)
        )
    }

    /// The descriptor that names the symbol itself: the last one that is not
    /// a type parameter or parameter. `None` for locals and for parameters,
    /// which have no graph node of their own.
    #[must_use]
    pub fn primary(&self) -> Option<&Descriptor> {
        match self {
            Symbol::Local => None,
            Symbol::Global { descriptors, .. } => {
                let last = descriptors.last()?;
                if matches!(last.suffix, Suffix::Parameter) {
                    return None;
                }
                descriptors
                    .iter()
                    .rev()
                    .find(|d| !matches!(d.suffix, Suffix::TypeParameter | Suffix::Parameter))
            }
        }
    }
}

/// Parses a symbol string. `None` when it does not follow the grammar.
#[must_use]
pub fn parse(symbol: &str) -> Option<Symbol> {
    if let Some(id) = symbol.strip_prefix("local ") {
        return (!id.is_empty()).then_some(Symbol::Local);
    }
    let mut rest = symbol;
    let mut fields = Vec::with_capacity(4);
    for _ in 0..4 {
        let (field, tail) = header_field(rest)?;
        fields.push(field);
        rest = tail;
    }
    let mut fields = fields.into_iter();
    let package = Package {
        scheme: fields.next()?,
        manager: fields.next()?,
        name: fields.next()?,
        version: fields.next()?,
    };
    if package.scheme.is_empty() || package.scheme == "local" {
        return None;
    }
    Some(Symbol::Global {
        package,
        descriptors: descriptors(rest)?,
    })
}

/// Reads one space-terminated header field, unescaping double spaces.
fn header_field(input: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = input.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == ' ' {
            if matches!(chars.peek(), Some((_, ' '))) {
                chars.next();
                out.push(' ');
                continue;
            }
            let field = if out == "." { String::new() } else { out };
            return Some((field, &input[i + 1..]));
        }
        out.push(c);
    }
    None
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '+' | '-' | '$')
}

/// Reads a simple or backtick-escaped name and returns it with the rest.
fn name(input: &str) -> Option<(String, &str)> {
    if let Some(body) = input.strip_prefix('`') {
        let mut out = String::new();
        let mut chars = body.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if c == '`' {
                if matches!(chars.peek(), Some((_, '`'))) {
                    chars.next();
                    out.push('`');
                    continue;
                }
                return Some((out, &body[i + 1..]));
            }
            out.push(c);
        }
        return None;
    }
    let end = input.find(|c: char| !is_ident(c)).unwrap_or(input.len());
    Some((input[..end].to_owned(), &input[end..]))
}

fn descriptors(mut input: &str) -> Option<Vec<Descriptor>> {
    let mut out = Vec::new();
    while !input.is_empty() {
        if out.len() >= 256 {
            return None;
        }
        let (descriptor, rest) = if let Some(body) = input.strip_prefix('[') {
            let (n, rest) = name(body)?;
            (
                Descriptor {
                    name: n,
                    suffix: Suffix::TypeParameter,
                },
                rest.strip_prefix(']')?,
            )
        } else if let Some(body) = input.strip_prefix('(') {
            let (n, rest) = name(body)?;
            (
                Descriptor {
                    name: n,
                    suffix: Suffix::Parameter,
                },
                rest.strip_prefix(')')?,
            )
        } else {
            let (n, rest) = name(input)?;
            let mut chars = rest.chars();
            let suffix = match chars.next()? {
                '/' => Suffix::Namespace,
                '#' => Suffix::Type,
                '.' => Suffix::Term,
                ':' => Suffix::Meta,
                '!' => Suffix::Macro,
                '(' => {
                    let close = rest.find(')')?;
                    let after = rest[close + 1..].strip_prefix('.')?;
                    out.push(Descriptor {
                        name: n,
                        suffix: Suffix::Method,
                    });
                    input = after;
                    continue;
                }
                _ => return None,
            };
            (Descriptor { name: n, suffix }, chars.as_str())
        };
        out.push(descriptor);
        input = rest;
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn primary(s: &str) -> Option<(String, Suffix)> {
        parse(s)?.primary().map(|d| (d.name.clone(), d.suffix))
    }

    #[test]
    fn parses_rust_analyzer_symbols() {
        assert_eq!(
            primary(
                "rust-analyzer cargo scip_ambiguous 0.1.0 alpha/impl#[Config][Validate]check()."
            ),
            Some(("check".into(), Suffix::Method))
        );
        assert_eq!(
            primary("rust-analyzer cargo scip_ambiguous 0.1.0 beta/Config#"),
            Some(("Config".into(), Suffix::Type))
        );
        assert_eq!(
            primary("rust-analyzer cargo scip_ambiguous 0.1.0 alpha/impl#[Config]"),
            Some(("impl".into(), Suffix::Type))
        );
        assert_eq!(
            primary("rust-analyzer cargo std https://github.com/rust-lang/rust/library/std vec/macros/vec!"),
            Some(("vec".into(), Suffix::Macro))
        );
    }

    #[test]
    fn handles_escapes_empty_fields_locals_and_parameters() {
        let Some(Symbol::Global {
            package,
            descriptors,
        }) = parse("scip-ts npm . . `a b``c`.")
        else {
            panic!("expected a global symbol");
        };
        assert_eq!(package.manager, "npm");
        assert_eq!(package.name, "");
        assert_eq!(descriptors[0].name, "a b`c");
        let Some(Symbol::Global { package, .. }) = parse("s m my  pkg 1 x.") else {
            panic!("expected a global symbol");
        };
        assert_eq!(package.name, "my pkg");
        assert_eq!(parse("local 12"), Some(Symbol::Local));
        assert!(primary("local 12").is_none());
        assert!(primary("s m p v f().(arg)").is_none());
    }

    #[test]
    fn rejects_malformed_symbols() {
        for bad in [
            "",
            "local ",
            "s m p",
            "s m p v name",
            "s m p v f(.",
            "s m p v `open",
            "s m p v [T",
        ] {
            assert!(parse(bad).is_none(), "{bad:?}");
        }
    }
}
