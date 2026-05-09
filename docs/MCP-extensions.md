# MCP Extensions — Candidate Tools

Patterns observed across Claude Code sessions that current tokensave tools don't cover well.
Each entry explains what triggered it, what the workaround looked like, and what the tool should do.

## Telemetry summary (2026-05-04)

Cross-project scan of ~32 project transcripts (~161 MB) backing the gap analysis below:

| Metric | Count |
|---|---|
| Total Bash invocations | 4591 |
| Total Read calls | 2023 (1571 with offset/limit) |
| Total tokensave tool calls | ~200 |
| `cargo check/test/clippy/build` | 777 |
| `grep` for source code | 1049 (618 for symbol-skeleton extraction) |
| `find -name` for files | 92 |
| `sed -n 'A,Bp'` line-range reads | 52 |
| `git log` / `show` / `blame` | 130 / 32 / 1 |

The 23× ratio of Bash : tokensave for code research is the headline. Three highest-volume
gaps — compile diagnostics, file-symbol skeletons, and AST-pattern search — are responsible
for roughly 60% of it. New entries below (`tokensave_diagnostics`, `tokensave_unsafe_patterns`,
`tokensave_signature_search`, etc.) target each in turn; the existing `tokensave_outline`
proposal already covers the symbol-skeleton case and is reinforced by the new evidence.

---

## `tokensave_field_sites`

**Trigger:** Evolving a struct field — adding `last_sync_duration_ms` to `GraphStats` required
finding every place `last_sync_at` was *written* across the codebase. After `tokensave_search`
identified the symbol, 4 separate `grep` passes were needed to locate all write sites in different
files and function bodies.

**Gap:** `tokensave_callers` covers call sites of functions/methods. Nothing covers field
reads vs. writes.

**Proposed API:**
```json
{ "field": "last_sync_at" }
{ "field": "GraphStats::last_sync_at" }
```

**Returns:** Two lists — `write_sites` and `read_sites` — each with file, line, enclosing
function, and a short code snippet. Optionally filterable to writes-only.

**Value:** Any time a field is renamed, removed, or gets a new invariant, the write-site list
is the exact blast radius. Currently requires multi-file grep + manual triage.

---

## `tokensave_implementations`

**Trigger:** Adding 9 new language extractors required understanding the existing extractor
interface. Multiple `grep -r "fn extensions"` and `grep -r "fn language_name"` passes were made
across all `*_extractor.rs` files to see how existing types implement the trait.

**Gap:** `tokensave_type_hierarchy` handles class inheritance. Nothing returns "all types that
implement method X" with their concrete bodies.

**Proposed API:**
```json
{ "trait": "LanguageExtractor" }
{ "method": "extensions" }
{ "method": "count_complexity" }
```

**Returns:** For each implementing type: the type name, file, and the method body (or bodies, if
overloaded). When querying by trait, returns all method implementations grouped by type.

**Value:** The canonical "how does an existing implementation look?" question when adding a new
implementor to an interface. Currently answered by grepping for each method name separately.

---

## `tokensave_constructors`

**Trigger:** Adding a required field `last_sync_duration_ms` to `GraphStats` required finding
every struct literal `GraphStats { ... }` to add the new field. `tokensave_callers` doesn't cover
struct construction — only function calls.

**Gap:** Struct literal construction sites are invisible to the existing call-graph tools.

**Proposed API:**
```json
{ "struct": "GraphStats" }
```

