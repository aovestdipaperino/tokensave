//! Baseline policy for the token-savings metric.
//!
//! Different tools return fundamentally different amounts of source text,
//! and — critically — the *same* tool can return anywhere from a full file
//! to a two-line stub depending on its arguments and cache state:
//! `tokensave_read` alone can return the whole file, a line range, a symbol
//! map, a signature list, or (on a cache hit) a handful of metadata fields
//! with no source at all. `tokensave_body` returns only the selected
//! symbols' bodies, `tokensave_diff_context` returns symbol references and
//! affected-test paths, and `tokensave_blame` returns change metadata —
//! none of them ever echo a full file. Charging any of these the full
//! weight of every touched file as the "baseline" it avoided reading
//! overstates savings, sometimes wildly (e.g. `tokensave_dead_code` touching
//! 50 files but returning a few hundred bytes of symbol names; a cached
//! `tokensave_read` claiming a large file's full weight for a stub).
//!
//! [`baseline_policy`] classifies each tool so [`cap_baseline`] can scale the
//! baseline to what was actually delivered, instead of the full (and usually
//! unrealistic) file weight. No current tool is exempted from the cap: for a
//! genuine full-file delivery the response (JSON-wrapped and escaped) is
//! always at least as large as the source it carries, so the cap never binds
//! in that case — see [`REF_CAP_K`] — while every partial/reference/cached
//! response is scaled down to match what was actually returned.
//!
//! This module also covers the other side of the ledger that the original
//! metric missed entirely: [`request_overhead_tokens`] approximates the cost
//! of *sending* a tool call (name + arguments + JSON-RPC framing), and
//! [`schema_overhead_tokens`] approximates the one-time cost of the tool
//! schema listing itself, both of which belong in `after` alongside the
//! response text.

use serde_json::Value;

use super::ToolDefinition;
use crate::context::read_modes::estimate_tokens;

/// How a tool's `before` (baseline) token count should be computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselinePolicy {
    /// The tool returns full (or near-full) file content — charge the full
    /// weight of every touched file.
    FullFile,
    /// The tool returns references or summaries, not file content — cap the
    /// baseline relative to what was actually returned (see
    /// [`cap_baseline`]).
    Reference,
}

/// Reference-tool baselines are capped at `content_tokens * REF_CAP_K`: a
/// rough upper bound on how much source an agent would plausibly have read
/// by hand to find what the tool just returned, scaled to the size of the
/// answer rather than the number of files merely referenced.
///
/// Deliberately scaled against the *response content* alone, not the total
/// `after` (which also carries per-session schema overhead and per-call
/// request overhead) — those overheads are real MCP costs, but they say
/// nothing about how much source the tool's answer stood in for, and
/// including them would loosen the cap most on exactly the calls (the first
/// of a session) where it matters most.
const REF_CAP_K: u64 = 4;

/// Classifies a tool by name for the token-savings baseline calculation.
///
/// Every tool currently defaults to [`BaselinePolicy::Reference`] — the
/// conservative choice, since claiming full-file weight for a tool nobody
/// has vetted would silently inflate savings. This includes tools like
/// `tokensave_read` and `tokensave_body` that *can* return large amounts of
/// source: because the cap scales with what the response actually
/// delivered (see [`REF_CAP_K`]), a genuine full-file response — always at
/// least as large, byte for byte, as the source it wraps — is never
/// actually reduced by the cap, while a cache-hit stub, a `mode=lines`
/// slice, or a symbol-only answer is scaled down to match. `FullFile` is
/// kept available for a future tool that can be shown to always deliver at
/// least full-file weight regardless of arguments or cache state; no
/// current tool qualifies.
pub fn baseline_policy(_tool_name: &str) -> BaselinePolicy {
    BaselinePolicy::Reference
}

/// Applies `policy` to a raw full-file baseline (`full_file_tokens`, the sum
/// of touched files' token weight) given the tokens actually present in the
/// response content (`content_tokens` — *not* the overhead-inclusive
/// `after`; see [`REF_CAP_K`]).
///
/// `FullFile` tools keep the raw sum unchanged. `Reference` tools are capped
/// at `content_tokens * REF_CAP_K`, so claimed savings scale with the size of
/// the answer instead of the number of files merely referenced.
pub fn cap_baseline(policy: BaselinePolicy, full_file_tokens: u64, content_tokens: u64) -> u64 {
    match policy {
        BaselinePolicy::FullFile => full_file_tokens,
        BaselinePolicy::Reference => full_file_tokens.min(content_tokens.saturating_mul(REF_CAP_K)),
    }
}

