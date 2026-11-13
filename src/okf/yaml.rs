//! Bounded YAML frontmatter loading.
//!
//! `yaml-rust2` resolves an alias by cloning the anchored node, so a small
//! document can expand exponentially ("billion laughs"). The loader first
//! replays the parser events and counts the nodes the document would expand
//! to, and refuses it before building any tree when that count is over
//! budget. The resulting tree is converted to `serde_json::Value`, keeping
//! key order, so callers can use the JSON accessors and round-trip unknown
//! keys unchanged.

use std::collections::HashMap;

use serde_json::Map;
use serde_json::Value;
use yaml_rust2::parser::Event;
use yaml_rust2::parser::MarkedEventReceiver;
use yaml_rust2::parser::Parser;
use yaml_rust2::scanner::Marker;
use yaml_rust2::Yaml;
use yaml_rust2::YamlLoader;

/// Largest number of nodes a frontmatter block may expand to, aliases
/// included. Real OKF frontmatter holds a few dozen nodes.
pub const MAX_EXPANDED_NODES: u64 = 20_000;

/// Deepest nesting accepted in a frontmatter block.
pub const MAX_DEPTH: usize = 64;

/// Why a frontmatter block could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YamlError {
    /// The text is not valid YAML; the message comes from the parser.
    Syntax(String),
    /// The document expands to more than [`MAX_EXPANDED_NODES`] nodes or
    /// nests deeper than [`MAX_DEPTH`].
    TooLarge,
    /// The text holds more than one YAML document.
    MultipleDocuments,
}

impl std::fmt::Display for YamlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(message) => write!(f, "invalid YAML: {message}"),
            Self::TooLarge => write!(
                f,
                "YAML expands to more than {MAX_EXPANDED_NODES} nodes or nests deeper than \
                 {MAX_DEPTH} levels"
            ),
            Self::MultipleDocuments => write!(f, "frontmatter holds more than one YAML document"),
        }
    }
}

/// Counts the nodes a YAML stream expands to without building it.
#[derive(Default)]
struct ExpansionBudget {
    /// Nodes produced so far, each alias counted as its anchored subtree.
    expanded: u64,
    /// Expanded size of every anchored node that has been closed.
    anchor_sizes: HashMap<usize, u64>,
    /// Open collections: anchor ID (0 when none) and `expanded` at start.
    open: Vec<(usize, u64)>,
    documents: usize,
    exceeded: bool,
}

impl ExpansionBudget {
    fn add(&mut self, nodes: u64) {
        self.expanded = self.expanded.saturating_add(nodes);
        if self.expanded > MAX_EXPANDED_NODES {
            self.exceeded = true;
        }
    }

    fn close(&mut self) {
        if let Some((anchor, start)) = self.open.pop() {
            if anchor != 0 {
                self.anchor_sizes
                    .insert(anchor, self.expanded.saturating_sub(start));
            }
        }
    }
}

impl MarkedEventReceiver for ExpansionBudget {
    fn on_event(&mut self, event: Event, _mark: Marker) {
        if self.exceeded {
            return;
        }
        match event {
            Event::DocumentStart => self.documents += 1,
            Event::Scalar(_, _, anchor, _) => {
                self.add(1);
                if anchor != 0 {
                    self.anchor_sizes.insert(anchor, 1);
                }
            }
            Event::SequenceStart(anchor, _) | Event::MappingStart(anchor, _) => {
                self.add(1);
                self.open.push((anchor, self.expanded));
                if self.open.len() > MAX_DEPTH {
                    self.exceeded = true;
                }
            }
            Event::SequenceEnd | Event::MappingEnd => self.close(),
            Event::Alias(anchor) => {
                let size = self.anchor_sizes.get(&anchor).copied().unwrap_or(1);
                self.add(size);
            }
            Event::Nothing | Event::StreamStart | Event::StreamEnd | Event::DocumentEnd => {}
        }
    }
}

