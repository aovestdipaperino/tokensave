//! Engine-neutral search intents rendered to FTS5 or Tantivy query syntax.
//!
//! Every scenario and benchmark query is written once as a [`Q`] and rendered
//! for each engine, so both engines receive the same intent. Text inside a
//! term is always quoted, as production's `to_fts_match_query` does, so
//! punctuation (`data-api`, `a::b`) is tokenized rather than parsed as
//! operators by either engine.
//!
//! A construct that the target engine cannot express is returned as
//! [`Unsupported`] instead of being rewritten into a different query, unless
//! the caller explicitly asks for a [`PrefixPolicy`] that degrades it. That
//! keeps semantic substitutions visible in every report.

use std::fmt;

#[derive(Clone, Debug)]
pub enum Q {
    /// Quoted term; several tokens become a phrase in both engines.
    Term(String),
    /// Quoted prefix term (`"text"*`), production's per-word form.
    Prefix(String),
    And(Vec<Q>),
    Or(Vec<Q>),
    /// `left NOT right`.
    Not(Box<Q>, Box<Q>),
    /// Restrict a term or phrase to one indexed column.
    Column(&'static str, Box<Q>),
}

impl Q {
    pub fn term(s: &str) -> Self {
        Q::Term(s.to_string())
    }

    pub fn prefix(s: &str) -> Self {
        Q::Prefix(s.to_string())
    }

    pub fn and(items: Vec<Q>) -> Self {
        Q::And(items)
    }

    pub fn or(items: Vec<Q>) -> Self {
        Q::Or(items)
    }

    pub fn not(left: Q, right: Q) -> Self {
        Q::Not(Box::new(left), Box::new(right))
    }

    /// Production `to_fts_match_query`: every whitespace word becomes a
    /// quoted prefix term, joined with `OR`.
    pub fn production_match(raw: &str) -> Option<Self> {
        let words: Vec<Q> = raw
            .split_whitespace()
            .map(sanitize)
            .filter(|w| !w.is_empty())
            .map(Q::Prefix)
            .collect();
        match words.len() {
            0 => None,
            1 => words.into_iter().next(),
            _ => Some(Q::Or(words)),
        }
    }

    /// Production `to_fts_phrase_query`: the words as one exact phrase, only
    /// for multi-word input.
    pub fn production_phrase(raw: &str) -> Option<Self> {
        let words: Vec<String> = raw
            .split_whitespace()
            .map(sanitize)
            .filter(|w| !w.is_empty())
            .collect();
        (words.len() >= 2).then(|| Q::Term(words.join(" ")))
    }

    /// Same as [`Q::production_match`] without the prefix operator.
    pub fn exact_match(raw: &str) -> Option<Self> {
        let words: Vec<Q> = raw
            .split_whitespace()
            .map(sanitize)
            .filter(|w| !w.is_empty())
            .map(Q::Term)
            .collect();
        match words.len() {
            0 => None,
            1 => words.into_iter().next(),
            _ => Some(Q::Or(words)),
        }
    }
}

/// Strips the characters that would end a quoted string in either grammar.
pub fn sanitize(s: &str) -> String {
    s.chars().filter(|c| *c != '"' && *c != '\\').collect()
}

/// Tokens as Tantivy's `default`/`simple` tokenizers split them: runs of
/// `char::is_alphanumeric`.
pub fn tantivy_tokens(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported(pub String);

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How a Tantivy rendering treats a prefix term that tokenizes to a single
/// token, which Tantivy's query parser cannot express.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixPolicy {
    /// Report it as unsupported (default; no silent substitution).
    Strict,
    /// Replace it with an exact term. Results are labeled "degraded".
    DegradeToExact,
    /// Send it as a plain term to an n-gram index, where any substring
    /// matches. Results are labeled "ngram".
    NgramSubstring,
}

pub fn render_fts5(q: &Q) -> String {
    match q {
        Q::Term(t) => format!("\"{}\"", sanitize(t)),
        Q::Prefix(t) => format!("\"{}\"*", sanitize(t)),
        Q::And(v) => join(v.iter().map(render_fts5), " AND "),
        Q::Or(v) => join(v.iter().map(render_fts5), " OR "),
        Q::Not(a, b) => format!("({} NOT {})", render_fts5(a), render_fts5(b)),
        Q::Column(c, inner) => format!("{c} : {}", render_fts5(inner)),
    }
}

pub fn render_tantivy(q: &Q, policy: PrefixPolicy) -> Result<String, Unsupported> {
    Ok(match q {
        Q::Term(t) => {
            let t = sanitize(t);
            if tantivy_tokens(&t).is_empty() {
                return Err(Unsupported(format!(
                    "term {t:?} has no Tantivy tokens (an empty phrase is a parse error)"
                )));
            }
            format!("\"{t}\"")
        }
        Q::Prefix(t) => {
            let t = sanitize(t);
            match (tantivy_tokens(&t).len(), policy) {
                (0, _) => {
                    return Err(Unsupported(format!("prefix {t:?} has no Tantivy tokens")));
                }
                (1, PrefixPolicy::Strict) => {
                    return Err(Unsupported(
                        "single-token prefix query: Tantivy's query parser accepts \"a b\"* \
                         (phrase prefix, >= 2 tokens) only; \"a\"* is a parse error and a* \
                         silently matches nothing"
                            .into(),
                    ));
                }
                (1, PrefixPolicy::DegradeToExact | PrefixPolicy::NgramSubstring) => {
                    format!("\"{t}\"")
                }
                (_, PrefixPolicy::NgramSubstring) => format!("\"{t}\""),
                _ => format!("\"{t}\"*"),
            }
        }
        Q::And(v) => join_result(v, " AND ", policy)?,
        Q::Or(v) => join_result(v, " OR ", policy)?,
        // Tantivy's `a AND NOT b` makes `NOT b` a pure-negative clause, which
        // matches no document, so the whole query returns nothing. Required
        // and prohibited clauses (`+a -b`) express FTS5's binary NOT.
        Q::Not(a, b) => format!(
            "(+{} -{})",
            render_tantivy(a, policy)?,
            render_tantivy(b, policy)?
        ),
        Q::Column(c, inner) => format!("{c}:{}", render_tantivy(inner, policy)?),
    })
}

fn join(parts: impl Iterator<Item = String>, sep: &str) -> String {
    let parts: Vec<String> = parts.collect();
    if parts.len() == 1 {
        return parts.into_iter().next().unwrap_or_default();
    }
    format!("({})", parts.join(sep))
}

fn join_result(v: &[Q], sep: &str, policy: PrefixPolicy) -> Result<String, Unsupported> {
    let parts = v
        .iter()
        .map(|q| render_tantivy(q, policy))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(join(parts.into_iter(), sep))
}
