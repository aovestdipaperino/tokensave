//! Symbol-name resolution shared by `tokensave_body` (#651) and the traversal
//! tools that accept a `symbol` in place of a `node_id` (#669):
//! `tokensave_callers`, `tokensave_callees`, `tokensave_impact` and
//! `tokensave_type_hierarchy`.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde_json::{json, Map, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::{Node, NodeKind};

/// Default and maximum number of matches a traversal tool answers for a
/// `symbol` that names several definitions (#669).
const MAX_MATCHES_DEFAULT: usize = 3;
const MAX_MATCHES_MAX: usize = 20;

/// Default and maximum number of results reported per match by callers,
/// callees and impact (#669).
pub(super) const RESULT_LIMIT_DEFAULT: usize = 50;
const RESULT_LIMIT_MAX: usize = 500;

/// Search hits consulted to order several matches the way `tokensave_search`
/// ranks them.
const SEARCH_RANK_WINDOW: usize = 100;

/// Search hits listed when a `symbol` matches nothing by name.
const SUGGESTION_COUNT: usize = 5;

/// Resolves `symbol` to nodes: the literal qualified-name / name lookup
/// first, then a member-path match that treats `.` and `::` alike, so
/// `Type.Member`, `Ns.Type.Member` and `Type::Member` all work regardless of
/// whether the language stores a namespace as `App.Tests` or `App::Tests`.
pub(super) async fn resolve_symbol_nodes(cg: &TokenSave, symbol: &str) -> Result<Vec<Node>> {
    let literal = cg.get_nodes_by_qualified_name(symbol).await?;
    if !literal.is_empty() {
        return Ok(literal);
    }
    let wanted = member_path_segments(symbol);
    let Some(last) = wanted.last().filter(|_| wanted.len() >= 2) else {
        return Ok(literal);
    };
    let by_name = cg.get_nodes_by_name(last).await?;
    Ok(by_name
        .into_iter()
        .filter(|n| member_path_segments(&n.qualified_name).ends_with(&wanted))
        .collect())
}

/// Splits a qualified name on both `::` and `.` into non-empty segments.
pub(super) fn member_path_segments(name: &str) -> Vec<&str> {
    name.split("::")
        .flat_map(|part| part.split('.'))
        .filter(|seg| !seg.is_empty())
        .collect()
}

/// A node's qualified name without the leading file-path segment(s) the
/// extractors prepend, e.g. `src/A.cs::src/A.cs::App::A::Run` → `App::A::Run`.
pub(super) fn display_qualified_name(n: &Node) -> &str {
    let prefix = format!("{}::", n.file_path);
    let mut qn = n.qualified_name.as_str();
    while let Some(rest) = qn.strip_prefix(prefix.as_str()) {
        qn = rest;
    }
    qn
}

/// Ordering key used by `handle_body` to choose between same-named symbols.
/// Lower number = higher preference (sorted ascending). Callable kinds rank
/// best because the user almost always asks for "show me the body of X"
/// expecting a function or method; type definitions are next; fields,
/// variants, use statements come last.
pub(super) fn body_kind_preference(kind: &NodeKind) -> u8 {
    match kind {
        NodeKind::Function
        | NodeKind::Method
        | NodeKind::SingletonMethod
        | NodeKind::StructMethod
        | NodeKind::Constructor
        | NodeKind::AbstractMethod
        | NodeKind::ArrowFunction
        | NodeKind::Procedure => 0,
        NodeKind::Struct
        | NodeKind::Enum
        | NodeKind::Trait
        | NodeKind::Class
        | NodeKind::InnerClass
        | NodeKind::Interface
        | NodeKind::InterfaceType
        | NodeKind::Record
        | NodeKind::CaseClass
        | NodeKind::DataClass
        | NodeKind::SealedClass
        | NodeKind::TypeAlias
        | NodeKind::Union
        | NodeKind::Typedef => 1,
        NodeKind::Impl => 2,
        NodeKind::Const | NodeKind::Static | NodeKind::Macro | NodeKind::PreprocessorDef => 3,
        NodeKind::Field
        | NodeKind::ValField
        | NodeKind::VarField
        | NodeKind::Property
        | NodeKind::CSharpProperty
        | NodeKind::EnumVariant => 4,
        NodeKind::Use | NodeKind::Include => 5,
        _ => 6,
    }
}

