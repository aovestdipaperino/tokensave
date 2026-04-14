# Implementation Plan: `tokensave_explore`

**Goal:** Port CodeGraph's `codegraph_explore` architectural pattern into tokensave,
closing the 92%-fewer-tool-calls gap on Explore agent sessions.

**Scope:** 4 changes — one new MCP tool, one helper on the DB/context layer,
two prompt rule updates (tool description + CLAUDE.md).  
No new crates, no schema migration, no breaking changes to existing tools.

---

## Background: what the gap actually is

`tokensave_context` with `include_code: true` is functionally equivalent to what
`codegraph_explore` does internally. The gap is **not** in the query logic; it is in:

1. **Output format** — `tokensave_context` returns structured symbol metadata with
   optional snippets. `codegraph_explore` returns flat, file-labelled source sections
   that the agent can reason about directly, without a second tool call.

2. **Call budget in the tool description** — the MCP tool description itself tells the
   agent how many times to call it. The agent stops exploring after the budget is
   exhausted. This one prompt-engineering trick is responsible for most of the
   tool-call reduction.

3. **Session deduplication** — successive calls within the same agent session should
   not repeat file sections already returned.

4. **Routing rules** — the current CLAUDE.md rules say "never spawn Explore agents".
   CodeGraph's rules say "if you spawn an Explore agent, give it this prompt". Both
   paths need to be covered.

---

## Change 1 — New function `build_explore_sections` in `src/context.rs`

This is the data layer. It wraps the existing `build_context` pipeline but changes
what it returns.

### What `build_context` currently returns (simplified)

```
ContextResult {
    entry_points: Vec<NodeSummary>,   // name, kind, file, line, optional snippet
    related: Vec<NodeSummary>,
    total_nodes: usize,
}
```

### What `build_explore_sections` must return

```rust
pub struct ExploreSection {
    /// Relative file path (e.g. "src/auth/login.rs")
    pub file: String,
    /// 1-based inclusive line range of the extracted source
    pub start_line: u32,
    pub end_line: u32,
    /// Language string for the fenced code block ("rust", "java", etc.)
    pub language: String,
    /// Full source text of the range
    pub code: String,
    /// One-sentence explanation of why this section is relevant
    pub relevance: String,
    /// Node ID(s) that caused this section to be included
    pub node_ids: Vec<String>,
}

pub struct ExploreSections {
    pub question: String,
    pub sections: Vec<ExploreSection>,
    /// Node IDs already returned — used for deduplication across calls
    pub seen_node_ids: HashSet<String>,
    /// Total nodes examined before pruning
    pub graph_nodes_visited: usize,
}
```

### Algorithm (in `build_explore_sections`)

```
fn build_explore_sections(
    db: &Database,
    question: &str,
    max_sections: usize,           // derived from node count, see Change 2
    exclude_node_ids: &HashSet<String>,  // already-seen from prior calls
) -> Result<ExploreSections>
```

Step 1 — **Entry point discovery** (reuse existing `context_search` logic):
  - FTS5 full-text search on `question` → up to 8 candidates
  - Vector cosine search (if embeddings available) → up to 8 candidates
  - Merge by score, deduplicate, take top 12

Step 2 — **Graph expansion** (reuse existing BFS in `build_context`):
  - From each entry point, follow outgoing `calls` and `imports` edges, depth 2
  - Follow incoming `calls` edges, depth 1 (callers are context for the callee)
  - Collect unique node IDs, excluding `exclude_node_ids`

Step 3 — **Source extraction** (new logic):
  - For each collected node, read its file from disk using `node.file_path`,
    `node.start_line`, `node.end_line`
  - Merge overlapping or adjacent ranges within the same file
    (avoid emitting the same file twice with 10-line gaps between ranges)
  - After merging, cap at `max_sections` sections by dropping lowest-scored nodes

Step 4 — **Relevance annotation** (lightweight):
  - For each section, generate a one-sentence relevance string from the node kind
    and the entry point that caused it:
    ```
    "Entry point for '{query}' — {kind} {name} called by {caller_count} callers"
    "Caller of `{entry_point}` — traces how this function is invoked"
    "Import dependency of `{entry_point}` — required by {n} symbols in the result"
    ```
  - No LLM call. Pure template strings from the graph metadata.

Step 5 — **Format output**:
  - Return `ExploreSections` with all sections, the union of seen node IDs,
    and graph visit count for the status line.

### Key implementation notes

- **File reads are synchronous** — `tokensave_explore` reads actual file bytes.
  This is a deliberate design choice: the index stores line numbers but not the full
  source text (to keep the DB small). Reading at query time costs ~1ms per file and
  is imperceptible at MCP latency.

