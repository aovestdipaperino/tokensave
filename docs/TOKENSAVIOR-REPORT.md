# Token Savior vs Tokensave -- Feature Delta Analysis

Both projects share the same core mission: structural code navigation via MCP
to reduce token consumption. Token Savior is Python (105 tools), tokensave is
Rust (38 tools). The navigation cores are comparable -- both index by symbol,
resolve callers/callees, compute impact, detect dead code, etc. The interesting
delta is in the features token-savior has built *around* the code graph.

Source: <https://github.com/mibayy/token-savior>

---

## High-Value Features Worth Porting

### 1. Persistent Memory Engine (21 tools)

The single largest feature gap. Token Savior stores "observations" (decisions,
bugfixes, conventions, guardrails, research notes) in SQLite with FTS5 +
optional vector embeddings (`all-MiniLM-L6-v2`, 384d, fused via RRF).

Key sub-features:

- **Observation types** (12 kinds: guardrail, convention, decision, bugfix,
  error_pattern, command, research, note, idea, etc.) with per-type TTL decay
- **Progressive disclosure** -- 3-layer contract:
  `memory_index` (~15 tokens/hit) -> `memory_search` (~60) -> `memory_get` (~200).
  Keeps memory lookups cheap by default
- **Bayesian validity scores** -- each observation carries a prior updated on
  access; stale ones are surfaced with decreasing confidence rather than
  silently trusted
- **Contradiction detection** at save time -- compares against existing
  observations and flags conflicts
- **ROI tracking** -- access_count x context_weight; unused observations age
  out, high-ROI ones get promoted
- **Auto-promotion** -- a `note` accessed 5+ times becomes a `convention`;
  a `warning` accessed 5+ times becomes a `guardrail`
- **MDL (Minimum Description Length) distillation** -- compresses redundant
  observations into conventions
- **Symbol staleness** -- observations linked to symbols are invalidated when
  the symbol's content hash changes
- **Inter-agent memory bus** -- volatile observations shared between subagents
  during a session

**Effort:** Large -- this is a full subsystem. But it directly addresses the
"Claude forgets everything between sessions" problem. Tokensave could either
build this natively in Rust (big lift) or expose hooks that integrate with
Claude Code's existing auto-memory system. A middle ground: implement a
lightweight observation store in the existing SQLite DB with FTS5, decay, and
the 3-layer progressive disclosure pattern.

### 2. Context Packer (knapsack optimization)

Given a token budget and candidate symbols (each with a relevance score and
token cost), picks the optimal bundle via greedy fractional knapsack. The
`score_symbol()` function combines:

- Jaccard similarity to query
- Call-graph distance proximity
- Recency (days since last touch)
- Access frequency

**Effort:** Small-medium. Tokensave's `tokensave_context` already does
relevance ranking, but the explicit budget-aware knapsack with composite
scoring is more principled. Could improve context quality significantly.

### 3. Program Slicer (backward slice)

Given a variable and a line, computes the minimal set of statements that
affect that variable's value. Uses AST-level data-dependency analysis
(assignments, AugAssign, for-target bindings) plus enclosing control-flow.
Reports 92% line reduction.

**Effort:** Medium. Currently Python-only (uses `ast` module). For Rust,
would need tree-sitter-based equivalent. Very useful for debugging scenarios
("why is X wrong at line N").

### 4. Breaking Change Detection

Compares working tree against a git ref and reports functions/methods whose
signatures changed incompatibly. Classifies as:

| ChangeType | Severity |
|---|---|
| SIGNATURE_CHANGED | breaking |
| REMOVED | breaking |
| BODY_ONLY_CHANGED | info |
| ADDED | info |

**Effort:** Medium. Tokensave already has `tokensave_diff_context` for
symbol-level diffs but doesn't classify them by breaking-change severity.
Adding a severity classifier on top of the existing diff would be relatively
straightforward.

### 5. Edit Verifier (Proof-Carrying Edits)

Before applying a symbol replacement, answers four static-analysis questions:

1. Is the public signature preserved?
2. Are tests available for this symbol?
3. Are raised exceptions unchanged?
4. Are external side effects unchanged?

