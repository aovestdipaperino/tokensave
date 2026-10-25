//! Model pricing lookup for Claude models.
//!
//! Pricing lifecycle:
//! 1. **Cached file** at `~/.tokensave/pricing.json` -- checked first.
//! 2. **Embedded fallback** baked into the binary -- used when no cache exists.
//! 3. **Remote refresh** from `LiteLLM`'s public pricing JSON -- fetched at most
//!    once every 24 hours, stored to the cache file.
//!
//! All prices are per million tokens (`MTok`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// `LiteLLM` pricing data URL. Public, no authentication required.
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

/// Model families tokensave prices: Anthropic Claude plus the `OpenAI` families
/// the Codex CLI reports (`gpt-*`, `o1`/`o1-*`, `o3`/`o3-*`, `o4`/`o4-*`,
/// `codex`/`codex-*`).
///
/// New families are added here. A model outside these families is left unpriced
/// on purpose rather than guessed at.
fn has_family_prefix(model_id: &str, family: &str) -> bool {
    model_id == family || model_id.starts_with(&format!("{family}-"))
}

fn is_metered_model(model_id: &str) -> bool {
    has_family_prefix(model_id, "claude")
        || model_id.starts_with("gpt-")
        || has_family_prefix(model_id, "o1")
        || has_family_prefix(model_id, "o3")
        || has_family_prefix(model_id, "o4")
        || has_family_prefix(model_id, "codex")
}

fn is_provider_prefixed_model(model_id: &str) -> bool {
    matches!(
        model_id.split_once('/').map(|(provider, _)| provider),
        Some("bedrock" | "vertex" | "azure")
    )
}

/// Whether tokensave has a price for `model`.
///
/// `false` means the turn's USD cost is unknown, not that it is free: callers
/// surface an explicit unpriced state rather than a misleading `$0.00`. This is
/// the guard that keeps a new or routed model from silently reading as free
/// (see `cost_of_turn`, which returns `0.0` for any unknown model).
pub fn is_priced(model: &str) -> bool {
    lookup(model).is_some()
}

