//! Static (per-repo) query catalog used by the criterion bench.
//!
//! Queries are constructed once after the repo is indexed: the `QueryContext`
//! is sampled from real graph state (top-ranked nodes, real qualified names,
//! file directory prefixes) so calls like `tokensave_callers` receive valid
//! `node_id`s.
//!
//! Write queries (`str_replace`, `multi_str_replace`, `insert_at`,
//! `ast_grep_rewrite`) declare a `scratch_path` + `init_content`: the bench
//! harness rewrites the scratch file before *every* timed iteration so the
//! tool's "match must be unique" precondition keeps holding. A
//! `git stash --include-untracked` at the end of the run reverts all
//! scratch-file churn.

use serde_json::{json, Value};

use tokensave::tokensave::TokenSave;
use tokensave::types::NodeKind;

/// Distinguishes read-only queries from queries that mutate a scratch file.
#[derive(Clone)]
pub enum QueryKind {
    Read,
    Write {
        /// File path *relative to the project root* that must be (re)written
        /// before each timed iteration.
        scratch_path: String,
        /// Bytes the scratch file is reset to before each iter.
        init_content: String,
    },
}

/// One concrete tool invocation: the MCP tool name, its args, and the kind
/// (read vs. write) that drives criterion's iteration strategy.
#[derive(Clone)]
pub struct Query {
    pub label: &'static str,
    pub tool: &'static str,
    pub args: Value,
    pub kind: QueryKind,
}

impl Query {
    fn read(label: &'static str, tool: &'static str, args: Value) -> Self {
        Self {
            label,
            tool,
            args,
            kind: QueryKind::Read,
        }
    }

    fn write(
        label: &'static str,
        tool: &'static str,
        args: Value,
        scratch_path: String,
        init_content: String,
    ) -> Self {
        Self {
            label,
            tool,
            args,
            kind: QueryKind::Write {
                scratch_path,
                init_content,
            },
        }
    }
}

/// All queries we run for one tool. The harness invariant is `queries.len() == 5`.
pub struct ToolGroup {
    pub tool: &'static str,
    pub queries: Vec<Query>,
}

/// Sampled data drawn from a freshly indexed graph. Built once per repo.
pub struct QueryContext {
    pub function_ids: Vec<String>,
    pub struct_ids: Vec<String>,
    pub any_ids: Vec<String>,
    pub function_qnames: Vec<String>,
    pub dir_prefixes: Vec<String>,
}

impl QueryContext {
    /// Pick the i-th id with wrap-around. Returns `"missing"` if no samples
    /// exist — the tool handler will report a not-found error, which is still
    /// useful timing data (and the bench label keeps the case obvious).
    fn pick(slice: &[String], i: usize) -> String {
        if slice.is_empty() {
            "missing".to_string()
        } else {
            slice[i % slice.len()].clone()
        }
    }
}

pub async fn build_context(cg: &TokenSave) -> QueryContext {
    let all = cg.get_all_nodes().await.unwrap_or_default();

    let mut function_ids = Vec::new();
    let mut struct_ids = Vec::new();
    let mut function_qnames = Vec::new();
    let mut any_ids = Vec::new();

    for n in &all {
        any_ids.push(n.id.clone());
        match n.kind {
            NodeKind::Function | NodeKind::Method => {
                function_ids.push(n.id.clone());
                if !n.qualified_name.is_empty() && function_qnames.len() < 64 {
                    function_qnames.push(n.qualified_name.clone());
                }
            }
            NodeKind::Struct | NodeKind::Class => {
                struct_ids.push(n.id.clone());
            }
            _ => {}
        }
    }

    function_ids.truncate(64);
    struct_ids.truncate(64);
    any_ids.truncate(64);

    let files = cg.get_all_files().await.unwrap_or_default();

    // Collect first-segment directory prefixes from the sample files (so
    // `path_prefix` queries are valid for *this* repo regardless of layout).
    let mut dir_prefixes: Vec<String> = files
        .iter()
        .filter_map(|f| f.path.split('/').next().map(str::to_string))
        .collect();
    dir_prefixes.sort();
    dir_prefixes.dedup();
    dir_prefixes.truncate(5);

    QueryContext {
        function_ids,
        struct_ids,
        any_ids,
        function_qnames,
        dir_prefixes,
    }
}

fn five<F: FnMut(usize) -> Query>(mut f: F) -> Vec<Query> {
    (0..5).map(&mut f).collect()
}

