# Evaluation preparation record — 2026-10-27

Refs #832. No real-model results are available from this environment.

| Check | Result |
| --- | --- |
| Original Pi checkout | Commit `d5b3de22d0e24a3e9c38a9410d3329b099717283`, unchanged |
| Isolated source | Local clone under `/tmp`, same commit, dependencies hydrated with `npm ci --ignore-scripts`; matching published 1.4.0 provider data supplied |
| Source runtime | Temporary native-TypeScript Node 22.22.1; Pi source reports version 1.4.0 |
| Published host | Temporary `@earendil-works/pi-coding-agent@1.4.0`, installed with scripts disabled, reports 1.4.0 |
| Model discovery | `get_available_models` returns `[]` for both source and published host in isolated profiles inheriting the process environment |
| Local endpoints | No available model at loopback Ollama 11434, LM Studio 1234 or OpenAI-compatible 8080; repeated outside the network sandbox |
| Credentials | No credential file inspected, printed or committed; no model call made |
| Python offline tests | Five passed: settlement ordering, strict LF framing with U+2028 content, provider error handling, deadline/reaping, tool-delta replay and unknown state |
| Node observer test | One passed; observation leaves messages unchanged |
| Real Pi lifecycle harness | Five tests in two files passed against the isolated source, including observer/provider state equivalence and the four existing lifecycle/compaction cases |
| Diff checks | `git diff --check` passed |

The source-host checks use its supported RPC and extension interfaces. No Pi core
code was changed. This record deliberately distinguishes real host execution
with a faux provider from a real-model evaluation. The former passed; the latter
is blocked by the absence of an available model. Independent correctness review,
matched real-model usage/cost/latency, post-compaction model routing and adapter
behavior remain unmeasured. No read enforcement or savings claim is justified
by these results.

Resource checks cover fixture cleanup and child reaping on a command deadline;
the matched paid workload and long-running memory/CPU stability were not run.
Rust source and SQL are unchanged; the existing workspace environment failures
are documented in docs/PI-INTEGRATION.md. No Cargo checks were repeated for
these standalone scripts. Upstream master guidance was compared before editing
AGENTS.md; the only addition documents the new commands. Synchronization of that
shared command guidance to beta remains required when integrating this work.