- **Missing files** — if a file has been deleted since the last sync, skip the section
  and add a `// [file deleted since last sync]` note in the output.

- **Large files / large functions** — cap any single section at 300 lines. If a
  function is longer than 300 lines, emit lines `[start_line, start_line+300]` and
  append `// ... (truncated, N lines total)`.

- **Binary / non-UTF8 files** — handle the same way as sync: skip with a note.

---

## Change 2 — New MCP tool `tokensave_explore` in `src/mcp_tools.rs`

### Input schema

```json
{
  "name": "tokensave_explore",
  "description": "...",   // see below
  "inputSchema": {
    "type": "object",
    "properties": {
      "question": {
        "type": "string",
        "description": "Natural language question about the codebase. E.g. 'How does the payment retry logic work?' or 'Where is the JWT token validated?'"
      },
      "seen": {
        "type": "array",
        "items": { "type": "string" },
        "description": "Node IDs returned by previous tokensave_explore calls in this session. Pass the seen_node_ids from the previous response to avoid duplicate sections.",
        "default": []
      }
    },
    "required": ["question"]
  }
}
```

### Output format

The response is **plain text** (not JSON), formatted as Markdown source sections.
This is intentional: the agent reads it directly, like it would a file.

```
## tokensave_explore: "How does the payment retry logic work?"
Explored 847 nodes, returning 6 sections.

### src/payments/retry.rs  (lines 45–112)
```rust
pub fn schedule_retry(payment_id: Uuid, attempt: u32) -> Result<()> {
    // ... full source ...
}
```
> Entry point — matched "retry logic" via FTS5. Called by 3 callers.

### src/payments/retry.rs  (lines 180–203)
```rust
fn backoff_delay(attempt: u32) -> Duration {
    // ...
}
```
> Callee of `schedule_retry` — computes exponential backoff.

... (additional sections)

seen_node_ids: ["fn:src/payments/retry.rs:45:schedule_retry", ...]
Call budget: 2 of 5 used. Pass seen_node_ids to next call to avoid repeats.
```

The trailing `seen_node_ids` line and the call budget line are machine-readable
markers. The agent extracts them for the next call.

### Tool description (the critical part)

This is where the call budget lives. The description must be **dynamically generated**
based on project size at server startup, not hardcoded. Add a `explore_call_budget`
function called once when the MCP server initialises:

```rust
fn explore_call_budget(total_nodes: usize) -> u8 {
    match total_nodes {
        0..=5_000    => 3,
        5_001..=20_000  => 4,
        20_001..=80_000 => 5,
        80_001..=250_000 => 7,
        _            => 10,
    }
}
```

The tool description string (abbreviated — write the full version):

```
Use tokensave_explore to answer questions about how this codebase works.
It returns full source code sections for every relevant symbol in ONE call
— no grep, no file reads, no glob needed.

CALL BUDGET: {budget} calls maximum for this project ({node_count} nodes).
Stop after {budget} calls. If the question is not fully answered, synthesise
from what you have — do not exceed the budget.

WORKFLOW:
1. Call tokensave_explore with your question.
2. Read the returned source sections. They are the actual code — trust them.
3. If a section references something not yet explained, call again with a
   more specific follow-up question AND pass the seen_node_ids from the
   previous response to avoid duplicate sections.
4. Do NOT call Read, glob, grep, or list_directory. The source sections are
   the files. If you feel the urge to read a file, call tokensave_explore
   with a more targeted question instead.

OUTPUT: Markdown source sections with file paths, line numbers, and a
one-sentence relevance note per section.
```

The `{budget}` and `{node_count}` placeholders are filled at MCP server startup
using the project's node count from the DB, and the description is stored as a
`String` field on the MCP server struct (same pattern as other dynamic fields in
the codebase).

### Tool handler (in `handle_request` / the existing MCP dispatch match)

```rust
"tokensave_explore" => {
    let question = params.get_str("question")?;
    let seen_ids: HashSet<String> = params
        .get_array("seen")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let node_count = db.total_node_count()?;
    let budget = explore_call_budget(node_count);
    let max_sections = (budget as usize) * 3;  // ~3 sections per call worth of budget

    let result = build_explore_sections(&db, question, max_sections, &seen_ids)?;
    let text = format_explore_sections(&result, budget);

    accumulate_tokens_saved(&global_db, estimate_tokens_saved_explore(&result));
    Ok(mcp_text_response(text))
}
```

### Token-saving accounting

`tokensave_explore` is the highest-value tool — each call replaces many `Read` calls.
Estimate tokens saved as: `(sections.len() * 2 + 5) * AVG_FILE_TOKENS` where
`AVG_FILE_TOKENS` is a constant (e.g. 800). This is deliberately conservative.

