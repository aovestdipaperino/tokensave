# Caveman compatibility

TokenSave remains the semantic retrieval server. Caveman remains independently
installed and managed. When a recognisable Caveman installation is found,
TokenSave adds a conditional cooperation paragraph to MCP `initialize`
instructions. It asks for targeted semantic queries, avoids redundant exploration,
leaves prose style to an active Caveman skill, and requires exact preservation of
code and semantic data. It does not activate Caveman. Restart the MCP connection
after installation, removal, or changing compatibility preferences.

No result formatter, public tool schema, database query, model selection,
provider route, proxy setting, or third-party instruction file is changed.
Existing TokenSave agent rules and hooks continue to govern code retrieval;
Caveman's unrelated behavior is outside their scope. User instructions prevail.
TokenSave never installs Caveman and requires no new dependency.

## Configuration and diagnostics

Automatic cooperation defaults on. Set this top-level key in
`~/.tokensave/config.toml` to opt out:

```toml
caveman_compatibility = false
```

Alternatively set `TOKENSAVE_CAVEMAN_COMPATIBILITY=0` for a particular MCP server
process. This disables the additional instructions, while diagnostics remain
available. It does not disable Caveman or alter its files.

```bash
tokensave doctor --caveman
tokensave doctor --caveman --agent codex
tokensave doctor --caveman --agent claude --session-id YOUR_CLAUDE_SESSION_ID
```

These commands emit read-only JSON and skip the ordinary doctor's database and
network checks. Reports distinguish installation (`absent`, `installed`,
`partial`, `unsupported`) from session activation (`unknown`, `inactive`,
`active`). Missing home discovery returns an empty report list. Paths are
reported, but configuration bodies, environment values and credentials are not.

A skill alone means **available**, not active or inactive. An installed skill
without an authoritative session identity has `activation: unknown`. Only an
explicitly selected Claude session with verified Caveman 4.0.1 metadata may be
reported active or inactive. A missing session record stays unknown. A stored
`off` is inactive. The machine-wide `.caveman-active` mirror and `defaultMode`
never establish session activation. Other agents' manual `/caveman` activation
is not externally observable through a verified shared interface.

## Verified scope

