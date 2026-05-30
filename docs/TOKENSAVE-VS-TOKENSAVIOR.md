# tokensave vs token-savior

A capability + performance comparison between the two tools, with measured
results from a synthetic indexer benchmark and a 96-task agent benchmark
(`Mibayy/tsbench`).

| | tokensave | token-savior |
|---|---|---|
| **Implementation** | Rust + SQLite + tree-sitter | Python + in-memory dict |
| **Languages indexed** | 46 via tree-sitter grammars (Rust, Python, Go, TS/JS, Java, Kotlin, Scala, C#, Swift, C/C++, Ruby, PHP, Dart, Lua, Perl, Bash, Pascal, COBOL, Fortran, …) | 7 via regex annotators (Python, TS/JS, Rust, Go, C, C#) + 8 config formats (TOML/JSON/YAML/XML/INI/HCL/env/Dockerfile) |
| **MCP tools exposed** | 70+ (one fewer without `ast-grep`) | 68 (`full`), 15 (`optimized` default) |
| **Branch-aware indexing** | yes — per-branch DB, `tokensave branch …` | no |
| **Index freshness** | on-demand staleness check per MCP call + catch-up sync on connect | no |
| **Health analytics** | 11 dedicated tools (complexity, hotspots, dead-code, redundancy, doc-coverage, coupling, dsm, gini, …) | 2 (dead-code, complexity) |
| **Edit primitives** | symbol-aware (`replace_symbol`, `insert_at_symbol`) + range (`str_replace`, `insert_at`, `ast_grep_rewrite`) | `replace_symbol_source`, `insert_near_symbol`, `add_field_to_model`, `move_symbol` |
| **Shell-output compaction** | out of scope — pair with [RTK](https://github.com/aovestdipaperino/rtk) | yes (34 compactors in v4.3+) |
| **Config / Docker linting** | no | yes (`analyze_config`, `analyze_docker`) |
| **Indexer cold time (FastAPI)** | **2.15 s** | 6.21 s |
| **Impact analysis (FastAPI)** | **0.57 ms** | 24.4 ms |
| **tsbench agent score** | **184 / 192 (95.8%)** | 188 / 192 (97.9%) |
| **Active tokens / task (tsbench)** | **695** | 3 395 |

---

## 1. Where tokensave is stronger

### 1.1 Language coverage and parsing strategy

Both tools are multi-language, but they take different approaches.

**token-savior** ships hand-written regex-based annotators per language:
Python, TypeScript/JavaScript, Rust, Go, C, C# (each 200–750 lines of
regex + brace-matching), plus structured-format annotators for TOML,
JSON, YAML, XML, INI, HCL, `.env*`, and Dockerfile. Regex parsing is
simple to extend, deliberately permissive on partial / malformed code,
but doesn't carry type or scope information across constructs — it
catches function/class declarations and import sites cleanly but
won't resolve, for example, which `impl` block a Rust method
belongs to or which interface a TS class implements.

**tokensave** parses **46 languages** through tree-sitter grammars —
Rust, Python, Go, TypeScript / TSX / JavaScript, Java, Kotlin, Scala,
C#, Swift, C, C++, Objective-C, Ruby, PHP, Dart, Lua, Perl, Bash,
Pascal, COBOL, Fortran, OCaml, Haskell, Elixir, Erlang, Clojure, Julia,
R, GLSL, Nix, Zig, F#, VB.NET, SQL, Protobuf, Dockerfile, TOML,
GW-BASIC, MSBASIC2, QBasic, QuickBasic, Lean, Quint. The full AST is
available to the extractor, so edge kinds beyond "this name appears
here" are recoverable: `implements`, `extends`, `type_of`, `annotates`,
`returns`, `receives`, in addition to `calls` and `uses`. The trade-off
is one tree-sitter grammar (and matching extractor) per language —
adding a language is a several-hundred-line commitment vs. token-
savior's regex-and-go.

Practical implication: on the seven languages both tools cover,
token-savior gets file outlines fast and reliably; tokensave gets
richer graph queries (impact radius, call chains, type hierarchies)
because the edges exist in the first place. On anything outside
token-savior's annotator set — Java, Kotlin, Scala, Swift, C++, Ruby,
PHP, Dart, Lua, etc. — tokensave is the only option that yields a
structured index.

### 1.2 Branch-aware indexing

tokensave maintains a separate SQLite DB per tracked branch:

```bash
tokensave branch add feature/foo   # snapshot from ancestor + incremental sync
tokensave branch list              # show tracked branches + their DB sizes
tokensave branch gc                # drop DBs for branches deleted from git
```

Switching branches doesn't invalidate the index. `tokensave_branch_diff` and
`tokensave_branch_search` query specific branches without disturbing the
active one. token-savior caches a single working-tree index.

### 1.3 Larger structural-analysis surface

The graph itself carries richer edges (`calls`, `uses`, `type_of`,
`implements`, `extends`, `annotates`, `returns`, `receives`), and tokensave
exposes 11 health-focused MCP tools that have no token-savior equivalent:

| Tool | Purpose |
|---|---|
| `tokensave_hotspots` | High-churn × high-complexity files |
| `tokensave_complexity` | Cyclomatic + branch / loop / return / nesting counts |
| `tokensave_doc_coverage` | % of public items with docstrings, per module |
| `tokensave_god_class` | Outsized classes (members, fan-in, fan-out) |
| `tokensave_coupling` | Inter-module afferent / efferent coupling |
| `tokensave_dsm` | Design-structure matrix |
| `tokensave_inheritance_depth` | Deep / fragile class hierarchies |
| `tokensave_recursion` | Direct and indirect cycles in the call graph |
| `tokensave_redundancy` | Semantic-similarity duplicate detection |
| `tokensave_gini` | Imbalance metric across files / modules |
| `tokensave_health` | Composite scorecard (rolls up the above) |
| `tokensave_test_risk` | Files at risk for the next regression |
| `tokensave_dependency_depth` | Longest paths in the dependency DAG |

Plus dedicated tools for cross-cutting agent workflows — `tokensave_diagnose`
(triages a stuck session), `tokensave_run_affected_tests`,
`tokensave_diagnostics` (TypeScript LSP integration), `tokensave_outline`,
`tokensave_signature_search`, `tokensave_type_hierarchy`,
`tokensave_field_sites`, `tokensave_constructors`,
`tokensave_unsafe_patterns`, `tokensave_unused_imports`,
`tokensave_implementations`, `tokensave_impls`, `tokensave_derives`.

### 1.4 On-demand index freshness

When `tokensave serve` is running, every MCP tool call first runs a
staleness check (gated by a 30 s cooldown) and re-syncs any touched files
before answering; the server also runs a catch-up sync when it connects.
The agent's query always sees fresh data without a long-lived watcher
process. (An embedded `ProjectWatcher` shipped in 6.0.0 but was removed in
6.1.0 after it caused runaway CPU/memory on large monorepos.) Token-
savior's cache is invalidated on a per-tool basis and re-built lazily.

### 1.5 Rust + SQLite: durable and fast

- **Cold index** is ~3× faster on real Python codebases (FastAPI: 2.15 s vs
  6.21 s — see §3).
- **Impact analysis** is ~43× faster on the same project (0.57 ms vs 24.4 ms
  handler time), because the reverse-dependency walk runs against an indexed
  SQLite table rather than a pure-Python `dict[str, list[str]]`.
- The graph **persists** on disk and survives process restarts; token-savior
  rebuilds in-process on each MCP session start (with its own cache file as a
  shortcut).
- **Memory profile**: a tokensave session is ~150 MiB RSS for a 2 700-file
  project regardless of how many tools are queried; token-savior loads the
  whole graph into the Python heap on each tool call sequence.

### 1.6 Per-call timing telemetry

`tokensave serve --timings` annotates every `tools/call` response with
`_meta.duration_us` — the pure handler execution time. Lets agents (and
benchmarks) attribute latency to actual query work vs. JSON-RPC / stdio /
parser overhead. There is no equivalent in token-savior.

### 1.7 Larger edit toolkit

tokensave exposes a hierarchy of edit primitives at three levels of
abstraction:

| Level | Tool | When to use |
|---|---|---|
| Symbol | `tokensave_replace_symbol` | "rewrite this function's body" |
| Symbol | `tokensave_insert_at_symbol` | "add a method before/after this one" |
| Anchor | `tokensave_insert_at` | "insert below this line" |
| Range  | `tokensave_str_replace`, `tokensave_multi_str_replace` | exact-string surgery |
| AST    | `tokensave_ast_grep_rewrite` | structural rewrite via ast-grep pattern |

Every edit reindexes the touched file automatically, so subsequent queries
see the new state without an explicit refresh.

### 1.8 Walk-up discovery + scope prefixes

If `tokensave serve` is launched from a subdirectory of an indexed project,
it (a) walks up the filesystem to find the `.tokensave/` dir, (b) computes a
`scope_prefix` from the relative path, and (c) applies that prefix as a
default file filter on every query. The agent sees results from "where it
lives" without needing to know the project root.

### 1.9 First-class CLI on top of the MCP

Every MCP tool has a parallel CLI subcommand. Useful for one-off inspection
or shell pipelines without spinning up an MCP client:

```bash
tokensave query handle_body
tokensave body  handle_body
tokensave impact handle_body --max-depth 3
tokensave callers extract_lines
tokensave files --pattern '**/*.rs'
tokensave affected src/cli.rs
tokensave bench       # built-in retrieval benchmark
```

### 1.10 Reproducible benchmark harness

`tokensave bench` ships with a default TOML query set
(`benchmarks/queries/default.toml`) and emits a colored table or JSON of
retrieval-savings ratios for the current project. token-savior's benchmarks
are external (`Mibayy/tsbench`) and require a synthetic project.

---

## 2. Where tokensave doesn't go (and what we do instead)

### 2.1 Config / env-var linting

token-savior's `analyze_config(checks=["orphans", "secrets"])` cross-references
declared env vars against actual reads, flags orphan declarations
(`ORPHAN-*`), undeclared reads (`UNDECL-*`), and embedded secrets
(`SECRET-*`). tokensave does not parse `.env*` files. **Workaround**: agents
fall back to `Read` + `Grep` on `.env*` plus reading the relevant
`os.environ.get` / `process.env.X` sites via `tokensave_search`.

### 2.2 Dockerfile linting

`analyze_docker` checks for common Dockerfile anti-patterns (`DOCKER-*`,
`INFRA-*`) — unsafe `apt-get` flags, missing layer caching, root user, etc.
tokensave indexes Dockerfiles as files but doesn't lint them. **Workaround**:
`Read` the Dockerfile, judge against the rubric.

These two tools are *linters*, not code-intelligence tools. They're useful
in agentic workflows, but they belong in their own category. We chose to
keep tokensave focused on code-graph queries.

### 2.3 Language-specific refactor primitives

token-savior ships:

- `add_field_to_model` — Prisma / Pydantic / dataclass / TS interface
  field insertion
- `move_symbol` — function-or-class move with cross-file import rewrite

tokensave doesn't have these. **Workaround**: combine
`tokensave_replace_symbol` / `tokensave_str_replace` / `tokensave_insert_at`
with `Edit` for import-site fixups. On `tsbench` this scored full marks on
TASK-012 (the canonical add-field-to-model task) without a dedicated tool —
the agent uses `tokensave_str_replace` against the model block directly.

These are arguably out of scope: a single `add_field` tool needs syntax
support per (language × ORM × type-system) combination; a single
`move_symbol` needs robust import-graph rewriting per language. Both are
multi-thousand-LoC commitments per dialect.

### 2.4 Shell-output compaction (handled by RTK)

tokensave's scope is the *code-graph query layer*; it deliberately does not
intercept shell-tool output. For that layer, pair tokensave with
[RTK (Rust Token Killer)](https://github.com/aovestdipaperino/rtk) — a
transparent CLI proxy that compacts verbose `git diff`, `kubectl logs`,
`pytest`, `gh run view`, etc. before they hit the agent's context. RTK is
a separate concern with no overlap with tokensave's code-graph
responsibilities.

### 2.5 Batched name lookups

token-savior accepts `names=[...]` lists in find / get_function_source /
get_full_context: one round-trip retrieves multiple symbols. tokensave's
equivalent tools each take a single symbol or node_id. Issuing N MCP calls
pays N × stdio JSON-RPC overheads (~300 µs each per the §3 benchmark).
For typical agent workloads — 5–10 queries per task — the absolute cost is
sub-millisecond per call, but at hundreds of queries it adds up.
**Mitigation**: `tokensave_context` retrieves a relevance-ranked bundle of
related symbols in one call when the task is open-ended, which usually
substitutes for batch lookups.

### 2.6 Cross-project memory

token-savior maintains a project-keyed memory store (`memory_search`,
`memory_save`) so notes from one project surface in another. tokensave has
narrower `tokensave_record_decision` / `tokensave_record_code_area` /
`tokensave_session_recall` tools but they're scoped to a single project
and not yet wired into a global recall mechanism.

---

## 3. Indexer-level benchmark (FastAPI)

Adapted from token-savior's own `benchmarks/run_benchmarks.py`. Same clone
of FastAPI, same random sample of symbols (seed=42), shared between both
tools so per-query rows are directly comparable. tokensave is driven through
a long-lived `tokensave serve --timings` session — the per-query column
reports the handler's `_meta.duration_us`, stripping JSON-RPC / stdio /
Python-parse overhead. token-savior runs in-process.

Script: [`benchmarks/run_benchmarks.py`](../benchmarks/run_benchmarks.py).
Latest report: [`benchmarks/comparison-report.md`](../benchmarks/comparison-report.md).

| Metric | token-savior | tokensave | Delta |
|---|---:|---:|---:|
| Cold index | 6.21 s | **2.15 s** | **tokensave 2.9× faster** |
| Warm reindex | 1.58 s | **1.36 s** | tokensave 1.2× |
| Cache / DB size | **5.2 MiB** | 28.6 MiB | token-savior 5.5× smaller |
| Files indexed | 2 715 | 2 677 | tied |
| Symbols / nodes | 2 740 | **29 019** | tokensave 10× richer graph |
| `find_symbol` avg | **0.004 ms** | 0.136 ms | token-savior 34× (dict vs SQLite probe) |
| `get_function_source` avg | 0.133 ms | **0.117 ms** | tokensave 1.1× |
| `get_change_impact` avg | 24.37 ms | **0.57 ms** | **tokensave 43× faster** |

The compact-on-disk row goes to token-savior: it stores a JSON dict of
symbols, while tokensave stores the full graph (typed nodes + edges) in
SQLite. The 10× symbol/node count is what you pay for that — and what
makes the impact-radius walk possible.

The `find_symbol` row is the only tokensave loss on lookup latency, and
it's a real difference: token-savior's `dict.get(name)` is ~30× faster
than a SQLite index probe (4 µs vs 136 µs). At agent timescales (seconds
per turn) the absolute gap is invisible, but the architectural cost is
real and worth knowing.

---

## 4. Agent-level benchmark (`Mibayy/tsbench`, 96 tasks)

The agent benchmark Mibayy uses to publish token-savior's 97.9% headline:
96 synthetic tasks on a generated SaaS-style project, scored against
`GROUND_TRUTH.json`. Each task: Claude Opus 4.7 driven via `claude -p`,
armed with the MCP server under test plus a tightly tuned system prompt.

Our adaptation:

- Fork `bench.py` → `bench_tokensave.py`. Swap the MCP config to launch
  `tokensave serve -p <root> --timings`.
- Rewrite `SYSTEM_PROMPT_TS` to map each token-savior tool to its tokensave
  equivalent (or, where none exists, a `Read` / `Edit` fallback).
- Relax `--disallowedTools` from `["Read","Grep","Glob","Agent"]` to
  `["Agent"]` only — needed because tokensave doesn't provide
  `analyze_config` / `analyze_docker` / `add_field_to_model`.
- Update the prefix matcher to `mcp__tokensave__*`.
- No CLAUDE.md baking, no `--bare` (Max OAuth incompatibility), no
  rubric-specific prompt tuning.

### Headline

| | tokensave | token-savior v4.0 |
|---|---:|---:|
| **Score** | **184 / 192 (95.8%)** | 188 / 192 (97.9%) |
| Score distribution | 90 × 2/2, 4 × 1/2, **2 × 0/2** | 92 × 2/2, 4 × 1/2, 0 × 0/2 |
| Tasks completed | 96 / 96 | 96 / 96 |
| Wall time / task | 18.1 s | 18.9 s |
| Active tokens / task | **695** | 3 395 |
| Cache_creation / task | 18 589 | 2 560 |
| Tool-call mix (TS / total) | 97 / 147 (66% TS) | n/a |
| Wall time, full run | 29 min | n/a |

### The 8 lost points

| Task | Cat. | Score | Root cause |
|---|---|:-:|---|
| TASK-008 | audit | 0/2 | `tokensave_dead_code` returns a different candidate set than token-savior's `find_dead_code` — semantic mismatch with the ground-truth `DEAD-*` IDs |
| TASK-015 | edit | 0/2 | `tokensave_insert_at_symbol` places the new function differently than `insert_near_symbol` would have — both succeed, but the grader expects token-savior's placement convention |
| TASK-007 | explanation | 1/2 | Entry-point answer missing one rubric keyword |
| TASK-018 | debug | 1/2 | **Self-inflicted**: `tokensave_redundancy` correctly flagged our `bench_tokensave.py` fork as a real duplicate of `bench.py`; the grader expected only the curated `DUP-*` pairs |
| TASK-086 | documentation | 1/2 | Module README missing rubric keywords |
| TASK-087 | documentation | 1/2 | Same class — token-savior's BENCHMARK-SUMMARY lists TASK-087 as one of their own known 1/2 misses |

### What's notable

- **First-attempt, untuned.** token-savior reports 97.9% after multiple
  rounds of audit + tuning (April–May 2026) including memory-hook fixes,
  schema thinning, and rubric-matched vocabulary in the system prompt.
  tokensave's 95.8% is the *first run* of an unmodified-rubric
  translation. Polishing the dead-code mapping (TASK-008) and the
  insertion-point semantics (TASK-015) alone should close most of the gap.
- **~5× more active-token-efficient per task.** 695 vs 3 395 active
  tokens. The cache_creation gap (18 589 vs 2 560) is from running
  without `--bare` (Max OAuth required it off in our environment); with
  `--bare` enabled the gap would close substantially.
- **66% MCP tool usage.** Two-thirds of tool calls were `mcp__tokensave__*`;
  the rest were Read / Grep / Edit / Bash fallbacks on the four task
  categories (config audit, Docker audit, field addition, complex moves)
  where tokensave has no direct tool. Zero `CANNOT_ANSWER` outcomes.
- **No timeouts, no harness errors.** All 96 tasks completed cleanly in 29
  minutes wall time across one Max OAuth session.

Reproduction harness, patch, and full per-task summary in
[`benchmarks/tsbench/`](../benchmarks/tsbench/) — see
[`benchmarks/tsbench/SUMMARY.md`](../benchmarks/tsbench/SUMMARY.md) and
[`benchmarks/tsbench/README.md`](../benchmarks/tsbench/README.md).

---

## 5. When to use which

**Use tokensave when:**
- Your project uses a language outside token-savior's annotator set
  (Java, Kotlin, Scala, Swift, C++, Ruby, PHP, Dart, Lua, OCaml,
  Haskell, Elixir, Erlang, Clojure, etc.).
- You need typed-edge queries (`implements`, `extends`, `type_of`,
  `annotates`, impact radius, call chains, type hierarchies) — these
  require AST-level parsing, not regex.
- You need cross-branch indexing, durable on-disk state, or live file
  watching.
- You need health analytics (hotspots, coupling, dsm, redundancy,
  dependency depth, complexity rollups).
- You want the lowest cold-index time and the lowest impact-analysis
  latency.
- You're running an agent that does graph-heavy work (refactors,
  cross-cutting renames, impact reviews).

**Use token-savior when:**
- Your project is in one of its six supported languages (Python,
  TS/JS, Rust, Go, C, C#) and you mostly need symbol lookups +
  file outlines — regex annotators are simple, permissive on
  partial code, and have negligible startup cost.
- You want `analyze_config` / `analyze_docker` linting out of the box.
- You're running on a token-savior-tuned tsbench rubric and need the
  last ~2 percentage points of score.

For shell-output token reduction (verbose `git diff` / `kubectl logs` /
`pytest` outputs), pair tokensave with
[RTK](https://github.com/aovestdipaperino/rtk) — that's a separate
problem from code-graph queries and RTK addresses it directly.
