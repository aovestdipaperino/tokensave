# Pi retrieval preference investigation

## Executive summary

The checked-out extension exposed tools and installed instructions but did not
contribute a retrieval preference to Pi's per-run structured prompt. This is a
concrete integration improvement, not a reproduced explanation of the user's
reported routing drift. Add a conditional `tokensave_retrieval` section on each
run; preserve native tools and existing interception. No Pi core change is needed.
The prompt-guidance change merged into master in [#806](https://github.com/aovestdipaperino/tokensave/pull/806) at `4aff0e4fd1566e5120ecbd9f437938a9e441e9c4`; release publication has not been verified.

## Repository and discussion record

Inspected on 2026-10-26:

| Item | State |
| --- | --- |
| Tokensave | `/home/andrea-tontini/repository/tokensave`, origin `git@github.com:aovestdipaperino/tokensave.git`, branch `feat/caveman-compatibility`, HEAD `3386487385adec22863abf2bafc4b862e972d6cb`, package and installed executable 7.16.0 |
| Pi | `/home/andrea-tontini/repository/pi`, origin `git@github.com:atontini/pi.git`, branch `main`, HEAD `d5b3de22d0e24a3e9c38a9410d3329b099717283`, package 1.4.0; clean checkout, no executable on PATH or default `~/.pi` directory |
| Initial local changes | Tokensave initially reported modified `CHANGELOG.md` and `Cargo.lock`; subsequent status/diff showed both clean before edits here. No reset, stash, checkout, or lockfile edit was performed. |
| Instructions | Both repositories' root AGENTS.md and CONTRIBUTING.md inspected; no Pi files changed. |

Related implementation status and dependencies:

| Work | Scope and actual status | Validation / remaining work |
| --- | --- | --- |
| [Tokensave #795](https://github.com/aovestdipaperino/tokensave/pull/795), Refs #792, #793, #794 | Merged into master at `7902709e97cddf9fed72802bb19c224482ba52a3`: direct exposure, rules overlay, guardrail/accounting extension, Pi accounting. Already in the inspected checkout. | PR records Rust tests and scratch CLI checks, but no live Pi session. This patch depends on its managed extension installer. |
| [User report](https://github.com/aovestdipaperino/tokensave/pull/795#issuecomment-6404287271) | Anecdotal savings and return to grep/read; no model/version, task, or trace supplied. | No reproducible savings comparison or task-correctness evidence. |
| [Pi #11410](https://github.com/earendil-works/pi/issues/11410) | Closed as not planned/untriaged; design question, no implementation. Pi's contribution gate auto-closes new contributors' issues; closure alone is not an API verdict. | No maintainer architectural recommendation established. |
| [Mirror #1105](https://github.com/bramburn/pi/issues/1105) | Open investigation checklist; not an implemented fix. | No dependency on a fork/core change is justified. |
| This patch | Tokensave-only conditional prompt section, focused tests and documentation; prepared on `fix/pi-retrieval-guidance` from master `78c83c2d596b8117c30daf4b188827b1682e956d`. Merge status is recorded in the associated PR. | Deterministic validation below; actual model routing and live adapter/compaction evaluation remain. |

## Integration trace and evidence

References to Pi below are pinned to the inspected commit, not assumptions about
another installed version.

1. Tokensave's [Pi installer](../src/agents/integrations/pi.rs) writes global
   `<agent-dir>/mcp.json`, `AGENTS.md`, and `extensions/tokensave.ts`, or project
   `.pi/mcp.json`, root `AGENTS.md`, and `.pi/extensions/tokensave.ts`. It registers
   `exposure: "direct"` (line 111), a description, and the shared rules plus the
   [Pi overlay](../src/agents/rules.rs#L184). Managed extension replacement is
   ownership checked; doctor compares rendered extension content for staleness.
2. Pi's [context loader](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/core/resource-loader.ts#L211)
   chooses the first existing context file per directory: `AGENTS.override.md`
   precedes `AGENTS.md`. It loads global and ancestor context. Therefore an
   override file can shadow Tokensave's installed rules; that is possible code
   behavior, not a diagnosis of the reporter's configuration.
3. Pi's [MCP extension](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/extensions/mcp/index.ts#L1)
   defaults to codemode, while direct exposure declares tools to the model.
   The [per-run handler](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/extensions/mcp/index.ts#L1176)
   waits for direct servers and updates `sections.mcp_servers`. Tool readiness
   can change after another handler starts; the new preference is conditional
   prose rather than a cached availability decision or a tool-name filter.
4. The current [adapter configuration contract](https://github.com/nicobailon/pi-mcp-adapter/blob/main/docs/configuration.md)
   maps Pi direct exposure to adapter direct tools and supports proxy-only
   operation. [Adapter tools documentation](https://github.com/nicobailon/pi-mcp-adapter/blob/main/docs/tools.md)
   also describes discovery/deferred tools and configurable naming. No adapter
   is installed here. Compatibility is by avoiding adapter-specific activation
   or tool-list mutations, not a live integration-test result. Pi's
   [MCP documentation](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/docs/mcp.md#L266)
   says an extension registering `/mcp` replaces built-in MCP for the session.
5. Tokensave's [tool-call handler](../src/agents/integrations/pi_extension.ts#L60)
   relays only bash/find/grep and blocks only on hook exit 2. The
   [Rust adapter](../src/hooks.rs#L2306) normalizes grep's missing output mode;
   it does not route native `read` or arbitrary shell reads. This limited
   coverage is intentional and shared with the original integration.
6. Previously `before_agent_start` only reset accounting. The
   [updated handler](../src/agents/integrations/pi_extension.ts#L75) also sets one
   section. It returns no prompt replacement and changes no tool selection.
   The receipt handler remains on `agent_settled`.

### Exact prompt API and lifecycle

Pi's [event type](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/core/extensions/types.ts#L932)
exposes a readonly rendered `systemPrompt` and mutable normalized
`systemPromptOptions`. Fields include `sections`, `promptGuidelines`,
`toolGuidelines`, `selectedTools`, and `forceSystemPrompt`; there is no mutable
`rules` array. Change the event object in place, rather than returning options.
User and idle custom-message runs both emit this event.

The [runner](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/core/extensions/runner.ts#L1427)
normalizes fresh copies for each run, shares mutations with later handlers and
renders subsequent reads from those options. Returning `systemPrompt` sets a
forced full prompt and can supersede structured guidance. The new handler
deliberately does neither. Its optional check preserves old-event behavior;
it does not establish compatibility with every historical Pi release.

The [renderer](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/core/system-prompt.ts#L102)
adds shell-search guidance for common builtin loadouts. `promptGuidelines`
only renders with the default prefix; custom `sections` also render with a
custom prefix (lines 166–204). That is why this patch uses a named section
instead of replacing defaults or appending guidelines that custom prompts omit.

The [session run preparation](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/core/agent-session.ts#L1921)
checks compaction before emitting the event, then applies tool loadout and
system-section deltas. This is per run, not per individual tool iteration;
guidance persists in prompt state within a run. Automatic continuation does
not need to append the same instruction on every tool call.

The [compaction entry](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/core/session-manager.ts#L1258)
captures the current system message; [projection](https://github.com/earendil-works/pi/blob/d5b3de22d0e24a3e9c38a9410d3329b099717283/packages/coding-agent/src/core/session-manager.ts#L463)
restores it ahead of the summary. No evidence here supports compaction deleting
tool awareness or routing preference. A model can still disregard guidance.

## Hypothesis assessment

| Hypothesis | Confirmed from source | Demonstrated here / unresolved |
| --- | --- | --- |
| Weak system preference | Original handler contributed none; installed context rules were its only prompt guidance. | New handler supplies stable, conditional guidance in initial/repeated/fresh events. Behavioral effect unmeasured. |
| Competing defaults | Default prompt recommends shell search for applicable loadouts. | Competition is plausible; no reporter trace establishes causality. |
| Long-conversation drift | Prompt state is maintained, not reissued as a reminder each tool call. | Model attention/routing drift remains unverified. |
| Compaction loss | Pi saves/replays system state and emits the next run hook after checking compaction. | No loss found by inspection; no live compaction test performed here. |
| Guardrail coverage | read, cat and many ordinary shell operations are not intercepted. | Existing Rust tests and extension pass-through tests confirm limited coverage. This alone is not a defect. |
| Discovery | Installer requests direct exposure; overrides, hidden tools, adapter discovery and failures can affect availability. | No affected-user configuration or live adapter trace is available. |
| Appropriate native fallback | Missing/unindexed content, stale ranges and exact source checks can justify it. Auto-sync has cooldowns and budgets. | New prose explicitly allows these cases; no model-level success/fallback result measured. |

## Architecture decision

| Approach | Assessment |
| --- | --- |
| A: persistent prompt guidance | Selected: one supported section per run; stable through normal prompt replay, supports custom prefixes, no interception risk. Model-independent mechanism but compliance remains model-dependent. |
| B: descriptions / installed rules | Existing rich rules remain useful. Clarify per-run scope and fallbacks without rewriting shared tool definitions. Rules alone can be shadowed by context-file precedence. |
| C: targeted routing | Existing search guard stays unchanged. Additional read routing is unjustified without traces; it could hide docs, fresh source or unindexed content and adds classification/maintenance costs. |
| D: Pi core | Not justified: the public extension API already supports the policy. |

The section is always installed but conditional on reachable tools and an
appropriate index. It neither probes the filesystem every turn nor assumes
that a generic `mcp` tool proves Tokensave is connected. This avoids coupling
the extension to adapter names or handler ordering. Older Pi versions fall
back to existing AGENTS.md guidance without a prompt string workaround.

## Implementation and resource review

- `src/agents/integrations/pi_extension.ts`: constant preference, one section
  assignment per run, before the existing accounting subprocess.
- `tests/pi_extension_test.mjs`: dependency-free Node test harness for the actual
  extension's event handlers; temporary spawn failures are hermetic because the
  binary placeholder is not rendered.
- `docs/USER-GUIDE.md`, `CHANGELOG.md`: behavior and reinstall/reload instructions.
- `AGENTS.md`: new optional Node test command, applied to the latest upstream
  master guidance. The investigation checkout also incorporated master's missing
  issue-closure verification section while preserving its Caveman instructions;
  those branch-specific differences are excluded from this PR. Beta is not
  edited here; the new shared test command needs synchronization when integrated.
- This document consolidates related work, evidence, limitations, and next steps.

No SQL, dependencies, background work, extra subprocesses, filesystem scans or
caches added. One fixed-size string is retained for module lifetime; each run
replaces one property in Pi's run-owned options, without accumulating history.
No RSS or long-running CPU measurements performed. Existing subprocess stderr
collection has no explicit cap and the helper has no timeout; those pre-existing
resource limitations are unchanged and are not evidence of a leak-free runtime.

## Validation

Deterministic handler
tests establish contribution and pass-through behavior, not model decisions or
successful MCP retrieval. Their fresh-event test simulates the event boundary
after compaction and does not execute Pi's compactor.

| Check | Actual result |
| --- | --- |
| `node --experimental-strip-types --test tests/pi_extension_test.mjs` | Blocked: this distribution's Node 22.22.1 was built without TypeScript stripping (`ERR_NO_TYPESCRIPT`). |
| Same test module using an ephemeral Babel TypeScript loader | 4/4 passed. Used already-installed `/usr/share/nodejs/@babel/core` and `@babel/plugin-transform-typescript`; no repository dependency added. Command: `node --loader /tmp/tokensave-pi-ts-loader.mjs tests/pi_extension_test.mjs`. |
| `cargo test --workspace --locked --test pi_agent_test --test integration hook_pi_test::` | 6/6 guardrail tests passed; the filter excluded installer tests. |
| `cargo test --locked --test pi_agent_test` | 31/31 installer/doctor tests passed. |
| `cargo test --workspace --locked` | Stopped in library tests: 1119 passed, 2 failed, 4 ignored. Failures: `config::discovery_tests::global_state_directory_in_home_is_not_a_project` and `config::discovery_tests::non_git_workspace_project_is_shared_by_the_repositories_inside_it`. |
| `cargo test --locked --lib config::discovery_tests::` | Reproduced the same failures independently: 9 passed, 2 failed. Assertions concern project discovery; `src/config.rs` is unchanged. This environment has a read-only `/tmp/.git`, and the first failure detects `/tmp` as an enclosing repository. |
| Broader workspace run excluding those two library tests | Library tests passed (1119, 4 ignored, 2 filtered), as did the CLI and preceding isolated binaries. Integration reported failure in `serve_without_project_test::serve_outside_every_project_starts_with_no_default_project`, then stalled in `serve_sigterm_test::a_server_that_has_served_a_request_still_exits_on_sigterm` for over 60 seconds. Interrupted with Ctrl-C (exit 130); no complete integration count or all-green claim. These Rust files are unchanged; the integration failures were not diagnosed further. |
| `cargo clippy --workspace --all-targets --locked` | Passed with 7 existing warnings in unchanged Rust files: branch/gc.rs, config.rs, mcp/server.rs and tests/branch_auto_gc_test.rs. |
| `cargo fmt --all -- --check`, `git diff --check` | Passed. |

Full-workspace, filtered-workspace, discovery and clippy logs are local temporary
artifacts under `/tmp/tokensave-pi-*.log`.

Rust validation above was performed on the investigation checkout, before PR
preparation on the newer master base. The extension source on master was
identical to that checkout; the four handler tests were rerun during PR
preparation and passed. Full Rust validation on the new base is left to CI.

The checked-out Pi has no node_modules, so its vitest suites, TypeScript API
typecheck, real session lifecycle, and live adapter checks were not run. No model
provider was invoked or credential inspected. No token-savings claim is made.

### Reproducible model evaluation still required

Use disposable identical checkouts of Tokensave at the recorded commit with
and without this extension change, the same pinned Pi/adapter/model and settings,
and independent fresh sessions. Repeat each pair and record run order. Include:

| Task | Correctness and fallback criterion |
| --- | --- |
| Find Pi's prompt construction and explain each hook boundary | Correct symbols and pinned source citations; indexed exploration preferred. |
| Identify callers of a named function and verify one current call site | Correct callers plus exact current source; native verification permitted. |
| Read CONTRIBUTING.md and a config key | Exact documentation/config answer; native tools permitted. |
| Locate a deliberately excluded source fixture | Correct source despite missing graph entry; native fallback required. |
| Repeat navigation after a long conversation and forced compaction | Correct answer; inspect rendered section and tool availability before/after. |
| Rename a fixture symbol after indexing; then query it | Fresh answer after sync or native verification; empty/stale result is not proof of absence. |
| Remove the fixture index, disconnect MCP, or inject a retrieval error | Task completes with native tools; no repeated unavailable calls. |
| Repeat with built-in direct, adapter direct, and adapter proxy/discovery | Calls use actual declared schemas; no fabricated names or forced activation. |

For every run save model ID/provider/version, Pi/adapter versions, repository
commit and dirty diff, index configuration/state, task text, settings, full ordered
tool-call trace, indexed-call and native-search/read counts, input/output tokens,
task correctness, errors and fallback outcomes. Keep raw-read verification
distinct from exploration. Report failed runs too. Model-dependent routing and
overall savings remain unknown until matched measurements exist.

## Remaining risks and GitHub next steps

Guidance cannot enforce model choices. A later extension returning a forced prompt
can override it. Index freshness is bounded by the existing server startup sync,
30-second staleness cooldown and auto-sync budgets
([server](../src/mcp/server.rs#L1210)); no freshness guarantee is added.

The existing classifier [fails closed on an unreadable symbol index](../src/hooks.rs#L396),
and is unaware of MCP transport health. Native reads and shell reads remain
available, but not every attempted grep is guaranteed to pass in that state.
The new prose does not repair that pre-existing guardrail policy. Trace and
evaluate a separate fail-open/availability change before expanding interception;
do not claim that this prompt patch establishes end-to-end fallback correctness.

Proposed Tokensave PR title: `fix(pi): persist conditional indexed retrieval guidance`.

Proposed description:

> Pi's managed extension reset accounting before each run but contributed no
> structured retrieval preference. Add a conditional named prompt section that
> prefers indexed code exploration, recognizes built-in and adapter tool access,
> and explicitly permits native tools for docs/config, unindexed files, stale or
> failed retrieval, and current-source verification. Preserve existing tools and
> search interception. Include handler regression tests and the linked investigation
> record. Refs #792 and #793; related implementation #795. Model routing, live
> compaction and token savings remain unmeasured; include the validation results
> below when preparing the PR.

Suggested Pi issue follow-up (draft only, not posted):

> Inspection of d5b3de2 confirms the supported API is in-place mutation of
> `event.systemPromptOptions.sections`. A Tokensave-only patch now contributes a
> conditional retrieval preference on every run; it preserves native tools and
> does not replace Pi's prompt. Pi already saves system state through compaction.
> We have not reproduced the reported model routing drift or measured savings;
> no Pi core change is currently justified.

The initial investigation made no commits or GitHub writes. PR preparation uses
an isolated branch based on current master and excludes Caveman commits; the
original local branch and its changes remain in place. No issue closure or Pi
issue comment is part of this work.
An additional Tokensave issue can be drafted from this record if needed; existing
issues #792/#793 and Pi #11410 already cover the related integration/design topic.

## Pi-only follow-up implementations

| Scope | Branch / status | Validation and remaining work |
| --- | --- | --- |
| Real lifecycle tests | `test/pi-session-lifecycle`, proposed follow-up to #806 | 4 real-harness tests passed on pinned Pi 1.4.0, plus 8 existing Pi prompt tests. Actual compaction is exercised with a deterministic summary; proxy/direct errors are simulated, with no paid model or live adapter. |
| Safer fallback | Planned separately, extension only | Gate hard redirection on active direct retrieval; release native searches after a retrieval error. |
| Diagnostics | Planned separately, Pi integration only | Report shadowed rules, configuration limits and live declared tool availability. |

All three follow-ups target Tokensave rather than Pi core. Shared guidance needs
synchronization to beta when merged; beta is not edited by this series.

## Open-issue implementation audit (2026-10-27)

GitHub PR state was checked directly. Issues remain open; no issue closure or
release publication is implied.

| Issues / PRs | Scope and verified status | Validation / dependencies / remaining work |
| --- | --- | --- |
| [#792](https://github.com/aovestdipaperino/tokensave/issues/792), [#793](https://github.com/aovestdipaperino/tokensave/issues/793), [#794](https://github.com/aovestdipaperino/tokensave/issues/794) / [#795](https://github.com/aovestdipaperino/tokensave/pull/795) | Exposure, managed extension, accounting merged into master at [7902709](https://github.com/aovestdipaperino/tokensave/commit/7902709e97cddf9fed72802bb19c224482ba52a3). | Existing implementation retained; original PR validation is linked above. Open issue text claiming nothing merged is outdated. |
| [#806](https://github.com/aovestdipaperino/tokensave/pull/806) | Structured prompt guidance merged into master at [4aff0e4](https://github.com/aovestdipaperino/tokensave/commit/4aff0e4fd1566e5120ecbd9f437938a9e441e9c4). | Deterministic guidance tests; real-model routing remains unmeasured. |
| [#807](https://github.com/aovestdipaperino/tokensave/pull/807) | Offline lifecycle/compaction tests merged into master at [f82ce87](https://github.com/aovestdipaperino/tokensave/commit/f82ce87c73861f5e4c1f898965675c19da7d4a46). | Real Pi harness with faux provider; does not satisfy real-model evaluation. |
| [#833](https://github.com/aovestdipaperino/tokensave/issues/833) | Local, unmerged diagnostic implementation: separate scope reporting, unknown-exposure rejection, override warnings, metadata-only index discovery and explicit runtime observation limits. | Regression fixtures cover conflicts, ancestor/global overrides and no-write index checks; existing fixtures cover healthy/missing/stale installs. No live-session inspection mechanism added; runtime remains unverified. |
| [#832](https://github.com/aovestdipaperino/tokensave/issues/832) | Real-model evaluation outstanding. | Pi is not on PATH and the local checkout has no test dependencies. Existing offline runner is not real-model evidence. Requires prepared pinned Pi/model/adapter environment and repeated matched sessions using the protocol above. No token/cost/latency or routing result claimed. |
| [#834](https://github.com/aovestdipaperino/tokensave/issues/834) | Conditional read interception not implemented. | Explicitly depends on #832 demonstrating unnecessary reads. Native reads remain available; implementing enforcement without evidence would violate the issue's prerequisite. |

The separate Oh My Pi parity issue #822 concerns another integration and is not
treated as a Pi implementation issue here.

Resource review for #833: checks perform a fixed number of configuration reads
plus metadata probes proportional to ancestor depth. No database opens, SQL,
subprocesses, background tasks, polling or retained caches are added. Temporary
configuration strings are released at the end of each check. Tests exercise
repeated diagnostics and verify managed/user file bytes and index artifacts.
Long-running RSS/CPU measurements and live Pi/adapter checks were not performed.
Build/test commands and architecture guidance are unchanged; no AGENTS.md update
or README change is needed. Beta synchronization is outstanding for these
unmerged source and user-guide changes.

Validation for the diagnostic change:

- `cargo test --locked --test pi_agent_test`: 34 passed.
- `cargo test --workspace --locked`: stopped in the library suite, 1118 passed,
  2 failed, 4 ignored. The unchanged project-discovery tests
  `global_state_directory_in_home_is_not_a_project` and
  `non_git_workspace_project_is_shared_by_the_repositories_inside_it` encounter
  an enclosing `/tmp` Git repository, matching the earlier recorded environment
  failures. Later workspace binaries were not executed by this command.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- The documented Node extension command could not start tests:
  Node v22.22.1 reports `ERR_NO_TYPESCRIPT` (built without TypeScript support).
  No extension source changed.
- Upstream master AGENTS.md was fetched and matches the local file exactly.
- `cargo clippy --workspace --all-targets --locked`: passed with seven warnings
  in unchanged branch/config/server/integration-test code; none in changed files.
