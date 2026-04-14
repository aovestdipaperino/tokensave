//! MCP tool definitions (JSON Schema descriptors).
//!
//! Each `def_*` function returns a `ToolDefinition` with the tool name,
//! description, JSON Schema for its input parameters, MCP annotations
//! (readOnlyHint, title), and optional `_meta` (anthropic/alwaysLoad).

use serde_json::{json, Value};

use super::ToolDefinition;

/// Read-only annotations shared by every tool.
fn read_only(title: &str) -> Value {
    json!({
        "readOnlyHint": true,
        "title": title
    })
}

/// Build a `ToolDefinition` with `readOnlyHint` annotation and no `_meta`.
fn def(name: &str, title: &str, description: &str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        annotations: Some(read_only(title)),
        meta: None,
    }
}

/// Build a `ToolDefinition` with `readOnlyHint` AND `anthropic/alwaysLoad`.
fn def_always_load(name: &str, title: &str, description: &str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        annotations: Some(read_only(title)),
        meta: Some(json!({ "anthropic/alwaysLoad": true })),
    }
}

/// Computes the call budget based on project size.
pub fn explore_call_budget(total_nodes: u64) -> u8 {
    match total_nodes {
        0..=5_000 => 3,
        5_001..=20_000 => 4,
        20_001..=80_000 => 5,
        80_001..=250_000 => 7,
        _ => 10,
    }
}

/// Generates the tokensave_context description with a dynamic call budget.
pub fn context_description(node_count: u64, budget: u8) -> String {
    format!(
        "Build an AI-ready context for a task description. Returns relevant symbols, \
         relationships, and optionally code snippets.\n\n\
         CALL BUDGET: {} calls maximum for this project ({} nodes). \
         Stop after {} calls. If the question is not fully answered, synthesise \
         from what you have — do not exceed the budget.",
        budget, node_count, budget
    )
}

/// Returns tool definitions with a dynamic call budget for tokensave_context.
pub fn get_tool_definitions_with_budget(node_count: u64, budget: u8) -> Vec<ToolDefinition> {
    let mut defs = get_tool_definitions();
    // Replace the context tool's description with the budgeted version
    for def in &mut defs {
        if def.name == "tokensave_context" {
            def.description = context_description(node_count, budget);
        }
    }
    defs
}

/// Returns the list of all tool definitions exposed by this MCP server.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let definitions = vec![
        def_search(),
        def_context(),
        def_callers(),
        def_callees(),
        def_impact(),
        def_node(),
        def_status(),
        def_files(),
        def_affected(),
        def_dead_code(),
        def_diff_context(),
        def_module_api(),
        def_circular(),
        def_hotspots(),
        def_similar(),
        def_rename_preview(),
        def_unused_imports(),
        def_rank(),
        def_largest(),
        def_coupling(),
        def_inheritance_depth(),
        def_distribution(),
        def_recursion(),
        def_complexity(),
        def_doc_coverage(),
        def_god_class(),
        def_changelog(),
        def_port_status(),
        def_port_order(),
        def_commit_context(),
        def_pr_context(),
        def_simplify_scan(),
        def_test_map(),
        def_type_hierarchy(),
        def_branch_search(),
        def_branch_diff(),
        def_branch_list(),
    ];
    debug_assert!(!definitions.is_empty(), "get_tool_definitions returned empty list");
    debug_assert!(definitions.iter().all(|d| d.name.starts_with("tokensave_")),
        "all tool definitions must have 'tokensave_' prefix");
    definitions
}

// ── alwaysLoad tools (loaded into the model prompt immediately) ─────────

fn def_search() -> ToolDefinition {
    def_always_load(
        "tokensave_search",
        "Search Symbols",
        "Search for symbols (functions, structs, traits, etc.) in the code graph by name or keyword.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string to match against symbol names"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": ["query"]
        }),
    )
}