/// Which kinds win when a `symbol` names definitions of several kinds. Only
/// the best tier present is answered.
#[derive(Clone, Copy)]
pub(super) enum KindRanking {
    /// Callers, callees, impact: a function and a type rank together, ahead
    /// of a same-named field, constant or import, so a class and its
    /// same-named constructor are both answered.
    Definition,
    /// Type hierarchy: a type outranks a same-named function, since only a
    /// type has implementors.
    TypeFirst,
}

impl KindRanking {
    fn rank(self, kind: &NodeKind) -> u8 {
        let pref = body_kind_preference(kind);
        match self {
            Self::Definition => pref.max(1),
            Self::TypeFirst => match pref {
                0 => 1,
                1 => 0,
                other => other,
            },
        }
    }
}

/// Nodes sharing one qualified name. Overloads are answered together.
pub(super) struct MatchGroup {
    /// Never empty; sorted by file and line.
    pub(super) nodes: Vec<Node>,
}

impl MatchGroup {
    fn head(&self) -> &Node {
        &self.nodes[0]
    }

    /// Ids of every node in the group.
    pub(super) fn ids(&self) -> Vec<String> {
        self.nodes.iter().map(|n| n.id.clone()).collect()
    }

    /// The group's heading: qualified name, kind, file, line and node id,
    /// plus every overload's id and location when there are several.
    pub(super) fn header(&self) -> Map<String, Value> {
        let n = self.head();
        let mut m = Map::new();
        m.insert(
            "qualified_name".to_string(),
            json!(display_qualified_name(n)),
        );
        m.insert("kind".to_string(), json!(n.kind.as_str()));
        m.insert("file".to_string(), json!(n.file_path));
        m.insert("line".to_string(), json!(super::display_line(n.start_line)));
        m.insert(
            "end_line".to_string(),
            json!(super::display_line(n.end_line)),
        );
        m.insert("node_id".to_string(), json!(n.id));
        if self.nodes.len() > 1 {
            let overloads: Vec<Value> = self
                .nodes
                .iter()
                .map(|o| {
                    json!({
                        "node_id": o.id,
                        "file": o.file_path,
                        "line": super::display_line(o.start_line),
                        "end_line": super::display_line(o.end_line),
                    })
                })
                .collect();
            m.insert("overloads".to_string(), Value::Array(overloads));
        }
        m
    }

    /// The heading as one text line: `App::A::Run (method) src/A.cs:5-9 id=…`.
    pub(super) fn header_line(&self) -> String {
        let n = self.head();
        let mut line = node_line(n);
        if self.nodes.len() > 1 {
            let others: Vec<String> = self.nodes[1..]
                .iter()
                .map(|o| {
                    format!(
                        "{}:{}-{} id={}",
                        o.file_path,
                        super::display_line(o.start_line),
                        super::display_line(o.end_line),
                        o.id
                    )
                })
                .collect();
            let _ = write!(line, " (overloads: {})", others.join(", "));
        }
        line
    }
}

/// One node as a text line: qualified name, kind, file, line range and id.
fn node_line(n: &Node) -> String {
    format!(
        "{} ({}) {}:{}-{} id={}",
        display_qualified_name(n),
        n.kind.as_str(),
        n.file_path,
        super::display_line(n.start_line),
        super::display_line(n.end_line),
        n.id
    )
}

/// One node's heading fields, without results.
fn node_header(n: &Node) -> Value {
    json!({
        "qualified_name": display_qualified_name(n),
        "kind": n.kind.as_str(),
        "file": n.file_path,
        "line": super::display_line(n.start_line),
        "end_line": super::display_line(n.end_line),
        "node_id": n.id,
    })
}

/// What a traversal tool was asked about.
pub(super) enum Target {
    /// An explicit `node_id` (or a `symbol` that is itself a node id): the
    /// tool answers with its single-node output.
    NodeId(String),
    /// A `symbol` name, resolved to zero or more match groups.
    Symbol(SymbolMatches),
}