/// Parse `LiteLLM`'s JSON format into our pricing table.
///
/// `LiteLLM` uses per-token costs (e.g. `3e-06` for $3/MTok). We filter to
/// the metered model families and convert to per-MTok.
fn parse_litellm_json(json: &str) -> Option<HashMap<String, ModelPricing>> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let obj = parsed.as_object()?;

    let mut table: HashMap<String, ModelPricing> = HashMap::new();

    for (model_id, entry) in obj {
        // Only include the model families tokensave meters: Anthropic Claude
        // plus the OpenAI families the Codex CLI reports. Everything else is
        // skipped so the prefix matcher in `lookup` stays unambiguous.
        if !is_metered_model(model_id) {
            continue;
        }

        // Skip Bedrock/Vertex/Azure provider-prefixed entries -- we want the
        // canonical model names that match what the CLIs report. Check both
        // the model ID and metadata because LiteLLM entries are not uniform.
        if is_provider_prefixed_model(model_id) {
            continue;
        }
        if let Some(provider) = entry.get("litellm_provider").and_then(|v| v.as_str()) {
            if provider.starts_with("bedrock")
                || provider.starts_with("vertex")
                || provider.starts_with("azure")
            {
                continue;
            }
        }

        let input = entry
            .get("input_cost_per_token")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let output = entry
            .get("output_cost_per_token")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let cache_write = entry
            .get("cache_creation_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let cache_read = entry
            .get("cache_read_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);

        // Keep explicit zero-cost entries so known free models remain distinct
        // from unknown models. Reject entries with no numeric pricing fields.
        let has_pricing = [
            "input_cost_per_token",
            "output_cost_per_token",
            "cache_creation_input_token_cost",
            "cache_read_input_token_cost",
        ]
        .iter()
        .any(|field| {
            entry
                .get(*field)
                .and_then(serde_json::Value::as_f64)
                .is_some()
        });
        if !has_pricing {
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
    lookup_in_table(get_table(), model)
}

fn lookup_in_table<'a>(
    table: &'a HashMap<String, ModelPricing>,
    model: &str,
) -> Option<&'a ModelPricing> {
    // Try exact match first
    if let Some(p) = table.get(model) {
        return Some(p);
    }

    // Fall back to longest prefix match
    let mut best: Option<(&str, &ModelPricing)> = None;
    for (key, pricing) in table {
        let has_delimited_suffix = model
            .strip_prefix(key.as_str())
            .and_then(|suffix| suffix.chars().next())
            .is_none_or(|ch| !ch.is_ascii_alphanumeric());
        if model.starts_with(key.as_str())
            && has_delimited_suffix
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

/// Fetch fresh pricing from `LiteLLM` and save to the cache file.
///
/// Returns `true` if the cache was updated, `false` on any failure.
/// Best-effort: never blocks longer than `FETCH_TIMEOUT`, failures
/// are silently ignored.
pub fn refresh_pricing() -> bool {
    let agent = crate::cloud::agent_with_timeout(FETCH_TIMEOUT);
    let Ok(mut resp) = agent.get(LITELLM_PRICING_URL).call() else {
        return false;
    };
    let Ok(body) = resp.body_mut().read_to_string() else {
        return false;
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
/// Uses `last_pricing_fetch_at` in `UserConfig` for TTL tracking.
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
#[allow(clippy::unwrap_used)]
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
        // A model outside every metered family stays unpriced regardless of any
        // cached pricing file on the host.
        assert!(lookup("totally-made-up-model-zzz-2099").is_none());
    }

    #[test]
    fn unknown_model_is_explicitly_unpriced_not_free() {
        // Regression: a new or routed model must be reported as unpriced so
        // callers can show an explicit unpriced state, never a silent $0.00
        // that reads as a free turn.
        let unknown = "some-brand-new-model-2099";
        assert!(!is_priced(unknown));
        assert!(lookup(unknown).is_none());
        assert!(is_priced("claude-opus-4"));
        // cost_of_turn collapses unknown to 0.0, which is exactly why is_priced
        // must gate whether that 0.0 means "free" or "unknown".
        assert!(cost_of_turn(unknown, 1_000_000, 1_000_000, 0, 0).abs() < f64::EPSILON);
        assert!(cost_of_turn("claude-opus-4", 0, 0, 0, 0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_litellm_keeps_openai_codex_families() {
        // The Codex CLI reports gpt-* models; they must survive the filter now,
        // while unrelated providers are still dropped.
        let json = r#"{
            "gpt-5-mini": {
                "input_cost_per_token": 1e-07,
                "output_cost_per_token": 4e-07,
                "litellm_provider": "openai",
                "mode": "chat"
            },
            "some-random-llm": {
                "input_cost_per_token": 1e-06,
                "output_cost_per_token": 1e-06,
                "litellm_provider": "other",
                "mode": "chat"
            }
        }"#;
        let table = parse_litellm_json(json).unwrap();
        assert!(table.contains_key("gpt-5-mini"));
        assert!(!table.contains_key("some-random-llm"));
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
        let cost = (1_000_000.0 / mtok) * p.input_per_mtok + (100_000.0 / mtok) * p.output_per_mtok;
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
        // Both claude and gpt-4o are metered families and should be included.
        assert_eq!(table.len(), 2);
        assert!(table.contains_key("claude-sonnet-4-6-20250514"));
        assert!(table.contains_key("gpt-4o"));
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

    #[test]
    fn parse_litellm_keeps_all_metered_families_and_excludes_provider_duplicates() {
        let json = r#"{
            "claude-sonnet-4": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "anthropic"},
            "gpt-5": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "o1": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "o3": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "o4": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "codex-mini": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "gpt-5-free": {"input_cost_per_token": 0.0, "output_cost_per_token": 0.0, "litellm_provider": "openai"},
            "gpt-5-no-price": {"litellm_provider": "openai"},
            "bedrock/claude-sonnet-4": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "bedrock_converse"},
            "vertex/gpt-5": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "vertex_ai"},
            "azure/o3": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "azure"},
            "vertex/gpt-5-no-metadata": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06},
            "azure/o3-nonmatching-metadata": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "not-claude-model": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "anthropic"},
            "autocodex": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "o100": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "openai"},
            "unrelated-model": {"input_cost_per_token": 1e-06, "output_cost_per_token": 2e-06, "litellm_provider": "other"}
        }"#;
        let table = parse_litellm_json(json).unwrap();
        for model in ["claude-sonnet-4", "gpt-5", "o1", "o3", "o4", "codex-mini"] {
            assert!(table.contains_key(model), "missing {model}");
        }
        assert!(table.contains_key("gpt-5-free"));
        assert!(table["gpt-5-free"].input_per_mtok.abs() < f64::EPSILON);
        assert!(!table.contains_key("gpt-5-no-price"));
        for model in [
            "bedrock/claude-sonnet-4",
            "vertex/gpt-5",
            "azure/o3",
            "vertex/gpt-5-no-metadata",
            "azure/o3-nonmatching-metadata",
            "not-claude-model",
            "autocodex",
            "o100",
            "unrelated-model",
        ] {
            assert!(!table.contains_key(model), "unexpected {model}");
        }
    }

    #[test]
    fn lookup_prefers_the_longest_matching_prefix() {
        let table = HashMap::from([
            (
                "gpt-5".to_string(),
                ModelPricing {
                    input_per_mtok: 1.0,
                    output_per_mtok: 0.0,
                    cache_write_per_mtok: 0.0,
                    cache_read_per_mtok: 0.0,
                },
            ),
            (
                "gpt-5-mini".to_string(),
                ModelPricing {
                    input_per_mtok: 2.0,
                    output_per_mtok: 0.0,
                    cache_write_per_mtok: 0.0,
                    cache_read_per_mtok: 0.0,
                },
            ),
            (
                "o1".to_string(),
                ModelPricing {
                    input_per_mtok: 3.0,
                    output_per_mtok: 0.0,
                    cache_write_per_mtok: 0.0,
                    cache_read_per_mtok: 0.0,
                },
            ),
        ]);
        assert!(
            (lookup_in_table(&table, "gpt-5-mini-2025")
                .unwrap()
                .input_per_mtok
                - 2.0)
                .abs()
                < f64::EPSILON
        );
        assert!(lookup_in_table(&table, "gpt-50").is_none());
        assert!(lookup_in_table(&table, "o100").is_none());
    }
}
