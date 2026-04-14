//! Model pricing lookup for Claude models.
//!
//! Pricing lifecycle:
//! 1. **Cached file** at `~/.tokensave/pricing.json` -- checked first.
//! 2. **Embedded fallback** baked into the binary -- used when no cache exists.
//! 3. **Remote refresh** from LiteLLM's public pricing JSON -- fetched at most
//!    once every 24 hours, stored to the cache file.
//!
//! All prices are per million tokens (MTok).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// LiteLLM pricing data URL. Public, no authentication required.
/// See: <https://github.com/BerriAI/litellm>
const LITELLM_PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// Timeout for the pricing fetch request.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Cache TTL: 24 hours.
const CACHE_TTL_SECS: i64 = 86400;

/// Per-model pricing in USD per million tokens.
#[derive(Clone)]
pub struct ModelPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_write_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

/// Path to the cached pricing file: `~/.tokensave/pricing.json`.
fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave").join("pricing.json"))
}

/// The embedded pricing table -- compiled into the binary as a fallback.
fn embedded_table() -> HashMap<String, ModelPricing> {
    let mut m = HashMap::new();

    // Opus 4.5 / 4.6 (current pricing as of 2026-04)
    m.insert(
        "claude-opus-4".to_string(),
        ModelPricing {
            input_per_mtok: 5.0,
            output_per_mtok: 25.0,
            cache_write_per_mtok: 6.25,
            cache_read_per_mtok: 0.50,
        },
    );
    m.insert(
        "claude-sonnet-4".to_string(),
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_write_per_mtok: 3.75,
            cache_read_per_mtok: 0.30,
        },
    );
    m.insert(
        "claude-haiku-4".to_string(),
        ModelPricing {
            input_per_mtok: 0.80,
            output_per_mtok: 4.0,
            cache_write_per_mtok: 1.0,
            cache_read_per_mtok: 0.08,
        },
    );
    m.insert(
        "claude-3-5-sonnet".to_string(),
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_write_per_mtok: 3.75,
            cache_read_per_mtok: 0.30,
        },
    );
    m.insert(
        "claude-3-5-haiku".to_string(),
        ModelPricing {
            input_per_mtok: 0.80,
            output_per_mtok: 4.0,
            cache_write_per_mtok: 1.0,
            cache_read_per_mtok: 0.08,
        },
    );
    m.insert(
        "claude-3-opus".to_string(),
        ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
            cache_write_per_mtok: 18.75,
            cache_read_per_mtok: 1.50,
        },
    );

    m
}

/// Parse LiteLLM's JSON format into our pricing table.
///
/// LiteLLM uses per-token costs (e.g. `3e-06` for $3/MTok). We filter to
/// Claude models only and convert to per-MTok.
fn parse_litellm_json(json: &str) -> Option<HashMap<String, ModelPricing>> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let obj = parsed.as_object()?;

    let mut table: HashMap<String, ModelPricing> = HashMap::new();

    for (model_id, entry) in obj {
        // Only include Claude/Anthropic models
        if !model_id.contains("claude") {
            continue;
        }

        // Skip Bedrock/Vertex provider-prefixed entries -- we want the
        // canonical model names that match what Claude Code reports.
        if let Some(provider) = entry.get("litellm_provider").and_then(|v| v.as_str()) {
            if provider.starts_with("bedrock") || provider.starts_with("vertex") {
                continue;
            }
        }

        let input = entry
            .get("input_cost_per_token")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let output = entry
            .get("output_cost_per_token")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let cache_write = entry
            .get("cache_creation_input_token_cost")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let cache_read = entry
            .get("cache_read_input_token_cost")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // Skip entries with no pricing data
        if input == 0.0 && output == 0.0 {
            continue;
        }

        // Convert per-token to per-MTok
        let pricing = ModelPricing {
            input_per_mtok: input * 1_000_000.0,
            output_per_mtok: output * 1_000_000.0,
            cache_write_per_mtok: cache_write * 1_000_000.0,
            cache_read_per_mtok: cache_read * 1_000_000.0,
        };

        table.insert(model_id.clone(), pricing);
    }

    if table.is_empty() {
        None
    } else {
        Some(table)
    }
}

/// Try to load pricing from the cache file.
fn load_cached() -> Option<HashMap<String, ModelPricing>> {
    let path = cache_path()?;
    let contents = std::fs::read_to_string(path).ok()?;
    parse_litellm_json(&contents)
}

/// Build the merged pricing table: cached file over embedded fallback.
fn build_table() -> HashMap<String, ModelPricing> {
    let mut table = embedded_table();

    // Overlay cached entries (which may have newer models or updated prices)
    if let Some(cached) = load_cached() {
        for (model_id, pricing) in cached {
            table.insert(model_id, pricing);
        }
    }

    table
}

/// Get the global pricing table (initialized once per process).
fn get_table() -> &'static HashMap<String, ModelPricing> {
    use std::sync::OnceLock;
    static TABLE: OnceLock<HashMap<String, ModelPricing>> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

/// Look up pricing for a model ID. Matches the longest prefix.
/// Returns `None` for unknown models.
pub fn lookup(model: &str) -> Option<&'static ModelPricing> {
    let table = get_table();

    // Try exact match first
    if let Some(p) = table.get(model) {
        return Some(p);
    }

    // Fall back to longest prefix match
    let mut best: Option<(&str, &ModelPricing)> = None;
    for (key, pricing) in table {
        if model.starts_with(key.as_str())
            && best.is_none_or(|(bp, _)| key.len() > bp.len())
        {
            best = Some((key.as_str(), pricing));
        }
    }
    best.map(|(_, p)| p)
}