The investigation used TokenSave 7.16.0 at
`ed7ae0f49d4ee4442fceb00770c657789dffdc3e` and Caveman plugin/skills 4.0.1 at
[`9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4`](https://github.com/JuliusBrussee/caveman/tree/9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4).
Caveman's CLI/proxy version is independent of the plugin release and is not
inferred from skill metadata. Unknown skill versions support only conditional
instructions; explicit releases other than 4.0.1 disable version-specific
interpretation. This deliberately narrow check is not a claim about all 4.x
versions. Revalidate upstream before expanding it.

| Surface | Finding and implementation |
| --- | --- |
| MCP contracts | Tool discovery uses `tools/list`; results include text content, often containing JSON, exact source, signatures, paths and relationships. Only `initialize.instructions` receives the cooperation paragraph. |
| Agent installation | Discover project/user skills for Claude, Codex, Cursor, Copilot, Gemini, OpenCode, Pi and OMP, plus shared `.agents/skills` and host-hook bundled skills. Claude's `plugins/installed_plugins.json` can locate its cached plugin. No cache-directory scan or third-party executable is needed. |
| Custom homes | Runtime honors `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and OpenCode's `XDG_CONFIG_HOME`. Other custom homes, remote agents, inherited skill roots and unlisted agents require manual inspection. |
| Activation | [INSTALL.md](https://github.com/JuliusBrussee/caveman/blob/9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4/INSTALL.md) documents Claude per-session mode files. State parsing follows [caveman-config.js](https://github.com/JuliusBrussee/caveman/blob/9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4/src/hooks/caveman-config.js). No portable MCP session-to-host-session identity bridge was found. |
| Behavior | [The skill](https://github.com/JuliusBrussee/caveman/blob/9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4/skills/caveman/SKILL.md) preserves technical payloads and governs prose/tool-run narration. TokenSave does not duplicate its brevity rules. |
| Delegation | Caveman's optional Cavecrew agents overlap with TokenSave's existing code-exploration guardrails. Targeted semantic retrieval remains preferred; user-requested delegation and unrelated tasks retain their existing behavior. No Cavecrew configuration is rewritten. |
| Catalog shrinking | [caveman-shrink](https://github.com/JuliusBrussee/caveman/blob/9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4/shrink/README.md) changes tool catalogs, including lossy descriptions. It is distinct from result compression. No shrink wrapper is installed or removed by TokenSave. Tool-selection equivalence still needs a model evaluation. |
| Input reduction | The [proxy](https://github.com/JuliusBrussee/caveman/blob/9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4/proxy/README.md) can compress model-bound tool results and uses recovery. No documented per-TokenSave result-exclusion control was verified. An MCP server cannot set model-request proxy headers. No invented escape marker or additional result wrapper is emitted. |
| Routing | Caveman Auto/provider wrapping is independently managed. TokenSave does not read credentials, set models, add a proxy, or replace an existing endpoint. |

Discovery reads fixed paths, at most 16 registry entries and at most 64 KiB per
artifact. No cache, background task, subprocess or per-tool discovery is added.
Session files must be regular, not symlinked, and no larger than 64 bytes. The
probe never writes. Skill symlinks are allowed for normal skills installations.
A retained linked plugin manifest can identify the release; copied skills often
cannot. Source identity checks are discovery hints, not signature verification.

Startup configuration diagnostics inspect the current project's `.caveman/config.json`
and `.caveman.json`, plus `~/.config/caveman/config.json`. JSONC/UTF-8 are understood;
UTF-16, ancestor traversal, Caveman environment overrides and alternate user-config
roots are not interpreted. Consequently this is not a replacement for
`caveman status` or `caveman doctor`. Unknown startup modes, malformed metadata,
missing registry payloads and disabled Claude plugin settings produce warnings.
A detected Cavecrew investigator file produces a delegation-overlap warning.
Arbitrary instruction conflicts cannot be reliably inferred from prose: check
Cavecrew/repository instructions manually when they impose incompatible searches.

## Accuracy boundary

TokenSave emits exactly the same semantic results with cooperation on or off.
A separate proxy can still transform the model-bound request after the MCP
response leaves TokenSave. A prompt cannot enforce byte preservation there.
Diagnostics warn about this limitation whenever installation evidence is present.
Use Caveman's own documented controls to disable lossy input compression for an
accuracy-sensitive run, independently of voice mode. For the inspected CLI,
[`caveman wrap --off <agent>`](https://github.com/JuliusBrussee/caveman/blob/9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4/docs/technical/cli-reference.md)
runs byte-safe pass-through recording. This is an operator choice, never a
TokenSave configuration edit. Recovery of originals does
not prove that the model saw exact originals. Proxy byte preservation and model
routing integration are **unsupported**, not silently enabled.

## Validation and implementation status

This document is the consolidated implementation record for detection, MCP
cooperation, diagnostics, and benchmarks. The implementation is published in
[draft PR #800](https://github.com/aovestdipaperino/tokensave/pull/800),
Refs [#797](https://github.com/aovestdipaperino/tokensave/issues/797), and remains
unmerged. A live metered-agent telemetry adapter, independently scored model
tasks, and actual A/B measurements are unfinished; the runner and its synthetic
accounting tests do not complete performance validation. The default-on
cooperation preference also needs reconciliation with #797's opt-in adoption
criterion before readiness. The companion Headroom evaluation is tracked in
[#798](https://github.com/aovestdipaperino/tokensave/issues/798); it remains a
separate adoption decision. No issue is closed by this PR.

| Component | Status | Validation / remaining work |
| --- | --- | --- |
| Read-only Rust discovery and diagnostics | Implemented locally | Fixture coverage in `tests/caveman_test.rs`; supported/inactive/unknown, malformed, opt-out, custom homes, plugin metadata, removal. |
| MCP cooperation | Implemented locally | Only initialization instructions change. Semantic preservation regression in `tests/mcp_handler_test.rs`. Real agent activation and proxy ingress still require manual trials. |
| Preferences | Implemented locally | Default-on setting follows the existing config/state and concurrent-save ownership patterns. |
| Performance comparison | Runner implemented; measurements pending | [Protocol and adapter contract](../scripts/caveman_bench/README.md). No real agent A/B trials, token savings, completion or cost claims. A telemetry-enabled agent and billing evidence are required. |

Run focused Rust checks with:

```bash
cargo test --locked --no-default-features --features lite --test integration caveman_test::
cargo test --locked --no-default-features --features lite --test integration caveman_discovery_preserves_exact_semantic_results
cargo test --locked --lib user_config::tests
cargo test --locked --test integration env_var_namespace_test::
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
```

The Python benchmark tests validate accounting and input rejection, using
synthetic events; they are not performance evidence. Resource review: allocations
are bounded by the fixed discovery candidates and per-file cap, files close by
RAII, initialization/doctor are the only discovery paths, no growing cache is
retained. Repeated fixture probes exercise cleanup; long-running server RSS and
proxy workloads have not been measured. SQL source is unchanged, so no query
inventory entries change.

Validation completed on Linux with the pinned Rust 1.98.1 toolchain:

- `cargo check --locked --no-default-features --features lite`: passed.
- `cargo test --workspace --locked`: 4,439 passed, 4 ignored, no failures.
- Focused semantic-preservation, configuration merge and environment-policy
  checks passed, including the new CLI and delegation-overlap cases in the full run.
- `cargo clippy --workspace --all-targets --locked`: passed; unrelated existing
  warnings remain.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Final `cargo test --workspace --locked caveman`: all 12 focused tests passed
  after the final style cleanup; Clippy passed with no warnings in `src/caveman.rs`.
- Python benchmark unit/accounting tests: 3 passed, including paired fresh-clone
  trials with synthetic telemetry. These are not model performance measurements.

The upstream master guidance was checked against the fetched original repository;
shared Caveman guidance is also drafted for beta. A companion beta documentation
worktree is prepared but unmerged. README content outside installation is identical
in those working copies, preserving beta's source-only installation and master’s
stable installation. Beta retains its query-debugging guidance. The beta source
port and live agent benchmarks remain pending; companion documentation must ship
with that source port, rather than advertising the feature on a published branch
without its implementation. The current changes are prepared on
`feat/caveman-compatibility` and published as draft PR #800 against master.
No merge or release is part of this work; beta synchronization remains a separate
pending change.
