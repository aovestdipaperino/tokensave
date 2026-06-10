//! Helpers built on `yaml-rust2` so each YAML-based ecosystem module stays
//! short. Wraps the `Yaml` enum with convenience accessors that mirror our
//! existing `toml::Value` / `serde_json::Value` patterns.

use yaml_rust2::{Yaml, YamlLoader};

/// Parse a YAML document and return its first root node. Errors collapse to
/// `None` — callers fall back to returning empty dep lists when the file is
/// unreadable.
pub fn parse_root(raw: &str) -> Option<Yaml> {
    YamlLoader::load_from_str(raw).ok()?.into_iter().next()
}

/// Read the scalar value of `node[key]` as a string. Numbers and booleans are
/// coerced to their `Display` form so manifests like `version: 1.2` (treated
/// as a real by YAML) still yield `"1.2"`.
pub fn as_string(node: &Yaml, key: &str) -> Option<String> {
    let v = node_field(node, key)?;
    yaml_to_string(v)
}

pub fn node_field<'a>(node: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    let map = node.as_hash()?;
    map.iter()
        .find(|(k, _)| k.as_str() == Some(key))
        .map(|(_, v)| v)
}

pub fn yaml_to_string(node: &Yaml) -> Option<String> {
    match node {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Real(s) => Some(s.clone()),
        Yaml::Boolean(b) => Some(b.to_string()),
        Yaml::Null => None,
        _ => None,
    }
}

/// Iterate the entries of a hash node, returning `(key, value)` pairs where
/// the key is a string. Non-string keys are skipped.
pub fn hash_entries<'a>(node: &'a Yaml) -> impl Iterator<Item = (String, &'a Yaml)> + 'a {
    node.as_hash().into_iter().flat_map(|m| {
        m.iter().filter_map(|(k, v)| {
            let key = match k {
                Yaml::String(s) => Some(s.clone()),
                Yaml::Integer(i) => Some(i.to_string()),
                _ => None,
            };
            key.map(|k| (k, v))
        })
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_map() {
        let root = parse_root("name: foo\nversion: 1.0").unwrap();
        assert_eq!(as_string(&root, "name").as_deref(), Some("foo"));
        assert_eq!(as_string(&root, "version").as_deref(), Some("1.0"));
    }

    #[test]
    fn hash_entries_iterates_keys() {
        let root = parse_root("deps:\n  a: 1\n  b: 2\n").unwrap();
        let deps = node_field(&root, "deps").unwrap();
        let keys: Vec<_> = hash_entries(deps).map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }
}