fn def_context() -> ToolDefinition {
    def_always_load(
        "tokensave_context",
        "Task Context",
        &context_description(0, 3),
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Natural language description of the task or question"
                },
                "max_nodes": {
                    "type": "number",
                    "description": "Maximum number of symbols to include (default: 20)"
                },
                "include_code": {
                    "type": "boolean",
                    "description": "If true, include source code snippets for key symbols (default: false)"
                },
                "max_code_blocks": {
                    "type": "number",
                    "description": "Maximum number of code snippets when include_code is true (default: 5)"
                },
                "mode": {
                    "type": "string",
                    "enum": ["explore", "plan"],
                    "description": "Context mode: 'explore' (default) for general exploration, 'plan' for implementation planning (adds extension points, dependency order, test coverage)"
                },
                "keywords": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Extra search keywords for synonym expansion. Use this when the task uses conceptual terms that may not match symbol names — e.g. for 'authentication', pass [\"login\", \"session\", \"credential\", \"token\", \"auth\"]. The graph is searched for each keyword independently."
                },
                "exclude_node_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Node IDs to exclude from results (pass seen_node_ids from previous call for session deduplication)"
                },
                "merge_adjacent": {
                    "type": "boolean",
                    "description": "When true, merge code blocks from the same file whose line ranges are adjacent or overlapping (default: false)"
                }
            },
            "required": ["task"]
        }),
    )
}

fn def_status() -> ToolDefinition {
    def_always_load(
        "tokensave_status",
        "Graph Status",
        "Return aggregate statistics about the code graph (node/edge/file counts, DB size, etc.).",
        json!({
            "type": "object",
            "properties": {}
        }),
    )
}

// ── Deferred tools (discovered via ToolSearch on demand) ────────────────

fn def_callers() -> ToolDefinition {
    def(
        "tokensave_callers",
        "Callers",
        "Find all callers of a given node (function, method, etc.) up to a specified depth.",
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to find callers for"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum traversal depth (default: 3)"
                }
            },
            "required": ["node_id"]
        }),
    )
}

fn def_callees() -> ToolDefinition {
    def(
        "tokensave_callees",
        "Callees",
        "Find all callees of a given node (function, method, etc.) up to a specified depth.",
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to find callees for"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum traversal depth (default: 3)"
                }
            },
            "required": ["node_id"]
        }),
    )
}

fn def_impact() -> ToolDefinition {
    def(
        "tokensave_impact",
        "Impact Radius",
        "Compute the impact radius of a node: all symbols that directly or indirectly depend on it.",
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to compute impact for"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum traversal depth (default: 3)"
                }
            },
            "required": ["node_id"]
        }),
    )
}

fn def_node() -> ToolDefinition {
    def(
        "tokensave_node",
        "Node Details",
        "Retrieve detailed information about a single node by its ID.",
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to retrieve"
                }
            },
            "required": ["node_id"]
        }),
    )
}

fn def_files() -> ToolDefinition {
    def(
        "tokensave_files",
        "File List",
        "List indexed project files. Use to explore file structure without reading file contents.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path"
                },
                "pattern": {
                    "type": "string",
                    "description": "Filter files matching this glob pattern (e.g. '**/*.rs')"
                },
                "format": {
                    "type": "string",
                    "enum": ["flat", "grouped"],
                    "description": "Output format: flat (one per line) or grouped by directory (default: grouped)"
                }
            }
        }),
    )
}

fn def_affected() -> ToolDefinition {
    def(
        "tokensave_affected",
        "Affected Tests",
        "Find test files affected by changed source files via dependency graph traversal.",
        json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of changed file paths to analyze"
                },
                "depth": {
                    "type": "number",
                    "description": "Maximum dependency traversal depth (default: 5)"
                },
                "filter": {
                    "type": "string",
                    "description": "Custom glob pattern for test files (default: common test patterns)"
                }
            },
            "required": ["files"]
        }),
    )
}

fn def_dead_code() -> ToolDefinition {
    def(
        "tokensave_dead_code",
        "Dead Code",
        "Find symbols with no incoming edges (potentially unreachable code). Excludes main, test functions, and public items.",
        json!({
            "type": "object",
            "properties": {
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Node kinds to check (default: [\"function\", \"method\"])"
                }
            }
        }),
    )
}

fn def_diff_context() -> ToolDefinition {
    def(
        "tokensave_diff_context",
        "Diff Context",
        "Given changed file paths, return semantic context: which symbols were modified, what depends on them, and affected tests.",
        json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of changed file paths"
                },
                "depth": {
                    "type": "number",
                    "description": "Maximum impact traversal depth (default: 2)"
                }
            },
            "required": ["files"]
        }),
    )
}

