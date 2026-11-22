# Bundle history

## 2026-10-28

- **Updated** `metrics/gross-margin-legacy.md` with `superseded_by: /metrics/gross-margin.md`, so that consumers can follow the deprecated metric to its replacement.

## 2026-10-23

- **Migrated** the bundle to OKF v0.3. Path fields now start with `/` so that they resolve from the bundle root. The bundle-root `index.md` declares `okf_version: "0.3"`. This log no longer has frontmatter. The `not` key in `metrics/gross-margin.md` is now the extension key `x-acme-not`.

## 2026-07-01

- **Verified** the full bundle for OKF v0.2 conformance. `human:kliu@acme` reviewed all `verified` and `sources` entries.

## 2026-06-30

- **Re-generated** `metrics/revenue.md`, `computations/revenue-ytd.md`, and `policies/revenue-recognition.md` after Finance published the FY2026 revenue recognition policy addendum. Updated `stale_after` on both revenue concepts to `2026-12-31T00:00:00Z`.

## 2026-04-15

- **Deprecated** the legacy gross-margin definition. Original file moved to `metrics/gross-margin-legacy.md` with `status: deprecated`. New definition at `metrics/gross-margin.md` implements the FY2026 Cost Allocation Standard (includes fulfillment and shipping costs in COGS).

## 2026-02-10

- **Bundle bootstrapped** by `reference_agent/gemini-2.5-pro` from the BigQuery `INFORMATION_SCHEMA` and a 90-day sample of `region-us.INFORMATION_SCHEMA.JOBS_BY_PROJECT`. Initial trust tier: machine-confirmed across the board; finance-critical concepts flagged for human review.