/// A `symbol` resolved to match groups, in `tokensave_search` rank order.
pub(super) struct SymbolMatches {
    pub(super) symbol: String,
    /// The first `max_matches` groups; the tool answers each.
    pub(super) answered: Vec<MatchGroup>,
    /// Groups past `max_matches`, listed without results.
    pub(super) unanswered: Vec<MatchGroup>,
    max_matches: usize,
    /// Closest search hits when nothing matched by name; listed, never
    /// answered, since a fuzzy hit is not what the caller named.
    suggestions: Vec<Node>,
}

/// Reads `node_id` (alias `id`) or `symbol` from `args`. `node_id` wins when
/// both are given; neither is an error.
pub(super) async fn resolve_target(
    cg: &TokenSave,
    args: &Value,
    scope_prefix: Option<&str>,
    ranking: KindRanking,
) -> Result<Target> {
    let id = ["node_id", "id"]
        .iter()
        .find_map(|key| args.get(*key).and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty());
    if let Some(id) = id {
        return Ok(Target::NodeId(id.to_string()));
    }
    let symbol = args
        .get("symbol")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: symbol (or node_id)".to_string(),
        })?;
    let max_matches =
        args.get("max_matches")
            .and_then(Value::as_u64)
            .map_or(MAX_MATCHES_DEFAULT, |v| {
                usize::try_from(v)
                    .unwrap_or(MAX_MATCHES_MAX)
                    .clamp(1, MAX_MATCHES_MAX)
            });

    let mut nodes =
        super::filter_by_scope(resolve_symbol_nodes(cg, symbol).await?, scope_prefix, |n| {
            &n.file_path
        });
    let mut seen: HashSet<String> = HashSet::new();
    nodes.retain(|n| seen.insert(n.id.clone()));

    if nodes.is_empty() {
        // The CLI binds a positional argument to `symbol`, so
        // `tokensave tool callers <node-id>` keeps working.
        if let Some(node) = cg.get_node(symbol).await? {
            return Ok(Target::NodeId(node.id));
        }
        let hits = cg.search(symbol, SUGGESTION_COUNT).await?;
        let suggestions = super::filter_by_scope(hits, scope_prefix, |r| &r.node.file_path)
            .into_iter()
            .map(|r| r.node)
            .take(SUGGESTION_COUNT)
            .collect();
        return Ok(Target::Symbol(SymbolMatches {
            symbol: symbol.to_string(),
            answered: Vec::new(),
            unanswered: Vec::new(),
            max_matches,
            suggestions,
        }));
    }

    if let Some(best) = nodes.iter().map(|n| ranking.rank(&n.kind)).min() {
        nodes.retain(|n| ranking.rank(&n.kind) == best);
    }

    // Overloads share a qualified name and form one group.
    let mut groups: Vec<MatchGroup> = Vec::new();
    let mut by_qname: HashMap<String, usize> = HashMap::new();
    for node in nodes {
        if let Some(&idx) = by_qname.get(&node.qualified_name) {
            groups[idx].nodes.push(node);
        } else {
            by_qname.insert(node.qualified_name.clone(), groups.len());
            groups.push(MatchGroup { nodes: vec![node] });
        }
    }
    for group in &mut groups {
        group
            .nodes
            .sort_by(|a, b| (&a.file_path, a.start_line).cmp(&(&b.file_path, b.start_line)));
    }

    // Order several groups as `tokensave_search` ranks them; nodes it does
    // not return go last, by file and line.
    if groups.len() > 1 {
        let ranks: HashMap<String, usize> = cg
            .search(symbol, SEARCH_RANK_WINDOW)
            .await?
            .into_iter()
            .enumerate()
            .map(|(i, r)| (r.node.id, i))
            .collect();
        groups.sort_by_cached_key(|g| {
            let rank = g
                .nodes
                .iter()
                .filter_map(|n| ranks.get(&n.id).copied())
                .min()
                .unwrap_or(usize::MAX);
            let head = g.head();
            (rank, head.file_path.clone(), head.start_line)
        });
    }
    let unanswered = groups.split_off(groups.len().min(max_matches));
    Ok(Target::Symbol(SymbolMatches {
        symbol: symbol.to_string(),
        answered: groups,
        unanswered,
        max_matches,
        suggestions: Vec::new(),
    }))
}

