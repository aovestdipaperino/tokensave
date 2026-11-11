# Network Calls and Privacy

Every network call tokensave makes, what it sends, and how to opt out.

tokensave's core functionality (indexing, search, graph queries, MCP server) is **100% local** -- your code never leaves your machine.

| Call | Data sent | When | Opt-out |
|------|-----------|------|---------|
| Worldwide counter upload | Token count (a number) + country (from IP) | sync, status, MCP sessions | `tokensave disable-upload-counter` |
| Worldwide counter read | Nothing (GET request) | status | N/A (read-only, 1s timeout) |
| Version check | Nothing (GET request) | status (cached 5m), sync (parallel) | N/A (1s timeout, no-op on failure) |
| Model pricing refresh | Nothing (GET request) | `tokensave cost` (cached 24h) | N/A (5s timeout, falls back to embedded pricing) |

The worldwide counter upload sends a single HTTP POST with a JSON body like `{"amount": 4823}`. No cookies, no tracking, no user ID. The Cloudflare Worker logs the country of your IP address (derived from request headers) for aggregate geographic statistics -- your actual IP address is not stored.

Opt-in performance profiling (`tokensave profile`) is local only and makes no network calls.

The model pricing refresh fetches a public JSON file from GitHub (`raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json`) to keep Claude model pricing up to date for `tokensave cost`. No data is sent -- it is a plain HTTPS GET. The response is cached at `~/.tokensave/pricing.json` for 24 hours. If the fetch fails, tokensave uses its compiled-in pricing table.