---

## Change 3 — Update CLAUDE.md rules written by `tokensave install`

The existing rules (`src/agents/claude.rs`, `src/agents/mod.rs`) tell Claude:
> NEVER use Explore agents when tokensave is available.

Keep that rule. Add a **second paragraph** for the case where an Explore agent
is explicitly requested by the user or launched by a sub-task:

```markdown
## When you spawn an Explore agent in a tokensave-enabled project

If you do spawn an Explore agent (e.g. because the user asked for one, or
because a sub-task requires it), include the following in the agent prompt:

> This project has tokensave initialised (.tokensave/ exists). Use
> `tokensave_explore` as your ONLY exploration tool. Call it with your
> question in plain English. Do not call Read, glob, grep, or
> list_directory — the source sections returned by tokensave_explore ARE
> the relevant code. Follow the call budget in the tool description.
> Pass `seen_node_ids` from each response to the next call.

This prevents agents from falling back to file scanning even when the
main-session hook does not intercept the spawn.
```

Also add `tokensave_explore` to the `EXPECTED_TOOL_PERMS` array so that
`tokensave install` writes the permission grant and `tokensave doctor` validates it.

---

## Change 4 — Update `tokensave doctor` and `tokensave_status`

### `tokensave doctor`

Add one new check:

```
  MCP tools
  ✔ All 35 tool permissions granted   ← count goes from 34 to 35
```

If `tokensave_explore` is not in the permission list, show the existing
"run tokensave install" prompt.

### `tokensave_status` MCP tool

Add a line to the status output:

```
explore budget: {budget} calls  ({node_count} nodes)
```

This lets the agent (or developer) see at a glance what budget applies
to the current project.

---

## File-by-file change summary

| File | Change |
|------|--------|
| `src/context.rs` | Add `ExploreSection`, `ExploreSections` types; add `build_explore_sections()` function; add `format_explore_sections()` |
| `src/db.rs` (or `src/database.rs`) | Add `total_node_count() -> Result<usize>` if not already present |
| `src/mcp_tools.rs` (or wherever `get_tool_definitions` lives) | Add `def_tokensave_explore()` helper; generate description with dynamic budget |
| `src/mcp_server.rs` (or `main.rs`) | Store `explore_budget: u8` computed at startup; dispatch `tokensave_explore` in handle_request |
| `src/agents/claude.rs` | Extend `CLAUDE_MD_RULES` string with the Explore agent prompt paragraph |
| `src/agents/mod.rs` | Add `"mcp__tokensave__tokensave_explore"` to `EXPECTED_TOOL_PERMS` |
| `src/main.rs` (doctor command) | Increment expected tool count from 34 to 35 |
| `tests/` | Add `explore_test.rs` with at least: empty-question error, single-section result, seen-deduplication, budget-tier thresholds |

---

## What NOT to change

- **`tokensave_context`** — leave it exactly as is. It serves a different role
  (structured metadata for the main session or targeted lookups). Do not merge.

- **The PreToolUse hook** — keep blocking Explore agents. `tokensave_explore`
  handles the case where agents slip through or are explicitly spawned; the hook
  remains the primary defence.

- **Database schema** — no migration needed. `build_explore_sections` reads file
  paths and line numbers already stored in the `nodes` table, then reads source
  from disk.

- **Cargo.toml** — no new dependencies. File I/O uses `std::fs`. String formatting
  uses existing helpers.

---

## Acceptance criteria

1. `tokensave doctor` reports 35 permissions granted and shows the explore budget.

2. An Explore agent given only `tokensave_explore` (no file tools) can answer
   "how does X work?" for a mid-size Rust project in ≤ 4 calls, returning no
   duplicate sections across calls.

3. `seen_node_ids` from call N, passed as `seen` to call N+1, produces a disjoint
   set of sections (zero overlap in node IDs).

4. On a project with 0 nodes (freshly created, not indexed), `tokensave_explore`
   returns a clear error: `"Index is empty. Run tokensave sync first."`.

5. `tokensave install` on a fresh Claude Code setup writes the `tokensave_explore`
   permission and the updated CLAUDE.md paragraph.

---

## Estimated effort

| Task | Complexity | ~Lines |
|------|-----------|--------|
| `build_explore_sections` + types | Medium | ~180 |
| `format_explore_sections` | Low | ~60 |
| MCP tool definition + handler | Low | ~80 |
| Dynamic budget in description | Trivial | ~15 |
| Agent rules update | Trivial | ~20 |
| Permission list + doctor count | Trivial | ~10 |
| Tests (`explore_test.rs`) | Medium | ~150 |
| **Total** | | **~515** |

No external dependencies. No schema changes. Safe to ship in a single PR.
