//! HTTP client for the worldwide token counter Cloudflare Worker.
//!
//! All operations are best-effort with timeouts. Failures are silently
//! ignored and never block the CLI.

use std::time::Duration;

/// The Cloudflare Worker endpoint URL.
/// TODO: Replace with actual deployed worker URL before release.
const WORKER_URL: &str = "https://tokensave-counter.enzinol.workers.dev";

/// Timeout for flush (upload) requests.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// Timeout for fetching the worldwide total (used in status).
const FETCH_TIMEOUT: Duration = Duration::from_secs(1);

/// Response from the worker's POST /increment and GET /total endpoints.
#[derive(serde::Deserialize)]
struct WorkerResponse {
    total: u64,
}

/// Uploads pending tokens to the worldwide counter.
/// Returns the new worldwide total on success, or None on any failure.
pub fn flush_pending(amount: u64) -> Option<u64> {
    if amount == 0 {
        return None;
    }
    let body = serde_json::json!({ "amount": amount });
    let agent = ureq::AgentBuilder::new().timeout(FLUSH_TIMEOUT).build();
    let resp = agent
        .post(&format!("{WORKER_URL}/increment"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .ok()?;
    let parsed: WorkerResponse = resp.into_json().ok()?;
    Some(parsed.total)
}

/// Fetches the current worldwide total from the worker.
/// Returns None on timeout, network error, or parse failure.
pub fn fetch_worldwide_total() -> Option<u64> {
    let agent = ureq::AgentBuilder::new().timeout(FETCH_TIMEOUT).build();
    let resp = agent
        .get(&format!("{WORKER_URL}/total"))
        .call()
        .ok()?;
    let parsed: WorkerResponse = resp.into_json().ok()?;
    Some(parsed.total)
}