fn def_module_api() -> ToolDefinition {
    def(
        "tokensave_module_api",
        "Module API",
        "Show the public API surface of a file or directory: all pub symbols sorted by file and line.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path or directory prefix to inspect"
                }
            },
            "required": ["path"]
        }),
    )
}

fn def_circular() -> ToolDefinition {
    def(
        "tokensave_circular",
        "Circular Deps",
        "Detect circular dependencies between files in the code graph.",
        json!({
            "type": "object",
            "properties": {
                "max_depth": {
                    "type": "number",
                    "description": "Maximum cycle detection depth (default: 10)"
                }
            }
        }),
    )
}

fn def_hotspots() -> ToolDefinition {
    def(
        "tokensave_hotspots",
        "Hotspots",
        "Find symbols with the highest connectivity (most incoming + outgoing edges).",
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "number",
                    "description": "Maximum number of hotspots to return (default: 10)"
                }
            }
        }),
    )
}

fn def_similar() -> ToolDefinition {
    def(
        "tokensave_similar",
        "Similar Symbols",
        "Find symbols with similar names using full-text search and substring matching.",
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol name to find similar matches for"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results (default: 10)"
                }
            },
            "required": ["symbol"]
        }),
    )
}

fn def_rename_preview() -> ToolDefinition {
    def(
        "tokensave_rename_preview",
        "References",
        "Show all references to a symbol -- all edges where the node appears as source or target.",
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to find references for"
                }
            },
            "required": ["node_id"]
        }),
    )
}

fn def_unused_imports() -> ToolDefinition {
    def(
        "tokensave_unused_imports",
        "Unused Imports",
        "Find import/use nodes that are never referenced by any other node.",
        json!({
            "type": "object",
            "properties": {}
        }),
    )
}

fn def_rank() -> ToolDefinition {
    def(
        "tokensave_rank",
        "Rank",
        "Rank nodes by edge count for a given relationship type (calls, implements, extends, etc.).",
        json!({
            "type": "object",
            "properties": {
                "edge_kind": {
                    "type": "string",
                    "enum": ["implements", "extends", "calls", "uses", "contains", "annotates", "derives_macro"],
                    "description": "The relationship type to rank by (e.g. 'implements' to find most-implemented interfaces)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["incoming", "outgoing"],
                    "description": "Edge direction: 'incoming' ranks targets (default, e.g. most-implemented interface), 'outgoing' ranks sources (e.g. class that implements the most interfaces)"
                },
                "node_kind": {
                    "type": "string",
                    "description": "Optional filter for node kind (e.g. 'interface', 'class', 'trait', 'function', 'method')"
                },
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path (e.g. 'src/main/java')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": ["edge_kind"]
        }),
    )
}

fn def_largest() -> ToolDefinition {
    def(
        "tokensave_largest",
        "Largest Symbols",
        "Rank nodes by size (line count). Find the largest classes, longest methods, biggest enums, etc.",
        json!({
            "type": "object",
            "properties": {
                "node_kind": {
                    "type": "string",
                    "description": "Filter by node kind (e.g. 'class', 'method', 'function', 'interface', 'enum', 'struct')"
                },
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path (e.g. 'src/main/java')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    )
}

fn def_coupling() -> ToolDefinition {
    def(
        "tokensave_coupling",
        "Coupling",
        "Rank files by coupling: fan_in (most depended on) or fan_out (most dependencies).",
        json!({
            "type": "object",
            "properties": {
                "direction": {
                    "type": "string",
                    "enum": ["fan_in", "fan_out"],
                    "description": "fan_in: files depended on by the most others. fan_out: files that depend on the most others (default: fan_in)"
                },
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path (e.g. 'src/main/java')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    )
}

fn def_inheritance_depth() -> ToolDefinition {
    def(
        "tokensave_inheritance_depth",
        "Inheritance Depth",
        "Find the deepest class/interface inheritance hierarchies by walking extends chains.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path (e.g. 'src/main/java')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    )
}

fn def_distribution() -> ToolDefinition {
    def(
        "tokensave_distribution",
        "Distribution",
        "Show node kind distribution (classes, methods, fields, etc.) per file or directory.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory or file path prefix to filter (e.g. 'src/main/java/com/example'). Omit for entire codebase."
                },
                "summary": {
                    "type": "boolean",
                    "description": "If true, aggregate counts across all matching files instead of per-file breakdown (default: false)"
                }
            }
        }),
    )
}

fn def_recursion() -> ToolDefinition {
    def(
        "tokensave_recursion",
        "Recursion",
        "Detect recursive and mutually-recursive call cycles in the call graph.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path (e.g. 'src/main/java')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of cycles to return (default: 10)"
                }
            }
        }),
    )
}