fn dir(ctx: &QueryContext, i: usize) -> String {
    if ctx.dir_prefixes.is_empty() {
        "src".to_string()
    } else {
        ctx.dir_prefixes[i % ctx.dir_prefixes.len()].clone()
    }
}

/// Root directory (relative to the project root) where all write-query
/// scratch files live. Kept on a single path so `git stash --include-untracked`
/// at end-of-bench reverts everything in one shot.
pub const SCRATCH_DIR: &str = ".tokensave-bench-scratch";

fn scratch(name: &str) -> String {
    format!("{SCRATCH_DIR}/{name}")
}

pub fn build_queries(ctx: &QueryContext) -> Vec<ToolGroup> {
    // ── read query inputs ────────────────────────────────────────────────
    let search_terms = ["main", "init", "parse", "error", "config"];
    let context_tasks = [
        "How is configuration loaded at startup?",
        "Where are command-line arguments parsed?",
        "How are errors defined and propagated?",
        "How is logging configured?",
        "How are tests organized?",
    ];
    let kinds_for_largest = [
        json!(["function"]),
        json!(["method"]),
        json!(["struct"]),
        json!(["class"]),
        json!(["module"]),
    ];
    let file_globs = ["**/*.rs", "**/*.c", "**/*.py", "**/*.js", "**/*.ts"];
    let rank_kinds = ["calls", "uses", "contains", "type_of", "implements"];

    let mut groups: Vec<ToolGroup> = Vec::new();

    groups.push(ToolGroup {
        tool: "tokensave_search",
        queries: five(|i| {
            Query::read(
                "term",
                "tokensave_search",
                json!({ "query": search_terms[i], "limit": 20 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_context",
        queries: five(|i| {
            Query::read(
                "task",
                "tokensave_context",
                json!({ "task": context_tasks[i], "max_nodes": 20 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_callers",
        queries: five(|i| {
            Query::read(
                "by_id",
                "tokensave_callers",
                json!({ "node_id": QueryContext::pick(&ctx.function_ids, i), "max_depth": 3 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_callees",
        queries: five(|i| {
            Query::read(
                "by_id",
                "tokensave_callees",
                json!({ "node_id": QueryContext::pick(&ctx.function_ids, i), "max_depth": 3 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_node",
        queries: five(|i| {
            Query::read(
                "by_id",
                "tokensave_node",
                json!({ "id": QueryContext::pick(&ctx.any_ids, i) }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_by_qualified_name",
        queries: five(|i| {
            Query::read(
                "qname",
                "tokensave_by_qualified_name",
                json!({ "qualified_name": QueryContext::pick(&ctx.function_qnames, i) }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_signature",
        queries: five(|i| {
            Query::read(
                "by_id",
                "tokensave_signature",
                json!({ "id": QueryContext::pick(&ctx.function_ids, i) }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_impact",
        queries: five(|i| {
            Query::read(
                "by_id",
                "tokensave_impact",
                json!({ "node_id": QueryContext::pick(&ctx.function_ids, i), "max_depth": 2 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_body",
        queries: five(|i| {
            Query::read(
                "by_id",
                "tokensave_body",
                json!({ "id": QueryContext::pick(&ctx.function_ids, i) }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_files",
        queries: five(|i| {
            Query::read(
                "glob",
                "tokensave_files",
                json!({ "pattern": file_globs[i] }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_complexity",
        queries: {
            let mut v = Vec::with_capacity(5);
            v.push(Query::read(
                "all",
                "tokensave_complexity",
                json!({ "limit": 20 }),
            ));
            for i in 0..4 {
                v.push(Query::read(
                    "scoped",
                    "tokensave_complexity",
                    json!({ "path": dir(ctx, i), "limit": 20 }),
                ));
            }
            v
        },
    });

    groups.push(ToolGroup {
        tool: "tokensave_doc_coverage",
        queries: {
            let mut v = vec![Query::read("all", "tokensave_doc_coverage", json!({}))];
            for i in 0..4 {
                v.push(Query::read(
                    "scoped",
                    "tokensave_doc_coverage",
                    json!({ "path": dir(ctx, i) }),
                ));
            }
            v
        },
    });

    groups.push(ToolGroup {
        tool: "tokensave_largest",
        queries: five(|i| {
            Query::read(
                "by_kind",
                "tokensave_largest",
                json!({ "kinds": kinds_for_largest[i].clone(), "limit": 20 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_hotspots",
        queries: five(|i| {
            Query::read(
                "limit",
                "tokensave_hotspots",
                json!({ "limit": 10 + (i as u32) * 10 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_god_class",
        queries: five(|i| {
            Query::read(
                "threshold",
                "tokensave_god_class",
                json!({ "min_methods": 5 + (i as u32) * 5 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_module_api",
        queries: five(|i| {
            Query::read(
                "scoped",
                "tokensave_module_api",
                json!({ "path": dir(ctx, i) }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_derives",
        queries: five(|i| {
            Query::read(
                "by_id",
                "tokensave_derives",
                json!({ "id": QueryContext::pick(&ctx.struct_ids, i) }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_dead_code",
        queries: five(|i| {
            Query::read(
                "scoped",
                "tokensave_dead_code",
                json!({ "path": dir(ctx, i) }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_rank",
        queries: five(|i| {
            Query::read(
                "by_kind",
                "tokensave_rank",
                json!({ "kind": rank_kinds[i], "limit": 20 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_coupling",
        queries: five(|i| {
            Query::read(
                "scoped",
                "tokensave_coupling",
                json!({ "path": dir(ctx, i), "limit": 20 }),
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_circular",
        queries: five(|i| {
            Query::read(
                "depth",
                "tokensave_circular",
                json!({ "max_depth": 4 + (i as u32) * 2 }),
            )
        }),
    });

    // ── write queries ────────────────────────────────────────────────────
    //
    // Each iteration must start from a known file state because the edit
    // primitives reject ambiguous/zero matches. The harness re-writes
    // `scratch_path` from `init_content` before every timed iter
    // (via `iter_batched`).

    groups.push(ToolGroup {
        tool: "tokensave_str_replace",
        queries: five(|i| {
            let path = scratch(&format!("str_replace_{i}.txt"));
            // Body grows with `i` so the 5 cases sweep small → larger payloads.
            let body = "ALPHA\nBETA\nGAMMA\nDELTA\n".repeat(1 + i * 4);
            let content = format!("{body}TARGET_{i}\nTAIL\n");
            Query::write(
                "vary_size",
                "tokensave_str_replace",
                json!({
                    "path": path,
                    "old_str": format!("TARGET_{i}"),
                    "new_str": format!("REPLACED_{i}"),
                }),
                path.clone(),
                content,
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_multi_str_replace",
        queries: five(|i| {
            let path = scratch(&format!("multi_{i}.txt"));
            let n = i + 1; // 1..=5 replacements
            let mut content = String::new();
            let mut replacements = Vec::with_capacity(n);
            for k in 0..n {
                content.push_str(&format!("MARK_{k}\nfiller line {k}\n"));
                replacements.push([format!("MARK_{k}"), format!("DONE_{k}")]);
            }
            Query::write(
                "n_repls",
                "tokensave_multi_str_replace",
                json!({ "path": path, "replacements": replacements }),
                path.clone(),
                content,
            )
        }),
    });

    groups.push(ToolGroup {
        tool: "tokensave_insert_at",
        queries: five(|i| {
            let path = scratch(&format!("insert_{i}.txt"));
            // 5 distinct anchor lines; each iter inserts after the matching line.
            let lines: Vec<String> = (0..5).map(|k| format!("LINE_{k}")).collect();
            let content = format!("{}\n", lines.join("\n"));
            Query::write(
                "by_anchor",
                "tokensave_insert_at",
                json!({
                    "path": path,
                    "anchor": format!("LINE_{i}"),
                    "content": "// inserted by bench\n",
                    "before": false,
                }),
                path.clone(),
                content,
            )
        }),
    });

    if ast_grep_on_path() {
        groups.push(ToolGroup {
            tool: "tokensave_ast_grep_rewrite",
            queries: five(|i| {
                let path = scratch(&format!("ast_grep_{i}.rs"));
                // Provide a small rust source with a function whose name we'll
                // rewrite. ast-grep's metavariable syntax is `$NAME`.
                let content = format!("pub fn bench_target_{i}() {{\n    let _ = {i};\n}}\n");
                Query::write(
                    "rename_fn",
                    "tokensave_ast_grep_rewrite",
                    json!({
                        "path": path,
                        "pattern": format!("fn bench_target_{i}() {{ $$$BODY }}"),
                        "rewrite": format!("fn bench_renamed_{i}() {{ $$$BODY }}"),
                    }),
                    path.clone(),
                    content,
                )
            }),
        });
    }

    groups
}

/// Mirrors `tokensave::mcp::tools::ast_grep_available` without depending on
/// internal-module visibility: shells out to `ast-grep --version`. Cached
/// for the lifetime of the bench process.
fn ast_grep_on_path() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("ast-grep")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}