/// Parses one YAML document into JSON, refusing over-budget documents.
///
/// Empty text yields `Value::Null`. Mapping keys that are not strings are
/// rendered with their scalar text; a non-scalar key becomes its JSON text.
pub fn load(text: &str) -> Result<Value, YamlError> {
    let mut budget = ExpansionBudget::default();
    Parser::new_from_str(text)
        .load(&mut budget, true)
        .map_err(|e| YamlError::Syntax(e.to_string()))?;
    if budget.exceeded {
        return Err(YamlError::TooLarge);
    }
    if budget.documents > 1 {
        return Err(YamlError::MultipleDocuments);
    }
    let docs = YamlLoader::load_from_str(text).map_err(|e| YamlError::Syntax(e.to_string()))?;
    Ok(docs.first().map_or(Value::Null, to_json))
}

/// Converts a YAML node to JSON. Real numbers that JSON cannot represent
/// (`.nan`, `.inf`) and unresolved aliases keep their text form.
pub fn to_json(node: &Yaml) -> Value {
    match node {
        Yaml::Null | Yaml::BadValue => Value::Null,
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Integer(i) => Value::from(*i),
        Yaml::Real(text) => text
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map_or_else(|| Value::String(text.clone()), Value::Number),
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Array(items) => Value::Array(items.iter().map(to_json).collect()),
        Yaml::Hash(entries) => {
            let mut map = Map::with_capacity(entries.len());
            for (key, value) in entries {
                map.insert(key_text(key), to_json(value));
            }
            Value::Object(map)
        }
        Yaml::Alias(id) => Value::String(format!("*{id}")),
    }
}

fn key_text(key: &Yaml) -> String {
    match key {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        Yaml::Null | Yaml::BadValue => "null".to_string(),
        other => to_json(other).to_string(),
    }
}

/// Writes `value` as a YAML 1.2 flow node.
///
/// JSON is a subset of YAML 1.2, so JSON text is a valid flow node: strings
/// are double-quoted with JSON escapes, which YAML accepts unchanged.
pub fn flow(value: &Value) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    #[test]
    fn loads_mapping_in_order_and_keeps_timestamps_as_text() {
        let value = load("type: Metric\nat: 2026-06-30T14:00:00Z\nn: 3\n").unwrap_or_default();
        let keys: Vec<&str> = value
            .as_object()
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(keys, ["type", "at", "n"]);
        assert_eq!(value["at"], "2026-06-30T14:00:00Z");
        assert_eq!(value["n"], 3);
    }

    #[test]
    fn refuses_exponential_alias_expansion() {
        let mut text = String::from("a: &a [x, x, x, x, x, x, x, x, x, x]\n");
        let mut previous = 'a';
        for name in 'b'..='j' {
            let refs = vec![format!("*{previous}"); 10].join(", ");
            let _ = writeln!(text, "{name}: &{name} [{refs}]");
            previous = name;
        }
        assert_eq!(load(&text), Err(YamlError::TooLarge));
    }

    #[test]
    fn accepts_small_aliases() {
        let value = load("a: &x {k: v}\nb: *x\n").unwrap_or_default();
        assert_eq!(value["b"]["k"], "v");
    }

    #[test]
    fn refuses_deep_nesting() {
        let text = format!("{}{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        assert!(load(&text).is_err());
    }

    #[test]
    fn refuses_multiple_documents_and_bad_syntax() {
        assert_eq!(load("a: 1\n---\nb: 2\n"), Err(YamlError::MultipleDocuments));
        assert!(matches!(load("a: [1, 2"), Err(YamlError::Syntax(_))));
    }

    #[test]
    fn flow_output_round_trips() {
        let original = serde_json::json!({"t": "a: \"b\"\n#c", "l": [1, true, null]});
        let parsed = load(&format!("v: {}", flow(&original))).unwrap_or_default();
        assert_eq!(parsed["v"], original);
    }
}
