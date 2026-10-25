//! Criterion benchmark: tokensave MCP tools against large, real-world repos.
//!
//! What it does:
//!  1. Reads `TOKENSAVE_BENCH_REPOS_DIR` (on-disk cache for the cloned repos).
//!     If unset, prints a message and registers zero benchmarks.
//!  2. For each selected repo (see `repos::REPOS`, optionally filtered with
//!     `TOKENSAVE_BENCH_REPOS=name1,name2`), shallow-clones it (`git fetch
//!     --depth 1`) at a constant ref the first time it is encountered.
//!  3. Opens (or initialises) the `.tokensave/` database for the repo and
//!     **always runs a full `index_all()` first** — equivalent to
//!     `tokensave sync --force` — so every bench run starts from a freshly
//!     synced graph regardless of how stale the cached index is.
//!  4. Samples the resulting graph to build one query catalog per repo:
//!     ≥ 5 queries per tool, each holding concrete `node_id` / qualified-name
//!     / file-pattern arguments drawn from real graph state. Write queries
//!     (`str_replace`, `multi_str_replace`, `insert_at`, `ast_grep_rewrite`)
//!     also declare a scratch file that is rewritten before every timed
//!     iteration via `iter_batched`.
//!  5. Runs every (repo × tool × query) combination through criterion with
//!     `sample_size = 10` and `measurement_time = 30s`.
//!  6. When all benches finish, runs `git stash --include-untracked` inside
//!     each prepared repo so mutations made by the write benches are reverted.
//!
//! Environment variables:
//!   TOKENSAVE_BENCH_REPOS_DIR   required — root directory for cloned repos
//!   TOKENSAVE_BENCH_REPOS       optional — comma-separated repo subset
//!   TOKENSAVE_BENCH_SKIP_CLONE  optional — fail rather than clone

mod queries;
mod repos;

use std::path::PathBuf;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use serde_json::Value;
use tokio::runtime::Runtime;

use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

use queries::{build_context, build_queries, Query, QueryKind, ToolGroup, SCRATCH_DIR};
use repos::{ensure_cloned, repos_root, restore_repo, selected_repos, Repo};

/// Per-repo state we hand to criterion: indexed graph + frozen query catalog.
struct RepoBench {
    cg: TokenSave,
    dir: PathBuf,
    name: &'static str,
    groups: Vec<ToolGroup>,
}

async fn prepare_repo(rt_root: &std::path::Path, repo: Repo) -> Result<RepoBench, String> {
    let dir = ensure_cloned(rt_root, repo)?;

    let cg = if TokenSave::is_initialized(&dir) {
        TokenSave::open(&dir)
            .await
            .map_err(|e| format!("open {}: {e}", repo.name))?
    } else {
        TokenSave::init(&dir)
            .await
            .map_err(|e| format!("init {}: {e}", repo.name))?
    };

    // Always force a full re-index before benching, matching
    // `tokensave sync --force`. This guarantees the cached graph reflects the
    // pinned checkout regardless of how the repo dir was previously left.
    eprintln!(
        "[bench] force-indexing {} (sync --force equivalent)...",
        repo.name
    );
    cg.index_all()
        .await
        .map_err(|e| format!("index {}: {e}", repo.name))?;

    let ctx = build_context(&cg).await;
    let groups = build_queries(&ctx);
    Ok(RepoBench {
        cg,
        dir,
        name: repo.name,
        groups,
    })
}

fn run_query(rt: &Runtime, cg: &TokenSave, q: &Query) -> Value {
    rt.block_on(async {
        // We don't care about the response shape, only that the call completes.
        // `unwrap_or_else` keeps the bench going even if a sampled id was stale
        // (e.g. graph state shifted between context-build and call) — the timing
        // for the error path is still representative of dispatcher overhead.
        match handle_tool_call(cg, q.tool, q.args.clone(), None, None).await {
            Ok(res) => res.value,
            Err(_) => Value::Null,
        }
    })
}

/// Re-create `scratch_path` (relative to `project_root`) with `init_content`.
/// Runs before every timed iteration of a write bench so the edit primitive's
/// uniqueness check keeps passing.
fn reset_scratch(project_root: &std::path::Path, scratch_path: &str, init_content: &str) {
    let abs = project_root.join(scratch_path);
    if let Some(parent) = abs.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&abs, init_content) {
        eprintln!(
            "[bench] WARNING: failed to write scratch {}: {e}",
            abs.display()
        );
    }
}

fn bench_all(c: &mut Criterion) {
    let Some(root) = repos_root() else {
        eprintln!(
            "[bench] TOKENSAVE_BENCH_REPOS_DIR is unset — skipping large-repo benchmarks. \
             Set it to a writable directory to enable them."
        );
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&root) {
        eprintln!("[bench] cannot create {}: {e}", root.display());
        return;
    }

    let rt = Runtime::new().expect("create tokio runtime");

    let repos = selected_repos();
    if repos.is_empty() {
        eprintln!("[bench] no repos selected (TOKENSAVE_BENCH_REPOS filter excluded everything)");
        return;
    }

    let mut prepared: Vec<RepoBench> = Vec::new();
    for repo in &repos {
        match rt.block_on(prepare_repo(&root, *repo)) {
            Ok(rb) => prepared.push(rb),
            Err(e) => eprintln!("[bench] skipping {}: {e}", repo.name),
        }
    }

    for rb in &prepared {
        for group in &rb.groups {
            let mut g = c.benchmark_group(format!("{}/{}", rb.name, group.tool));
            g.throughput(Throughput::Elements(1));
            for (i, q) in group.queries.iter().enumerate() {
                let id = BenchmarkId::new(q.label, i);
                match &q.kind {
                    QueryKind::Read => {
                        g.bench_with_input(id, q, |b, q| {
                            b.iter(|| run_query(&rt, &rb.cg, q));
                        });
                    }
                    QueryKind::Write {
                        scratch_path,
                        init_content,
                    } => {
                        let root = rb.cg.project_root().to_path_buf();
                        let scratch = scratch_path.clone();
                        let init = init_content.clone();
                        g.bench_with_input(id, q, |b, q| {
                            b.iter_batched(
                                || reset_scratch(&root, &scratch, &init),
                                |()| run_query(&rt, &rb.cg, q),
                                BatchSize::SmallInput,
                            );
                        });
                    }
                }
            }
            g.finish();
        }
    }

    // Revert all scratch-file churn (and any other accidental edits) in each
    // repo we touched. `git stash --include-untracked` puts everything aside;
    // we then drop the stash so the working tree matches the pinned ref again.
    for rb in &prepared {
        eprintln!(
            "[bench] reverting changes in {} (git stash + drop)...",
            rb.name
        );
        let _ = std::fs::remove_dir_all(rb.dir.join(SCRATCH_DIR));
        if let Err(e) = restore_repo(&rb.dir) {
            eprintln!("[bench] WARNING: revert failed for {}: {e}", rb.name);
        }
    }
}

fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(30))
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = bench_all
}
criterion_main!(benches);