fn def_complexity() -> ToolDefinition {
    def(
        "tokensave_complexity",
        "Complexity",
        "Rank functions/methods by composite complexity score (lines + fan-out + fan-in).",
        json!({
            "type": "object",
            "properties": {
                "node_kind": {
                    "type": "string",
                    "description": "Filter by node kind (default: function and method)"
                },
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path (e.g. 'src/main/java')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    )
}

fn def_doc_coverage() -> ToolDefinition {
    def(
        "tokensave_doc_coverage",
        "Doc Coverage",
        "Find public symbols missing documentation (docstrings).",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory or file path prefix to filter (e.g. 'src/main'). Omit for entire codebase."
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 50)"
                }
            }
        }),
    )
}

fn def_god_class() -> ToolDefinition {
    def(
        "tokensave_god_class",
        "God Classes",
        "Find classes with the most members (methods + fields).",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path (e.g. 'src/main/java')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    )
}

fn def_changelog() -> ToolDefinition {
    def(
        "tokensave_changelog",
        "Changelog",
        "Generate a semantic diff/changelog between two git refs, categorizing symbols as added, removed, or modified.",
        json!({
            "type": "object",
            "properties": {
                "from_ref": {
                    "type": "string",
                    "description": "Starting git ref (commit, branch, tag)"
                },
                "to_ref": {
                    "type": "string",
                    "description": "Ending git ref (commit, branch, tag)"
                }
            },
            "required": ["from_ref", "to_ref"]
        }),
    )
}

fn def_port_status() -> ToolDefinition {
    def(
        "tokensave_port_status",
        "Port Status",
        "Compare symbols between source and target directories to track porting progress.",
        json!({
            "type": "object",
            "properties": {
                "source_dir": {
                    "type": "string",
                    "description": "Path prefix for source code (e.g. 'src/python/')"
                },
                "target_dir": {
                    "type": "string",
                    "description": "Path prefix for target code (e.g. 'src/rust/')"
                },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Node kinds to compare (default: [\"function\", \"method\", \"class\", \"struct\", \"interface\", \"trait\", \"enum\", \"module\"])"
                }
            },
            "required": ["source_dir", "target_dir"]
        }),
    )
}

fn def_port_order() -> ToolDefinition {
    def(
        "tokensave_port_order",
        "Port Order",
        "Topological sort of symbols in a directory -- port leaves first, dependents after.",
        json!({
            "type": "object",
            "properties": {
                "source_dir": {
                    "type": "string",
                    "description": "Path prefix for source code (e.g. 'src/python/')"
                },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Node kinds to include (default: [\"function\", \"method\", \"class\", \"struct\", \"interface\", \"trait\", \"enum\", \"module\"])"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of symbols to return (default: 50)"
                }
            },
            "required": ["source_dir"]
        }),
    )
}

fn def_commit_context() -> ToolDefinition {
    def(
        "tokensave_commit_context",
        "Commit Context",
        "Semantic summary of uncommitted changes for drafting a commit message. Returns changed symbols, file roles, and recent commit style.",
        json!({
            "type": "object",
            "properties": {
                "staged_only": {
                    "type": "boolean",
                    "description": "If true, only analyze staged changes (default: false = all uncommitted changes)"
                }
            }
        }),
    )
}

fn def_pr_context() -> ToolDefinition {
    def(
        "tokensave_pr_context",
        "PR Context",
        "Semantic summary of changes between two git refs for drafting a pull request description.",
        json!({
            "type": "object",
            "properties": {
                "base_ref": {
                    "type": "string",
                    "description": "Base branch or ref to compare against (default: 'main')"
                },
                "head_ref": {
                    "type": "string",
                    "description": "Head branch or ref (default: 'HEAD')"
                }
            }
        }),
    )
}

fn def_simplify_scan() -> ToolDefinition {
    def(
        "tokensave_simplify_scan",
        "Simplify Scan",
        "Quality analysis of changed files: duplications, dead code, coupling, and complexity hotspots.",
        json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Changed file paths to analyze"
                }
            },
            "required": ["files"]
        }),
    )
}