/// Approximate token overhead of the JSON-RPC envelope wrapping a tool call,
/// beyond the tool name and arguments themselves (the `"jsonrpc"`, `"method"`,
/// `"id"`, `"params"` keys and delimiters). Small and constant regardless of
/// call size.
const FRAMING_CONST: u64 = 8;

/// Approximates the token cost of *sending* a tool call: the tool name, its
/// serialized arguments, and [`FRAMING_CONST`] for JSON-RPC framing.
///
/// This is the request-side cost the original metric omitted entirely — only
/// the response text was measured, so a call could report "savings" while
/// itself carrying substantial cost (a large `arguments` payload, e.g. a long
/// `tokensave_context` `task` string). Never zero: even a bare call carries
/// the tool name and envelope.
pub fn request_overhead_tokens(tool_name: &str, arguments: &Value) -> u64 {
    u64::from(estimate_tokens(tool_name))
        + u64::from(estimate_tokens(&arguments.to_string()))
        + FRAMING_CONST
}

/// Approximates the one-time token cost of the tool schemas the client loads
/// into context at the start of a session. Charged once, on the first
/// `tools/call` of the session.
///
/// Pass only the schemas that are actually resident up front — the
/// `anthropic/alwaysLoad` subset (see
/// [`get_always_load_tool_definitions`](super::get_always_load_tool_definitions)).
/// Deferred tools are not in context and would over-state the estimate.
pub fn schema_overhead_tokens(definitions: &[ToolDefinition]) -> u64 {
    let json = serde_json::to_string(definitions).unwrap_or_default();
    u64::from(estimate_tokens(&json))
}