Produces a SAFE TO APPLY / REVIEW REQUIRED certificate.

**Effort:** Medium. Language-specific (currently Python AST). Would need
tree-sitter-based approach for multi-language support. High value for safe
refactoring workflows.

### 6. Markov Prefetcher

Learns tool-call transition probabilities across sessions (e.g., after
`get_function_source(X)`, the next call is `get_dependents(X)` 70% of the
time). Pre-warms likely next responses.

**Effort:** Small. Simple first-order Markov model persisted as JSON.
Could reduce latency on sequential navigation patterns. Tokensave already
tracks tool calls for stats -- extending to transition probabilities is
incremental.

### 7. TCA (Co-Activation Tensor)

Records which symbols are touched together in a session, computes normalized
PMI scores. At query time, returns top-K co-actives of a seed symbol. "If
you're looking at symbol A, you probably also need symbol B."

**Effort:** Small. JSON-persisted pairwise counts. Directly enhances
`tokensave_context` relevance ranking.

### 8. LinUCB Contextual Bandit for Injection Ranking

Uses the LinUCB algorithm (Li et al. 2010) to learn which observations are
most useful to inject given the current context. 10-dim feature vector:
type_score, age_score, access_score, semantic_sim, mode_match,
tokens_used_pct, task_is_edit, task_is_debug, symbol_match, has_context.
Pure Python linear algebra, no numpy.

**Effort:** Small-medium. Only relevant if the memory engine is ported.
Elegant solution to the "what to inject at session start" problem.

### 9. Session Warm-Start

Computes a 32-dim session signature vector and finds similar historical
sessions by cosine similarity to pre-warm caches/contexts. Signature
dimensions cover tool usage fractions, duration, turn count, mode one-hot,
and a hash projection of touched symbols.

**Effort:** Small. Only useful if session tracking and memory are implemented.

### 10. Checkpoints (file-level save/restore)

Create/list/delete/restore/diff checkpoints for bounded file sets before
mutations. Symbol-level diff against checkpoints.

**Effort:** Small. Tokensave has no file mutation tools, so this is only
relevant if edit operations are added.

---

## Features of Lower Priority

| Feature | Why Lower | Notes |
|---|---|---|
| Safe editing tools (replace_symbol_source, insert_near_symbol, move_symbol, add_field, refactor) | Claude Code already has Edit/Write tools | Would duplicate existing functionality |
| Test runner (run_impacted_tests) | Tokensave already has `tokensave_test_map`; running tests is better left to the shell | Adding test execution increases attack surface |
| Docker/multi-project | Niche | Low demand |
| Web viewer (htmx + SSE) | Nice-to-have diagnostics | Not core functionality |
| Telegram notifications | Niche integration | |
| LLM auto-extraction | Requires API key, adds cost | Better handled by hooks |
| Leiden community detection | Academic, diminishing returns over simpler clustering | |
| Tool profiles (full/core/nav) | Tokensave already loads all 38 tools | Useful at 105 tools, less so at 38 |

---

## Recommended Porting Priority

```
Priority 1 (high impact, tractable)
  Context Packer (budget-aware knapsack)          ~1-2 days
  Breaking Change Detection (severity layer)      ~2-3 days
  Markov Prefetcher (tool-call transitions)       ~1 day
  TCA Co-Activation Tensor (symbol pairs)         ~1 day

Priority 2 (high impact, larger effort)
  Memory Engine (lightweight version)             ~1-2 weeks
    Observation store + FTS5
    Progressive disclosure (3 layers)
    Decay + TTL
    Contradiction detection
  Program Slicer (tree-sitter based)              ~3-5 days

Priority 3 (contingent on Priority 2)
  Edit Verifier / Proof-Carrying Edits            ~3 days
  LinUCB Injection Ranking                        ~2 days
  Session Warm-Start                              ~1-2 days
  ROI Tracking + Auto-Promotion                   ~2 days
```

The **Context Packer** and **Co-Activation Tensor** are the quickest wins --
they directly improve `tokensave_context` quality with minimal new
infrastructure. The **Memory Engine** is the most transformative but also the
largest undertaking.