fn def_test_map() -> ToolDefinition {
    def(
        "tokensave_test_map",
        "Test Map",
        "Map source symbols to their test functions. Shows which tests cover which source code.",
        json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Source file path to find test coverage for"
                },
                "node_id": {
                    "type": "string",
                    "description": "Specific node ID to find test coverage for (alternative to file)"
                }
            }
        }),
    )
}

fn def_type_hierarchy() -> ToolDefinition {
    def(
        "tokensave_type_hierarchy",
        "Type Hierarchy",
        "Show the full type hierarchy for a trait/interface/class: all implementors and extenders, recursively.",
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The type node ID to build the hierarchy for"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum inheritance depth to traverse (default: 5)"
                }
            },
            "required": ["node_id"]
        }),
    )
}

fn def_branch_search() -> ToolDefinition {
    def(
        "tokensave_branch_search",
        "Cross-Branch Search",
        "Search for symbols in another branch's code graph. Opens the target branch's DB and runs a search query against it.",
        json!({
            "type": "object",
            "properties": {
                "branch": {
                    "type": "string",
                    "description": "Branch name to search in (must be tracked via `tokensave branch add`)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query string to match against symbol names"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": ["branch", "query"]
        }),
    )
}

fn def_branch_diff() -> ToolDefinition {
    def(
        "tokensave_branch_diff",
        "Branch Diff",
        "Compare the code graphs of two branches. Shows symbols added, removed, and changed (signature differs) between base and head.",
        json!({
            "type": "object",
            "properties": {
                "base": {
                    "type": "string",
                    "description": "Base branch name (e.g. 'main'). Defaults to the project's default branch."
                },
                "head": {
                    "type": "string",
                    "description": "Head branch name (e.g. 'feature/foo'). Defaults to the current branch."
                },
                "file": {
                    "type": "string",
                    "description": "Optional file path filter — only show diffs for symbols in this file"
                },
                "kind": {
                    "type": "string",
                    "description": "Optional kind filter — only show diffs for this symbol kind (e.g. 'function', 'struct')"
                }
            }
        }),
    )
}

fn def_branch_list() -> ToolDefinition {
    def(
        "tokensave_branch_list",
        "List Tracked Branches",
        "List all tracked branches with their DB sizes, parent branch, and last sync time. Returns an empty list if multi-branch is not active.",
        json!({
            "type": "object",
            "properties": {}
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explore_call_budget_tiers() {
        assert_eq!(explore_call_budget(0), 3);
        assert_eq!(explore_call_budget(5000), 3);
        assert_eq!(explore_call_budget(5001), 4);
        assert_eq!(explore_call_budget(20000), 4);
        assert_eq!(explore_call_budget(20001), 5);
        assert_eq!(explore_call_budget(80000), 5);
        assert_eq!(explore_call_budget(80001), 7);
        assert_eq!(explore_call_budget(250000), 7);
        assert_eq!(explore_call_budget(250001), 10);
    }

    #[test]
    fn test_context_description_contains_budget() {
        let desc = context_description(5000, 4);
        assert!(desc.contains("4 calls maximum"), "description should contain budget: {desc}");
        assert!(desc.contains("5000 nodes"), "description should contain node count: {desc}");
    }

    #[test]
    fn test_get_tool_definitions_with_budget() {
        let defs = get_tool_definitions_with_budget(10000, 4);
        let context_tool = defs.iter().find(|d| d.name == "tokensave_context").unwrap();
        assert!(context_tool.description.contains("4 calls maximum"));
        assert!(context_tool.description.contains("10000 nodes"));
    }
}