/// Settles one call's raw signed savings (`before as i64 - after as i64`)
/// against `debt`, the carried-forward shortfall from earlier calls whose
/// `after` exceeded `before`. Returns `(new_debt, credited)`: the updated
/// debt, and the non-negative amount this call should actually add to the
/// persisted saved-tokens counter.
///
/// Crediting `raw_delta.max(0)` per call independently — with no memory
/// between calls — silently discards the shortfall from a call whose
/// `after` exceeds `before`. That happens most notably on the call that
/// absorbs the one-time schema-listing charge: if that charge dwarfs the
/// call's own `before`, only `before` tokens of it are ever recovered and
/// the rest just vanishes, instead of offsetting real savings from later
/// calls. This carries the shortfall forward as `debt` and pays it down
/// out of later surplus before crediting anything further, so across a
/// session the total credited converges on `max(0, sum of every call's raw
/// before - after)` rather than `sum of every call's max(0, before -
/// after)`.
pub fn settle_session_debt(debt: i64, raw_delta: i64) -> (i64, u64) {
    let available = raw_delta - debt;
    if available >= 0 {
        (0, available as u64)
    } else {
        (-available, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_delivery_tools_default_to_reference() {
        // These can each return anywhere from full-file content down to a
        // metadata-only stub depending on arguments/cache state, so none of
        // them get an unconditional FullFile classification — the cap
        // (see below) scales to what was actually delivered instead.
        assert_eq!(baseline_policy("tokensave_read"), BaselinePolicy::Reference);
        assert_eq!(baseline_policy("tokensave_body"), BaselinePolicy::Reference);
        assert_eq!(
            baseline_policy("tokensave_diff_context"),
            BaselinePolicy::Reference
        );
        assert_eq!(baseline_policy("tokensave_diff"), BaselinePolicy::Reference);
        assert_eq!(
            baseline_policy("tokensave_blame"),
            BaselinePolicy::Reference
        );
    }

    #[test]
    fn unknown_and_reference_tools_default_to_reference() {
        assert_eq!(
            baseline_policy("tokensave_dead_code"),
            BaselinePolicy::Reference
        );
        assert_eq!(
            baseline_policy("tokensave_search"),
            BaselinePolicy::Reference
        );
        assert_eq!(
            baseline_policy("tokensave_some_future_tool"),
            BaselinePolicy::Reference
        );
    }

    #[test]
    fn reference_cap_is_a_no_op_when_content_matches_or_exceeds_the_file() {
        // The invariant the module doc relies on: a genuine full-file
        // response is JSON-wrapped and escaped, so its token count is never
        // smaller than the raw source it carries. Reference's cap
        // (content_tokens * REF_CAP_K) must therefore never bind in that
        // case — this guards against a future response-format change (e.g.
        // a more compact encoding) silently reintroducing the
        // over-estimation this module fixes.
        let full_file_tokens = 10_000;
        let content_tokens = 10_500; // wrapped/escaped response >= source
        assert_eq!(
            cap_baseline(BaselinePolicy::Reference, full_file_tokens, content_tokens),
            full_file_tokens
        );
    }

    #[test]
    fn full_file_baseline_is_never_capped() {
        assert_eq!(cap_baseline(BaselinePolicy::FullFile, 100_000, 10), 100_000);
    }

    #[test]
    fn reference_baseline_is_capped_at_k_times_content() {
        // 50 touched files worth 100,000 tokens, but the response content is
        // tiny — capped at content_tokens * REF_CAP_K (4), not the raw file sum.
        assert_eq!(cap_baseline(BaselinePolicy::Reference, 100_000, 10), 40);
    }

    #[test]
    fn reference_baseline_passes_through_when_under_the_cap() {
        // A small touched-file sum under the cap is left unchanged.
        assert_eq!(cap_baseline(BaselinePolicy::Reference, 30, 100), 30);
    }

    #[test]
    fn reference_baseline_with_zero_content_tokens_caps_to_zero() {
        assert_eq!(cap_baseline(BaselinePolicy::Reference, 100_000, 0), 0);
    }

    #[test]
    fn request_overhead_is_never_zero() {
        // Even a bare call with empty arguments still carries the tool name
        // and JSON-RPC framing cost.
        let overhead = request_overhead_tokens("tokensave_status", &serde_json::json!({}));
        assert!(overhead >= FRAMING_CONST);
    }

    #[test]
    fn request_overhead_grows_with_argument_size() {
        let small = request_overhead_tokens("tokensave_context", &serde_json::json!({"task": "x"}));
        let large = request_overhead_tokens(
            "tokensave_context",
            &serde_json::json!({"task": "x".repeat(1000)}),
        );
        assert!(large > small);
    }

    #[test]
    fn settle_session_debt_defers_a_shortfall_instead_of_discarding_it() {
        // A call whose after exceeds before (e.g. one absorbing a large
        // schema charge against a small before) credits nothing and turns
        // the whole shortfall into debt, rather than losing everything past
        // `before`.
        let (debt, credited) = settle_session_debt(0, -9_800);
        assert_eq!(debt, 9_800);
        assert_eq!(credited, 0);
    }

    #[test]
    fn settle_session_debt_pays_down_before_crediting_anything() {
        // Surplus from a later call first pays down outstanding debt; none
        // of it reaches the persisted counter until the debt is gone.
        let (debt, credited) = settle_session_debt(9_800, 900);
        assert_eq!(debt, 8_900);
        assert_eq!(credited, 0);
    }

    #[test]
    fn settle_session_debt_credits_the_remainder_once_debt_is_cleared() {
        // Once a call's surplus exceeds the remaining debt, the excess is
        // credited and debt returns to zero.
        let (debt, credited) = settle_session_debt(100, 150);
        assert_eq!(debt, 0);
        assert_eq!(credited, 50);
    }

    #[test]
    fn settle_session_debt_over_a_session_matches_the_signed_total() {
        // End-to-end: schema charge (-1700), then two calls of +900 each.
        // The naive per-call-saturated sum would be 0 + 900 + 900 = 1800;
        // the debt-carrying total instead matches the true signed sum, 100.
        let mut debt = 0i64;
        let mut total_credited = 0u64;
        for raw_delta in [-1_700, 900, 900] {
            let (new_debt, credited) = settle_session_debt(debt, raw_delta);
            debt = new_debt;
            total_credited += credited;
        }
        assert_eq!(debt, 0);
        assert_eq!(total_credited, 100);
    }

    #[test]
    fn schema_overhead_scales_with_definition_count() {
        let def = |name: &str| ToolDefinition {
            name: name.to_string(),
            description: "d".repeat(100),
            input_schema: serde_json::json!({}),
            annotations: None,
            meta: None,
        };
        let few = schema_overhead_tokens(&[def("a")]);
        let many = schema_overhead_tokens(&[def("a"), def("b"), def("c"), def("d"), def("e")]);
        assert!(many > few);
        assert!(few > 0);
    }
}