**Returns:** Every location where the struct is instantiated as a literal (not via a constructor
function), with file, line, and the field list present in that literal. Bonus: flag sites that
are missing the requested field (useful when you've just added one).

**Value:** Making a struct field required is a frequent refactor. The `cargo check` error list
covers this for Rust, but the tool gives the list *before* compilation, with context, and works
for languages without exhaustive struct checking.

---

## `tokensave_outline`

**Trigger:** In quality-improvement sessions, the common opening move was
`wc -l src/main.rs && grep -n "^pub fn\|^pub struct\|^pub enum\|^impl"` — a "get bearings"
sweep before diving into a large file. `tokensave_module_api` shows only the public API;
`tokensave_context` is heavier than needed when you just want navigation landmarks.
The cross-project scan found **618** invocations of this pattern, the single largest
grep category — confirming it as a recurring opening move on essentially any large file.

**Gap:** No lightweight "what's in this file?" dump that includes private symbols with line numbers.

**Proposed API:**
```json
{ "file": "src/tokensave.rs" }
{ "file": "src/tokensave.rs", "kinds": ["function", "struct", "impl"] }
```

**Returns:** A flat list of `{kind, name, line, visibility}` for every top-level symbol in the
file, sorted by line. No code bodies — just the map. Response should be cheap (graph lookup,
no snippet extraction).

**Value:** Turns "where is X defined in this file?" from a Read + manual scan into a single
call. Also useful as a pre-flight before `tokensave_context` — orient first, then zoom.

---

## `tokensave_feature_map`

**Trigger:** Investigating whether logging was implemented required
`grep -r "setLevel\|logging\|log_level\|LoggingLevel\|set_level"` across the entire project.
The concept could live in function names, config keys, environment variable names, string
literals, or comments — `tokensave_search` only matches symbol names.

**Gap:** Symbol search only covers named graph nodes. Concepts encoded in string literals,
env var names, config keys, and comments are invisible.

**Proposed API:**
```json
{ "concept": "logging" }
{ "concept": "authentication", "keywords": ["auth", "token", "session", "credential"] }
```

**Returns:** Matching symbols (as `tokensave_search` would), plus: string literal occurrences,
comment occurrences, and config key matches — each with file and line. Grouped by match type
so symbol hits stay prominent.

**Value:** The "is feature X even present in this codebase?" probe. Especially useful for
inherited or unfamiliar codebases where the feature may not use the obvious identifier names.

---

## `tokensave_diagnostics`

**Trigger:** `cargo check`, `cargo test`, `cargo clippy`, and `cargo build` were invoked
**777 times** across all scanned conversations, with single sessions hitting 100+. Each
invocation re-compiles, dumps raw error text into the conversation, and Claude then has to
parse it back into structured form — usually then `Read` the offending file to see context.
The same loop repeats in TypeScript projects via `tsc` and Python via `pyright`.

**Gap:** No tokensave tool wraps the compiler / type-checker. Errors arrive as flat text
disconnected from the graph; affected-set narrowing (already available via
`tokensave_affected`) is not used to scope checks.

**Proposed API:**
```json
{ "scope": "workspace" }
{ "scope": "affected", "since": "HEAD~1" }
{ "scope": "package", "name": "tokensave" }
{ "scope": "file", "path": "src/mcp/tools/handlers.rs" }
```

**Returns:** `errors`, `warnings`, each with `file`, `line_start`, `line_end`, `code`
(e.g. `E0599`, `unused_imports`), `message`, `enclosing_symbol` (graph node id), and a
short pre-extracted snippet. Caches the last clean state and the result digest so a
no-op re-run is free.

**Value:** Replaces a 3-step loop (run cargo → parse text → read file) with a single
structured response that's already mapped to graph nodes. The biggest single Bash : tokensave
gap by raw count. Affected-set scoping turns 30-second full builds into sub-second
incremental checks during refactor cycles.

---

## `tokensave_unsafe_patterns`

**Trigger:** Security and quality-review conversations repeatedly grep for
`panic!`, `unwrap()`, `expect(`, `todo!()`, `unimplemented!()`, and `unsafe` blocks.
Same pattern surfaces under "production-validator" and "rust-performance-reviewer"
agent runs. Regex grep produces false positives (string literals containing the word,
identifiers like `unwrapped_value`).

**Gap:** No AST-aware finder for these markers. `tokensave_todos` covers comment-style
markers (`TODO`, `FIXME`) but not call-expression patterns like `.unwrap()` or `panic!()`.

**Proposed API:**
```json
{ "kinds": ["unwrap", "expect", "panic", "todo", "unimplemented", "unsafe_block"] }
{ "kinds": ["unwrap"], "path": "src/mcp/" }
{ "kinds": ["panic"], "exclude_tests": true }
```

**Returns:** Per match: `kind`, `file`, `line`, `enclosing_symbol`, `snippet`, and
`in_test` flag. AST-matched, so `let unwrapped = ...` and `"do not unwrap"` don't false-fire.

**Value:** Replaces a class of recurring greps in review/audit work. The `in_test` flag
matters because a `.unwrap()` in a test is fine; in `lib.rs` it's a panic site.
Pairs with `tokensave_test_risk` for "what risky paths aren't tested?"

---

## `tokensave_signature_search`

**Trigger:** Refactor questions of the form "find all functions returning
`Result<_, MyError>`", "find all `impl Display for ...`", "find every async function
that takes `&mut self`". Currently answered by `tokensave_search` (name-only) plus
multi-file `Read` to inspect signatures one by one.

**Gap:** `tokensave_search` matches on symbol names. `tokensave_implementations`
(also proposed) covers trait/method implementor lists. Neither searches by
signature shape — return types, parameter types, generic bounds, attributes.

**Proposed API:**
```json
{ "returns": "Result<_, anyhow::Error>" }
{ "returns": "impl Future" }
{ "params": ["&mut self", "_"], "async": true }
{ "attribute": "#[tokio::test]" }
```

**Returns:** Matching symbols with `file`, `line`, `signature`, `kind`, and the matched
sub-pattern. Implementation can reuse the AST matcher already feeding `tokensave_ast_grep_rewrite`.

**Value:** Unlocks signature-based refactoring questions that currently force a grep
+ manual filter. Smaller volume than `tokensave_outline` but high token cost when it
does come up — a single signature query can replace dozens of file Reads.

---

## `tokensave_lockfile_diff`

**Trigger:** Release work repeatedly hits patterns like
`git show v3.4.4:Cargo.lock | grep -A5 'name = "zip"'` to figure out what dep versions
changed between two refs. Same pattern for `package-lock.json`, `pnpm-lock.yaml`, `uv.lock`.

**Gap:** No tool parses lockfiles. `tokensave_branch_diff` covers symbol-level diffs
but not dependency version changes.

**Proposed API:**
```json
{ "from": "v3.4.4", "to": "v3.4.5" }
{ "from": "main", "to": "HEAD", "package": "zip" }
```

**Returns:** Lists of `added`, `removed`, `bumped` (with old/new version), and
`yanked`. Detects lockfile format from the path.

**Value:** Single call to answer "what shifted in deps for this release?", which today
takes 2-3 `git show | grep` invocations per package.

---

## `tokensave_external_node`

**Trigger:** Many conversations dive into `~/.cargo/registry/src/...` to read the source
of a dependency (figuring out an API, debugging a panic from inside a crate). This requires
`find` plus path navigation plus `Read`. Matching pattern in TS via `node_modules/`.

**Gap:** Tokensave's graph is project-local. External symbols are unreachable; users have
to leave the graph entirely.

**Proposed API:**
```json
{ "crate": "tokio", "symbol": "spawn" }
{ "crate": "anyhow", "version": "1.0.86", "symbol": "Error::root_cause" }
```

**Returns:** Resolved file path inside the registry, `signature`, `body`, and `doc`
when present. Optionally indexes the crate on demand into a side graph so subsequent
`tokensave_callers`-style queries work against external code.

**Value:** Removes the registry-spelunking workaround. On-demand indexing is a larger
investment but turns "I need to read a dep's source" into a single graph query.

---

## `tokensave_config`

**Trigger:** This repo (and most others) sees frequent greps over `Cargo.toml`,
`.github/workflows/*.yml`, `tsconfig.json`, `pyproject.toml`, `package.json` during
release/CI work — 62 such greps recorded. Tokensave's tree-sitter pipeline doesn't
parse YAML/TOML structurally.

**Gap:** No structured query into config files. Users grep for keys, then re-read
to verify the value.

**Proposed API:**
```json
{ "path": "Cargo.toml", "key": "package.version" }
{ "path": ".github/workflows/release.yml", "key": "jobs.publish.steps[*].uses" }
{ "glob": "**/Cargo.toml", "key": "dependencies.tokio" }
```

**Returns:** Parsed value(s), `file`, `line` of the key, and on `glob` queries a
list of matches across the workspace.

**Value:** Replaces a recurring grep + re-read pattern in release / CI conversations.
Smaller blast radius than diagnostics but disproportionately common in the tokensave
repo's own conversation history.

---

## `tokensave_macro_expand`

**Trigger:** Rust-specific. When debugging a macro-heavy file (procedural macros, `derive`,
custom `macro_rules!`), tokensave's tree-sitter view shows pre-expansion source — which
hides the actual generated code that produces the compiler error. Workaround is `cargo expand`,
which is slow and dumps an entire crate's worth of post-macro source.

**Gap:** No expanded-source view tied to a specific symbol or file region.

**Proposed API:**
```json
{ "symbol": "MyStruct" }
{ "file": "src/foo.rs", "line": 42 }
```

**Returns:** Expanded source for the requested region only, with `file`, `line_range`,
and `source` (post-expansion). Caches per-revision so repeated queries on a clean tree
are free.

**Value:** Niche but high token-cost when it does come up — a single `cargo expand`
on a workspace can be tens of thousands of lines. Region-scoped expansion is the difference
between "usable" and "fall back to raw source."

---

## Design observation: heavy file re-reads

Not a tool proposal — but the scan found single-conversation Read counts of 76× for
`src/tokensave.rs`, 85× for `claurst/src-rust/crates/api/src/lib.rs`, 47× for
`src/daemon.rs`. This suggests either (a) the assistant doesn't trust prior cached snippets
to still be current, or (b) it lacks a "what's changed in this file since my last call"
signal and re-reads defensively. Worth investigating before adding more tools — could be
a `tokensave_context` mode flag (`incremental: true`) that returns only deltas since the
last call within a session, rather than a new top-level tool.

---

---

## `tokensave_body` ✅ implemented

**Status:** Shipped. Handler at `src/mcp/tools/handlers.rs:handle_body`, definition at
`src/mcp/tools/definitions.rs:def_body`. Tests in `tests/mcp_handler_test.rs` (3 cases).

**Trigger:** In `claurst` (a Rust project separate from tokensave), the dominant navigation
pattern was `grep -A 20 "pub fn resolve_provider_api_key"` — reading a function or constant body
by name without knowing which file it lives in. The same `grep -A N` form appeared 15+ times for
functions, constants (`CCH_SEED`, `ANTHROPIC_BETA_HEADER`, `CLIENT_ID`), and struct fields.
Tokensave was not active for that project, but the pattern maps directly to what
`tokensave_search` + file offset `Read` + manual body extraction would do in three steps.

**Gap (closed):** No tool took a symbol name and returned its source body in a single call.
The previous flow was: `tokensave_search` → read `file:line` → `Read` with offset → extract
manually.

**Shipped API:**
```json
{ "symbol": "resolve_provider_api_key" }
{ "symbol": "CCH_SEED", "limit": 5 }
{ "symbol": "GraphStats::last_sync_at" }
```

**Returns:** `match_count` and a `matches` array. Each match has: `id`, `name`,
`qualified_name`, `kind`, `file`, `start_line`, `end_line`, `signature`, `body`. Exact name
matches are preferred over fuzzy matches; falls back to ranked search results when no exact
match exists.

---

## `tokensave_todos` ✅ implemented

**Status:** Shipped. Handler at `src/mcp/tools/handlers.rs:handle_todos`, definition at
`src/mcp/tools/definitions.rs:def_todos`. Tests in `tests/mcp_handler_test.rs` (3 cases).

**Trigger:** In `bruto-pascal-lang`, three separate grep passes were made across the whole source
tree to find `TODO`, `FIXME`, `XXX`, `HACK`, `WIP`, and `unimplemented!()` markers — each
refining the pattern to catch more variants. These were used to build a "what needs finishing"
picture before starting a work session.

**Gap (closed):** No tool queried the project for annotation/comment markers with structured
output including the enclosing symbol.

**Shipped API:**
```json
{}
{ "kinds": ["TODO", "FIXME", "UNIMPLEMENTED"] }
{ "path": "src/codegen.rs", "limit": 50 }
```

**Returns:** `match_count`, `by_kind` count summary, and a `markers` array. Each marker has:
`kind`, `file`, `line`, `text` (trimmed line content), `enclosing` (qualified name of the
smallest enclosing symbol, or null if at file scope). Word-boundary matching prevents
false positives like `todoist`.

**Defaults:** `TODO`, `FIXME`, `XXX`, `HACK`, `WIP`, `NOTE`, `UNIMPLEMENTED`. Matched
case-insensitively. Default limit is 200, max 2000.

---

## Notes / Prioritization

Observed across all projects (tokensave + claurst + bruto-pascal + amexx + others). The
`Evidence` column shows raw count of the workaround pattern from the 2026-05-04 telemetry
scan, where measurable.

| Tool | Status | Evidence | Complexity | Impact |
|---|---|---|---|---|
| `tokensave_body` | ✅ shipped | 1571 targeted Reads + 52 sed -n | Low | **High** |
| `tokensave_todos` | ✅ shipped | scattered across projects | Low | Medium |
| `tokensave_diagnostics` | proposed | 777 cargo invocations | High (compiler integration) | **Very high** |
| `tokensave_outline` | proposed | 618 symbol-skeleton greps | Very low (file → node list) | **High** |
| `tokensave_unsafe_patterns` | proposed | recurring in review/audit | Low (AST predicates) | High |
| `tokensave_implementations` | proposed | tokensave 22ff55cd, 67f09223 | Low (method edges) | High |
| `tokensave_signature_search` | proposed | smaller volume, high token cost | Medium (AST matcher) | High |
| `tokensave_field_sites` | proposed | tokensave e790d3c4, 67f09223 | Medium (field write edges) | High |
| `tokensave_constructors` | proposed | tokensave e790d3c4 | Low (struct-literal node kind) | Medium |
| `tokensave_feature_map` | proposed | tokensave 67f09223, others | High (full-text index) | Medium |
| `tokensave_config` | proposed | 62 config greps | Medium (TOML/YAML parser) | Medium |
| `tokensave_lockfile_diff` | proposed | 6 lockfile greps | Low (lockfile parser) | Low-Medium |
| `tokensave_external_node` | proposed | recurring registry spelunking | High (on-demand indexing) | Medium |
| `tokensave_macro_expand` | proposed | niche, high cost when used | High (cargo expand integration) | Low-Medium |

**Build order recommendation (revised after telemetry scan):**

1. `tokensave_outline` — trivial query, addresses the single largest grep category (618 hits).
   One afternoon.
2. `tokensave_unsafe_patterns` — AST predicates on top of the existing matcher. Replaces a
   recurring review-time grep family. Half-day.
3. `tokensave_diagnostics` — biggest single Bash : tokensave gap. Highest impact even though
   complexity is the highest in this set; structured cargo errors mapped to graph nodes
   compress hundreds of recurring tool cycles. Multi-day.
4. `tokensave_implementations` — query existing method edges filtered by implementor. Low risk.
5. `tokensave_signature_search` — extends the matcher used by `ast_grep_rewrite`; small surface.
6. `tokensave_constructors` — depends on struct-literal edge kind; check schema.
7. `tokensave_field_sites` — requires field-level read/write edges; may need schema work.
8. `tokensave_config` — TOML/YAML parsers exist; mostly path-and-key resolution.
9. `tokensave_lockfile_diff` — small, isolated, parser-bound.
10. `tokensave_feature_map` — requires a parallel full-text index; largest investment.
11. `tokensave_external_node` — on-demand indexing of registry crates is a meaningful design
    project; defer until prior items show whether the demand is sustained.
12. `tokensave_macro_expand` — niche; integrate `cargo expand` only if `tokensave_diagnostics`
    work surfaces a clear tie-in (macro-generated errors).

Before starting, also weigh the **heavy re-read observation** above — solving that signal
inside `tokensave_context` (incremental delta mode) might remove a class of redundant Reads
without any new tool.

`tokensave_field_sites`, `tokensave_feature_map`, and `tokensave_external_node` may need a
graph schema audit before scoping. The others should be buildable against the existing
node/edge model.
