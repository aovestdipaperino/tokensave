# Network Calls and Privacy

Every network call tokensave makes, what it sends, and how to opt out.

tokensave's core functionality (indexing, search, graph queries, MCP server) is **100% local** -- your code never leaves your machine.

| Call | Data sent | When | Opt-out |
|------|-----------|------|---------|
| Worldwide counter upload | Token count (a number) + country (from IP) | sync, status, MCP sessions | `tokensave disable-upload-counter`, `TOKENSAVE_TELEMETRY=off` or `DO_NOT_TRACK=1` |
| Worldwide counter read | Nothing (GET request) | status, doctor | `TOKENSAVE_TELEMETRY=off` or `DO_NOT_TRACK=1` |
| Version check | Nothing (GET request) | status (cached 5m), sync (parallel) | N/A (1s timeout, no-op on failure) |
| Model pricing refresh | Nothing (GET request) | `tokensave cost` (cached 24h) | N/A (5s timeout, falls back to embedded pricing) |

The worldwide counter upload sends a single HTTP POST with a JSON body like `{"amount": 4823}`. No cookies, no tracking, no user ID. The Cloudflare Worker logs the country of your IP address (derived from request headers) for aggregate geographic statistics -- your actual IP address is not stored.

Opt-in performance profiling (`tokensave profile`) is local only and makes no network calls.

The model pricing refresh fetches a public JSON file from GitHub (`raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json`) to keep Claude model pricing up to date for `tokensave cost`. No data is sent -- it is a plain HTTPS GET. The response is cached at `~/.tokensave/pricing.json` for 24 hours. If the fetch fails, tokensave uses its compiled-in pricing table.

## Turning telemetry off

tokensave's only telemetry is the worldwide counter. To turn it off for every command run from an environment, such as a CI job or a shell profile, set either variable:

```bash
export TOKENSAVE_TELEMETRY=off   # also: false, 0, no, disable, disabled
export DO_NOT_TRACK=1            # the cross-tool convention; any value but 0, false, no, off or empty
```

While either is set, tokensave makes no request to the counter (no upload, no total, no country flags) and does not queue the tokens saved meanwhile for a later upload. The variables take precedence over `upload_enabled` in `~/.tokensave/config.toml`, but they only turn telemetry off: `TOKENSAVE_TELEMETRY=on` does not re-enable an upload that `tokensave disable-upload-counter` turned off. `tokensave status` still shows the last worldwide total it saved, and `tokensave doctor` names the variable that disabled telemetry. Version checks (`TOKENSAVE_UPDATE_CHECK=off` turns off the passive one), the pricing refresh and commands that exist to reach the network, such as `tokensave upgrade`, are not telemetry and are unaffected.

## Dependency advisory lookups

`tokensave vuln-reachability` uses local cached data by default. Passing
`--online`, or starting MCP with `TOKENSAVE_OSV_ONLINE=1`, authorizes sending
dependency ecosystems, package names and versions to OSV and fetching advisory
records by ID. These lookups send no source code, call graph data or project
paths. The cache lives in the project's index database.