impl SymbolMatches {
    fn unanswered_hint(&self) -> String {
        format!(
            "{} more match(es) past max_matches={} are listed without results; \
             re-call with node_id set to one of them, or raise max_matches.",
            self.unanswered.len(),
            self.max_matches
        )
    }

    fn not_found_hint(&self) -> String {
        let mut hint = format!("No symbol named '{}' found.", self.symbol);
        if !self.suggestions.is_empty() {
            hint.push_str(
                " Closest search matches are listed without results; \
                 re-call with node_id set to one of them.",
            );
        }
        hint
    }

    /// The JSON envelope: one entry per answered match, its heading followed
    /// by the tool's own fields (`bodies`, in `answered` order), then the
    /// matches past `max_matches` without results.
    pub(super) fn to_json(&self, bodies: Vec<Map<String, Value>>) -> Value {
        let matches: Vec<Value> = self
            .answered
            .iter()
            .zip(bodies)
            .map(|(group, body)| {
                let mut m = group.header();
                m.extend(body);
                Value::Object(m)
            })
            .collect();
        let mut out = Map::new();
        out.insert("symbol".to_string(), json!(self.symbol));
        out.insert(
            "match_count".to_string(),
            json!(self.answered.len() + self.unanswered.len()),
        );
        out.insert("matches".to_string(), Value::Array(matches));
        if !self.unanswered.is_empty() {
            let unanswered: Vec<Value> = self
                .unanswered
                .iter()
                .map(|g| Value::Object(g.header()))
                .collect();
            out.insert("unanswered".to_string(), Value::Array(unanswered));
            out.insert("hint".to_string(), json!(self.unanswered_hint()));
        }
        if self.answered.is_empty() {
            if !self.suggestions.is_empty() {
                let suggestions: Vec<Value> = self.suggestions.iter().map(node_header).collect();
                out.insert("suggestions".to_string(), Value::Array(suggestions));
            }
            out.insert("hint".to_string(), json!(self.not_found_hint()));
        }
        Value::Object(out)
    }

    /// Text form for tools whose output is text: `## <heading>` and the
    /// tool's own output (`bodies`, in `answered` order) per match, then the
    /// matches past `max_matches` without results.
    pub(super) fn to_text(&self, bodies: Vec<String>) -> String {
        let mut out = String::new();
        if self.answered.is_empty() {
            let _ = writeln!(out, "{}", self.not_found_hint());
            for n in &self.suggestions {
                let _ = writeln!(out, "{}", node_line(n));
            }
            return out;
        }
        for (group, body) in self.answered.iter().zip(bodies) {
            if !out.is_empty() {
                out.push('\n');
            }
            let _ = writeln!(out, "## {}", group.header_line());
            out.push_str(&body);
        }
        if !self.unanswered.is_empty() {
            let _ = writeln!(out, "\n{}", self.unanswered_hint());
            for group in &self.unanswered {
                let _ = writeln!(out, "{}", group.header_line());
            }
        }
        out
    }
}

/// Reads the per-match `limit`: `default` when absent, at least 1 and at
/// most `RESULT_LIMIT_MAX` otherwise.
pub(super) fn result_limit(args: &Value, default: Option<usize>) -> Option<usize> {
    args.get("limit")
        .and_then(Value::as_u64)
        .map_or(default, |v| {
            Some(
                usize::try_from(v)
                    .unwrap_or(RESULT_LIMIT_MAX)
                    .clamp(1, RESULT_LIMIT_MAX),
            )
        })
}

/// Caps `items` at `limit` and reports it the way literal search does
/// (#645): `count` shown, `total` found, `truncated` when they differ.
pub(super) fn capped_list(key: &str, mut items: Vec<Value>, limit: usize) -> Map<String, Value> {
    let total = items.len();
    items.truncate(limit);
    let mut m = Map::new();
    m.insert("count".to_string(), json!(items.len()));
    m.insert("total".to_string(), json!(total));
    m.insert("truncated".to_string(), json!(total > items.len()));
    m.insert(key.to_string(), Value::Array(items));
    m
}