/// Compute the dollar cost of a single turn.
pub fn cost_of_turn(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_write_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    let Some(p) = lookup(model) else {
        return 0.0;
    };
    let mtok = 1_000_000.0;
    (input_tokens as f64 / mtok) * p.input_per_mtok
        + (output_tokens as f64 / mtok) * p.output_per_mtok
        + (cache_write_tokens as f64 / mtok) * p.cache_write_per_mtok
        + (cache_read_tokens as f64 / mtok) * p.cache_read_per_mtok
}

/// Fetch fresh pricing from LiteLLM and save to the cache file.
///
/// Returns `true` if the cache was updated, `false` on any failure.
/// Best-effort: never blocks longer than `FETCH_TIMEOUT`, failures
/// are silently ignored.
pub fn refresh_pricing() -> bool {
    let agent = crate::cloud::agent_with_timeout(FETCH_TIMEOUT);
    let mut resp = match agent.get(LITELLM_PRICING_URL).call() {
        Ok(r) => r,
        Err(_) => return false,
    };
    let body: String = match resp.body_mut().read_to_string() {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Validate that it parses before writing
    if parse_litellm_json(&body).is_none() {
        return false;
    }

    // Write to cache
    let Some(path) = cache_path() else {
        return false;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, body).is_ok()
}

/// Refresh pricing if the cache is stale (older than 24 hours).
/// Uses `last_pricing_fetch_at` in UserConfig for TTL tracking.
pub fn refresh_if_stale() {
    let mut config = crate::user_config::UserConfig::load();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if now - config.last_pricing_fetch_at < CACHE_TTL_SECS {
        return;
    }

    if refresh_pricing() {
        config.last_pricing_fetch_at = now;
        config.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_table_has_opus() {
        let table = embedded_table();
        let p = table.get("claude-opus-4").unwrap();
        assert!(p.input_per_mtok > 0.0);
        assert!(p.output_per_mtok > 0.0);
    }

    #[test]
    fn test_lookup_finds_claude_model() {
        // lookup should find a match for any Claude model via prefix
        let p = lookup("claude-opus-4-6-20250414");
        assert!(p.is_some());
        let p = p.unwrap();
        assert!(p.input_per_mtok > 0.0);
        assert!(p.output_per_mtok > 0.0);
    }

    #[test]
    fn test_lookup_sonnet() {
        let p = lookup("claude-sonnet-4-6").unwrap();
        assert!(p.input_per_mtok > 0.0);
    }

    #[test]
    fn test_lookup_unknown() {
        assert!(lookup("gpt-4o-2024-05-13").is_none());
    }

    #[test]
    fn test_cost_of_turn_nonzero() {
        // Any Claude model should produce a nonzero cost for nonzero tokens
        let cost = cost_of_turn("claude-opus-4-6", 1_000_000, 100_000, 0, 0);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_cost_of_turn_with_cache_tokens() {
        let cost = cost_of_turn("claude-opus-4-6", 0, 0, 500_000, 1_000_000);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_cost_of_turn_unknown_model() {
        let cost = cost_of_turn("unknown-model", 1_000_000, 100_000, 0, 0);
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_embedded_cost_calculation() {
        // Test against the embedded table directly, not the merged table
        let table = embedded_table();
        let p = table.get("claude-opus-4").unwrap();
        let mtok = 1_000_000.0;
        let cost = (1_000_000.0 / mtok) * p.input_per_mtok
            + (100_000.0 / mtok) * p.output_per_mtok;
        // 1M input * 5/MTok + 100k output * 25/MTok = 5.0 + 2.5 = 7.5
        assert!((cost - 7.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_litellm_json() {
        let json = r#"{
            "claude-sonnet-4-6-20250514": {
                "input_cost_per_token": 3e-06,
                "output_cost_per_token": 1.5e-05,
                "cache_creation_input_token_cost": 3.75e-06,
                "cache_read_input_token_cost": 3e-07,
                "litellm_provider": "anthropic",
                "max_tokens": 64000,
                "mode": "chat"
            },
            "gpt-4o": {
                "input_cost_per_token": 2.5e-06,
                "output_cost_per_token": 1e-05,
                "litellm_provider": "openai",
                "max_tokens": 16384,
                "mode": "chat"
            }
        }"#;
        let table = parse_litellm_json(json).unwrap();
        // Only claude model should be included
        assert_eq!(table.len(), 1);
        assert!(table.contains_key("claude-sonnet-4-6-20250514"));
        let p = &table["claude-sonnet-4-6-20250514"];
        assert!((p.input_per_mtok - 3.0).abs() < 0.001);
        assert!((p.output_per_mtok - 15.0).abs() < 0.001);
        assert!((p.cache_write_per_mtok - 3.75).abs() < 0.001);
        assert!((p.cache_read_per_mtok - 0.30).abs() < 0.001);
    }

    #[test]
    fn test_parse_litellm_skips_bedrock() {
        let json = r#"{
            "anthropic.claude-opus-4-6-v1": {
                "input_cost_per_token": 5e-06,
                "output_cost_per_token": 2.5e-05,
                "litellm_provider": "bedrock_converse",
                "mode": "chat"
            },
            "claude-opus-4-6-20250514": {
                "input_cost_per_token": 1.5e-05,
                "output_cost_per_token": 7.5e-05,
                "litellm_provider": "anthropic",
                "mode": "chat"
            }
        }"#;
        let table = parse_litellm_json(json).unwrap();
        // Bedrock entry should be skipped
        assert_eq!(table.len(), 1);
        assert!(table.contains_key("claude-opus-4-6-20250514"));
    }

    #[test]
    fn test_parse_litellm_invalid() {
        assert!(parse_litellm_json("not json").is_none());
        assert!(parse_litellm_json("{}").is_none()); // empty = no claude models
    }
}
