use std::path::Path;
use tokensave::tokensave::TokenSave;

/// Opens an existing project, or tells the user to run `tokensave init` first.
pub async fn ensure_initialized(project_path: &Path) -> tokensave::errors::Result<TokenSave> {
    if TokenSave::is_initialized(project_path) {
        return TokenSave::open(project_path).await;
    }
    Err(tokensave::errors::TokenSaveError::Config {
        message: format!(
            "no TokenSave index found at '{}' — run 'tokensave init' first",
            project_path.display()
        ),
    })
}

/// Fallback for `serve`: when CWD-based discovery fails, check the global DB
/// for registered projects. When multiple projects exist, pick the best match
/// against cwd: prefer a project that is an ancestor of cwd (cwd is inside the
/// project), then a project that is a descendant of cwd (project is under cwd).
/// Among multiple matches, the deepest (most specific) path wins.
pub async fn resolve_serve_from_global_db() -> Option<std::path::PathBuf> {
    let gdb = tokensave::global_db::GlobalDb::open().await?;
    let mut paths: Vec<String> = gdb.list_project_paths().await;
    // Keep only projects whose .tokensave dir still exists on disk.
    paths.retain(|p| {
        std::path::Path::new(p)
            .join(".tokensave/tokensave.db")
            .exists()
    });
    if paths.len() == 1 {
        return Some(std::path::PathBuf::from(paths.remove(0)));
    }
    if paths.is_empty() {
        return None;
    }

    // Multiple projects — try to resolve using cwd.
    let cwd = std::env::current_dir().ok()?;
    let cwd = cwd.canonicalize().unwrap_or(cwd);

    // Priority 1: cwd is inside a project (project is ancestor of cwd).
    // Pick the deepest ancestor (most specific match).
    let mut ancestors: Vec<_> = paths
        .iter()
        .filter_map(|p| {
            let pp = std::path::Path::new(p).canonicalize().ok()?;
            cwd.starts_with(&pp)
                .then(|| (pp.components().count(), p.clone()))
        })
        .collect();
    ancestors.sort_by(|a, b| b.0.cmp(&a.0)); // deepest first
    if let Some((_, best)) = ancestors.into_iter().next() {
        return Some(std::path::PathBuf::from(best));
    }

    // Priority 2: a project is under cwd (cwd is ancestor of project).
    // Pick the shallowest descendant (closest child).
    let mut descendants: Vec<_> = paths
        .iter()
        .filter_map(|p| {
            let pp = std::path::Path::new(p).canonicalize().ok()?;
            pp.starts_with(&cwd)
                .then(|| (pp.components().count(), p.clone()))
        })
        .collect();
    descendants.sort_by(|a, b| a.0.cmp(&b.0)); // shallowest first
    if let Some((_, best)) = descendants.into_iter().next() {
        return Some(std::path::PathBuf::from(best));
    }

    // No cwd-based match — report the ambiguity.
    eprintln!("Multiple tokensave projects found — pass -p <path> to select one:");
    for p in &paths {
        eprintln!("  {p}");
    }
    None
}

/// Last-resort fallback for `serve`: peek at the first stdin line to read the
/// MCP `initialize` request's `roots` array.  If a root matches a registered
/// project, return its path.  The raw line is stored in `out` so the caller
/// can replay it into the MCP transport (the server still needs to see it).
pub async fn resolve_serve_from_mcp_roots(out: &mut Option<String>) -> Option<std::path::PathBuf> {
    use tokio::io::AsyncBufReadExt;
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut line = String::new();
    // Read the first non-empty line (should be the `initialize` request).
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => return None, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    break;
                }
            }
            Err(_) => return None,
        }
    }
    *out = Some(line.trim().to_string());

    let parsed: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let roots = parsed.pointer("/params/roots").and_then(|v| v.as_array())?;

    let gdb = tokensave::global_db::GlobalDb::open().await?;
    let mut registered: Vec<String> = gdb.list_project_paths().await;
    registered.retain(|p| {
        std::path::Path::new(p)
            .join(".tokensave/tokensave.db")
            .exists()
    });

    // Try each root URI — first match wins.
    for root in roots {
        let uri = root.get("uri").and_then(|v| v.as_str()).unwrap_or_default();
        let root_path = uri.strip_prefix("file://").unwrap_or(uri);
        let root_path = std::path::Path::new(root_path);
        // Exact match: the root IS a registered project.
        if let Some(hit) = registered
            .iter()
            .find(|p| std::path::Path::new(p) == root_path)
        {
            return Some(std::path::PathBuf::from(hit));
        }
        // Walk up from the root to find the nearest enclosing project.
        if let Some(discovered) = tokensave::config::discover_project_root(root_path) {
            return Some(discovered);
        }
    }
    None
}

/// BFS through file dependents to find test files affected by changes.
pub async fn find_affected_tests(
    cg: &TokenSave,
    changed_files: &[String],
    max_depth: usize,
    custom_filter: Option<&str>,
) -> tokensave::errors::Result<Vec<String>> {
    debug_assert!(
        !changed_files.is_empty(),
        "find_affected_tests called with no changed files"
    );
    debug_assert!(
        max_depth > 0,
        "find_affected_tests max_depth must be positive"
    );
    use std::collections::{HashSet, VecDeque};

    let custom_glob = custom_filter.and_then(|p| glob::Pattern::new(p).ok());

    // Pre-compute files with inline test modules.
    let files_with_inline_tests = cg
        .get_files_with_test_annotations()
        .await
        .unwrap_or_default();
    let matches_test = |path: &str| -> bool {
        if let Some(ref g) = custom_glob {
            g.matches(path)
        } else {
            tokensave::tokensave::is_test_file(path) || files_with_inline_tests.contains(path)
        }
    };

    let mut affected: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();

    // Seed: changed files that are themselves tests go directly into the result
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    for file in changed_files {
        if matches_test(file) {
            affected.insert(file.clone());
        }
        if visited.insert(file.clone()) {
            queue.push_back((file.clone(), 0));
        }
    }

    // BFS through file dependents
    while let Some((file, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let dependents = cg.get_file_dependents(&file).await?;
        for dep in dependents {
            if !visited.insert(dep.clone()) {
                continue;
            }
            if matches_test(&dep) {
                affected.insert(dep.clone());
            } else {
                queue.push_back((dep, depth + 1));
            }
        }
    }

    let mut result: Vec<String> = affected.into_iter().collect();
    result.sort();
    Ok(result)
}
